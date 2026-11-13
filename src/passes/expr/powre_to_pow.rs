//! Passes `powre_to_pow` and `powre_to_pow_dyn`: a design in, the same design out with every
//! `pow_re(x, y)` (Xyce's `**`: the real part of the complex power) of one feature context
//! written `pow(x, y)` where the two are equal on every evaluation: `y` a whole literal (both
//! are `x.powf(y)`, the sign of a negative `x` carried), or `x` never negative (`abs`,
//! `sqrt_abs`, `exp`, `sqr`, `pow_abs`, a literal `>= +0`). They differ only for a negative `x`
//! and a fractional `y`, where `pow` is a domain error and `pow_re` is `|x|ʸ·cos(πy)`: any other
//! call stays, note `U0104`. `x` and `y` are each evaluated once, in the same order; `0` to a
//! negative power is a domain error in both.
//!
//! `powre_to_pow` removes `Func(PowRe)` and requires `Func(Pow)`; `_dyn` the dynamic form.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::Pow`, `Func::PowRe`); `crates/ir/src/eval.rs`.
//!
//! Contents: `pub fn powre_to_pow(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn powre_to_pow_dyn(&mut ir::Design, &mut ir::Diags)`

use super::POW_BASE;
use super::nodes::{form, func_form, literal, never_negative, note_left};
use crate::ir;

/// Rewrites `PowRe` in every root of the plain form.
pub fn powre_to_pow(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn powre_to_pow_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let d = b.design();
        let ir::Expr::Call(ir::Func::PowRe, args) = d.expr(e) else {
            return None;
        };
        let &[x, y] = d.exprs(args) else {
            unreachable!("check: pow_re takes two arguments")
        };
        let whole = literal(d, y).is_some_and(|k| k.fract() == 0.0);
        if !whole && !never_negative(d, x) {
            return None;
        }
        Some(b.call(ir::Func::Pow, &[x, y]))
    });
    note_left(design, diags, func_form(ir::Func::PowRe, dynamic), POW_BASE);
}
