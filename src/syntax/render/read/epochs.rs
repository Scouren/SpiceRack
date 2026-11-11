//! A testbench's plan cards in read order → the run [`Plan`]: every
//! analysis in run order, each tagged with the circuit state (epoch) it
//! runs on, and the alters that make each state. An epoch is the one
//! before it (the netlist as written, for epoch 0) and its alter delta;
//! an alter after a run opens the next epoch. A plan with no alter is one
//! epoch, and the writers take the path they took before the plan (the
//! single-epoch invariant).
//!
//! The table grows a column with the reader that fills it: the alter
//! rows hold top-level parameters (VACASK's `alter` of `__topinst__`, an
//! ngspice `.control` script's `alterparam` at its `reset`);
//! instance and model alters and the loop rows come with their readers
//! (ZERO-LEDGER P5-050, P5-001 step 3).
//!
//! Cites: DESIGN-control-epochs §2 (the six questions, the contracts,
//! N4), §6 commits 1 and 2 (ZERO-LEDGER P5-001); ngspice manual §13.5.5
//! (`alterparam`) and VACASK `docs/cmd-alter.md` (what a later epoch is).
//!
//! Contents:
//! - `pub(in crate::syntax::render) struct Plan`, `impl Default for Plan`
//! - `pub(in crate::syntax::render) fn plan(cards: Vec<(BenchCard, u32)>) -> Plan`
//! - `Plan::check(&self)`, `Plan::single_epoch(&self) -> bool`,
//!   `Plan::delta(&self, i: usize) -> std::ops::Range<usize>`

use super::testbench::{Ana, BenchCard};

/// The run plan: one row per run in `runs`, `run_tok` and `run_epoch`
/// (equal lengths), one row per alter in `alter_key`, `alter_value` and
/// `alter_tok` (equal lengths), one entry per epoch boundary in
/// `epoch_alters`. [`Plan::check`] states the invariants between them.
pub(in crate::syntax::render) struct Plan {
    /// Each run's analysis, in run order (the read order).
    pub(in crate::syntax::render) runs: Vec<Ana>,
    /// Each run's statement's first token.
    pub(in crate::syntax::render) run_tok: Vec<u32>,
    /// Each run's epoch: nondecreasing, below the epoch count.
    pub(in crate::syntax::render) run_epoch: Vec<u32>,
    /// Epoch `k`'s alter delta is the alter rows
    /// `epoch_alters[k]..epoch_alters[k + 1]`: nondecreasing, one entry
    /// more than there are epochs, the last one the alter count.
    pub(in crate::syntax::render) epoch_alters: Vec<u32>,
    /// Each alter row's top-level parameter.
    pub(in crate::syntax::render) alter_key: Vec<String>,
    /// Each alter row's new value.
    pub(in crate::syntax::render) alter_value: Vec<f64>,
    /// Each alter row's statement's first token.
    pub(in crate::syntax::render) alter_tok: Vec<u32>,
}

impl Default for Plan {
    /// The plan of a deck with no testbench: one epoch, no run.
    fn default() -> Plan {
        plan(Vec::new())
    }
}

/// The plan of a testbench's plan cards (its analyses and alters), each
/// with its statement's first token, in read order: one run per analysis,
/// in the epoch open when it is read; one row per alter, the first alter
/// after a run opening the next epoch (so an alter with no run since the
/// last one joins its delta: N4). The alters after the last run are left
/// out: no run reads them.
///
/// # Panics
/// On a card that is no plan card: the folds hand over analyses and
/// alters only.
pub(in crate::syntax::render) fn plan(cards: Vec<(BenchCard, u32)>) -> Plan {
    let count = cards.len();
    // `epoch_alters` holds each epoch's first row while the plan is
    // built; the end of the last one closes it below.
    let mut p = Plan {
        runs: Vec::with_capacity(count),
        run_tok: Vec::with_capacity(count),
        run_epoch: Vec::with_capacity(count),
        epoch_alters: vec![0],
        alter_key: Vec::new(),
        alter_value: Vec::new(),
        alter_tok: Vec::new(),
    };
    // Whether the open epoch has a run yet.
    let mut ran = false;
    for (card, tok) in cards {
        match card {
            BenchCard::Ana(a) => {
                p.run_epoch.push(p.epoch_alters.len() as u32 - 1);
                p.runs.push(a);
                p.run_tok.push(tok);
                ran = true;
            }
            BenchCard::Alter { key, value, .. } => {
                if ran {
                    p.epoch_alters.push(p.alter_key.len() as u32);
                    ran = false;
                }
                p.alter_key.push(key);
                p.alter_value.push(value);
                p.alter_tok.push(tok);
            }
            // else: the folds hand over plan cards only; any other is their bug.
            other => panic!("plan: {other:?} is no plan card"),
        }
    }
    if !ran {
        // The open epoch has no run: its alters go, and the epoch with
        // them unless it is the only one.
        let start = p.epoch_alters[p.epoch_alters.len() - 1] as usize;
        p.alter_key.truncate(start);
        p.alter_value.truncate(start);
        p.alter_tok.truncate(start);
        if p.epoch_alters.len() > 1 {
            p.epoch_alters.pop();
        }
    }
    p.epoch_alters.push(p.alter_key.len() as u32);
    let rows = p.runs.len() + p.alter_key.len();
    assert!(rows <= count, "plan: a row per card at most");
    p.check();
    p
}

