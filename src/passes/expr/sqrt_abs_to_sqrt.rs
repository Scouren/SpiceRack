//! Passes `sqrt_abs_to_sqrt` and `sqrt_abs_to_sqrt_dyn`: a design in, the same design out with every `sqrt_abs(x)`
//! of one feature context written `sqrt(abs(x))`: its definition, `√|x|`; `abs(x) >= +0`, so
//! `sqrt`'s domain always holds. `x` is read once. It refuses nothing.
//!
//! `sqrt_abs_to_sqrt` removes `Func(SqrtAbs)` and requires `Func(Sqrt)`, `Func(Abs)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::SqrtAbs`, its definition: the spec); the lead's list of
//! what VACASK lacks (`dialect-vacask` 51ee264a, its expression tables).
//!
//! Contents: `pub fn sqrt_abs_to_sqrt(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn sqrt_abs_to_sqrt_dyn(&mut ir::Design, &mut ir::Diags)`

use super::BOTH_FORMS;
use super::nodes::{form, func_form, note_left};
use crate::ir;

/// Rewrites `SqrtAbs` in every root of the plain form.
pub fn sqrt_abs_to_sqrt(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn sqrt_abs_to_sqrt_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        let ir::Expr::Call(ir::Func::SqrtAbs, args) = b.design().expr(e) else {
            return None;
        };
        let x = b.design().exprs(args)[0];
        let abs = b.call(ir::Func::Abs, &[x]);
        Some(b.call(ir::Func::Sqrt, &[abs]))
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::SqrtAbs, dynamic),
        BOTH_FORMS,
    );
}
