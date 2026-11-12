//! Passes `pow_to_powabs` and `pow_to_powabs_dyn`: a design in, the same design out with every
//! signed power `pow(x, y)` (`Pow`) of one feature context written with the powers ngspice has:
//! a whole literal `y` even → `pow_abs(x, y)`, odd → `pow_signed(x, y)` (ngspice's `pwr`: the
//! sign of `x` carried, so `pow(-0, 3)` stays `-0`); any other literal `y` → `pow_abs(x, y)`,
//! equal on `Pow`'s domain (`x < 0` with a fractional `y` is a domain error there: R12, not
//! carried); a non-literal `y` → `pow_abs(x, y)` when `x` cannot be negative or `-0` (`abs`,
//! `sqrt_abs`, `exp`, `sqr`, `pow_abs`, a literal), else the call keeps its form with note
//! `U0104`. `x` and `y` are each evaluated once, in the same order.
//!
//! `pow_to_powabs` removes `Func(Pow)` and requires `Func(PowAbs)`, `Func(PowSigned)`; `_dyn`
//! the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2 (`abs_power_in`); `crates/ir/src/expr.rs`
//! (`Func::Pow`, `Func::PowAbs`, `Func::PowSigned`).
//!
//! Contents: `pub fn pow_to_powabs(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn pow_to_powabs_dyn(&mut ir::Design, &mut ir::Diags)`

use super::POW_BASE;
use super::nodes::{form, func_form, literal, note_left};
use crate::ir;

/// Rewrites `Pow` in every root of the plain form.
pub fn pow_to_powabs(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn pow_to_powabs_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let d = b.design();
        let ir::Expr::Call(ir::Func::Pow, args) = d.expr(e) else {
            return None;
        };
        let &[x, y] = d.exprs(args) else {
            unreachable!("check: pow takes two arguments")
        };
        let f = match literal(d, y) {
            Some(k) if k.fract() == 0.0 && (k / 2.0).fract() != 0.0 => ir::Func::PowSigned,
            Some(_) => ir::Func::PowAbs,
            None if never_negative(d, x) => ir::Func::PowAbs,
            None => return None,
        };
        Some(b.call(f, &[x, y]))
    });
    note_left(design, diags, func_form(ir::Func::Pow, dynamic), POW_BASE);
}

/// Whether `x` is `>= +0` on every evaluation (never negative, never `-0`).
fn never_negative(d: &ir::Design, x: ir::ExprId) -> bool {
    match d.expr(x) {
        ir::Expr::Num(n) => d.literal_value(d.num(n)).is_sign_positive(),
        ir::Expr::Call(f, _) => matches!(
            f,
            ir::Func::Abs | ir::Func::SqrtAbs | ir::Func::Exp | ir::Func::Sqr | ir::Func::PowAbs
        ),
        _ => false,
    }
}
