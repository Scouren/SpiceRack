//! A run plan whose circuit states differ in top-level parameters → the
//! states as one table the target runs the whole deck over: Xyce's
//! `.DATA` rows under `.STEP DATA=`, LTspice's index step with each
//! parameter a `table()` of the index, HSPICE's `.ALTER` blocks. Each of
//! the three runs every analysis of the deck at every state, so a table
//! holds a plan only when every state runs the same analyses; the plan
//! is then cut to the first state's runs and every writer step after it
//! sees a deck of one circuit.
//!
//! Cites: Xyce RG 7.10 §2.1.2 (`.DATA`: "each row … a different sweep
//! step, so multiple parameters can be changed simultaneously"; a
//! globally scoped `.PARAM`), §2.1.36.5 (`.STEP DATA=`); LTspice help
//! `.STEP` ("The first .step dimension is sorted to be in increasing
//! order": a list of states would lose its order and, run, its repeated
//! values), `.PARAM` (`table(x,a,b,c,d,...)`); HSPICE UG E-2010.12
//! pp.107-108 (a `.PARAM` of the same name in an `.ALTER` block replaces
//! the value; each block reruns the analyses before the first), cmdref
//! pp.32-34; DESIGN-control-epochs §3 (`StepStates`), §7 (the `.ALTER`
//! conditions); ZERO-LEDGER P5-001 step 5. Run on Xyce 7.10, LTspice
//! 26.1.1 and ESPice 0.3.0 reading HSPICE
//! (`research/impl/evidence/P5-001-s5/RESULTS.txt`): each form gives the
//! states' values in state order; ESPice keeps an `.IF` block's first
//! branch after an `.ALTER`, and the manuals do not say HSPICE picks it
//! anew, so the blocks are not written beside a conditional block.
//!
//! Contents:
//! - `pub(super) struct States`
//! - `W::take_states(&mut self, tb: &mut Tb) -> Option<States>`
//! - `W::state_value(&self, a: u32) -> Option<String>`
//! - `W::state_steps(&mut self)`, `W::alter_blocks(&mut self)`

use super::W;
use super::bench::Tb;
use crate::syntax::ast::Node;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::render::out::Space;

/// The circuit states of a deck whose plan was cut to its first state's
/// runs: one row per state in `values` (row-major, `keys.len()` wide),
/// row 0 the netlist's own values with the first state's alters applied.
/// Invariant (asserted in `States::rows`): a key, a card per key, whole
/// rows, a state.
pub(super) struct States {
    /// Each altered top-level parameter as its `.param` card names it,
    /// in the order the alters first name them.
    keys: Vec<String>,
    /// Each key's `.param` assignment.
    nodes: Vec<u32>,
    /// Each state's value of each key.
    values: Vec<f64>,
}

/// Xyce's data table of the states.
const DATA: &str = "spicerack_states";

/// LTspice's index of the state, 0 the first.
const STATE: &str = "spicerack_state";

impl W<'_, '_> {
    /// The states of `tb`'s plan, where the target's `Construct::Epochs`
    /// cell spells a second circuit state as a table of states and the
    /// plan fits one (`W::states_of`); `tb`'s plan is then its first
    /// state's runs. `None` otherwise, the plan untouched: a plan of one
    /// state, a cell that answers at the alters (`W::scripted`), or a
    /// plan no table holds, refused at each alter statement (W0130).
    pub(super) fn take_states(&mut self, tb: &mut Tb) -> Option<States> {
        let tabled = matches!(
            self.cells[Construct::Epochs as usize],
            Action::Spell(Alt::StatesData | Alt::StatesTable | Alt::AlterBlocks)
        );
        if !tabled || tb.plan.single_epoch() {
            return None;
        }
        // Paired with `plan`'s own check.
        tb.plan.check();
        let Some(states) = self.states_of(tb) else {
            let mut toks = tb.plan.alter_tok.clone();
            toks.dedup();
            for tok in toks {
                self.refuse(Code::W0130, tok);
            }
            return None;
        };
        tb.plan.keep_first_epoch();
        assert!(tb.plan.single_epoch(), "take_states: one state's runs left");
        Some(states)
    }

    /// The value the `.param` card `a` is written with when it declares a
    /// state key: the first state's, or on LTspice (several states) its
    /// table over the state index, which the index step then sets. `None`
    /// for any other card, and for every card of a deck with no states.
    pub(super) fn state_value(&self, a: u32) -> Option<String> {
        let s = self.states.as_ref()?;
        let j = s.nodes.iter().position(|&n| n == a)?;
        let column: Vec<String> = s.rows().map(|r| self.sp.decimal(r[j], self.x)).collect();
        let table = self.cells[Construct::Epochs as usize] == Action::Spell(Alt::StatesTable);
        if !table || column.len() == 1 {
            return Some(column[0].clone());
        }
        let points: Vec<String> = column
            .iter()
            .enumerate()
            .map(|(k, v)| format!("{k},{v}"))
            .collect();
        Some(format!("table({STATE},{})", points.join(",")))
    }

    /// The step over the states, before the analyses: Xyce's data table
    /// and its `.step data=`, LTspice's index step. Nothing for one state
    /// (its values are the `.param` cards': [`W::state_value`]), for
    /// HSPICE ([`W::alter_blocks`]) and for a deck with no states.
    pub(super) fn state_steps(&mut self) {
        let Some(s) = &self.states else { return };
        let count = s.rows().len();
        if count < 2 {
            return;
        }
        let cell = self.cells[Construct::Epochs as usize];
        let mut lines: Vec<String> = Vec::new();
        if cell == Action::Spell(Alt::StatesData) {
            lines.push(format!(".data {DATA}"));
            lines.push(format!("+ {}", s.keys.join(" ")));
            for row in s.rows() {
                let vals: Vec<String> = row.iter().map(|v| self.sp.decimal(*v, self.x)).collect();
                lines.push(format!("+ {}", vals.join(" ")));
            }
            lines.push(".enddata".to_string());
            lines.push(format!(".step data={DATA}"));
        } else if cell == Action::Spell(Alt::StatesTable) {
            lines.push(format!(".step param {STATE} 0 {} 1", count - 1));
        }
        for l in &lines {
            self.o.token(l, Space::Line);
        }
    }

