//! A run plan with a second circuit state → the target's answer to it
//! and ngspice's control-script lines: before the first run of an epoch,
//! each top-level parameter its delta sets (`alterparam`), then `reset`,
//! which reads the deck again with the new values (an `.if` on one picks
//! its branch anew).
//!
//! Cites: ngspice manual §13.5.5 (`alterparam`: followed by `reset`),
//! §13.5.65 (`reset`); DESIGN-control-epochs §3 (the spellings by
//! operation); ZERO-LEDGER P5-001 step 2. Run on ngspice-45
//! (`research/impl/evidence/P5-001-s2/`): the key is read in any case,
//! and a parameter set by `alterparam` stays set across later `reset`s,
//! so an epoch writes its own delta only.
//!
//! Contents:
//! - `W::scripted(&mut self, plan: &Plan) -> bool`
//! - `W::alter_lines(&mut self, plan: &Plan, i: usize)`

use super::W;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::epochs::Plan;

impl W<'_, '_> {
    /// Whether the target runs `plan`'s epochs from a control script: its
    /// `Construct::Epochs` cell, asked once at each alter statement (a
    /// refusal or a note is recorded there).
    pub(super) fn scripted(&mut self, plan: &Plan) -> bool {
        assert!(!plan.single_epoch(), "scripted: a second circuit state");
        let mut toks = plan.alter_tok.clone();
        toks.dedup();
        assert!(!toks.is_empty(), "scripted: an alter statement");
        let mut act = Action::Native;
        for tok in toks {
            act = self.ask(Construct::Epochs, tok);
        }
        act == Action::Spell(Alt::ControlScript)
    }

    /// The script lines run `i` of `plan` starts with: none unless it is
    /// its epoch's first run and the epoch has a delta; else each
    /// parameter of the delta, then `reset`.
    pub(super) fn alter_lines(&mut self, plan: &Plan, i: usize) {
        let rows = plan.delta(i);
        if rows.is_empty() {
            return;
        }
        for k in rows {
            let v = self.sp.decimal(plan.alter_value[k], self.x);
            let line = format!("alterparam {}={v}", plan.alter_key[k]);
            self.o.token(&line, Space::Line);
        }
        self.o.token("reset", Space::Line);
    }
}
