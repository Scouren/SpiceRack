//! Passes `sqr_to_mul` and `sqr_to_mul_dyn`: a design in, the same design out with every `sqr(x)`
//! of one feature context written `x * x`: its definition, one multiplication (ngspice numparam's `sqr`).
//! Not `pow(x, 2)`: that is the library's `pow`, which is not guaranteed to round as one
//! multiplication does, so only `x * x` is the member's value bit for bit. `sqr` is Real and a
//! product of two Ints is Int (bounded by 2⁵³), so an `x` the tag table does not call Real is
//! written `1 * x * x`. `x` is read twice: an `x` that may hold a draw keeps the call with note
//! `U0105`.
//!
//! `sqr_to_mul` removes `Func(Sqr)` and requires `Binary(Mul)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::Sqr`, its definition: the spec); the lead's list of
//! what VACASK lacks (`dialect-vacask` 51ee264a, its expression tables).
//!
//! Contents: `pub fn sqr_to_mul(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sqr_to_mul_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, keep_bounded, note_left_copying, plain};
use super::tags::{REAL, Tags};
use crate::ir;

/// Rewrites `Sqr` in every root of the plain form.
pub fn sqr_to_mul(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn sqr_to_mul_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    let mut tags = Tags::new(design);
    let reverted = keep_bounded(design, &mut |d| {
        ir::rewrite_exprs_in(d, form(dynamic), &mut |b, e| {
            tags.of(b.design(), e);
            let ir::Expr::Call(ir::Func::Sqr, args) = b.design().expr(e) else {
                return None;
            };
            let x = b.design().exprs(args)[0];
            if draws.holds(b.design(), x) {
                return None;
            }
            let left = match tags.of(b.design(), x) == REAL {
                true => x,
                false => {
                    let one = plain(b, 1, 0);
                    b.binary(ir::BinOp::Mul, one, x)
                }
            };
            let square = b.binary(ir::BinOp::Mul, left, x);
            for n in [left, square] {
                tags.of(b.design(), n);
            }
            Some(square)
        });
    });
    note_left_copying(
        design,
        diags,
        func_form(ir::Func::Sqr, dynamic),
        DRAW_TWICE,
        &reverted,
    );
}
