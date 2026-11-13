//! A subcircuit that instantiates itself, directly or through another (item 3 of `vacask2`):
//! VACASK refuses the hierarchy when the top level reaches the cycle and runs a deck whose cycle
//! nothing instantiates. The reader's side is the fixture `read/devices/b_recursion`.
//!
//! Oracle: VACASK 36be560 on the settle decks `b_cycle_*.sim`.
//!
//! Contents: tests only.

use super::includes::vacask;

// ---- private ----

#[test]
fn vacask_refuses_a_reached_recursion_and_runs_an_unreached_one() {
    for deck in ["b_cycle_self", "b_cycle_two"] {
        let out = vacask(deck);
        assert!(
            out.contains("Hierarchical recursion detected."),
            "{deck}: {out}"
        );
    }
    let out = vacask("b_cycle_unused");
    assert!(out.contains("Running analysis 'op1'."), "{out}");
    assert!(!out.contains("recursion"), "{out}");
}
