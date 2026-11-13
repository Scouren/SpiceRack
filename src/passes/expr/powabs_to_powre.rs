//! Passes `powabs_to_powre` and `powabs_to_powre_dyn`: a design in, the same design out with
//! every `pow_abs(x, y)` (`|x|ʸ`, ngspice's B-source `**`) of one feature context written
//! `pow_re(abs(x), y)` (Xyce's `**`). Exact: `abs(x) >= 0`, so `pow_re`'s negative-base rule
//! never applies, and both are `|x|.powf(y)` with the one domain error at `x == 0, y < 0`; `x`
//! and `y` are each evaluated once, in the same order. Bit for bit equal on finite operands
//! (R12: a non-finite operand is outside every function's domain: `abs` refuses it where
//! `pow_abs` would not see it). It refuses nothing but a function body its callers read in both
//! forms (A28): note `U0112`.
//!
//! The one rule for `pow_abs`: a target with `Pow` and no `PowRe` (VACASK) gets
//! `pow(abs(x), y)` through `powre_to_pow` (the base `abs(x)` is never negative). A direct
//! `pow_abs → pow(abs(x), y)` rule undid `pow_to_powabs`, so for a target with `PowRe` and
//! neither `Pow` nor `PowAbs` (Xyce) the fixed point left `pow_abs` refused.
//!
//! `powabs_to_powre` removes `Func(PowAbs)` and requires `Func(Abs)`, `Func(PowRe)`;
//! `powabs_to_powre_dyn` removes `FuncDyn(PowAbs)` and requires their dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::PowAbs`, `Func::PowRe`).
//!
//! Contents: `pub fn powabs_to_powre(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn powabs_to_powre_dyn(&mut ir::Design, &mut ir::Diags)`

use super::BOTH_FORMS;
use super::nodes::{form, func_form, note_left};
use crate::ir;

/// Rewrites `pow_abs(x, y)` as `pow_re(abs(x), y)` in every root of the plain form.
pub fn powabs_to_powre(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn powabs_to_powre_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let ir::Expr::Call(ir::Func::PowAbs, args) = b.design().expr(e) else {
            return None;
        };
        let &[x, y] = b.design().exprs(args) else {
            unreachable!("check: pow_abs takes two arguments")
        };
        let abs = b.call(ir::Func::Abs, &[x]);
        Some(b.call(ir::Func::PowRe, &[abs, y]))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::PowAbs, dynamic),
        BOTH_FORMS,
    );
}
