//! The waves group's pass rows, in the order `legalize` tries them.
//!
//! Contents: `pub(crate) const PASSES: &[Pass]`

use super::*;
use crate::ir;
use crate::ir::CircuitExtra as C;
use crate::ir::Feature as X;
use crate::passes::Pass;

/// The group's passes, in the order `legalize` tries them. One row per line: a table.
#[rustfmt::skip]
pub(crate) const PASSES: &[Pass] = &[
    row("pulse_spice_to_vacask", X::Circuit(C::PulseSpice), &[X::Circuit(C::PulseVacask)], &[PULSE_LAWS_DIFFER], pulse_spice_to_vacask::pulse_spice_to_vacask),
];

// ---- private ----

/// A row: its name, the feature it removes, what its output may add, its notes and its run.
const fn row(
    name: &'static str,
    removes: ir::Feature,
    requires: &[ir::Feature],
    notes: &'static [ir::Code],
    run: fn(&mut ir::Design, &mut ir::Diags),
) -> Pass {
    let mut set = ir::FeatureSet::EMPTY;
    let mut i = 0;
    while i < requires.len() {
        set = set.with(requires[i]);
        i += 1;
    }
    Pass {
        name,
        removes,
        requires: set,
        notes,
        run,
    }
}
