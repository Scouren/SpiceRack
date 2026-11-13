//! The evidence for R72 and R75 (finding F8): which solution a DC point starts from, on the
//! simulators themselves. One bistable latch (`decks/latch*`) under nested sweeps:
//! - ngspice-45's nested `.dc` starts each outer step from the initial guess (`latch.sp`;
//!   `latch_desc.sp`, whose outer axis leaves the high state: at vin = 2 the middle state);
//! - VACASK's nested `sweep` carries the last point into the next outer step (`latch.sim`),
//!   and `continuation=0` makes exactly its own level start afresh (`latch_c0_{outer,inner,
//!   both}.sim`): on the outer level it gives ngspice's numbers.
//!
//! Oracle: the simulators' printed values, recorded in each deck's title; the points asserted
//! are the ones that decide the rulings, and they lie 1.5 V apart, far past any tolerance.
//!
//! Contents: `pub fn continuation_is_a_fact_of_each_sweep_level()`

use std::path::Path;

use crate::sim::{self, Ran, Sim};

/// The points of F8: outer step 2's first points, and `latch_desc`'s last outer step.
pub fn continuation_is_a_fact_of_each_sweep_level() {
    let c = |sim: Sim, deck: &str| -> Vec<f64> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/passes_run/decks")
            .join(deck);
        let text = std::fs::read_to_string(&path).expect("the deck");
        let plots = match sim::run(sim, &path, &text, &[], 60) {
            Ran::Plots(p) => p,
            Ran::Timeout => panic!("{deck}: past the CPU budget"),
            Ran::Failed(why) => panic!("{deck}: {why}"),
        };
        let p = &plots[0];
        let i = p
            .variables
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case("c") || v.name.eq_ignore_ascii_case("v(c)"))
            .unwrap_or_else(|| panic!("{deck}: no c"));
        p.real_data[i].clone()
    };
    let near = |deck: &str, got: f64, want: f64| {
        assert!(
            (got - want).abs() < 5e-3,
            "{deck}: v(c) {got}, recorded {want}"
        );
    };
    // outer step 2 (rin = 10.1k), vin = 2 V: point 7 of 14
    near("latch.sp", c(Sim::Ngspice, "latch.sp")[7], 3.490099);
    near("latch.sim", c(Sim::Vacask, "latch.sim")[7], 4.9734);
    let desc = c(Sim::Ngspice, "latch_desc.sp");
    near("latch_desc.sp", desc[6], 3.5);
    near("latch_desc.sp", desc[7], 3.5);
    near(
        "latch_c0_outer.sim",
        c(Sim::Vacask, "latch_c0_outer.sim")[7],
        3.4901,
    );
    let inner = c(Sim::Vacask, "latch_c0_inner.sim");
    near("latch_c0_inner.sim", inner[7], 4.9734);
    near("latch_c0_inner.sim", inner[8], 2.5);
    near(
        "latch_c0_both.sim",
        c(Sim::Vacask, "latch_c0_both.sim")[7],
        3.4901,
    );
}
