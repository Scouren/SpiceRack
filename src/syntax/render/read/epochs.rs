//! A testbench's plan cards in read order → the run [`Plan`]: every
//! analysis in run order, each tagged with the circuit state (epoch) it
//! runs on. Epoch 0 is the netlist as written; a later epoch is the one
//! before it and its alter delta. No reader gives a card that opens a
//! second epoch yet, so every plan is one epoch and the writers take the
//! path they took before the plan: the golden record is byte-identical
//! (the single-epoch invariant).
//!
//! The table grows a column with the reader that fills it: the alter
//! rows with VACASK's `alter` cards, the loop rows with the sweeps
//! (ZERO-LEDGER P5-001, steps 2 and 3).
//!
//! Cites: DESIGN-control-epochs §2 (the six questions, the contracts),
//! §6 commit 1 (ZERO-LEDGER P5-001); ngspice manual §13.5.3 (`alter`)
//! and VACASK `docs/cmd-alter.md` (what a later epoch is).
//!
//! Contents:
//! - `pub(in crate::syntax::render) struct Plan`, `impl Default for Plan`
//! - `pub(in crate::syntax::render) fn plan(cards: Vec<(BenchCard, u32)>) -> Plan`
//! - `Plan::check(&self)`, `Plan::single_epoch(&self) -> bool`

use super::testbench::{Ana, BenchCard};

/// The run plan: one row per run in `runs`, `run_tok` and `run_epoch`
/// (equal lengths), one entry per epoch boundary in `epoch_alters`.
/// [`Plan::check`] states the invariants between them.
pub(in crate::syntax::render) struct Plan {
    /// Each run's analysis, in run order (the read order).
    pub(in crate::syntax::render) runs: Vec<Ana>,
    /// Each run's statement's first token.
    pub(in crate::syntax::render) run_tok: Vec<u32>,
    /// Each run's epoch: nondecreasing, below the epoch count.
    pub(in crate::syntax::render) run_epoch: Vec<u32>,
    /// Epoch `k`'s alter delta is the alter rows
    /// `epoch_alters[k]..epoch_alters[k + 1]`: nondecreasing, one entry
    /// more than there are epochs. There is no alter row yet, so every
    /// entry is 0.
    pub(in crate::syntax::render) epoch_alters: Vec<u32>,
}

impl Default for Plan {
    /// The plan of a deck with no testbench: one epoch, no run.
    fn default() -> Plan {
        plan(Vec::new())
    }
}

/// The plan of a testbench's plan cards (its analyses), each with its
/// statement's first token, in read order: one run per card, every run in
/// epoch 0.
///
/// # Panics
/// On a card that is no plan card: the folds hand over analyses only.
pub(in crate::syntax::render) fn plan(cards: Vec<(BenchCard, u32)>) -> Plan {
    let count = cards.len();
    let mut p = Plan {
        runs: Vec::with_capacity(count),
        run_tok: Vec::with_capacity(count),
        run_epoch: Vec::with_capacity(count),
        epoch_alters: vec![0, 0],
    };
    for (card, tok) in cards {
        match card {
            BenchCard::Ana(a) => {
                p.runs.push(a);
                p.run_tok.push(tok);
                p.run_epoch.push(0);
            }
            // else: the folds hand over plan cards only; any other is their bug.
            other => panic!("plan: {other:?} is no plan card"),
        }
    }
    assert_eq!(p.runs.len(), count, "plan: one run per analysis card");
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
        assert!(self.epoch_alters.len() >= 2, "epoch_alters: an epoch");
        assert!(self.epoch_alters.is_sorted(), "epoch_alters: nondecreasing");
        assert!(self.run_epoch.is_sorted(), "run_epoch: nondecreasing");
        let epochs = self.epoch_alters.len() - 1;
        assert!(
            self.run_epoch.last().is_none_or(|&e| (e as usize) < epochs),
            "run_epoch: below the epoch count"
        );
    }

    /// Whether every run is on the netlist as written: the writers' path
    /// before the plan, byte for byte.
    pub(in crate::syntax::render) fn single_epoch(&self) -> bool {
        self.epoch_alters.len() == 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    #[should_panic(expected = "run_epoch: nondecreasing")]
    fn a_run_before_its_epoch_breaks_the_contract() {
        // No builder gives this: the epoch split relies on the order.
        let p = Plan {
            runs: vec![Ana::Op, Ana::Op],
            run_tok: vec![1, 2],
            run_epoch: vec![1, 0],
            epoch_alters: vec![0, 0, 0],
        };
        p.check();
    }

    #[test]
    #[should_panic(expected = "is no plan card")]
    fn a_card_that_is_no_plan_card_is_the_folds_bug() {
        plan(vec![(BenchCard::Temp(27.0), 1)]);
    }
}
