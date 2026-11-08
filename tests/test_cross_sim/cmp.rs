//! Two simulators' plots of one circuit -> pass or the first difference.
//! Plots pair by kind (op, dc, ac, tran: what the scale says, since each
//! simulator names and orders its plots its own way), vectors by a name
//! every simulator spells alike once normalized (`v(x)`, `X`, `x` are one
//! node; `i(v1)`, `V1#branch`, `v1:flow(br)` one branch current).
//!
//! The tolerance is each simulator's own accuracy, summed over the two
//! runs: ngspice and Xyce both converge Newton to RELTOL = 1e-3 of the
//! value plus VNTOL = 1e-6 V (ABSTOL = 1e-12 A for a current); VACASK and
//! ESPice share those defaults. An op, dc or ac point passes within
//! 2 * (RELTOL * swing + floor), the swing being the vector's largest
//! magnitude in the reference run, the floor VNTOL for a voltage and
//! ABSTOL + GMIN * Vmax for a current (Vmax the plot's largest node
//! voltage: each simulator puts GMIN = 1e-12 S on other junctions). A transient also carries the local
//! truncation error each integrator allows, TRTOL = 7 times RELTOL in
//! ngspice and Xyce, so a point passes within 1e-2 of the swing (rounded up
//! from 2 * 7e-3 / sqrt(2), the two errors independent) plus 2 * VNTOL,
//! through a time tube of 1% of the stop time (`tube.rs`): the two
//! simulators choose different steps and move an edge by a few of them.
//!
//! Cites: ngspice manual §11.1.1 (`.options` RELTOL, VNTOL, ABSTOL, TRTOL
//! defaults); Xyce Reference Guide 7.10 §2.1.20 (`.options device`/`timeint`
//! RELTOL, ABSTOL defaults).
//!
//! Contents:
//! - `pub struct Tol`, `Tol::of(decks: &[&str]) -> Tol` (an autonomous deck's transient by period
//!   and peak-to-peak)
//! - `pub fn compare(a: &[RawData], b: &[RawData], tol: Tol) -> Result<(), String>`

use std::collections::HashMap;

use num_complex::Complex64;
use spicerack::result::{RawData, VarType};

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
}

/// RELTOL, VNTOL, ABSTOL and GMIN, the defaults both simulators share.
const RELTOL: f64 = 1e-3;
const VNTOL: f64 = 1e-6;
const ABSTOL: f64 = 1e-12;
const GMIN: f64 = 1e-12;

/// A transient point's tolerance, relative to the swing (module doc).
const TRAN: f64 = 1e-2;

/// `b`'s plots against the reference `a`'s: the same count of each kind,
/// paired in order, and every vector they share within the tolerance. An op
/// plot `b` has where `a` has none passes: Xyce's `.op` beside another
/// analysis only prints the operating point (Xyce Reference Guide 7.10,
/// `.OP`), where ngspice runs it as an analysis of its own. So does an ac
/// plot beside a noise analysis where `a` has none: HSPICE runs `.NOISE`
/// on an `.AC` sweep, which the deck written for it holds (HSPICE Reference
/// Manual, `.NOISE`), and ngspice's integrated noise (a one-point plot,
/// `total`) where `a`, Xyce's, has only the spectrum.
pub fn compare(a: &[RawData], b: &[RawData], tol: Tol) -> Result<(), String> {
    let has = |k: &str| a.iter().any(|p| kind(p) == k);
    let (op, ac, total) = (has("op"), has("ac") || !has("noise"), has("total"));
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
        let pa = a.iter().filter(|p| kind(p) == k);
        let pb = b.iter().filter(|p| kind(p) == k);
        for (x, y) in pa.zip(pb) {
            plot(k, x, y, tol)?;
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
        };
        for line in decks.iter().flat_map(|d| d.lines()) {
            let l = line.trim_start().to_ascii_lowercase();
            t.autonomous &= !waveform_source(&l);
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
}

// ---- private ----

/// The analysis a plot holds: noise by its plot name (ngspice's noise
/// spectrum has a frequency scale too), else read off its scale, by its type
/// or its name where the type word is not ngspice's (VACASK types `time`
/// `notype`).
fn kind(p: &RawData) -> &'static str {
    let scale = p.variables.first();
    let named = |n: &str| scale.is_some_and(|v| v.name.eq_ignore_ascii_case(n));
    if p.plot_name.to_ascii_lowercase().contains("noise") {
        return if p.re(0).len() <= 1 { "total" } else { "noise" };
    }
    // A complex plot is an ac sweep, one point or many (`.ac dec 1 1k 1k`);
    // a real one-point plot an operating point whatever its scale (Xyce's
    // `-r` op plot is over TIME).
    if p.is_complex {
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

/// The time tube's half-width as a fraction of the stop time: 1%, or
/// `CROSS_SIM_TUBE` (triage: does a wider window clear an edge?).
fn tube() -> f64 {
    std::env::var("CROSS_SIM_TUBE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.01)
}

/// A vector's name as every simulator here spells it: lower case, `v(x)`
/// as `x`, a branch current as `<device>#branch`, hierarchy joined by `.`.
fn normal(name: &str) -> String {
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
    // A current's floor: ABSTOL plus what GMIN at the plot's largest node
    // voltage carries, since each simulator puts GMIN on other junctions.
    let vmax = (first..a.variables.len())
        .filter(|&i| a.variables[i].var_type == VarType::Voltage)
        .flat_map(|i| column(a, i))
        .map(|c| c.norm())
        .fold(0.0, f64::max);
    let mut shared = 0;
    for (i, v) in a.variables.iter().enumerate().skip(first) {
        let Some(&j) = in_b.get(&normal(&v.name)) else {
            continue;
        };
        shared += 1;
        let (ya, yb) = (column(a, i), column(b, j));
        let swing = ya.iter().map(|c| c.norm()).fold(0.0, f64::max);
        let floor = match v.var_type {
            VarType::Current => t.abstol + GMIN * vmax,
            _ => t.vntol,
        };
        if k == "tran" {
            let tol = TRAN * t.reltol / RELTOL * swing + 2.0 * floor;
            let (ra, rb) = (a.re(i), b.re(j));
            let dx = tube() * xa.last().copied().unwrap_or(0.0);
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
            let current = v.var_type == VarType::Current;
            if t.autonomous && oscillates_alike(&ta, &va, &xb, &rb, tol, current) {
                continue;
            }
            if let Some(p) = leaves_tube(&ta, &va, &xb, &rb, dx, tol) {
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
    if shared == 0 && a.variables.len() > first {
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
}
