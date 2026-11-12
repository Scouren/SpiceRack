//! Rules `hypot_to_sqrt` and `hypot_to_sqrt_dyn`, not registered (awaiting a ruling: the rule is
//! not exact): a design in, the same design out with every `hypot(x, y)` of one feature context
//! written `sqrt(x * x + y * y)`. Off by up to a few ulp where the squares are normal (measured in
//! `tests/passes_expr.rs`); past `|x| ≈ 1.34e154` the square overflows (a domain error where
//! `hypot` has a value) and below `≈ 1.5e-154` it underflows. `x` and `y` are read twice: an
//! operand that may hold a draw keeps the call with note `U0105`.
//!
//! Would remove `Func(Hypot)` and require `Func(Sqrt)`, `Binary(Mul)`, `Binary(Add)`; `_dyn`
//! the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2 ("≤ 2 ulp, measured, not exact").
//!
//! Contents: `pub fn hypot_to_sqrt(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn hypot_to_sqrt_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::context::rewrite_in;
use super::nodes::{Draws, func_form, note_left};
use crate::ir;

/// Rewrites `Hypot` as a square root in every root of the plain form.
pub fn hypot_to_sqrt(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn hypot_to_sqrt_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    rewrite_in(design, dynamic, &mut |b, e| {
        let ir::Expr::Call(ir::Func::Hypot, args) = b.design().expr(e) else {
            return None;
        };
        let &[x, y] = b.design().exprs(args) else {
            unreachable!("check: hypot takes two arguments")
        };
        if draws.holds(b.design(), x) || draws.holds(b.design(), y) {
            return None;
        }
        let xx = b.binary(ir::BinOp::Mul, x, x);
        let yy = b.binary(ir::BinOp::Mul, y, y);
        let sum = b.binary(ir::BinOp::Add, xx, yy);
        Some(b.call(ir::Func::Sqrt, &[sum]))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::Hypot, dynamic),
        DRAW_TWICE,
    );
}
