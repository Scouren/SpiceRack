//! Passes `round_half_even_to_away` and `round_half_even_to_away_dyn`: a design in, the same
//! design out with every `round_half_even(x)` (ngspice's `nint`) of one feature context written
//! `x - floor(x) == 0.5 ? 2 * round_half_away(x * 0.5) : round_half_away(x)`. Off a tie both
//! roundings agree; on a tie `x = k + ½` the half `x / 2 = k/2 + ¼` is no tie, so its nearest
//! whole number doubled is the even neighbour. `x - floor(x)` is exact (Sterbenz, or `x < 1`),
//! so the test finds exactly the ties. `x` is evaluated more than once: an `x` that may hold a
//! draw keeps the call with note `U0105`.
//!
//! `round_half_even_to_away` removes `Func(RoundHalfEven)` and requires `Expr(Ternary)`,
//! `Binary(Eq)`, `Binary(Sub)`, `Binary(Mul)`, `Func(Floor)`, `Func(RoundHalfAway)`; `_dyn` the
//! dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `crates/ir/src/expr.rs` (`Func::RoundHalfEven`).
//!
//! Contents: `pub fn round_half_even_to_away(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn round_half_even_to_away_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, note_left, plain};
use crate::ir;

/// Rewrites `RoundHalfEven` in every root of the plain form.
pub fn round_half_even_to_away(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn round_half_even_to_away_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let ir::Expr::Call(ir::Func::RoundHalfEven, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        if draws.holds(b.design(), x) {
            return None;
        }
        let floor = b.call(ir::Func::Floor, &[x]);
        let frac = b.binary(ir::BinOp::Sub, x, floor);
        let half = plain(b, 5, 1);
        let tie = b.binary(ir::BinOp::Eq, frac, half);
        let halved = b.binary(ir::BinOp::Mul, x, half);
        let near = b.call(ir::Func::RoundHalfAway, &[halved]);
        let two = plain(b, 2, 0);
        let even = b.binary(ir::BinOp::Mul, two, near);
        let away = b.call(ir::Func::RoundHalfAway, &[x]);
        Some(b.ternary(tie, even, away))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::RoundHalfEven, dynamic),
        DRAW_TWICE,
    );
}
