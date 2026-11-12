//! Passes `ramp_to_ternary` and `ramp_to_ternary_dyn`: a design in, the same design out with every `ramp(x)`
//! of one feature context written as its definition, `x > 0 ? x : 0` (`-0` gives `0`, as `Ramp` does; an `x` that may be Int is passed as `1 * x`, Real, while a typed division stands). `x` is evaluated twice: an `x` that may hold a draw keeps the call with note `U0105`.
//!
//! `ramp_to_ternary` removes `Func(Ramp)` and requires `Expr(Ternary)`, `Binary(Gt)`, `Binary(Mul)`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `crates/ir/src/expr.rs` (`Func::Ramp`).
//!
//! Contents: `pub fn ramp_to_ternary(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn ramp_to_ternary_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::context::rewrite_in;
use super::nodes::{Draws, func_form, note_left, plain, reads_tags};
use super::tags::{REAL, Tags};
use crate::ir;

/// Rewrites `Ramp` in every root of the plain form.
pub fn ramp_to_ternary(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn ramp_to_ternary_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    let untyped = !reads_tags(design);
    let mut tags = Tags::new(design);
    rewrite_in(design, dynamic, &mut |b, e| {
        tags.of(b.design(), e);
        let ir::Expr::Call(ir::Func::Ramp, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        if draws.holds(b.design(), x) {
            return None;
        }
        let zero = plain(b, 0, 0);
        let above = b.binary(ir::BinOp::Gt, x, zero);
        // `ramp` is Real; `x` passed through keeps its own tag, so an `x` that may be Int is
        // made Real (`1 * x` is `x` exactly) while a typed division may read it
        let pass = match untyped || tags.of(b.design(), x) == REAL {
            true => x,
            false => {
                let one = plain(b, 1, 0);
                b.binary(ir::BinOp::Mul, one, x)
            }
        };
        Some(b.ternary(above, pass, zero))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::Ramp, dynamic),
        DRAW_TWICE,
    );
}
