//! Passes `toint_to_trunc` and `toint_to_trunc_dyn`: a design in, the same design out with every
//! `int(x)` of Int type (`ToInt`, VACASK's `int`) of one feature context written `trunc(x)`
//! (`Trunc`, Real), once no typed division is left in the design: the value is the same, and
//! only `DivTyped` reads a tag (A12 K1). While one stands, the call keeps its form with note
//! `U0103` (it runs after `div_typed_to_div`, which removes them).
//!
//! `toint_to_trunc` removes `Func(ToInt)` and requires `Func(Trunc)`; `_dyn` the dynamic forms.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2; `rewrite/AMEND-A12.md` K1.
//!
//! Contents: `pub fn toint_to_trunc(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn toint_to_trunc_dyn(&mut ir::Design, &mut ir::Diags)`

use super::TAG_READ;
use super::nodes::{form, func_form, note_left, reads_tags};
use crate::ir;

/// Rewrites `ToInt` as `Trunc` in every root of the plain form.
pub fn toint_to_trunc(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn toint_to_trunc_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    if !reads_tags(design) {
        ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
            let ir::Expr::Call(ir::Func::ToInt, args) = b.design().expr(e) else {
                return None;
            };
            let x = b.design().exprs(args)[0];
            Some(b.call(ir::Func::Trunc, &[x]))
        });
    }
    note_left(design, diags, func_form(ir::Func::ToInt, dynamic), TAG_READ);
}
