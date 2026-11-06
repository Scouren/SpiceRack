//! A testbench analysis → its SPICE-family card: `.op`, `.dc` (log and
//! list axes in the target's form), `.ac`, `.tran`, `.noise`, `.tf`,
//! `.sens`, `.pz`, `.sp`, `.four`, and the refusal of one the target lacks.
//!
//! Cites: ngspice manual §11.1-§11.3 (analyses); HSPICE `.DC`, `START=`;
//! Xyce RG `.DC`, `.STEP`, Table 6-1; LTspice help `.tran`.
//!
//! Contents:
//! - `W::analysis(&mut self, a: &Ana, tok: u32) -> String`

use super::W;
use super::sweeps::{dc_axis, xyce_lin};
use crate::syntax::Dialect;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct, action};
use crate::syntax::render::read::Ana;

impl W<'_, '_> {
    /// `a`'s card (HSPICE's `.tran` with a step cap and `.noise` take two
    /// lines); an analysis the target lacks is refused, written empty.
    pub(super) fn analysis(&mut self, a: &Ana, tok: u32) -> String {
        let num = |w: &Self, v: f64| w.sp.number(v, w.x);
        let hs = self.d == Dialect::Hspice;
        let sweep = |w: &Self, s: &crate::syntax::render::read::Sweep| {
            format!(
                "{} {} {} {}",
                s.variation,
                s.points,
                num(w, s.start),
                num(w, s.stop)
            )
        };
        // Spectre's total-count `log` sweep: the SPICE targets have only
        // `dec`, `oct` and `lin` (W0031).
        let log = match a {
            Ana::Ac(sw) | Ana::Sp(sw) | Ana::Noise { sweep: sw, .. } | Ana::Sens(_, Some(sw)) => {
                sw.variation.eq_ignore_ascii_case("log")
            }
            _ => false,
        };
        if log {
            self.refuse(Code::W0031, tok);
            return String::new();
        }
        match a {
            Ana::Op => ".op".into(),
            Ana::Dc(sweeps) => {
                let mut s = String::from(".dc");
                for (src, a, b, c) in sweeps {
                    s.push_str(&format!(
                        " {src} {} {} {}",
                        num(self, *a),
                        num(self, *b),
                        num(self, *c)
                    ));
                }
                s
            }
            Ana::DcAxes(axes) => {
                let hspice = match self.ask(Construct::DcLogList, tok) {
                    Action::Native => false,
                    Action::Spell(Alt::DcType) => true,
                    _ => return String::new(),
                };
                let mut s = String::from(".dc");
                for ax in axes {
                    s.push_str(&dc_axis(self, ax, hspice));
                }
                s
            }
            Ana::Ac(sw) => format!(".ac {}", sweep(self, sw)),
            // HSPICE reads extra positional fields as further step/stop
            // segments, so the start is `START=` and the step cap `delmax`.
            Ana::Tran {
                step,
                stop,
                start,
                max_step,
                uic,
                ..
            } if hs => {
                let mut s = String::new();
                if let Some(ms) = max_step {
                    s.push_str(&format!(".option delmax={}\n", num(self, *ms)));
                }
                s.push_str(&format!(".tran {} {}", num(self, *step), num(self, *stop)));
                if let Some(st) = start {
                    s.push_str(&format!(" START={}", num(self, *st)));
                }
                if *uic {
                    s.push_str(" UIC");
                }
                s
            }
            Ana::Tran {
                step,
                stop,
                start,
                max_step,
                uic,
                startup,
            } => {
                // ngspice stops on a zero step (Xyce's and LTspice's "the
                // simulator's choice"): the span over 50, ngspice's own
                // default step cap (manual §11.3.10).
                let zero = *step <= 0.0 && self.d.family() == Dialect::Ngspice;
                let step = if zero {
                    (stop - start.unwrap_or(0.0)) / 50.0
                } else {
                    *step
                };
                let mut s = format!(".tran {} {}", num(self, step), num(self, *stop));
                if let Some(st) = start {
                    s.push_str(&format!(" {}", num(self, *st)));
                }
                if let Some(ms) = max_step {
                    if start.is_none() {
                        s.push_str(" 0");
                    }
                    s.push_str(&format!(" {}", num(self, *ms)));
                }
                if *uic {
                    s.push_str(" UIC");
                }
                // Refused on a target without it (`W::checks`).
                if *startup && action(self.d, Construct::TranStartup) == Action::Native {
                    s.push_str(" startup");
                }
                s
            }
            Ana::Noise {
                output,
                reference,
                source,
                sweep: sw,
                pps,
            } => {
                let out = if reference.is_empty() || reference == "0" {
                    format!("V({output})")
                } else {
                    format!("V({output},{reference})")
                };
                // HSPICE: `.noise ov src inter` takes its frequencies from
                // `.ac`.
                if hs {
                    return format!(
                        ".ac {}\n.noise {out} {source} {}",
                        sweep(self, sw),
                        pps.unwrap_or(0)
                    );
                }
                let mut s = format!(".noise {out} {source} {}", sweep(self, sw));
                // `pts_per_summary` is an ngspice/SPICE3 extension; LTspice's
                // `.noise` card stops at the stop frequency (W0033).
                match (pps, self.d) {
                    (Some(_), Dialect::Ltspice) => self.note(Code::W0033, tok),
                    (Some(p), _) => s.push_str(&format!(" {p}")),
                    (None, _) => {}
                }
                s
            }
            Ana::Tf(out, src) => match self.ask(Construct::Tf, tok) {
                Action::Native => format!(".tf {out} {src}"),
                _ => String::new(),
            },
            Ana::Four(f, outs) => {
                let mut s = format!(".four {}", num(self, *f));
                for o in outs {
                    s.push_str(&format!(" {o}"));
                }
                s
            }
            Ana::Sens(out, ac) => {
                let c = if ac.is_some() {
                    Construct::SensAc
                } else {
                    Construct::Sens
                };
                if self.ask(c, tok) != Action::Native {
                    return String::new();
                }
                let mut s = format!(".sens {out}");
                if let Some(sw) = ac {
                    s.push_str(&format!(" AC {}", sweep(self, sw)));
                }
                s
            }
            Ana::Pz(nodes, tf, pz) => match self.ask(Construct::PoleZero, tok) {
                Action::Native => format!(".pz {} {tf} {pz}", nodes.join(" ")),
                _ => String::new(),
            },
            Ana::Sp(sw) => match self.ask(Construct::SParameter, tok) {
                Action::Spell(Alt::XyceLin) => xyce_lin(self, sw, tok, &sweep(self, sw)),
                // ponytail: Rust `{}` numbers, byte for byte what the old
                // writer sent ngspice for this card.
                Action::Native => format!(
                    ".sp {} {} {} {}",
                    sw.variation, sw.points, sw.start, sw.stop
                ),
                _ => String::new(),
            },
            // No SPICE statement is Spectre's `montecarlo`: ngspice's Monte
            // Carlo is a `.control` loop, Xyce's `.sampling`, HSPICE's an
            // analysis's `sweep monte=`.
            Ana::MonteCarlo { .. } => {
                self.refuse(Code::W0015, tok);
                String::new()
            }
            Ana::Spectre { kind, .. } => {
                self.refuse(
                    crate::syntax::render::read::spectre_refusal(self.d, kind),
                    tok,
                );
                String::new()
            }
        }
    }
}
