//! Passes `powabs_to_pow` and `powabs_to_pow_dyn`: a design in, the same design out with every
//! `pow_abs(x, y)` (`|x|ʸ`, ngspice's B-source `**`) of one feature context written
//! `pow(abs(x), y)`. Exact: `abs(x) >= 0`, so `pow`'s negative-base rule never applies, and
//! both are `|x|.powf(y)` with the one domain error at `x == 0, y < 0`; `x` and `y` are each
//! evaluated once, in the same order, so a draw is neither duplicated nor reordered. Bit for
//! bit equal on finite operands (R12: a non-finite operand is outside every function's domain:
//! `abs` refuses it where `pow_abs` would not see it). It refuses nothing.
//!
//! `powabs_to_pow` removes `Func(PowAbs)` and requires `Func(Abs)`, `Func(Pow)`;
//! `powabs_to_pow_dyn` removes `FuncDyn(PowAbs)` and requires their dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1; `rewrite/AMEND-A12.md` A23; `crates/ir/src/expr.rs`
//! (`Func::Pow`, `Func::PowAbs`).
//!
//! Contents: `pub fn powabs_to_pow(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn powabs_to_pow_dyn(&mut ir::Design, &mut ir::Diags)`

use crate::ir;

/// Rewrites `pow_abs(x, y)` as `pow(abs(x), y)` in every root that does not read the circuit.
pub fn powabs_to_pow(design: &mut ir::Design, _diags: &mut ir::Diags) {
    ir::rewrite_exprs_in(design, ir::Form::Static, &mut rule);
}

/// Rewrites `pow_abs(x, y)` as `pow(abs(x), y)` in every root of the dynamic form: one that
/// reads the circuit or the temperature, or stands in a circuit field (a behavioural source, a
/// function body).
pub fn powabs_to_pow_dyn(design: &mut ir::Design, _diags: &mut ir::Diags) {
    ir::rewrite_exprs_in(design, ir::Form::Dynamic, &mut rule);
}

// ---- private ----

/// `pow_abs(x, y)` → `pow(abs(x), y)`; any other node is kept.
fn rule(b: &mut ir::Builder<'_>, e: ir::ExprId) -> Option<ir::ExprId> {
    let ir::Expr::Call(ir::Func::PowAbs, args) = b.design().expr(e) else {
        return None;
    };
    let &[x, y] = b.design().exprs(args) else {
        unreachable!("check: pow_abs takes two arguments")
    };
    let abs = b.call(ir::Func::Abs, &[x]);
    Some(b.call(ir::Func::Pow, &[abs, y]))
}
