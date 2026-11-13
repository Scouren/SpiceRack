//! Two simulators' plots of one circuit -> pass or the first difference.
//! Plots pair by kind (op, dc, ac, tran: what the scale says, since each
//! simulator names and orders its plots its own way), vectors by a name
//! every simulator spells alike once normalized (`v(x)`, `X`, `x` are one
//! node; `i(v1)`, `V1#branch`, `v1:flow(br)` one branch current).
//! Plots of one kind pair by the grid of their swept vector, two over one
//! grid by their values (`compare`).
//!
//! The tolerance is each simulator's own accuracy, summed over the two
//! runs: ngspice and Xyce both converge Newton to RELTOL = 1e-3 of the
//! value plus VNTOL = 1e-6 V (ABSTOL = 1e-12 A for a current); VACASK and
//! ESPice share those defaults. An op, dc or ac point passes within
//! 2 * (RELTOL * swing + floor), the swing being the vector's largest
//! magnitude in the reference run, the floor VNTOL for a voltage and
//! ABSTOL + GMIN * Vmax for a current (Vmax the plot's largest node
//! voltage: each simulator puts GMIN = 1e-12 S on other junctions). A matched pair of vectors is
//! one quantity with one type: the one either raw file states, the name's when both say `notype`
//! (VACASK types every vector so; `typed`), so a VACASK reference's branch currents are held to
//! the current floor, not to VNTOL, a million times looser. A transient also carries the local
//! truncation error each integrator allows, TRTOL = 7 times RELTOL in
//! ngspice and Xyce, so a point passes within 1e-2 of the swing (rounded up
//! from 2 * 7e-3 / sqrt(2), the two errors independent) plus 2 * VNTOL,
//! through a time tube of 1% of the stop time (`tube.rs`): the two
//! simulators choose different steps and move an edge by a few of them.
//! In the last tube width before the common end the band stops with the
//! run: a sample out of it there passes when the other run's end lies in
//! this run's band (`end_holds`), since a run that lags by less than the
//! tube has not reached the value yet.
//!
//! A caller may tighten each of the four (`Tol::tighten`: RELTOL, VNTOL,
//! ABSTOL, the tube), never loosen one: a value above the default is refused.
//! Shared by `test_cross_sim` and `test_pair` (`#[path]`; each crate root
//! also holds `tube` and `vacask`).
//!
//! Under `uic` the point at t = 0 is compared by neither harness: it is each simulator's unsolved
//! start (VACASK stores the stated initial conditions as they stand, a floating capacitor's
//! a = 2, b = 0; ngspice stores its first consistent solve, a = 0.667, b = -1.333), a solution of
//! the circuit on neither side (`without_start`, called by the cross-sim `uic_start` and by the
//! pair test when the legalized design's transient has `uic`: a fact of the run, never a deck
//! key). Every point from the first step on is compared, so a wrong initial condition still
//! shows. It is the cross-sim harness's existing rule given one home, a guard for a simulator
//! that stores its unsolved start: neither pair cell needs it today, since ngspice-45 stores no
//! row at t = 0 and the comparison covers only the time both runs hold (it was not shown red).
//!
//! The pair test's refinement class (`simulator-differs:refinement`, rulings R84) is not a pass
//! and not a conversion finding: a transient that leaves the tube is run again with both decks'
//! step capped at span/2000 (`sim::refined`), and agreeing there, its difference is the two
//! simulators' step control. span/2000 because span/200 is not fine enough: resonator.sim (a
//! lossless LC) still differs at span/200 and agrees at span/2000, as at span/20000. dio.sim (a
//! diode's reverse recovery) does not agree refined (rulings R84a, F52): VACASK converges at
//! first order to a current 3.5 % from ngspice's, so it stays `mismatch`. A factor on the truncation-error test (`trtol`,
//! `tran_lteratio`) is a hint and never mapped (R84): giving both the same factor does not give
//! them the same steps (resonator with `trtol=3.5` still left the tube). It covers transients
//! only: a DC or AC difference has no step to refine and stays `mismatch`.
//!
//! Cites: ngspice manual §11.1.1 (`.options` RELTOL, VNTOL, ABSTOL, TRTOL
//! defaults); Xyce Reference Guide 7.10 §2.1.20 (`.options device`/`timeint`
//! RELTOL, ABSTOL defaults).
//!
//! Contents:
//! - `pub struct Tol`, `Tol::of(decks: &[&str]) -> Tol` (an autonomous deck's transient by period
//!   and peak-to-peak), `Tol::tighten(&mut self, key: &str, value: f64) -> Result<(), String>`,
//!   `Tol::scaled(self, f: f64) -> Tol`
//! - `pub fn compare(a: &[RawData], b: &[RawData], tol: Tol) -> Result<(), String>`
//! - `pub fn kind(p: &RawData) -> &'static str`, `pub fn normal(name: &str) -> String`
//! - `pub fn without_start(plots: &mut [RawData])` (the `uic` rule)

