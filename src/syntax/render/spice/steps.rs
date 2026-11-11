//! The deck's `.step`s → the SPICE family's `.step` cards, one a `.step`,
//! in read order. Xyce and LTspice loop the first card innermost, so the
//! cards keep the order the source's simulator ran its loops in, whether
//! a card is a range or a list. On LTspice a range ends on its stop
//! value, so one whose grid ends short of it is written as the list of
//! its points.
//!
//! Cites: Xyce RG 7.10 §2.1.36 (`.STEP`: "the first .STEP line … will be
//! the innermost loop"; lin, dec, oct or a list), p.155; LTspice help
//! `.step` (`.step param`, `temp` bare; lin, oct, dec or a list); HSPICE
//! UG `.TEMP t1 t2 …`. Run on Xyce 7.10 and LTspice 26.1.1
//! (`research/impl/evidence/P5-100/RESULTS.txt`): a range and a list
//! loop in card order, in either order; LTspice's grid
//! (`research/impl/evidence/P5-099/RESULTS.txt`).
//!
//! Contents:
//! - `W::step_cards(&mut self, tb: &Tb)`

use super::W;
use super::bench::Tb;
use crate::syntax::behaviour::AnalysisForm;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::step_points;

impl W<'_, '_> {
    /// The deck's `.step` cards in read order, ranges and lists as they
    /// stood among each other: the first card is the innermost loop.
    pub(super) fn step_cards(&mut self, tb: &Tb) {
        let mut cards = self.steps(tb);
        cards.extend(self.step_lists(tb));
        assert!(
            cards.len() <= tb.steps.len() + tb.step_lists.len(),
            "step_cards: a card a `.step` at most"
        );
        // Read order: a token's index grows along the deck.
        cards.sort_by_key(|c| c.0);
        for (_, line) in &cards {
            self.o.token(line, Space::Line);
        }
    }
}

// ---- private ----

impl W<'_, '_> {
    /// The ranged `.step`s' cards, each with its first token (Xyce's
    /// `.step <param>`, the others' `.step param`); a total-count `log`
    /// refused (W0016).
    fn steps(&mut self, tb: &Tb) -> Vec<(u32, String)> {
        let mut out = Vec::with_capacity(tb.steps.len());
        for (sweep, param, [a, b, c], tok) in &tb.steps {
            if self.ask(Construct::Step, *tok) != Action::Native {
                continue;
            }
            // A total-count `log`: Xyce's `.step` sweeps lin, dec, oct or
            // a list (RG p.155), LTspice's lin, oct, dec or a list.
            if *sweep == Some("log") {
                self.refuse(Code::W0016, *tok);
                continue;
            }
            // LTspice ends a range on its stop value (26.1.1, run): a grid
            // that ends short of it is the list of its points. LTspice
            // sorts a list (help `.STEP`), so only an ascending one.
            // ponytail: a downward range is still written as a range,
            // which LTspice runs upward (ZERO-LEDGER P5-104).
            let lt = self.b.analysis_form == AnalysisForm::Ltspice;
            let short = step_points(*sweep, [*a, *b, *c])
                .filter(|(points, short)| lt && *short && points.is_sorted());
            let line = match (short, self.b.analysis_form, sweep) {
                (Some((points, _)), ..) => self.list_card(param, &points),
                (None, AnalysisForm::Xyce, Some(s)) => format!(".step {s} {param} {a} {b} {c}"),
                (None, AnalysisForm::Xyce, None) => format!(".step {param} {a} {b} {c}"),
                (None, _, Some(s)) => format!(".step {s} param {param} {a} {b} {c}"),
                (None, _, None) => format!(".step param {param} {a} {b} {c}"),
            };
            out.push((*tok, line));
        }
        out
    }

    /// The list `.step`s' cards, each with its first token; HSPICE's
    /// temperature list as `.temp t1 t2 …`.
    fn step_lists(&mut self, tb: &Tb) -> Vec<(u32, String)> {
        let mut out = Vec::with_capacity(tb.step_lists.len());
        for (param, values, tok) in &tb.step_lists {
            let act = self.ask(Construct::StepList, *tok);
            let vals: Vec<String> = values.iter().map(|v| self.sp.number(*v, self.x)).collect();
            let temp = param.eq_ignore_ascii_case("temp");
            // HSPICE UG `.TEMP t1 t2 …`: the analyses at each temperature.
            if act == Action::Spell(Alt::TempList) {
                match temp {
                    true => out.push((*tok, format!(".temp {}", vals.join(" ")))),
                    false => self.refuse(Code::W0016, *tok),
                }
                continue;
            }
            if act != Action::Native {
                continue;
            }
            out.push((*tok, self.list_card(param, values)));
        }
        out
    }

    /// A list `.step` card: Xyce RG `.STEP <name> LIST`; LTspice help
    /// `.step param <name> list`, `temp` bare.
    fn list_card(&self, param: &str, values: &[f64]) -> String {
        let vals: Vec<String> = values.iter().map(|v| self.sp.number(*v, self.x)).collect();
        let name = match (self.b.analysis_form, param.eq_ignore_ascii_case("temp")) {
            (AnalysisForm::Ltspice, false) => format!("param {param}"),
            _ => param.to_string(),
        };
        format!(".step {name} list {}", vals.join(" "))
    }
}
