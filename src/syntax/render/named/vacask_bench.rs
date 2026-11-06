//! The testbench → VACASK's `control` block: one `options` line, the
//! stepped parameters as control variables swept ahead of each analysis,
//! and the analyses, initial conditions on each `analysis` line; a Spectre
//! `montecarlo` as an `mc` loop around its inner analysis.
//!
//! Cites: VACASK `share/doc/vacask/demo` (`analysis`, `sweep`, `options`,
//! `var`), `docs/cmd-analysis-mc.md` (`mc`; as the old VACASK writer
//! probed them); Spectre Reference 19.1 pp.165-174 (`montecarlo`).
//!
//! Contents:
//! - `W::control(&mut self, tb: &Tb)`

use super::W;
use super::bench::{Step, Tb, bare_node};
use super::lift::draw_scopes;
use super::vacask::{num, respell};
use crate::syntax::Dialect;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};
use crate::syntax::render::read::{Ana, DcSweep, Sweep};

/// `.options` keys that only steer a simulator's bookkeeping or printing:
/// dropping one changes no number in the raw file.
const COSMETIC_OPTIONS: &[&str] = &[
    "noacct",
    "acct",
    "list",
    "node",
    "post",
    "trans",
    "nopage",
    "nomod",
    "lvlcod",
    "klu",
    "sparse",
    "savecurrents",
    "numdgt",
];

/// `.options KEY=VALUE` whose VACASK spelling is the same word and the same
/// quantity (`itl1`, `maxord` are rejected there). `scale` is the
/// `$simparam("scale")` VACASK's `sp_*` modules read where ngspice's code
/// reads `CKTscale` (`devices/spice/*.va` headers; `lib/osdidevice.cpp`
/// `simParamNames`).
const NUMERIC_OPTIONS: &[&str] = &[
    "reltol", "abstol", "vntol", "chgtol", "gmin", "temp", "tnom", "scale",
];

impl W<'_, '_> {
    /// The `control` block: options, then per analysis the stepped sweeps
    /// and the analysis, its initial conditions on its `analysis` line.
    pub(super) fn control(&mut self, tb: &Tb) {
        self.unwritten_outputs(tb);
        self.line("");
        self.line("control");
        let mut opts = String::from("  options rawfile=\"binary\"");
        for (k, v, tok) in &tb.options {
            let k = k.to_ascii_lowercase();
            if NUMERIC_OPTIONS.contains(&k.as_str()) {
                opts.push_str(&format!(" {k}={}", respell(v)));
            } else if k == "method" {
                match v.to_ascii_lowercase().as_str() {
                    "trap" | "trapezoidal" => opts.push_str(" tran_method=\"trap\""),
                    "gear" | "gear2" => opts.push_str(" tran_method=\"gear2\""),
                    _ => {
                        self.ask(Construct::UnmappedOption, *tok);
                    }
                }
            } else if !COSMETIC_OPTIONS.contains(&k.as_str()) {
                self.ask(Construct::UnmappedOption, *tok);
            } else {
                self.note(Code::W0024, *tok);
            }
        }
        for (k, v, _) in &tb.vacask {
            opts.push_str(&format!(" {k}={v}"));
        }
        if let Some(t) = tb.temp {
            opts.push_str(&format!(" temp={}", num(t)));
        }
        if let Some(t) = tb.tnom {
            opts.push_str(&format!(" tnom={}", num(t)));
        }
        self.line(&opts);
        if let Some((_, tok)) = tb.measures.first() {
            self.ask(Construct::Measure, *tok);
        }
        for (_, p, [start, ..], _) in &tb.steps {
            self.line(&format!("  var {}={}", p.to_ascii_lowercase(), num(*start)));
        }
        let mut sweeps: Vec<String> = tb.steps.iter().filter_map(|s| self.step_sweep(s)).collect();
        // A list `.step` is a `sweep … values=[…]` (`lib/answeep.cpp`
        // `setupValueSweep`); `temp` is the `temp` option's.
        for (p, v, _) in &tb.step_lists {
            let name = p.to_ascii_lowercase();
            let what = match name.as_str() {
                "temp" => "option=\"temp\"".to_string(),
                _ => {
                    self.line(&format!("  var {name}={}", num(v[0])));
                    format!("variable=\"{name}\"")
                }
            };
            sweeps.push(format!(
                "sweep step_{} {what} {}",
                sanitize(&name),
                values(v)
            ));
        }
        let mut ic = String::new();
        if !tb.ics.is_empty() {
            ic.push_str(&node_list("ic", &tb.ics));
        }
        if !tb.nodesets.is_empty() {
            ic.push_str(&node_list("nodeset", &tb.nodesets));
        }
        // `Construct::Save` is `Spell(Omit)`: VACASK saves every node.
        for l in &tb.extra {
            self.line(&format!("  {l}"));
        }
        for (idx, (a, tok)) in tb.analyses.iter().enumerate() {
            // A `sweep` binds to the next `analysis`: the stepped sweeps
            // are repeated ahead of each.
            for s in &sweeps {
                self.line(&format!("  {s}"));
            }
            let lines = match a {
                Ana::MonteCarlo { .. } => self.vacask_mc(idx, a, *tok, tb),
                a => self.vacask_analysis(idx, a, *tok),
            };
            for l in lines {
                let ic = match l.trim_start().starts_with("analysis") {
                    true => ic.as_str(),
                    false => "",
                };
                self.line(&format!("  {l}{ic}"));
            }
        }
        self.line("endc");
    }