use std::collections::HashMap;

use num_complex::Complex64;
use spicerack::result::{RawData, VarInfo, VarType};

use super::tube::leaves_tube;

/// The accuracy a pair of decks asks of its simulators: each of RELTOL,
/// VNTOL and ABSTOL the larger of the default and any `.options` value
/// either deck sets (Xyce's `.options timeint reltol=` among them).
#[derive(Clone, Copy, Debug)]
pub struct Tol {
    reltol: f64,
    vntol: f64,
    abstol: f64,
    /// No independent source has a waveform: an autonomous circuit, whose
    /// transient is compared by period and peak-to-peak.
    autonomous: bool,
    /// The writer noted outputs the target writes no vector for (W0052: an
    /// integrated noise, an `@dev[param]`, an `.op` beside another analysis
    /// where the target wrote no op plot): the reference's are not compared.
    pub noted_drops: bool,
    /// A source's AC magnitude is not 1: the input-referred noise is
    /// divided by it in Xyce, not in ngspice (G-NOISE-INPUT-REF), so it is
    /// not compared.
    ac_scaled: bool,
    /// The transient time tube's half-width, a fraction of the stop time.
    tube: f64,
}

/// RELTOL, VNTOL, ABSTOL and GMIN, the defaults both simulators share.
const RELTOL: f64 = 1e-3;
const VNTOL: f64 = 1e-6;
const ABSTOL: f64 = 1e-12;
const GMIN: f64 = 1e-12;

/// A transient point's tolerance, relative to the swing (module doc).
const TRAN: f64 = 1e-2;

/// `b`'s plots against the reference `a`'s: the same count of each kind,
/// paired by grid, and every vector they share within the tolerance. An op
/// plot `b` has where `a` has none passes: Xyce's `.op` beside another
/// analysis only prints the operating point (Xyce Reference Guide 7.10,
/// `.OP`), where ngspice runs it as an analysis of its own. So does an ac
/// plot beside a noise analysis where `a` has none: HSPICE runs `.NOISE`
/// on an `.AC` sweep, which the deck written for it holds (HSPICE Reference
/// Manual, `.NOISE`), and ngspice's integrated noise (a one-point plot,
/// `total`) where `a`, Xyce's, has only the spectrum. The other way round,
/// an op plot `a` has and `b` lacks passes when the writer noted the drop
/// (`Tol::noted_drops`): the deck written for Xyce holds the `.op` and Xyce
/// does not calculate it.
pub fn compare(a: &[RawData], b: &[RawData], tol: Tol) -> Result<(), String> {
    let has = |k: &str| a.iter().any(|p| kind(p) == k);
    let total = has("total") && !tol.noted_drops;
    // A noted `.op` the target did not run: the reference's is not compared.
    let unrun = tol.noted_drops && !b.iter().any(|p| kind(p) == "op");
    let (op, ac) = (has("op") && !unrun, has("ac") || !has("noise"));
    let kinds = |r: &[RawData]| {
        let mut k: Vec<&str> = r
            .iter()
            .map(kind)
            .filter(|&k| (op || k != "op") && (ac || k != "ac") && (total || k != "total"))
            .collect();
        k.sort_unstable();
        k
    };
    if kinds(a) != kinds(b) {
        return Err(format!("plots {:?} became {:?}", kinds(a), kinds(b)));
    }
    for k in ["op", "dc", "ac", "tran", "noise", "total"] {
        let pa: Vec<&RawData> = a.iter().filter(|p| kind(p) == k).collect();
        let mut pb: Vec<&RawData> = b.iter().filter(|p| kind(p) == k).collect();
        // Plots of one kind pair by the grid of their swept vector: ngspice
        // prepends each analysis job (`cktnewan.c:34-35`), so three `.dc`
        // cards run in reverse deck order. Two cards over one grid
        // (`.dc vgs 0 1.5 50m vbs …` and `.dc vgs 0 1.5 50m temp …`) pair
        // by their values: the first candidate whose vectors hold, else
        // the first.
        // ponytail: first fit, not a matching. A plot within tolerance of
        // two candidates can take the other's partner and fail the gate
        // (never pass it); a bipartite matching when a deck does.
        for x in pa {
            let near = |u: f64, v: f64| (u - v).abs() <= 1e-9 * u.abs().max(v.abs()).max(1e-30);
            let same = |p: &RawData, q: &RawData| {
                let (s, t) = (p.re(0), q.re(swept(q)));
                s.len() == t.len()
                    && s.first().zip(t.first()).is_some_and(|(&u, &v)| near(u, v))
                    && s.last().zip(t.last()).is_some_and(|(&u, &v)| near(u, v))
            };
            let hits: Vec<usize> = (0..pb.len()).filter(|&j| same(x, pb[j])).collect();
            let j = hits
                .iter()
                .copied()
                .find(|&j| hits.len() > 1 && plot(k, x, pb[j], tol).is_ok())
                .or(hits.first().copied())
                .unwrap_or(0);
            if j < pb.len() {
                plot(k, x, pb.remove(j), tol)?;
            }
        }
    }
    Ok(())
}