    /// HSPICE's `.alter` blocks, after the analyses: one per later state,
    /// each setting every state parameter, which is the state whether the
    /// blocks accumulate or each starts from the netlist (UG pp.107-108
    /// does not say; ESPice accumulates, run). Nothing for another
    /// target, for one state and for a deck with no states.
    pub(super) fn alter_blocks(&mut self) {
        let Some(s) = &self.states else { return };
        if self.cells[Construct::Epochs as usize] != Action::Spell(Alt::AlterBlocks) {
            return;
        }
        let mut lines: Vec<String> = Vec::new();
        for row in s.rows().skip(1) {
            let sets = s.keys.iter().zip(row);
            let sets: Vec<String> = sets
                .map(|(k, v)| format!("{k}={}", self.sp.decimal(*v, self.x)))
                .collect();
            lines.push(".alter".to_string());
            lines.push(format!(".param {}", sets.join(" ")));
        }
        for l in &lines {
            self.o.token(l, Space::Line);
        }
    }
}

// ---- private ----

impl States {
    /// The states' rows, the first state first.
    fn rows(&self) -> std::slice::Chunks<'_, f64> {
        assert!(
            !self.keys.is_empty() && !self.values.is_empty(),
            "states: a key, a state"
        );
        assert_eq!(self.keys.len(), self.nodes.len(), "states: a card a key");
        assert_eq!(self.values.len() % self.keys.len(), 0, "states: whole rows");
        self.values.chunks(self.keys.len())
    }
}

impl W<'_, '_> {
    /// The table of states of `tb`'s plan, `None` when no table holds
    /// it: a state runs other analyses than the first (the target runs
    /// them all at every state); the deck steps something itself; an
    /// altered parameter is not declared exactly once at the deck's top
    /// with one number, or a top parameter has the state index's name;
    /// HSPICE's blocks would sit beside a conditional block (module
    /// header).
    fn states_of(&self, tb: &Tb) -> Option<States> {
        let plan = &tb.plan;
        let epochs = plan.epoch_alters.len() - 1;
        // ponytail: `Ana` has no `PartialEq`, so two analyses compare by
        // their `Debug` text, which prints every field; derive
        // `PartialEq` if an `Ana` field ever prints less than it holds.
        let runs_of = |k: usize| -> Vec<String> {
            let runs = plan.runs.iter().zip(&plan.run_epoch);
            let runs = runs.filter(|(_, e)| **e as usize == k);
            runs.map(|(a, _)| format!("{a:?}")).collect()
        };
        let first = runs_of(0);
        let stepped = !tb.steps.is_empty() || !tb.step_lists.is_empty();
        if stepped || (1..epochs).any(|k| runs_of(k) != first) {
            return None;
        }
        // HSPICE beside a conditional block: lift this `if` when HSPICE
        // (or ESPice reading it) is shown to pick the branch anew.
        let blocks = self.cells[Construct::Epochs as usize] == Action::Spell(Alt::AlterBlocks);
        if blocks && epochs > 1 && self.conditional() {
            return None;
        }
        let name = |a: u32| self.rd.param_view(a).0;
        let top = &self.deck.top.params;
        if top.iter().any(|&a| name(a).eq_ignore_ascii_case(STATE)) {
            return None;
        }
        // The columns, and the row the netlist itself declares.
        let mut keys: Vec<String> = Vec::new();
        let mut nodes: Vec<u32> = Vec::new();
        let mut row: Vec<f64> = Vec::new();
        for key in &plan.alter_key {
            if keys.iter().any(|k| k.eq_ignore_ascii_case(key)) {
                continue;
            }
            let mut cards = top.iter().filter(|&&a| name(a).eq_ignore_ascii_case(key));
            let (Some(&a), None) = (cards.next(), cards.next()) else {
                return None;
            };
            row.push(self.rd.number(self.rd.ast.rhs(Node(a)))?);
            keys.push(name(a));
            nodes.push(a);
        }
        // Each state is the one before it with its own alters.
        let mut values: Vec<f64> = Vec::with_capacity(epochs * keys.len());
        for w in plan.epoch_alters.windows(2) {
            let delta = w[0] as usize..w[1] as usize;
            let sets = plan.alter_key[delta.clone()]
                .iter()
                .zip(&plan.alter_value[delta]);
            for (key, v) in sets {
                let j = keys.iter().position(|k| k.eq_ignore_ascii_case(key));
                row[j.expect("states_of: every alter's key is a column")] = *v;
            }
            values.extend_from_slice(&row);
        }
        assert!(
            !keys.is_empty(),
            "states_of: a second state alters a parameter"
        );
        assert_eq!(
            values.len(),
            epochs * keys.len(),
            "states_of: a row a state"
        );
        Some(States {
            keys,
            nodes,
            values,
        })
    }

    /// Whether the deck holds a conditional block: at its top, in a
    /// subcircuit, or in a library section or a subcircuit of one.
    fn conditional(&self) -> bool {
        let d = self.deck;
        let sections = d.sections.iter();
        let sections = sections.flat_map(|s| std::iter::once(&s.body).chain(&s.defs));
        let mut blocks = std::iter::once(&d.top).chain(&d.defs).chain(sections);
        blocks.any(|b| !b.conds.is_empty())
    }
}
