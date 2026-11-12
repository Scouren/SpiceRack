//! Passes `round_half_away_to_floor` and `round_half_away_to_floor_dyn`: a design in, the same
//! design out with every `round_half_away(x)` of one feature context written by sign:
//! `x >= 0 ? (x - floor(x) >= 0.5 ? floor(x) + 1 : floor(x)) : (ceil(x) - x >= 0.5 ? ceil(x) - 1
//! : ceil(x))` (never `floor(x + ½)`, which rounds `½ − ulp` up). Both differences are exact
//! (Sterbenz, or `|x| < 1`), `-0` stays `-0` (`floor(-0)`, `ceil(-0.3)`), and a whole `x` past
//! 2⁵² is its own floor. `x` is evaluated more than once: an `x` that may hold a draw keeps the
//! call with note `U0105`.
//!
//! `round_half_away_to_floor` removes `Func(RoundHalfAway)` and requires `Expr(Ternary)`,
//! `Binary(Ge)`, `Binary(Sub)`, `Binary(Add)`, `Func(Floor)`, `Func(Ceil)`; `_dyn` the dynamic
//! forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2; `crates/ir/src/expr.rs` (`Func::RoundHalfAway`).
//!
//! Contents: `pub fn round_half_away_to_floor(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn round_half_away_to_floor_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::context::rewrite_in;
use super::nodes::{Draws, func_form, note_left, plain};
use crate::ir;

/// Rewrites `RoundHalfAway` in every root of the plain form.
pub fn round_half_away_to_floor(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn round_half_away_to_floor_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    rewrite_in(design, dynamic, &mut |b, e| {
        let ir::Expr::Call(ir::Func::RoundHalfAway, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        if draws.holds(b.design(), x) {
            return None;
        }
        let (zero, half, one) = (plain(b, 0, 0), plain(b, 5, 1), plain(b, 1, 0));
        let up = b.binary(ir::BinOp::Ge, x, zero);
        let floor = b.call(ir::Func::Floor, &[x]);
        let below = b.binary(ir::BinOp::Sub, x, floor);
        let far = b.binary(ir::BinOp::Ge, below, half);
        let next = b.binary(ir::BinOp::Add, floor, one);
        let positive = b.ternary(far, next, floor);
        let ceil = b.call(ir::Func::Ceil, &[x]);
        let above = b.binary(ir::BinOp::Sub, ceil, x);
        let far = b.binary(ir::BinOp::Ge, above, half);
        let prev = b.binary(ir::BinOp::Sub, ceil, one);
        let negative = b.ternary(far, prev, ceil);
        Some(b.ternary(up, positive, negative))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::RoundHalfAway, dynamic),
        DRAW_TWICE,
    );
}