impl Tol {
    /// The tolerances `decks` (the source and the deck written from it)
    /// ask for: their `.option(s)` cards' `reltol`, `vntol`, `abstol`.
    pub fn of(decks: &[&str]) -> Tol {
        let mut t = Tol {
            reltol: RELTOL,
            vntol: VNTOL,
            abstol: ABSTOL,
            autonomous: true,
            noted_drops: false,
            ac_scaled: false,
            tube: tube(),
        };
        for line in decks.iter().flat_map(|d| d.lines()) {
            let l = line.trim_start().to_ascii_lowercase();
            t.autonomous &= !waveform_source(&l);
            t.ac_scaled |= ac_magnitude(&l).is_some_and(|m| m != 1.0);
            if !l.starts_with(".option") {
                continue;
            }
            let l = l.replace(" =", "=").replace("= ", "=");
            for (k, v) in l.split_whitespace().filter_map(|w| w.split_once('=')) {
                let Some((x, _, _)) = spicerack::unit::parse_spice_number(v) else {
                    continue;
                };
                let slot = match k {
                    "reltol" => &mut t.reltol,
                    "vntol" => &mut t.vntol,
                    "abstol" => &mut t.abstol,
                    _ => continue,
                };
                *slot = slot.max(x);
            }
        }
        t
    }

    /// `key` (`reltol`, `vntol`, `abstol`, `tube`) set to at most `value`
    /// (a tighter value already set stays), which may only tighten: `Err` for an unknown key, a negative value, or one
    /// above the default (RELTOL 1e-3, VNTOL 1e-6 V, ABSTOL 1e-12 A, tube
    /// 0.01), which would loosen the simulators' own accuracy. VNTOL and
    /// ABSTOL may not go below the default either: they are each simulator's
    /// own solver floor (a MOSFET in cutoff carries -2.502e-12 A on ngspice-45
    /// at any abstol and -2.693e-12 A on VACASK at its default 1e-12, the same
    /// once VACASK's abstol is 1e-15), so a deck asking for less than the
    /// solvers give would fail on their tolerance, not on the conversion.
    #[allow(dead_code)] // `test_cross_sim` tightens nothing; `test_pair` does
    pub fn tighten(&mut self, key: &str, value: f64) -> Result<(), String> {
        let (slot, default, floor) = match key {
            "reltol" => (&mut self.reltol, RELTOL, 0.0),
            "vntol" => (&mut self.vntol, VNTOL, VNTOL),
            "abstol" => (&mut self.abstol, ABSTOL, ABSTOL),
            "tube" => (&mut self.tube, 0.01, 0.0),
            _ => return Err(format!("{key}: not a tolerance")),
        };
        if value > default {
            return Err(format!("{key} = {value:e} loosens the default {default:e}"));
        }
        if value.is_nan() || value < floor {
            return Err(format!(
                "{key} = {value:e} is below the simulators' own floor {floor:e}"
            ));
        }
        // A tighter value already set (a deck's own, a narrower triage tube) stays.
        *slot = slot.min(value);
        Ok(())
    }

    /// Every value tolerance (RELTOL, VNTOL, ABSTOL) times `f`, for the pair
    /// test's margin probe only (`PAIR_MARGINS`, triage): not a deck's key.
    #[allow(dead_code)] // `test_cross_sim` has no margin probe
    pub fn scaled(mut self, f: f64) -> Tol {
        assert!((0.0..=1.0).contains(&f), "a fraction of the tolerance");
        self.reltol *= f;
        self.vntol *= f;
        self.abstol *= f;
        self
    }
}

