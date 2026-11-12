//! Passes `sgn_to_ternary` and `sgn_to_ternary_dyn`: a design in, the same design out with every `sgn(x)`
//! of one feature context written as its definition, `x > 0 ? 1 : (x < 0 ? -1 : 0)` (Real literals; `-0` gives `0`, as `Sgn` does). `x` is evaluated twice: an `x` that may hold a draw keeps the call with note `U0105`.
//!
//! `sgn_to_ternary` removes `Func(Sgn)` and requires `Expr(Ternary)`, `Binary(Gt)`, `Binary(Lt)`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `crates/ir/src/expr.rs` (`Func::Sgn`).
//!
//! Contents: `pub fn sgn_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sgn_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::context::rewrite_in;
use super::nodes::{Draws, func_form, note_left, plain};
use crate::ir;

/// Rewrites `Sgn` in every root of the plain form.
pub fn sgn_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn sgn_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    rewrite_in(design, dynamic, &mut |b, e| {
        let ir::Expr::Call(ir::Func::Sgn, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        if draws.holds(b.design(), x) {
            return None;
        }
        let (zero, one, minus) = (plain(b, 0, 0), plain(b, 1, 0), plain(b, -1, 0));
        let above = b.binary(ir::BinOp::Gt, x, zero);
        let below = b.binary(ir::BinOp::Lt, x, zero);
        let rest = b.ternary(below, minus, zero);
        Some(b.ternary(above, one, rest))
    });
    note_left(design, diags, func_form(ir::Func::Sgn, dynamic), DRAW_TWICE);
}
