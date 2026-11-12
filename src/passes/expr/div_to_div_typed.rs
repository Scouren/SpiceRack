//! Passes `div_to_div_typed` and `div_to_div_typed_dyn`: a design in, the same design out with
//! every real division `a / b` (`Div`) of one feature context written as a typed division
//! (`DivTyped`, VACASK's `/`) that cannot truncate: `DivTyped(a, b)` when the tag table says an
//! operand is Real on every evaluation, else `DivTyped(1 * a, b)` (a Real `1` makes the product,
//! and so the quotient, Real; `1 * a` is `a` exactly). The zero divisor is the same domain error
//! in both. Each operand is evaluated once, in the same order. It refuses nothing.
//!
//! `div_to_div_typed` removes `Binary(Div)` and requires `Binary(DivTyped)`, `Binary(Mul)`;
//! `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1 (runs i1, t3); `rewrite/AMEND-A12.md` K1.
//!
//! Contents: `pub fn div_to_div_typed(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn div_to_div_typed_dyn(&mut ir::Design, &mut ir::Diags)`

use super::nodes::form;
use super::nodes::plain;
use super::tags::{REAL, Tags};
use crate::ir;

/// Rewrites `Div` as a typed division that stays real, in every root of the plain form.
pub fn div_to_div_typed(design: &mut ir::Design, _diags: &mut ir::Diags) {
    run(design, false);
}

/// The same in every root of the dynamic form.
pub fn div_to_div_typed_dyn(design: &mut ir::Design, _diags: &mut ir::Diags) {
    run(design, true);
}

// ---- private ----

fn run(design: &mut ir::Design, dynamic: bool) {
    let mut tags = Tags::new(design);
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        tags.of(b.design(), e);
        let ir::Expr::Binary(ir::BinOp::Div, x, y) = b.design().expr(e) else {
            return None;
        };
        let real = tags.of(b.design(), x) == REAL || tags.of(b.design(), y) == REAL;
        let x = match real {
            true => x,
            false => {
                let one = plain(b, 1, 0);
                b.binary(ir::BinOp::Mul, one, x)
            }
        };
        let q = b.binary(ir::BinOp::DivTyped, x, y);
        for n in [x, q] {
            tags.of(b.design(), n);
        }
        Some(q)
    });
}
