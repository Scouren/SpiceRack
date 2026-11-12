//! Passes `step_to_ternary` and `step_to_ternary_dyn`: a design in, the same design out with every `step(x)`
//! of one feature context written as its definition, `x > 0 ? 1 : 0`. `x` is evaluated once.
//!
//! `step_to_ternary` removes `Func(Step)` and requires `Expr(Ternary)`, `Binary(Gt)`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `crates/ir/src/expr.rs` (`Func::Step`).
//!
//! Contents: `pub fn step_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn step_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::context::rewrite_in;
use super::nodes::plain;
use crate::ir;

/// Rewrites `Step` in every root of the plain form.
pub fn step_to_ternary(design: &mut ir::Design, _diags: &mut ir::Diags) {
    run(design, false);
}

/// The same in every root of the dynamic form.
pub fn step_to_ternary_dyn(design: &mut ir::Design, _diags: &mut ir::Diags) {
    run(design, true);
}

// ---- private ----

fn run(design: &mut ir::Design, dynamic: bool) {
    rewrite_in(design, dynamic, &mut |b, e| {
        let ir::Expr::Call(ir::Func::Step, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        let (zero, one) = (plain(b, 0, 0), plain(b, 1, 0));
        let above = b.binary(ir::BinOp::Gt, x, zero);
        Some(b.ternary(above, one, zero))
    });
}
