//! Stand-ins for the AMEND-A12 members (rulings R16–R22) that have not landed in `crates/ir`.
//! Each names the member that replaces it; the tests that need the member itself carry
//! `#[should_panic(expected = "A12")]` and go red (an unexpected pass) the day it lands, which
//! is when the stand-in goes. Never a conversion: a stand-in states the nearest meaning the IR
//! has today, the same way in the reader and the writer, so law L5 holds meanwhile.
//!
//! Cites: `rewrite/AMEND-A12.md` K1, K3, C5.
//!
//! Contents: `INT_FORM`, `DIV_TYPED`, `TO_INT`, `SGN_ZERO_POS`, `HINT`, `LIBRARY_LOAD`
//! (crate-private consts)

use crate::ir;

/// A12 K1: `NumForm::Int`, an integer literal (digits only, or hex). Until it lands an
/// integer literal is `Plain`, and the writer spells `Plain` as digits only.
pub(super) const INT_FORM: ir::NumForm = ir::NumForm::Plain;

/// A12 K1: `BinOp::DivTyped`, VACASK's `/` (truncating when both operands are integers). Until
/// it lands `/` is `Div`.
pub(super) const DIV_TYPED: ir::BinOp = ir::BinOp::Div;

/// A12 K1: `Func::ToInt`, VACASK's `int()` (truncation with an integer result). Until it lands
/// `int()` is `Trunc`.
pub(super) const TO_INT: ir::Func = ir::Func::Trunc;

/// A12 K1 addendum: `Func::SgnZeroPos`, VACASK's `sgn` (−1 below zero, else +1, typed as its
/// operand: run f4). Until it lands `expr_read` reads `sgn(x)` as the sugar `x >= 0 ? 1 : -1`,
/// exact in value while the IR has no types.
pub(super) const SGN_ZERO_POS: Option<ir::Func> = None;

/// A12 K3: `OpaqueClass::Hint`, a solver knob. Until it lands a hint is `Private`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "read_run (S4.3) reads solver knobs as hints")
)]
pub(super) const HINT: ir::OpaqueClass = ir::OpaqueClass::Private;

/// A12 C5: the item `DeviceLibrary{family, path}`. Until it lands a `load` of a library file
/// VACASK ships is an `Opaque` of this class.
pub(super) const LIBRARY_LOAD: ir::OpaqueClass = ir::OpaqueClass::Unread;

#[cfg(test)]
mod tests {
    use super::*;

    // Markers: each passes (it panics) while its A12 member is missing, and fails the day the
    // stand-in is replaced, until the marker goes.

    #[test]
    #[should_panic(expected = "A12 K1")]
    fn an_integer_literal_has_its_own_form() {
        assert!(
            INT_FORM != ir::NumForm::Plain,
            "A12 K1: NumForm::Int not landed"
        );
    }

    #[test]
    #[should_panic(expected = "A12 K1")]
    fn vacasks_division_is_typed() {
        assert!(
            DIV_TYPED != ir::BinOp::Div,
            "A12 K1: BinOp::DivTyped not landed"
        );
    }

    #[test]
    #[should_panic(expected = "A12 K1")]
    fn int_has_an_integer_result() {
        assert!(TO_INT != ir::Func::Trunc, "A12 K1: Func::ToInt not landed");
    }

    #[test]
    #[should_panic(expected = "A12 K1")]
    fn sgn_is_a_member_typed_as_its_operand() {
        assert!(
            SGN_ZERO_POS.is_some(),
            "A12 K1: Func::SgnZeroPos not landed"
        );
    }

    #[test]
    #[should_panic(expected = "A12 K3")]
    fn a_solver_knob_is_a_hint() {
        assert!(
            HINT != ir::OpaqueClass::Private,
            "A12 K3: OpaqueClass::Hint not landed"
        );
    }

    #[test]
    #[should_panic(expected = "A12 C5")]
    fn a_library_load_is_the_device_library() {
        assert!(
            LIBRARY_LOAD != ir::OpaqueClass::Unread,
            "A12 C5: DeviceLibrary not landed"
        );
    }
}
