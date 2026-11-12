//! Passes `explin14_to_ternary` and `explin14_to_ternary_dyn`: a design in, the same design out with every `exp_lin14(x)`
//! of one feature context written as its definition, `x <= 14 ? exp(x) : exp(14) * (1 + x - 14)` (ngspice's
//! B-source `exp` under `ngbehavior=ps`): `exp(14)` is the call, not a literal of its value, so
//! the knee is the same double. Only one branch is evaluated, so `x` is read twice (the test,
//! then its branch): an `x` that may hold a draw keeps the call with note `U0105`.
//!
//! `explin14_to_ternary` removes `Func(ExpLin14)` and requires `Expr(Ternary)`, `Binary(Le)`, `Binary(Mul)`, `Binary(Add)`, `Binary(Sub)`, `Func(Exp)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::ExpLin14`, its definition: the spec); the lead's list of
//! what VACASK lacks (`dialect-vacask` 51ee264a, its expression tables).
//!
//! Contents: `pub fn explin14_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn explin14_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, keep_bounded, note_left_copying, plain};
use crate::ir;

/// Rewrites `ExpLin14` in every root of the plain form.
pub fn explin14_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn explin14_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    let reverted = keep_bounded(design, &mut |d| {
        ir::rewrite_exprs_in(d, form(dynamic), &mut |b, e| {
            let ir::Expr::Call(ir::Func::ExpLin14, args) = b.design().expr(e) else {
                return None;
            };
            let x = b.design().exprs(args)[0];
            if draws.holds(b.design(), x) {
                return None;
            }
            let (knee, one) = (plain(b, 14, 0), plain(b, 1, 0));
            let below = b.binary(ir::BinOp::Le, x, knee);
            let curve = b.call(ir::Func::Exp, &[x]);
            let at_knee = b.call(ir::Func::Exp, &[knee]);
            let shifted = b.binary(ir::BinOp::Add, one, x);
            let past = b.binary(ir::BinOp::Sub, shifted, knee);
            let line = b.binary(ir::BinOp::Mul, at_knee, past);
            Some(b.ternary(below, curve, line))
        });
    });
    note_left_copying(
        design,
        diags,
        func_form(ir::Func::ExpLin14, dynamic),
        DRAW_TWICE,
        &reverted,
    );
}