/// The analysis a plot holds: noise by its plot name (ngspice's noise
/// spectrum has a frequency scale too), else read off its scale, by its type
/// or its name where the type word is not ngspice's (VACASK types `time`
/// `notype`).
pub fn kind(p: &RawData) -> &'static str {
    let scale = p.variables.first();
    let named = |n: &str| scale.is_some_and(|v| v.name.eq_ignore_ascii_case(n));
    let name = p.plot_name.to_ascii_lowercase();
    if name.contains("noise") {
        // By name where the simulator gives one (ngspice: "Noise Spectral
        // Density Curves", "Integrated Noise"): a one-point spectrum is
        // still a spectrum.
        return match (name.contains("integrated"), name.contains("density")) {
            (true, _) => "total",
            (_, true) => "noise",
            _ if p.re(0).len() <= 1 => "total",
            _ => "noise",
        };
    }
    // A complex plot is an ac sweep, one point or many (`.ac dec 1 1k 1k`);
    // a real one-point plot an operating point whatever its scale (Xyce's
    // `-r` op plot is over TIME).
    // A pole-zero plot (ngspice's "Pole-Zero Analysis") is complex too, one
    // point with no scale: an operating point's shape.
    if p.is_complex && !name.contains("pole") {
        return "ac";
    }
    if p.re(0).len() <= 1 {
        return "op";
    }
    match scale.map(|v| v.var_type) {
        Some(VarType::Time) => "tran",
        Some(VarType::Frequency) => "ac",
        _ if named("time") => "tran",
        _ if named("frequency") => "ac",
        _ => "dc",
    }
}

/// A vector's name as every simulator here spells it: lower case, `v(x)`
/// as `x`, a branch current as `<device>#branch`, hierarchy joined by `.`.
pub fn normal(name: &str) -> String {
    let n = name.trim().to_ascii_lowercase().replace(':', ".");
    if let Some(dev) = n.strip_suffix(".flow(br)") {
        return format!("{dev}#branch");
    }
    if let Some(inner) = n.strip_prefix("v(").and_then(|s| s.strip_suffix(')'))
        && !inner.contains(',')
    {
        return inner.to_string();
    }
    if let Some(inner) = n.strip_prefix("i(").and_then(|s| s.strip_suffix(')')) {
        return format!("{inner}#branch");
    }
    n
}

/// Each transient plot of `plots` without its point at t = 0 (the `uic` rule: module doc).
pub fn without_start(plots: &mut [RawData]) {
    for p in plots {
        let timed = kind(p) == "tran";
        let at0 = p
            .real_data
            .first()
            .is_some_and(|t| t.first() == Some(&0.0) && t.len() > 1);
        if timed && at0 {
            p.real_data
                .iter_mut()
                .filter(|c| !c.is_empty())
                .for_each(|c| {
                    c.remove(0);
                });
            p.complex_data
                .iter_mut()
                .filter(|c| !c.is_empty())
                .for_each(|c| {
                    c.remove(0);
                });
        }
    }
}

// ---- private ----

/// The vector a plot's points run along: VACASK writes a nested sweep's
/// vectors outer first, and the deck written for it names the inner one
/// `vsweep` (`render/named/vacask_bench.rs`), which is among the first four
/// as in `plot`; any other plot's is its first.
fn swept(p: &RawData) -> usize {
    p.variables
        .iter()
        .take(4)
        .position(|v| v.name.eq_ignore_ascii_case("vsweep"))
        .unwrap_or(0)
}

