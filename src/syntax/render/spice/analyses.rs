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
use super::sweeps::{dc_axis, has_ac, xyce_lin};
use crate::syntax::behaviour::AnalysisForm;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::read::{Ana, Card, Comp};

impl W<'_, '_> {
    /// `a`'s card (HSPICE's `.tran` with a step cap and `.noise` take two
    /// lines); an analysis the target lacks is refused, written empty.
    pub(super) fn analysis(&mut self, a: &Ana, tok: u32) -> String {
        let num = |w: &Self, v: f64| w.sp.number(v, w.x);
        let hs = self.b.analysis_form == AnalysisForm::Hspice;
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
                    let Some(src) = self.dc_source(src, tok) else {
                        return String::new();
                    };
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
                    // Loops of ngspice's control script around one `dc`
                    // (`loops.rs`; `run_analyses` runs them from the
                    // block); a `.dc` the script has no run for is refused.
                    Action::Spell(Alt::ControlScript) => {
                        let script = self.dc_loops(axes);
                        if script.is_none() {
                            self.refuse(Code::W0038, tok);
                        }
                        return script.unwrap_or_default();
                    }
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
                // START is when printing begins, 0 unless given (cmdref
                // `.TRAN`): a zero start is the card without the word.
                if let Some(st) = start.filter(|&st| st != 0.0) {
                    s.push_str(&format!(" START={}", num(self, st)));
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
                // simulator's choice"): one from the deck's sources
                // (`tstep`).
                let zero = *step <= 0.0
                    && matches!(
                        self.b.analysis_form,
                        AnalysisForm::Ngspice | AnalysisForm::Espice
                    );
                // No span to choose from: Xyce 7.10 aborts on such a
                // `.tran` itself, and span/50 would be a step ngspice
                // stops on ("TSTEP is invalid").
                let span = stop - start.unwrap_or(0.0);
                if zero && (span.is_nan() || span <= 0.0) {
                    self.refuse(Code::W0141, tok);
                    return String::new();
                }
                let step = if zero { self.chosen_step(span) } else { *step };
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
                if *startup && self.cells[Construct::TranStartup as usize] == Action::Native {
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
                    let ac = match has_ac(self, sw) {
                        true => String::new(),
                        false => format!(".ac {}\n", sweep(self, sw)),
                    };
                    return format!("{ac}.noise {out} {source} {}", pps.unwrap_or(0));
                }
                let mut s = format!(".noise {out} {source} {}", sweep(self, sw));
                // `pts_per_summary` is an ngspice/SPICE3 extension; LTspice's
                // `.noise` card stops at the stop frequency (W0033).
                match (pps, self.b.analysis_form) {
                    (Some(_), AnalysisForm::Ltspice) => self.note(Code::W0033, tok),
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
                // HSPICE's `.PZ` finds poles and zeros both, from a source
                // standing across the input pair.
                Action::Spell(Alt::HspicePz) => match self.across(&nodes[0], &nodes[1], tf) {
                    Some(src) if *pz == "pz" => {
                        let out = match nodes[3].as_str() {
                            "0" => format!("v({})", nodes[2]),
                            r => format!("v({},{r})", nodes[2]),
                        };
                        format!(".pz {out} {src}")
                    }
                    _ => {
                        self.refuse(Code::W0015, tok);
                        String::new()
                    }
                },
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
                    crate::syntax::render::read::spectre_refusal(self.b, kind),
                    tok,
                );
                String::new()
            }
        }
    }
}

// ---- private ----

impl W<'_, '_> {
    /// The name of the top's V (`vol`) or I (`cur`) source from `a` to
    /// `b`, as written.
    fn across(&self, a: &str, b: &str, tf: &str) -> Option<String> {
        let want = if tf == "vol" { 'V' } else { 'I' };
        self.deck.top.comps.iter().find_map(|c| {
            let Comp::Card(n) = *c else { return None };
            let Card::Source { letter, np, nm, .. } = self.rd.card_view(n) else {
                return None;
            };
            let across = np.eq_ignore_ascii_case(a) && nm.eq_ignore_ascii_case(b);
            (letter == want && across).then(|| self.names.decl(n).to_string())
        })
    }

    /// A Xyce deck's `.dc` sweep variable `src` as an ngspice or ESPice
    /// target reads it; any other pair writes it as read. Xyce sweeps a
    /// device parameter (`R1:R`) and a `.param`, which wins over a source
    /// of its name (Xyce RG §2.1.3); ngspice sweeps a source, a resistor
    /// by its name or `temp` (ngspice manual §11.3.2); ESPice sweeps a
    /// parameter as Xyce does. A top resistor's `:R` is the resistor;
    /// another device parameter, and a parameter for ngspice, is an
    /// analysis the target lacks (W0015): `None`.
    fn dc_source(&mut self, src: &str, tok: u32) -> Option<String> {
        let to = self.b.analysis_form;
        let family = matches!(to, AnalysisForm::Ngspice | AnalysisForm::Espice);
        if self.rd.b.analysis_form != AnalysisForm::Xyce || !family {
            return Some(src.to_string());
        }
        let top = crate::syntax::ast::NONE;
        let named = |a: &u32| self.rd.param_view(*a).0.eq_ignore_ascii_case(src);
        let param = self.deck.top.params.iter().any(named);
        let found = match src.split_once(':') {
            Some((dev, p)) if p.eq_ignore_ascii_case("r") => {
                self.names.reference(top, dev, 'R').map(str::to_string)
            }
            Some(_) => None,
            None if param && to == AnalysisForm::Ngspice => None,
            None => Some(src.to_string()),
        };
        if found.is_none() {
            self.refuse(Code::W0015, tok);
        }
        found
    }
}
