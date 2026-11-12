//! Passes `ln_abs_to_ln` and `ln_abs_to_ln_dyn`: a design in, the same design out with every `ln_abs(x)`
//! of one feature context written `ln(abs(x))`: its definition, `ln|x|`; `x = ±0` is `ln(+0)`, the one domain
//! error of both. `x` is read once. It refuses nothing.
//!
//! `ln_abs_to_ln` removes `Func(LnAbs)` and requires `Func(Ln)`, `Func(Abs)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::LnAbs`, its definition: the spec); the lead's list of
//! what VACASK lacks (`dialect-vacask` 51ee264a, its expression tables).
//!
//! Contents: `pub fn ln_abs_to_ln(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn ln_abs_to_ln_dyn(&mut ir::Design, &mut ir::Diags)`

use super::nodes::form;
use crate::ir;

/// Rewrites `LnAbs` in every root of the plain form.
pub fn ln_abs_to_ln(design: &mut ir::Design, _diags: &mut ir::Diags) {
    run(design, false);
}

/// The same in every root of the dynamic form.
pub fn ln_abs_to_ln_dyn(design: &mut ir::Design, _diags: &mut ir::Diags) {
    run(design, true);
}

// ---- private ----

fn run(design: &mut ir::Design, dynamic: bool) {
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let ir::Expr::Call(ir::Func::LnAbs, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        let abs = b.call(ir::Func::Abs, &[x]);
        Some(b.call(ir::Func::Ln, &[abs]))
    });
}
