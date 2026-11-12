//! The deck's `.step`s → the SPICE family's `.step` cards, one a `.step`,
//! in read order. Xyce and LTspice loop the first card innermost, so the
//! cards keep the order the source's simulator ran its loops in, whether
//! a card is a range or a list. On LTspice a range ends on its stop
//! value, so one whose grid ends short of it is written as the list of
//! its points.
//!
//! Cites: Xyce RG 7.10 §2.1.36 (`.STEP`: "the first .STEP line … will be
//! the innermost loop"; lin, dec, oct or a list), p.155; LTspice help
//! `.STEP` (its item: `param <name>`, `temp`, an independent source by
//! its name; lin, oct, dec or a list); HSPICE
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
use crate::syntax::render::swept::{Swept, step_makes, step_swept};

impl W<'_, '_> {
    /// The deck's `.step` cards in read order, ranges and lists as they
    /// stood among each other: the first card is the innermost loop.
    pub(super) fn step_cards(&mut self, tb: &Tb) {
        let mut cards = self.steps(tb);
        cards.extend(self.step_lists(tb));
        // A card a `.step`, and the declaration of what it made
        // (`W::step_decl`) ahead of it: the sort below is stable.
        assert!(
            cards.len() <= 2 * (tb.steps.len() + tb.step_lists.len()),
            "step_cards: a card and a declaration a `.step` at most"
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
    /// The ranged `.step`s' cards, each with its first token and its
    /// target's item ([`W::step_item`]); a total-count `log` refused
    /// (W0016).
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
            let Some(item) = self.step_item(param, *tok) else {
                continue;
            };
            let line = match (short, sweep) {
                (Some((points, _)), _) => self.list_card(&item, &points),
                (None, Some(s)) => format!(".step {s} {item} {a} {b} {c}"),
                (None, None) => format!(".step {item} {a} {b} {c}"),
            };
            out.extend(self.step_decl(param, &a.to_string()).map(|d| (*tok, d)));
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
            if let Some(item) = self.step_item(param, *tok) {
                let decl = vals.first().and_then(|v| self.step_decl(param, v));
                out.extend(decl.map(|d| (*tok, d)));
                out.push((*tok, self.list_card(&item, values)));
            }
        }
        out
    }

    /// A list `.step` card of `item` ([`W::step_item`]): Xyce RG `.STEP
    /// <name> LIST`; LTspice help `.STEP`, `.step <item> list`.
    fn list_card(&self, item: &str, values: &[f64]) -> String {
        let vals: Vec<String> = values.iter().map(|v| self.sp.number(*v, self.x)).collect();
        format!(".step {item} list {}", vals.join(" "))
    }

    /// The item a `.step` over `param` names on the target: the name
    /// itself (Xyce RG `.STEP`); on LTspice (help `.STEP`: "Temperature:
    /// temp", "Parameter RLOAD: param RLOAD", "Voltage source v1: v1")
    /// `temp`, `param <name>`, or a top-level V or I card with no waveform
    /// by its name ([`step_swept`]). `None`, refused (W0016), where
    /// LTspice has no item: a source with a waveform ("Can't sweep that.",
    /// 26.1.1), an R, C or L, another device, a device's or model's
    /// parameter (`.step param R1:R` steps a parameter no card reads:
    /// run, `research/impl/evidence/P3-134/run.txt`).
    // ponytail: a stepped R, C or L is refused; its lift is the card's
    // value as `{<name>}` beside `.step param <name>` when a deck needs it.
    fn step_item(&mut self, param: &str, tok: u32) -> Option<String> {
        if self.b.analysis_form != AnalysisForm::Ltspice {
            return Some(param.to_string());
        }
        match step_swept(self.rd, self.deck, &self.names, param) {
            Swept::Temp | Swept::Source { plain: true } => Some(param.to_string()),
            Swept::Param | Swept::Unseen => Some(format!("param {param}")),
            Swept::Source { plain: false } | Swept::Value(_) | Swept::Other => {
                self.refuse(Code::W0016, tok);
                None
            }
        }
    }

    /// `.param <name>=<first>` ahead of a `.step` that makes the parameter
    /// it steps ([`step_makes`]: LTspice's `.step param X` with no `.param
    /// X`), on a target whose own `.step` does not: Xyce steps a declared
    /// parameter (7.10, run: "contains unrecognized symbol: X" without the
    /// card, LTspice's numbers with it). LTspice makes its own. `first` is
    /// the step's first value as its card spells it.
    fn step_decl(&self, name: &str, first: &str) -> Option<String> {
        let makes = self.b.analysis_form != AnalysisForm::Ltspice
            && step_makes(self.rd, self.deck, &self.names, name);
        makes.then(|| format!(".param {name}={first}"))
    }
}