impl Plan {
    /// The contracts (DESIGN-control-epochs §2), checked where the plan is
    /// built ([`plan`]) and again where a writer starts to read it.
    ///
    /// # Panics
    /// When one fails: a bug in the builder.
    pub(in crate::syntax::render) fn check(&self) {
        assert_eq!(self.runs.len(), self.run_tok.len(), "one token per run");
        assert_eq!(self.runs.len(), self.run_epoch.len(), "one epoch per run");
        let alters = self.alter_key.len();
        assert_eq!(alters, self.alter_value.len(), "one value per alter");
        assert_eq!(alters, self.alter_tok.len(), "one token per alter");
        assert!(self.epoch_alters.len() >= 2, "epoch_alters: an epoch");
        assert!(self.epoch_alters.is_sorted(), "epoch_alters: nondecreasing");
        assert_eq!(
            self.epoch_alters.last().map(|&e| e as usize),
            Some(alters),
            "epoch_alters: ends at the alter count"
        );
        assert!(self.run_epoch.is_sorted(), "run_epoch: nondecreasing");
        let epochs = self.epoch_alters.len() - 1;
        assert!(
            self.run_epoch.last().is_none_or(|&e| (e as usize) < epochs),
            "run_epoch: below the epoch count"
        );
    }

    /// Whether every run is on the netlist as written (no alter): the
    /// writers' path before the plan, byte for byte.
    pub(in crate::syntax::render) fn single_epoch(&self) -> bool {
        self.epoch_alters.len() == 2 && self.alter_key.is_empty()
    }

    /// The alter rows run `i` starts with: its epoch's delta when `i` is
    /// the epoch's first run, none otherwise.
    ///
    /// # Panics
    /// When `i` is no run.
    pub(in crate::syntax::render) fn delta(&self, i: usize) -> std::ops::Range<usize> {
        assert!(i < self.runs.len(), "delta: run {i} of {}", self.runs.len());
        let e = self.run_epoch[i] as usize;
        if i > 0 && self.run_epoch[i - 1] as usize == e {
            return 0..0;
        }
        let rows = self.epoch_alters[e] as usize..self.epoch_alters[e + 1] as usize;
        assert!(rows.end <= self.alter_key.len(), "delta: rows of the plan");
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A top-parameter alter card (hand-built: no statement behind it).
    fn alter(key: &str, value: f64) -> BenchCard {
        BenchCard::Alter {
            key: key.into(),
            value,
            node: 0,
        }
    }

    #[test]
    fn analyses_alone_are_one_epoch_in_read_order() {
        // Hand-built cards, no reader: the tokens are made up.
        let cards = vec![
            (BenchCard::Ana(Ana::Op), 4),
            (BenchCard::Ana(Ana::Tf("v(o)".into(), "v1".into())), 9),
            (BenchCard::Ana(Ana::Op), 15),
        ];
        let p = plan(cards);
        assert_eq!(p.run_tok, [4, 9, 15]);
        assert_eq!(p.run_epoch, [0, 0, 0]);
        assert!(matches!(p.runs[1], Ana::Tf(..)));
        assert!(p.single_epoch());
    }

    #[test]
    fn no_testbench_is_one_empty_epoch() {
        let p = Plan::default();
        assert!(p.runs.is_empty() && p.single_epoch());
        assert_eq!(p.epoch_alters, [0, 0]);
    }

    #[test]
    fn an_alter_after_a_run_opens_the_next_epoch() {
        // op | a=2 | op | a=2 b=1 | op op: three epochs, and a delta is
        // its epoch's first run's only (the oracle: the split by hand).
        let cards = vec![
            (BenchCard::Ana(Ana::Op), 1),
            (alter("a", 2.0), 2),
            (BenchCard::Ana(Ana::Op), 3),
            (alter("a", 2.0), 4),
            (alter("b", 1.0), 4),
            (BenchCard::Ana(Ana::Op), 5),
            (BenchCard::Ana(Ana::Op), 6),
        ];
        let p = plan(cards);
        assert_eq!(p.run_epoch, [0, 1, 2, 2]);
        assert_eq!(p.epoch_alters, [0, 0, 1, 3]);
        assert_eq!(p.alter_key, ["a", "a", "b"]);
        assert_eq!(p.alter_value, [2.0, 2.0, 1.0]);
        assert_eq!(p.alter_tok, [2, 4, 4]);
        let deltas: Vec<_> = (0..4).map(|i| p.delta(i)).collect();
        assert_eq!(deltas, [0..0, 0..1, 1..3, 0..0]);
        assert!(!p.single_epoch());
    }

    #[test]
    fn alters_no_run_follows_are_left_out() {
        // N4: an alter before the first run is epoch 0's delta; the one
        // after the last run changes nothing a run reads.
        let cards = vec![
            (alter("a", 2.0), 1),
            (BenchCard::Ana(Ana::Op), 2),
            (alter("a", 3.0), 3),
        ];
        let p = plan(cards);
        assert_eq!(p.run_epoch, [0]);
        assert_eq!(p.epoch_alters, [0, 1]);
        assert_eq!(p.alter_value, [2.0]);
        assert_eq!(p.delta(0), 0..1);
        assert!(!p.single_epoch());
        let none = plan(vec![(alter("a", 2.0), 1)]);
        assert!(none.single_epoch() && none.alter_key.is_empty());
    }

    #[test]
    #[should_panic(expected = "run_epoch: nondecreasing")]
    fn a_run_before_its_epoch_breaks_the_contract() {
        // No builder gives this: the epoch split relies on the order.
        let p = Plan {
            runs: vec![Ana::Op, Ana::Op],
            run_tok: vec![1, 2],
            run_epoch: vec![1, 0],
            epoch_alters: vec![0, 0, 0],
            alter_key: vec![],
            alter_value: vec![],
            alter_tok: vec![],
        };
        p.check();
    }

    #[test]
    #[should_panic(expected = "is no plan card")]
    fn a_card_that_is_no_plan_card_is_the_folds_bug() {
        plan(vec![(BenchCard::Temp(27.0), 1)]);
    }
}
