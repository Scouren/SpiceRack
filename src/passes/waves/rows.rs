//! The waves group's pass rows, in the order `legalize` tries them.
//!
//! Contents: `pub(crate) const PASSES: &[Pass]`

use super::*;
use crate::ir;
use crate::ir::CircuitExtra as C;
use crate::ir::Feature as X;
use crate::ir::{BinOp as O, ExprTag as T};
use crate::passes::Pass;

/// The group's passes, in the order `legalize` tries them. One row per line: a table.
#[rustfmt::skip]
pub(crate) const PASSES: &[Pass] = &[
    row("given_zero_to_as_given", X::Circuit(C::GivenZeroSpiceDefault), &[X::Circuit(C::GivenZeroAsGiven), X::Expr(T::TStep), X::Expr(T::TStop), X::Expr(T::Ternary), X::Expr(T::Binary), X::Expr(T::Num), X::Binary(O::Eq), X::Binary(O::Div), X::Binary(O::Add), X::BinaryDyn(O::Eq), X::BinaryDyn(O::Div), X::BinaryDyn(O::Add)], &[GIVEN_ZERO_LEFT], given_zero_to_as_given::given_zero_to_as_given),
    row("pulse_spice_to_vacask", X::Circuit(C::PulseSpice), &[X::Circuit(C::PulseVacask)], &[PULSE_LAWS_DIFFER], pulse_spice_to_vacask::pulse_spice_to_vacask),
    // before as_given_to_given_zero: the laws agree only on an as-given pulse
    row("pulse_vacask_to_spice", X::Circuit(C::PulseVacask), &[X::Circuit(C::PulseSpice)], &[PULSE_LAWS_DIFFER], pulse_spice_to_vacask::pulse_vacask_to_spice),
    row("as_given_to_given_zero", X::Circuit(C::GivenZeroAsGiven), &[X::Circuit(C::GivenZeroSpiceDefault)], &[GIVEN_ZERO_KEPT], as_given_to_given_zero::as_given_to_given_zero),
    row("exp_td2_relative", X::Circuit(C::ExpTd2FromZero), &[X::Circuit(C::ExpTd2FromTd1), X::Expr(T::Num)], &[EXP_TD2_LEFT], exp_td2_relative::exp_td2_relative),
    row("exp_td2_absolute", X::Circuit(C::ExpTd2FromTd1), &[X::Circuit(C::ExpTd2FromZero), X::Expr(T::Binary), X::Binary(O::Add), X::BinaryDyn(O::Add)], &[EXP_TD2_LEFT], exp_td2_absolute::exp_td2_absolute),
    row("sffm_ngspice_to_spice3", X::Circuit(C::SffmNgspice), &[X::Circuit(C::SffmSpice3), X::Expr(T::Num)], &[SFFM_LAW_LEFT], sffm_ngspice_to_spice3::sffm_ngspice_to_spice3),
    row("sffm_spice3_to_ngspice", X::Circuit(C::SffmSpice3), &[X::Circuit(C::SffmNgspice)], &[SFFM_LAW_LEFT], sffm_spice3_to_ngspice::sffm_spice3_to_ngspice),
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
