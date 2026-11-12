//! Rules `atan2_to_atan` and `atan2_to_atan_dyn`, not registered (awaiting a ruling: the rule
//! is not bit-exact): a design in, the same design out with every `atan2(y, x)` of one feature
//! context written by quadrant, `x > 0 ? atan(y / x) : (x < 0 ? (y >= 0 ? atan(y / x) + π :
//! atan(y / x) − π) : (y > 0 ? π/2 : (y < 0 ? −π/2 : 0)))`. Off by the rounding of `y / x` and of
//! the `± π` (measured in `tests/passes_expr.rs`), and it loses the sign of a zero (`atan2(−0,
//! −1)` is `−π`, `atan2(+0, −0)` is `π`). `x` and `y` are read more than once: an operand that
//! may hold a draw keeps the call with note `U0105`.
//!
//! Would remove `Func(Atan2)` and require `Expr(Ternary)`, `Gt`, `Lt`, `Ge`, `Add`, `Sub`,
//! `Div`, `Func(Atan)`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2.
//!
//! Contents: `pub fn atan2_to_atan(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn atan2_to_atan_dyn(&mut ir::Design, &mut ir::Diags)`

use super::DRAW_TWICE;
use super::nodes::{Draws, form, func_form, note_left, plain};
use crate::ir;

/// Rewrites `Atan2` by quadrant in every root of the plain form.
pub fn atan2_to_atan(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn atan2_to_atan_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let draws = Draws::new(design);
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let ir::Expr::Call(ir::Func::Atan2, args) = b.design().expr(e) else {
            return None;
        };
        let &[y, x] = b.design().exprs(args) else {
            unreachable!("check: atan2 takes two arguments")
        };
        if draws.holds(b.design(), x) || draws.holds(b.design(), y) {
            return None;
        }
        // the literals of π and π/2 as the shortest decimals of their doubles
        let pi = plain(b, 3_141_592_653_589_793, 15);
        let half_pi = plain(b, 15_707_963_267_948_966, 16);
        let zero = plain(b, 0, 0);
        let ratio = b.binary(ir::BinOp::Div, y, x);
        let base = b.call(ir::Func::Atan, &[ratio]);
        let up = b.binary(ir::BinOp::Add, base, pi);
        let down = b.binary(ir::BinOp::Sub, base, pi);
        let y_up = b.binary(ir::BinOp::Ge, y, zero);
        let left = b.ternary(y_up, up, down);
        let minus_half = b.unary(ir::UnOp::Neg, half_pi);
        let y_down = b.binary(ir::BinOp::Lt, y, zero);
        let axis_low = b.ternary(y_down, minus_half, zero);
        let y_above = b.binary(ir::BinOp::Gt, y, zero);
        let axis = b.ternary(y_above, half_pi, axis_low);
        let x_low = b.binary(ir::BinOp::Lt, x, zero);
        let rest = b.ternary(x_low, left, axis);
        let x_high = b.binary(ir::BinOp::Gt, x, zero);
        Some(b.ternary(x_high, base, rest))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::Atan2, dynamic),
        DRAW_TWICE,
    );
}
