//! Passes `step_half_to_ternary` and `step_half_to_ternary_dyn`: a design in, the same design out with every `step_half(x)`
//! of one feature context written as its definition, `x > 0 ? 1 : (x < 0 ? 0 : 0.5)` (ngspice's `u`). `x` is evaluated twice: an `x` that may hold a draw keeps the call with note `U0105`.
//!
//! `step_half_to_ternary` removes `Func(StepHalf)` and requires `Expr(Ternary)`, `Binary(Gt)`, `Binary(Lt)`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `crates/ir/src/expr.rs` (`Func::StepHalf`).
//!
//! Contents: `pub fn step_half_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn step_half_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, keep_bounded, note_left_copying, plain};
use crate::ir;

/// Rewrites `StepHalf` in every root of the plain form.
pub fn step_half_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn step_half_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    let reverted = keep_bounded(design, &mut |d| {
        ir::rewrite_exprs_in(d, form(dynamic), &mut |b, e| {
            let ir::Expr::Call(ir::Func::StepHalf, args) = b.design().expr(e) else {
                return None;
            };
            let x = b.design().exprs(args)[0];
            if draws.holds(b.design(), x) {
                return None;
            }
            let (zero, one, half) = (plain(b, 0, 0), plain(b, 1, 0), plain(b, 5, 1));
            let above = b.binary(ir::BinOp::Gt, x, zero);
            let below = b.binary(ir::BinOp::Lt, x, zero);
            let rest = b.ternary(below, zero, half);
            Some(b.ternary(above, one, rest))
        });
    });
    note_left_copying(
        design,
        diags,
        func_form(ir::Func::StepHalf, dynamic),
        DRAW_TWICE,
        &reverted,
    );
}
