//! Passes `clamp_to_min_max` and `clamp_to_min_max_dyn`: a design in, the same design out with
//! every `clamp(x, lo, hi)` (Xyce's `limit`) of one feature context written
//! `min(max(x, lo), hi)`, the IR's own definition of `Clamp`. Exact on its domain: `x`, `lo`,
//! `hi` are each evaluated once, in the same order; `lo > hi`, `clamp`'s domain error, is not
//! carried (R12: what a function does outside its domain is no part of its meaning). `clamp` is
//! Real while `min` and `max` of Ints are Int: where a typed division stands (A12 K1: the one
//! reader of a tag) and the result may be Int, it is written `1 * min(…)`. It refuses nothing
//! but a function body its callers read in both forms (A28): note `U0112`.
//!
//! `clamp_to_min_max` removes `Func(Clamp)` and requires `Func(Min)`, `Func(Max)`,
//! `Binary(Mul)`; `_dyn` the dynamic forms.
//!
//! Cites: `crates/ir/src/expr.rs` (`Func::Clamp`); `crates/ir/src/eval.rs`.
//!
//! Contents: `pub fn clamp_to_min_max(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn clamp_to_min_max_dyn(&mut ir::Design, &mut ir::Diags)`

use super::BOTH_FORMS;
use super::nodes::{form, func_form, note_left, plain, reads_tags};
use super::tags::{REAL, Tags};
use crate::ir;

/// Rewrites `Clamp` in every root of the plain form.
pub fn clamp_to_min_max(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same in every root of the dynamic form.
pub fn clamp_to_min_max_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let untyped = !reads_tags(design);
    let mut tags = Tags::new(design);
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
        tags.of(b.design(), e);
        let ir::Expr::Call(ir::Func::Clamp, args) = b.design().expr(e) else {
            return None;
        };
        let &[x, lo, hi] = b.design().exprs(args) else {
            unreachable!("check: clamp takes three arguments")
        };
        let max = b.call(ir::Func::Max, &[x, lo]);
        let min = b.call(ir::Func::Min, &[max, hi]);
        if untyped || tags.of(b.design(), min) == REAL {
            return Some(min);
        }
        let one = plain(b, 1, 0);
        let m = b.binary(ir::BinOp::Mul, one, min);
        tags.of(b.design(), m);
        Some(m)
    });
    note_left(
        design,
        diags,
        func_form(ir::Func::Clamp, dynamic),
        BOTH_FORMS,
    );
}
