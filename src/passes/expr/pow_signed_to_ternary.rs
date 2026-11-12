//! Passes `pow_signed_to_ternary` and `pow_signed_to_ternary_dyn`: a design in, the same design out with every `pow_signed(x, y)`
//! of one feature context written as its definition, `x < 0 ? -pow(-x, y) : pow(x, y)` (ngspice's `pwr`): `-x`
//! is positive where it is raised, so `pow`'s negative-base rule never applies; at `x = ±0` the
//! second branch is `pow(±0, y)`, which keeps `-0` for an odd `y` as `pow_signed` does, and
//! errs for `y < 0` as `pow_signed` does. Only one branch is evaluated, so `y` is read once and
//! `x` twice (the test, then its branch): an `x` that may hold a draw keeps the call with note
//! `U0105`.
//!
//! `pow_signed_to_ternary` removes `Func(PowSigned)` and requires `Expr(Ternary)`, `Binary(Lt)`, `Unary(Neg)`, `Func(Pow)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::PowSigned`, its definition: the spec); the lead's list of
//! what VACASK lacks (`dialect-vacask` 51ee264a, its expression tables).
//!
//! Contents: `pub fn pow_signed_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn pow_signed_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, keep_bounded, note_left_copying, plain};
use crate::ir;

/// Rewrites `PowSigned` in every root of the plain form.
pub fn pow_signed_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn pow_signed_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    let reverted = keep_bounded(design, &mut |d| {
        ir::rewrite_exprs_in(d, form(dynamic), &mut |b, e| {
            let ir::Expr::Call(ir::Func::PowSigned, args) = b.design().expr(e) else {
                return None;
            };
            let &[x, y] = b.design().exprs(args) else {
                unreachable!("check: pow_signed takes two arguments")
            };
            if draws.holds(b.design(), x) {
                return None;
            }
            let zero = plain(b, 0, 0);
            let below = b.binary(ir::BinOp::Lt, x, zero);
            let minus_x = b.unary(ir::UnOp::Neg, x);
            let mirrored = b.call(ir::Func::Pow, &[minus_x, y]);
            let negative = b.unary(ir::UnOp::Neg, mirrored);
            let positive = b.call(ir::Func::Pow, &[x, y]);
            Some(b.ternary(below, negative, positive))
        });
    });
    note_left_copying(
        design,
        diags,
        func_form(ir::Func::PowSigned, dynamic),
        DRAW_TWICE,
        &reverted,
    );
}