    /// Analysis `idx`'s lines (`analysis <kind><idx+1> …`, a DC sweep's
    /// `sweep` lines first); empty when it is refused.
    fn vacask_analysis(&mut self, idx: usize, a: &Ana, tok: u32) -> Vec<String> {
        let kind = match a {
            Ana::Op => "op",
            Ana::Dc(_) | Ana::DcAxes(_) => "dc",
            Ana::Ac(_) => "ac",
            Ana::Tran { .. } => "tran",
            Ana::Noise { .. } => "noise",
            Ana::Tf(..) => "tf",
            Ana::Sens(..) | Ana::Pz(..) | Ana::Sp(..) | Ana::Four(..) => {
                self.refuse(Code::W0015, tok);
                return vec![];
            }
            Ana::MonteCarlo { .. } => unreachable!("written by vacask_mc"),
            Ana::Spectre { kind, .. } => {
                self.refuse(
                    crate::syntax::render::read::spectre_refusal(Dialect::Vacask, kind),
                    tok,
                );
                return vec![];
            }
        };
        let name = format!("{kind}{}", idx + 1);
        let mut out = Vec::new();
        match a {
            Ana::Op => out.push(format!("analysis {name} op")),
            Ana::Dc(sweeps) => {
                if sweeps.is_empty() {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                }
                for (i, (src, start, stop, step)) in sweeps.iter().enumerate().rev() {
                    let scale = match i {
                        0 => "vsweep".to_string(),
                        i => format!("sweep{i}"),
                    };
                    match sweep_line(&scale, src, *start, *stop, *step) {
                        Some(l) => out.push(l),
                        None => {
                            self.refuse(Code::W0015, tok);
                            return vec![];
                        }
                    }
                }
                out.push(format!("  analysis {name} op"));
            }
            // A log or list axis is its values (`DcSweep::values`), the
            // first axis innermost as for `Ana::Dc`.
            Ana::DcAxes(axes) => {
                if !matches!(self.ask(Construct::DcLogList, tok), Action::Native) {
                    return vec![];
                }
                for (i, ax) in axes.iter().enumerate().rev() {
                    let scale = match i {
                        0 => "vsweep".to_string(),
                        i => format!("sweep{i}"),
                    };
                    let line = match (&ax.sweep, ax.sweep.values()) {
                        (DcSweep::Lin(a, b, c), _) => sweep_line(&scale, &ax.src, *a, *b, *c),
                        (_, Some(v)) => Some(format!(
                            "sweep {scale} instance=\"{}\" parameter=\"dc\" {}",
                            ax.src.to_ascii_lowercase(),
                            values(&v)
                        )),
                        (_, None) => None,
                    };
                    let Some(line) = line else {
                        self.refuse(Code::W0015, tok);
                        return vec![];
                    };
                    out.push(line);
                }
                out.push(format!("  analysis {name} op"));
            }
            Ana::Ac(sw) => match freq_sweep(sw) {
                Some(f) => out.push(format!("analysis {name} ac {f}")),
                None => {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                }
            },
            Ana::Tran {
                step,
                stop,
                start,
                max_step,
                uic,
                ..
            } => {
                // VACASK always saves from t=0.
                if start.unwrap_or(0.0) != 0.0 {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                }
                let native = self.rd.d == Dialect::Vacask;
                let tmax = max_step.or((!native).then(|| step.min(stop / 50.0)));
                let mut s = format!(
                    "analysis {name} tran step={} stop={}",
                    num(*step),
                    num(*stop)
                );
                if let Some(t) = tmax {
                    s.push_str(&format!(" maxstep={}", num(t)));
                }
                s.push_str(match uic {
                    true => " icmode=\"uic\"",
                    false => " icmode=\"op\"",
                });
                out.push(s);
            }
            // `out=` names one node: a non-ground reference has no form.
            Ana::Noise {
                output,
                reference,
                source,
                sweep,
                ..
            } => {
                let f = freq_sweep(sweep);
                if !reference.is_empty() && reference != "0" || f.is_none() {
                    self.refuse(Code::W0015, tok);
                    return vec![];
                }
                out.push(format!(
                    "analysis {name} noise out=\"{}\" in=\"{}\" {}",
                    bare_node(output).to_ascii_lowercase(),
                    source.to_ascii_lowercase(),
                    f.expect("checked")
                ));
            }
            Ana::Tf(output, _) => out.push(format!(
                "analysis {name} dcxf out=\"{}\"",
                bare_node(output).to_ascii_lowercase()
            )),
            Ana::Sens(..)
            | Ana::Pz(..)
            | Ana::Sp(..)
            | Ana::Four(..)
            | Ana::MonteCarlo { .. }
            | Ana::Spectre { .. } => unreachable!("refused above"),
        }
        out
    }

