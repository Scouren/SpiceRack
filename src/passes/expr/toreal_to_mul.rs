//! Passes `toreal_to_mul` and `toreal_to_mul_dyn`: a design in, the same design out with every
//! `real(x)` (`ToReal`, A21: the value of `x`, Real) of one feature context written `x` when the
//! tag table says `x` is Real on every evaluation, else `1 * x` (a Real `1`: the product is `x`
//! exactly, and Real). Exact in value and tag; `x` is evaluated once. An `x` that may be a
//! string or a list (the table's `TEXT`) has no numeric tag: the call stays, note `U0117`.
//!
//! `toreal_to_mul` removes `Func(ToReal)` and requires `Binary(Mul)`; `_dyn` the dynamic form.
//!
//! Cites: `rewrite/AMEND-A12.md` A21; `rewrite/RULINGS.md` R40.
//!
//! Contents: `pub fn toreal_to_mul(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn toreal_to_mul_dyn(&mut ir::Design, &mut ir::Diags)`

use super::TEXT_OPERAND;
use super::nodes::plain;
use super::nodes::{form, func_form, note_left};
use super::tags::{REAL, TEXT, Tags};
use crate::ir;

/// Rewrites `ToReal` in every root of the plain form.
pub fn toreal_to_mul(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn toreal_to_mul_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let mut tags = Tags::new(design);
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        tags.of(b.design(), e);
        let ir::Expr::Call(ir::Func::ToReal, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        let t = tags.of(b.design(), x);
        if t & TEXT != 0 {
            return None;
        }
        if t == REAL {
            return Some(x);
        }
        let one = plain(b, 1, 0);
        let m = b.binary(ir::BinOp::Mul, one, x);
        tags.of(b.design(), m);
        Some(m)
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::ToReal, dynamic),
        TEXT_OPERAND,
    );
}