/// The time tube's half-width as a fraction of the stop time: 1%, or
/// `CROSS_SIM_TUBE` (triage: does a wider window clear an edge?).
fn tube() -> f64 {
    std::env::var("CROSS_SIM_TUBE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.01)
}

fn column(r: &RawData, i: usize) -> Vec<Complex64> {
    match r.complex(i) {
        Some(c) => c.to_vec(),
        None => r.re(i).iter().map(|&x| Complex64::new(x, 0.0)).collect(),
    }
}

/// One pair of plots of kind `k`.
fn plot(k: &str, a: &RawData, b: &RawData, t: Tol) -> Result<(), String> {
    // An op plot (and ngspice's one-point integrated noise) has no scale:
    // its first vector is a value.
    let first = usize::from(k != "op" && k != "total");
    let (xa, xb) = (a.re(0), b.re(0));
    if first == 1 && k != "tran" {
        let span = xa.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        // A grid point holds to 1e-9 of the span, or to 1e-6 of itself: a
        // log sweep's points are printed to 7 digits in Xyce's noise table.
        let close = |x: f64, y: f64| (x - y).abs() <= (1e-9 * span).max(1e-6 * x.abs());
        let same =
            |ys: &[f64]| ys.len() == xa.len() && xa.iter().zip(ys).all(|(&x, &y)| close(x, y));
        // A nested `.dc` is one plot over the inner source in ngspice; VACASK
        // writes each sweep's vector first, the outer one (`sweep1`) before
        // the inner (`vsweep`). The grid holds if any leading vector is `a`'s.
        let lead = b.variables.len().min(4);
        if !(0..lead).any(|j| same(&b.re(j))) {
            return Err(format!(
                "{k}: sweep grid changed ({} -> {} points)",
                xa.len(),
                xb.len()
            ));
        }
    }
    if k == "tran" {
        let (ea, eb) = (xa.last().copied(), xb.last().copied());
        if !matches!((ea, eb), (Some(x), Some(y)) if (x - y).abs() <= 1e-9 * x.abs().max(y.abs())) {
            return Err(format!("tran: stop time {ea:?} -> {eb:?}"));
        }
    }
    let mut in_b = HashMap::new();
    for (j, w) in b.variables.iter().enumerate().skip(first) {
        in_b.entry(normal(&w.name)).or_insert(j);
    }
    // A matched pair is one quantity with one type (`typed`): `types[i]` for `a`'s vector `i`.
    let types: Vec<VarType> = a
        .variables
        .iter()
        .map(|v| typed(v, in_b.get(&normal(&v.name)).map(|&j| &b.variables[j])))
        .collect();
    // A current's floor: ABSTOL plus what GMIN at the plot's largest node
    // voltage carries, since each simulator puts GMIN on other junctions.
    let vmax = (first..a.variables.len())
        .filter(|&i| types[i] == VarType::Voltage)
        .flat_map(|i| column(a, i))
        .map(|c| c.norm())
        .fold(0.0, f64::max);
    let mut shared = 0;
    for (i, v) in a.variables.iter().enumerate().skip(first) {
        if t.noted_drops && v.name.contains('@')
            || t.ac_scaled && v.name.to_ascii_lowercase().starts_with("inoise")
        {
            continue;
        }
        let Some(&j) = in_b.get(&normal(&v.name)) else {
            continue;
        };
        shared += 1;
        let (ya, yb) = (column(a, i), column(b, j));
        let swing = ya.iter().map(|c| c.norm()).fold(0.0, f64::max);
        let floor = match types[i] {
            VarType::Current => t.abstol + GMIN * vmax,
            _ => t.vntol,
        };
        if k == "tran" {
            let tol = TRAN * t.reltol / RELTOL * swing + 2.0 * floor;
            let (ra, rb) = (a.re(i), b.re(j));
            let dx = t.tube * xa.last().copied().unwrap_or(0.0);
            // Only the time both runs cover is compared: a TSTART that one
            // simulator prints from and the other does not (ISSUE_160).
            let lo = xa
                .first()
                .copied()
                .unwrap_or(0.0)
                .max(xb.first().copied().unwrap_or(0.0));
            let hi = xa
                .last()
                .copied()
                .unwrap_or(0.0)
                .min(xb.last().copied().unwrap_or(0.0));
            let keep: Vec<usize> = (0..xa.len())
                .filter(|&p| xa[p] >= lo && xa[p] <= hi)
                .collect();
            let (ta, va): (Vec<f64>, Vec<f64>) = keep.iter().map(|&p| (xa[p], ra[p])).unzip();
            let current = types[i] == VarType::Current;
            if t.autonomous && oscillates_alike(&ta, &va, &xb, &rb, tol, current) {
                continue;
            }
            if let Some(p) = leaves_tube(&ta, &va, &xb, &rb, dx, tol)
                && !(ta[p] > hi - dx && end_holds(&ta, &va, &xb, &rb, hi - dx, dx, tol))
            {
                return Err(format!(
                    "tran {}: {} leaves the tube at t={} (tol {tol:.3e})",
                    v.name, va[p], ta[p]
                ));
            }
        } else {
            let tol = 2.0 * (t.reltol * swing + floor);
            if ya.len() != yb.len() {
                return Err(format!(
                    "{k} {}: {} -> {} points",
                    v.name,
                    ya.len(),
                    yb.len()
                ));
            }
            if let Some(p) = (0..ya.len()).find(|&p| (ya[p] - yb[p]).norm() > tol) {
                let at = if first == 1 { xa[p] } else { 0.0 };
                return Err(format!(
                    "{k} {}: {} vs {} at {at} (tol {tol:.3e})",
                    v.name, ya[p], yb[p]
                ));
            }
        }
    }
    let dropped = |r: &RawData| r.variables.iter().skip(first).all(|v| v.name.contains('@'));
    if shared == 0 && a.variables.len() > first && !(t.noted_drops && dropped(a)) {
        let names = |r: &RawData| -> Vec<String> {
            r.variables
                .iter()
                .skip(first)
                .take(6)
                .map(|v| v.name.clone())
                .collect()
        };
        return Err(format!(
            "{k}: no vector in common ({:?} vs {:?})",
            names(a),
            names(b)
        ));
    }
    Ok(())
}

/// The one type of a matched pair of vectors, `a`'s and `b`'s (`None`: `a`'s alone): the type
/// either raw file states (ngspice types its vectors, `a`'s first), and only when both say
/// `notype` (VACASK types every vector so) the name's: `<element>:flow(br)` (a branch) and
/// `<element>.i` (`save p('r1', i)`) are currents, a node is a voltage, and a device quantity
/// (`@m1[gm]`) stays `Other`.
fn typed(a: &VarInfo, b: Option<&VarInfo>) -> VarType {
    let mut stated = [Some(a), b].into_iter().flatten().map(|v| v.var_type);
    if let Some(t) = stated.find(|&t| t != VarType::Other) {
        return t;
    }
    let n = a.name.to_ascii_lowercase();
    if n.ends_with(":flow(br)") || n.ends_with(".i") {
        VarType::Current
    } else if n.contains(['@', '[']) {
        VarType::Other
    } else {
        VarType::Voltage
    }
}

/// The AC magnitude an independent source `line` (lower case) gives,
/// `ac <mag>` or `ac=<mag>`; `None` when it gives none.
fn ac_magnitude(line: &str) -> Option<f64> {
    if !line.starts_with(['v', 'i']) {
        return None;
    }
    let w: Vec<&str> = line.split_whitespace().collect();
    let k = w.iter().position(|x| *x == "ac" || x.starts_with("ac="))?;
    let v = match w[k].strip_prefix("ac=") {
        Some(v) if !v.is_empty() => v,
        _ => w.get(k + 1)?.trim_start_matches('='),
    };
    spicerack::unit::parse_spice_number(v).map(|(x, _, _)| x)
}

/// Whether `line` (lower case) is an independent V or I source with a
/// time waveform (SPICE's and Xyce's spellings, with or without parens).
fn waveform_source(line: &str) -> bool {
    if !line.starts_with(['v', 'i']) {
        return false;
    }
    let words = [
        "sin", "pulse", "pwl", "exp", "sffm", "am", "trnoise", "trrandom",
    ];
    line.split(|c: char| c.is_whitespace() || c == '(')
        .skip(1)
        .any(|w| words.contains(&w))
}

/// Whether `(xb, yb)` from the time `from` on lies in the tube of
/// `(xa, ya)`: the tube turned around for the last tube width before the
/// common end, where the band a sample is held to stops with the run. A
/// run that lags by less than `dx` has not reached the other's last values,
/// but its own end lies in the other's band; a wrong value lies in neither.
fn end_holds(xa: &[f64], ya: &[f64], xb: &[f64], yb: &[f64], from: f64, dx: f64, tol: f64) -> bool {
    assert_eq!(xa.len(), ya.len(), "a sample has its time");
    assert_eq!(xb.len(), yb.len(), "a sample has its time");
    let k = xb.partition_point(|&x| x < from);
    leaves_tube(&xb[k..], &yb[k..], xa, ya, dx, tol).is_none()
}

/// An autonomous oscillation (mismatches.md §4.1: phases drift over many
/// cycles, pointwise comparison fails), over the second half of each run:
/// a node crosses its mid level at least 4 times in both, the mean periods
/// agree to TRAN (1%) and the peak-to-peak swings to `tol`; a current
/// (`current`: a supply's spikes, one per stage) by its mean to `tol`.
fn oscillates_alike(
    xa: &[f64],
    ya: &[f64],
    xb: &[f64],
    yb: &[f64],
    tol: f64,
    current: bool,
) -> bool {
    let half = |x: &[f64]| x.partition_point(|&t| t < x.last().copied().unwrap_or(0.0) / 2.0);
    let shape = |x: &[f64], y: &[f64]| -> Option<(f64, f64)> {
        let from = half(x);
        let (x, y) = (&x[from..], &y[from..]);
        let (lo, hi) = y
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
                (a.min(v), b.max(v))
            });
        let mid = (lo + hi) / 2.0;
        let ups: Vec<f64> = (1..y.len())
            .filter(|&k| y[k - 1] < mid && y[k] >= mid)
            .map(|k| x[k - 1] + (mid - y[k - 1]) / (y[k] - y[k - 1]) * (x[k] - x[k - 1]))
            .collect();
        (ups.len() >= 4).then(|| {
            (
                (ups[ups.len() - 1] - ups[0]) / (ups.len() - 1) as f64,
                hi - lo,
            )
        })
    };
    let mean = |x: &[f64], y: &[f64]| -> Option<f64> {
        let from = half(x);
        let (x, y) = (&x[from..], &y[from..]);
        let span = x.last()? - x.first()?;
        let area: f64 = (1..x.len())
            .map(|k| (y[k] + y[k - 1]) / 2.0 * (x[k] - x[k - 1]))
            .sum();
        (span > 0.0).then(|| area / span)
    };
    if current {
        return matches!((mean(xa, ya), mean(xb, yb)), (Some(a), Some(b)) if (a - b).abs() <= tol);
    }
    match (shape(xa, ya), shape(xb, yb)) {
        (Some((pa, sa)), Some((pb, sb))) => (pa - pb).abs() <= TRAN * pa && (sa - sb).abs() <= tol,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real plot named `plot_name` of `rows`: each vector's name, type and
    /// points.
    fn real(plot_name: &str, rows: &[(&str, VarType, [f64; 4])]) -> RawData {
        RawData {
            plot_name: plot_name.into(),
            variables: rows
                .iter()
                .enumerate()
                .map(|(index, &(name, var_type, _))| VarInfo {
                    index,
                    name: name.into(),
                    var_type,
                })
                .collect(),
            real_data: rows.iter().map(|r| r.2.to_vec()).collect(),
            ..Default::default()
        }
    }

    /// One nested `.dc` card as ngspice and ESPice write it: the inner sweep
    /// is the scale.
    fn ngspice(sweep: [f64; 4], out: [f64; 4]) -> RawData {
        real(
            "DC transfer characteristic",
            &[
                ("v(v-sweep)", VarType::Voltage, sweep),
                ("v(out)", VarType::Voltage, out),
            ],
        )
    }

    /// The same card as VACASK writes it from the deck written for it: the
    /// outer sweep first.
    fn vacask(outer: [f64; 4], sweep: [f64; 4], out: [f64; 4]) -> RawData {
        real(
            "Operating Point",
            &[
                ("sweep1", VarType::Other, outer),
                ("vsweep", VarType::Other, sweep),
                ("out", VarType::Other, out),
            ],
        )
    }

    #[test]
    fn every_simulators_spelling_of_one_vector_is_one_name() {
        for n in ["v(out)", "OUT", "out"] {
            assert_eq!(normal(n), "out");
        }
        for n in ["i(v1)", "V1#branch", "v1:flow(br)"] {
            assert_eq!(normal(n), "v1#branch");
        }
        assert_eq!(normal("X1:N2"), "x1.n2");
    }

    #[test]
    fn p1_045_a_run_that_lags_at_the_end_is_in_the_tube() {
        // An edge in the last 1% of the run, and the same edge half a tube
        // width later: when both runs stop the late one has not reached
        // the last values, so the tube fails there and only there, and the
        // late run's end lies in the first's band. A run that ends on
        // another value fails both ways.
        // Oracle: the tube's definition (a shift under dx is no
        // difference), on waveforms written out by hand. Checks: 4.
        let xs: Vec<f64> = (0..=1000).map(|k| f64::from(k) / 1000.0).collect();
        let edge = |t: f64| ((t - 0.99) / 0.01).clamp(0.0, 1.0);
        let a: Vec<f64> = xs.iter().map(|&t| edge(t)).collect();
        let late: Vec<f64> = xs.iter().map(|&t| edge(t - 0.005)).collect();
        let wrong: Vec<f64> = xs.iter().map(|&t| -edge(t)).collect();
        let (dx, tol) = (0.01, 0.01);
        let out = leaves_tube(&xs, &a, &xs, &late, dx, tol);
        assert!(out.is_some_and(|p| xs[p] > 1.0 - dx), "{out:?}");
        assert!(end_holds(&xs, &a, &xs, &late, 1.0 - dx, dx, tol));
        assert!(leaves_tube(&xs, &a, &xs, &wrong, dx, tol).is_some());
        assert!(!end_holds(&xs, &a, &xs, &wrong, 1.0 - dx, dx, tol));
    }

    #[test]
    fn p3_065_nested_dc_plots_pair_by_the_swept_vector() {
        // Two nested cards as ngspice-45 and VACASK wrote
        // `ngspice/examples/mos/nmos_pmos_drainresistor.sp`: ngspice the
        // last card first, VACASK in deck order with `sweep1` first.
        let (to1, to2) = ([0.0, 1.0, 0.0, 1.0], [0.0, 2.0, 0.0, 2.0]);
        let (low, high) = ([0.0, 1.0, 0.0, 2.0], [0.0, 6.0, 0.0, 8.0]);
        let outer = [0.0, 0.0, 5.0, 5.0];
        let a = [ngspice(to2, high), ngspice(to1, low)];
        let b = [vacask(outer, to1, low), vacask(outer, to2, high)];
        assert_eq!(compare(&a, &b, Tol::of(&[])), Ok(()));
        // A card whose values differ still fails.
        let off = [vacask(outer, to1, low), vacask(outer, to2, low)];
        assert!(compare(&a, &off, Tol::of(&[])).is_err());
    }

    #[test]
    fn p3_065_two_cards_over_one_grid_pair_by_their_values() {
        // `.dc vgsn 0 1.5 0.05 vbsn …` and `.dc vgsn 0 1.5 0.05 temp …`
        // (`nmos_pmos_BSIM330.sp`): one grid, ngspice the last card first,
        // ESPice in deck order.
        let grid = [0.0, 1.0, 0.0, 1.0];
        let (low, high) = ([0.0, 1.0, 0.0, 2.0], [0.0, 3.0, 0.0, 5.0]);
        let a = [ngspice(grid, high), ngspice(grid, low)];
        let b = [ngspice(grid, low), ngspice(grid, high)];
        assert_eq!(compare(&a, &b, Tol::of(&[])), Ok(()));
        // No candidate holds: the difference stands. A candidate is one
        // card's only: two equal reference plots need two.
        let off = [ngspice(grid, low), ngspice(grid, [0.0, 3.0, 0.0, 9.0])];
        assert!(compare(&a, &off, Tol::of(&[])).is_err());
        let twice = [ngspice(grid, low), ngspice(grid, low)];
        assert!(compare(&twice, &b, Tol::of(&[])).is_err());
    }

    #[test]
    fn a_vacask_reference_compares_its_currents_at_the_current_floor() {
        // VACASK types every vector `notype`, ngspice types its own: a matched pair is one
        // quantity, typed by the file that states it, by the name when neither does. Oracle:
        // the floors of the module doc written out by hand (a current's 2·(ABSTOL + GMIN·Vmax),
        // Vmax 5 V from the node `vdd`), against which the voltage floor 2·VNTOL = 2e-6 A was a
        // million times loose. Checks: 4.
        let op = |rows: &[(&str, VarType, [f64; 4])]| {
            let mut p = real("Operating Point", rows);
            p.real_data.iter_mut().for_each(|c| c.truncate(1));
            p
        };
        let vacask = |i: f64, j: f64| {
            op(&[
                ("vdd", VarType::Other, [5.0; 4]),
                ("v1:flow(br)", VarType::Other, [i; 4]),
                ("r1.i", VarType::Other, [j; 4]),
            ])
        };
        let ngspice = |i: f64, j: f64| {
            op(&[
                ("v(vdd)", VarType::Voltage, [5.0; 4]),
                ("i(v1)", VarType::Current, [i; 4]),
                ("r1.i", VarType::Other, [j; 4]),
            ])
        };
        let t = Tol::of(&[]);
        // A branch current 1 nA off: a difference (its tolerance 2·(RELTOL·1e-9 + 1e-12 + 5e-12)).
        assert!(compare(&[vacask(1e-9, 0.0)], &[ngspice(2e-9, 0.0)], t).is_err());
        // An element's current named `<element>.i`, untyped on both sides: a current too.
        assert!(compare(&[vacask(0.0, 1e-9)], &[ngspice(0.0, 2e-9)], t).is_err());
        // 8 pA off passes: GMIN·Vmax counts the untyped node `vdd` as a voltage.
        let near = 1e-9 + 8e-12;
        assert_eq!(
            compare(&[vacask(1e-9, 1e-9)], &[ngspice(near, near)], t),
            Ok(())
        );
        assert!(compare(&[vacask(1e-9, 1e-9)], &[ngspice(1e-9 + 2e-11, 1e-9)], t).is_err());
    }

    #[test]
    fn p1_066_a_noted_op_the_target_did_not_run_is_not_compared() {
        // ngspice runs `.op` and `.dc` (two plots); Xyce 7.10 on the deck
        // written for it runs the `.dc` alone and the writer notes the
        // `.op` (W0052). Oracle: Xyce 7.10.0 on `.op` + `.dc` over an RC
        // (research/impl/evidence/P1-066-xyce-one-analysis/results.txt,
        // a5_op_dc: one plot, "DC transfer characteristic (3 pts)"), and
        // the verdict table below written out by hand. Checks: 4.
        let sweep = [0.0, 1.0, 2.0, 3.0];
        let dc = |out: [f64; 4]| ngspice(sweep, out);
        let mut op = real("Operating Point", &[("v(out)", VarType::Voltage, [1.0; 4])]);
        op.real_data[0].truncate(1);
        let a = [dc(sweep), op.clone()];
        let mut noted = Tol::of(&[]);
        noted.noted_drops = true;
        // Noted and not run: the sweep alone is compared.
        assert_eq!(compare(&a, &[dc(sweep)], noted), Ok(()));
        // Not noted: the missing op is a difference.
        assert!(compare(&a, &[dc(sweep)], Tol::of(&[])).is_err());
        // Noted, and the target wrote an op all the same: it is compared.
        let mut off = op.clone();
        off.real_data[0][0] = 2.0;
        assert!(compare(&a, &[dc(sweep), off], noted).is_err());
        assert_eq!(compare(&a, &[dc(sweep), op], noted), Ok(()));
    }
}