    /// A Spectre `montecarlo` (`idx`, `a`) as VACASK's `mc` loop around
    /// the deck's analysis of its inner kind, named as the loop's own
    /// (`docs/cmd-analysis-mc.md` lines 7-23), `lh=0`: Spectre's default
    /// `sampling=standard` draws plainly (Reference 19.1 pp.165-174), and
    /// `seed` carries the seed, not Spectre's draws. Refused (`W0015`) with
    /// no analysis of that kind, and (`W0115`) where `variations` (default
    /// `process`, p.166) leaves out a scope the deck draws in, as `mc`
    /// varies every call.
    fn vacask_mc(&mut self, idx: usize, a: &Ana, tok: u32, tb: &Tb) -> Vec<String> {
        let Ana::MonteCarlo {
            name,
            runs,
            seed,
            variations,
            inner: (_, kind),
        } = a
        else {
            unreachable!("vacask_mc takes a montecarlo");
        };
        let (process, mismatch) = draw_scopes(self.rd, self.deck);
        let left_out = match variations.as_deref() {
            None | Some("process") => mismatch,
            Some("mismatch") => process,
            _ => false,
        };
        if left_out {
            self.refuse(Code::W0115, tok);
            return vec![];
        }
        let inner = tb
            .analyses
            .iter()
            .find(|(x, _)| spectre_kind(x) == Some(kind.to_ascii_lowercase().as_str()));
        let Some((inner, _)) = inner else {
            self.refuse(Code::W0015, tok);
            return vec![];
        };
        let body = self.vacask_analysis(idx, inner, tok);
        if body.is_empty() {
            return body;
        }
        let seed = seed.map(|s| format!(" seed={s}")).unwrap_or_default();
        let mut out = vec![format!(
            "mc {} samples={runs}{seed} lh=0",
            name.to_ascii_lowercase()
        )];
        out.extend(body.into_iter().map(|l| format!("  {l}")));
        out.push("endmc".into());
        out
    }

