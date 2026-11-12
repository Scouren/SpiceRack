//! Passes `div_typed_to_div` and `div_typed_to_div_dyn`: a design in, the same design out with
//! every typed division (`DivTyped`, VACASK's `/`) written with real division, by the tag
//! table: an operand Real on every evaluation → `a / b`; both Int on every evaluation →
//! `trunc(a / b)` (never `IntDiv`: ngspice's `\` is `trunc(|a / b|)`). The value is the same bit
//! for bit; an Int quotient becomes a Real whole number, which no reader can tell apart once no
//! typed division is left (only `DivTyped` reads a tag, A12 K1). So the tags of the two forms
//! are one question: each row runs only when the design holds its own form, and then rewrites
//! every typed division of both forms, deciding each by the table of the design as it was
//! (`Tags::set` keeps a rewritten quotient's tag for the divisions above it).
//!
//! A division whose operands' tags the design does not fix (a parameter an instance passes an
//! Int and another a Real, a formal two calls pass differently, a looped parameter) keeps its
//! form with note `U0101`, and then no Int quotient is rewritten either (a typed division left
//! standing could read its tag): those keep it with the same note.
//!
//! `div_typed_to_div` removes `Binary(DivTyped)`, `_dyn` removes `BinaryDyn(DivTyped)`; both
//! require `Div` and `Trunc` in both forms (the other form's divisions are rewritten too).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2; `rewrite/AMEND-A12.md` K1; `crates/ir/src/expr.rs`
//! (`BinOp::IntDiv`).
//!
//! Contents: `pub fn div_typed_to_div(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn div_typed_to_div_dyn(&mut ir::Design, &mut ir::Diags)`

use super::MIXED_DIVISION;
use super::nodes::{binary_form, live, note_left};
use super::tags::{INT, REAL, Tags};
use crate::ir;

/// Rewrites every typed division as real division, when the design holds one of the plain form.
pub fn div_typed_to_div(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, false);
}

/// The same, when the design holds a typed division of the dynamic form.
pub fn div_typed_to_div_dyn(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, true);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, dynamic: bool) {
    let own = binary_form(ir::BinOp::DivTyped, dynamic);
    if !ir::features_used(design).contains(own) {
        return;
    }
    let mut tags = Tags::new(design);
    let decided = |tags: &mut Tags, d: &ir::Design, x, y| {
        let (tx, ty) = (tags.of(d, x), tags.of(d, y));
        (tx == REAL || ty == REAL, tx == INT && ty == INT)
    };
    let mut ints = true;
    for e in live(design) {
        if let ir::Expr::Binary(ir::BinOp::DivTyped, x, y) = design.expr(e) {
            let (real, int) = decided(&mut tags, design, x, y);
            ints &= real || int;
        }
    }
    ir::rewrite_exprs(design, &mut |b, e| {
        tags.of(b.design(), e);
        let ir::Expr::Binary(ir::BinOp::DivTyped, x, y) = b.design().expr(e) else {
            return None;
        };
        let (real, int) = decided(&mut tags, b.design(), x, y);
        if !(real || int && ints) {
            return None;
        }
        let q = b.binary(ir::BinOp::Div, x, y);
        tags.of(b.design(), q);
        if real {
            return Some(q);
        }
        let t = b.call(ir::Func::Trunc, &[q]);
        tags.set(t, INT);
        Some(t)
    });
    for dynamic in [false, true] {
        let f = binary_form(ir::BinOp::DivTyped, dynamic);
        note_left(design, diags, f, MIXED_DIVISION);
    }
}
