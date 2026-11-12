//! Passes `trunc_to_ternary` and `trunc_to_ternary_dyn`: a design in, the same design out with
//! every `trunc(x)` of one feature context written `x < 0 ? ceil(x) : floor(x)`, its definition
//! (toward zero): for `x < 0` the ceiling, else the floor; `-0` is not below 0 and `floor(-0)`
//! is `-0`, as `trunc(-0)`. Real in both (`Trunc`, `Ceil` and `Floor` are Real), so exact in
//! value and tag. `x` is evaluated twice: an `x` that may hold a draw keeps the call with note
//! `U0105`.
//!
//! The case: ngspice's behavioural evaluator has no truncation (ngspice-45, a B source
//! `v=trunc(-2.5*v(a))`: "no such function 'trunc'", and the same for `int`; `floor` gives -3,
//! `ceil` -2), so `div_typed_to_div_dyn` and `toint_to_trunc_dyn`, which write `trunc`, become
//! legal there through this rule.
//!
//! `trunc_to_ternary` removes `Func(Trunc)` and requires `Expr(Ternary)`, `Binary(Lt)`,
//! `Func(Ceil)`, `Func(Floor)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::Trunc`); the run above (`passes` agent's settle,
//! scratch `trunc/`).
//!
//! Contents: `pub fn trunc_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn trunc_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, keep_bounded, note_left_copying, plain};
use crate::ir;

/// Rewrites `Trunc` in every root of the plain form.
pub fn trunc_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn trunc_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    let reverted = keep_bounded(design, &mut |d| {
        ir::rewrite_exprs_in(d, form(dynamic), &mut |b, e| {
            let ir::Expr::Call(ir::Func::Trunc, args) = b.design().expr(e) else {
                return None;
            };
            let x = b.design().exprs(args)[0];
            if draws.holds(b.design(), x) {
                return None;
            }
            let zero = plain(b, 0, 0);
            let below = b.binary(ir::BinOp::Lt, x, zero);
            let (up, down) = (b.call(ir::Func::Ceil, &[x]), b.call(ir::Func::Floor, &[x]));
            Some(b.ternary(below, up, down))
        });
    });
    note_left_copying(
        design,
        diags,
        func_form(ir::Func::Trunc, dynamic),
        DRAW_TWICE,
        &reverted,
    );
}