    /// A `.step`'s `sweep` line; `None` when it is refused (VACASK sweeps
    /// lin, dec or oct).
    fn step_sweep(&mut self, (sweep, param, [start, stop, step], tok): &Step) -> Option<String> {
        let name = param.to_ascii_lowercase();
        let scale = format!("step_{}", sanitize(&name));
        Some(match sweep {
            Some(mode @ ("dec" | "oct")) => format!(
                "sweep {scale} variable=\"{name}\" from={} to={} mode=\"{mode}\" points={}",
                num(*start),
                num(*stop),
                *step as u64
            ),
            Some("log") => {
                self.refuse(Code::W0016, *tok);
                return None;
            }
            _ => format!(
                "sweep {scale} variable=\"{name}\" from={} to={} step={}",
                num(*start),
                num(*stop),
                num(*step)
            ),
        })
    }
}

// ---- private ----

/// The Spectre analysis keyword of `a` (`[REF]`: an operating point is a
/// `dc` with no sweep, a transfer function `xf`); `None` for a Monte Carlo.
fn spectre_kind(a: &Ana) -> Option<&'static str> {
    Some(match a {
        Ana::Op | Ana::Dc(_) | Ana::DcAxes(_) => "dc",
        Ana::Ac(_) => "ac",
        Ana::Sp(_) => "sp",
        Ana::Tran { .. } => "tran",
        Ana::Noise { .. } => "noise",
        Ana::Tf(..) => "xf",
        Ana::Sens(..) => "sens",
        Ana::Pz(..) => "pz",
        Ana::Four(..) => "fourier",
        Ana::MonteCarlo { .. } | Ana::Spectre { .. } => return None,
    })
}

/// ` ic=["n"; v; …]`.
fn node_list(key: &str, items: &[(String, f64)]) -> String {
    let body: Vec<String> = items
        .iter()
        .map(|(n, v)| format!("\"{}\"; {}", n.to_ascii_lowercase(), num(*v)))
        .collect();
    format!(" {key}=[{}]", body.join("; "))
}

/// A DC sweep of instance `src`'s `dc` from `start` by `step` over the
/// whole intervals that fit (its last point `stop` within rounding);
/// `None` for a zero step or a count out of range.
fn sweep_line(scale: &str, src: &str, start: f64, stop: f64, step: f64) -> Option<String> {
    if step == 0.0 {
        return None;
    }
    let span = ((stop - start) / step).abs();
    let n = (span + 1e-9 * span.max(1.0)).floor();
    // `points=0` is one point, `from` (`lib/answeep.cpp`
    // `setupLinearSweep`: `end = points+1`): SPICE's `.dc v1 5 5 1`.
    if n.is_nan() || n > 1e7 {
        return None;
    }
    let last = start + (stop - start).signum() * n * step.abs();
    let last = match (last - stop).abs() <= 1e-9 * step.abs() {
        true => stop,
        false => last,
    };
    Some(format!(
        "sweep {scale} instance=\"{}\" parameter=\"dc\" from={} to={} mode=\"lin\" points={}",
        src.to_ascii_lowercase(),
        num(start),
        num(last),
        n as u64
    ))
}

/// `from= to= mode="dec" points=n` (a `lin` sweep counts intervals, not
/// points; one point is `points=0`, `lib/answeep.cpp` `setupLinearSweep`);
/// `None` for `log` or no point.
fn freq_sweep(sw: &Sweep) -> Option<String> {
    let n = match sw.variation {
        "dec" | "oct" => sw.points,
        "lin" => sw.points.saturating_sub(1),
        _ => return None,
    };
    let one = sw.variation == "lin" && sw.points == 1;
    (n != 0 || one).then(|| {
        format!(
            "from={} to={} mode=\"{}\" points={n}",
            num(sw.start),
            num(sw.stop),
            sw.variation
        )
    })
}

/// `values=[v1, v2, …]` in VACASK's numbers.
fn values(v: &[f64]) -> String {
    let v: Vec<String> = v.iter().map(|&x| num(x)).collect();
    format!("values=[{}]", v.join(", "))
}

/// VACASK identifiers carry no punctuation.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c.to_ascii_lowercase(),
            false => '_',
        })
        .collect()
}
