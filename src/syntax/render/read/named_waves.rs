//! A Spectre or VACASK source's waveform parameters → its [`Wave`] (or
//! none, for a DC source): pulse, sine, exp, am, fm and Spectre's pwl, as
//! the old lowering read them.
//!
//! Cites: the old lowering's `named_waveform`; Spectre Circuit Simulator
//! Reference 5.0 `[CMP]` (`vsource` #4, #17, #18, p.686); VACASK
//! `docs/dev-builtin-src.md`.
//!
//! Contents:
//! - `Rd::named_waveform(&self, p: &[(String, u32)]) -> Option<Option<Wave>>`

use super::{Num, Rd, Wave, WaveKind};
use crate::syntax::behaviour::NamedRules;

impl<'a> Rd<'a> {
    /// the old lowering's `named_waveform`.
    pub(super) fn named_waveform(&self, p: &[(String, u32)]) -> Option<Option<Wave>> {
        // A sine's damping key (VACASK `theta`).
        let damp = match self.b.named_rules {
            NamedRules::Vacask => "theta",
            _ => "damp",
        };
        let f = |k: &str| {
            self.get(p, k)
                .map_or(Some(Num::Num(0.0)), |v| self.irnum(v))
        };
        let ty = self
            .get(p, "type")
            .map(|t| self.text(t).trim_matches('"').to_string());
        let fm = ty.as_deref() == Some("fm")
            || (ty.as_deref() == Some("sine") && self.get(p, "fmmodindex").is_some());
        let spectre_exp = self.b.named_rules == NamedRules::Spectre && ty.as_deref() == Some("exp");
        if (fm || spectre_exp)
            && self
                .get(p, "delay")
                .is_some_and(|v| self.number(v) != Some(0.0))
        {
            return None;
        }
        if ty.as_deref() != Some("pwl") && self.get(p, "pwlperiod").is_some() {
            return None;
        }
        Some(Some(match ty.as_deref() {
            None | Some("dc") => return Some(None),
            Some("pulse") => Wave {
                kind: WaveKind::Pulse,
                args: vec![
                    f("val0")?,
                    f("val1")?,
                    f("delay")?,
                    f("rise")?,
                    f("fall")?,
                    f("width")?,
                    f("period")?,
                ],
                repeat: None,
                delay: None,
                cut: false,
            },
            Some("sine") if self.get(p, "fmmodindex").is_some() => Wave {
                kind: WaveKind::Sffm,
                args: vec![
                    f("sinedc")?,
                    f("ampl")?,
                    f("freq")?,
                    f("fmmodindex")?,
                    f("fmmodfreq")?,
                ],
                repeat: None,
                delay: None,
                cut: false,
            },
            Some("sine") => Wave {
                kind: WaveKind::Sin,
                args: vec![
                    f("sinedc")?,
                    f("ampl")?,
                    f("freq")?,
                    f("delay")?,
                    f(damp)?,
                    f("sinephase")?,
                ],
                repeat: None,
                delay: None,
                cut: false,
            },
            Some("exp") if self.b.named_rules == NamedRules::Vacask => {
                let n = |k: &str| self.get(p, k).map_or(Some(0.0), |v| self.number(v));
                Wave {
                    kind: WaveKind::Exp,
                    args: vec![
                        f("val0")?,
                        f("val1")?,
                        Num::Num(n("delay")?),
                        f("tau1")?,
                        Num::Num(n("delay")? + n("td2")?),
                        f("tau2")?,
                    ],
                    repeat: None,
                    delay: None,
                    cut: false,
                }
            }
            Some("exp") => Wave {
                kind: WaveKind::Exp,
                args: vec![
                    f("val0")?,
                    f("val1")?,
                    f("td1")?,
                    f("tau1")?,
                    f("td2")?,
                    f("tau2")?,
                ],
                repeat: None,
                delay: None,
                cut: false,
            },
            Some("fm") => Wave {
                kind: WaveKind::Sffm,
                args: vec![
                    f("sinedc")?,
                    f("ampl")?,
                    f("freq")?,
                    f("modindex")?,
                    f("modfreq")?,
                ],
                repeat: None,
                delay: None,
                cut: false,
            },
            Some("am") => Wave {
                kind: WaveKind::Am,
                args: vec![
                    f("ampl")?,
                    f("sinedc")?,
                    f("modfreq")?,
                    f("freq")?,
                    f("delay")?,
                ],
                repeat: None,
                delay: None,
                cut: false,
            },
            Some("pwl") if self.b.named_rules == NamedRules::Spectre => {
                let w = self.get(p, "wave")?;
                let xs: Vec<Num> = self
                    .velems(w)
                    .into_iter()
                    .map(|x| self.irnum(x))
                    .collect::<Option<_>>()?;
                if !xs.len().is_multiple_of(2) {
                    return None;
                }
                let repeat = match self.get(p, "pwlperiod") {
                    Some(v) if pwl_period(&xs)? == self.number(v)? => Some(Num::Num(0.0)),
                    Some(_) => return None,
                    None => None,
                };
                let delay = match self.get(p, "delay") {
                    Some(v) => Some(self.irnum(v)?).filter(|d| *d != Num::Num(0.0)),
                    None => None,
                };
                Wave {
                    kind: WaveKind::Pwl,
                    args: xs,
                    repeat,
                    delay,
                    cut: false,
                }
            }
            Some(_) => return None,
        }))
    }
}

// ---- private ----

/// The Spectre `pwlperiod` that repeats PWL points `xs` (time, value flat)
/// exactly as ngspice's `r=0` does, when one does: numeric points from
/// t = 0 whose last value equals the first, the last segment at least the
/// default `twidth` (period/1000) long (Spectre Reference 5.0, `vsource`
/// #17, #18, p.686). `None` otherwise.
fn pwl_period(xs: &[Num]) -> Option<f64> {
    let n = xs.len();
    if n < 4 {
        return None;
    }
    let (t0, v0) = (xs[0].num()?, xs[1].num()?);
    let (tp, tn, vn) = (xs[n - 4].num()?, xs[n - 2].num()?, xs[n - 1].num()?);
    (t0 == 0.0 && vn == v0 && tn > 0.0 && tn - tp >= tn / 1000.0).then_some(tn)
}
