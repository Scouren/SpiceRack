//! Passes `div_typed_to_div` and `div_typed_to_div_dyn`: a design in, the same design out with
//! every typed division (`DivTyped`, VACASK's `/`) of one form written with real division, by
//! the tag table: an operand Real on every evaluation → `a / b`; both Int on every evaluation
//! → `trunc(a / b)` (ngspice's `\` is `trunc(|a / b|)`). The value is the same
//! bit for bit; an Int quotient becomes a Real whole number, which no reader can tell apart
//! once no typed division reads it (only `DivTyped` reads a tag, A12 K1). Each row rewrites the
//! roots of its own form (R65: its `requires` are what it writes there), deciding each division
//! by the table of the design as it was (`Tags::set` keeps a rewritten quotient's tag for the
//! divisions above it). Where a typed division of the other form, which the row leaves, may
//! read an Int quotient's tag (`reach.rs`), no Int quotient is written in this run: the other
//! row, then `legalize`'s next round, writes them (a static root never reads a dynamic one).
//!
//! A division whose operands' tags the design does not fix (a parameter an instance passes an
//! Int and another a Real, a formal two calls pass differently, a looped parameter) keeps its
//! form with note `U0101`, and then no Int quotient is rewritten either (a typed division left
//! standing could read its tag): those keep it with the same note. An Int quotient left for a
//! division of the other form gets `U0119`.
//!
//! `div_typed_to_div` removes `Binary(DivTyped)` and requires `Binary(Div)`, `Func(Trunc)`;
//! `_dyn` removes `BinaryDyn(DivTyped)` and requires `BinaryDyn(Div)`, `FuncDyn(Trunc)`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2; `rewrite/AMEND-A12.md` K1.
//!
//! Contents: `pub fn div_typed_to_div(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn div_typed_to_div_dyn(&mut ir::Design, &mut ir::Diags)`

use super::nodes::{binary_form, form, kids, note_left};
use super::reach::tag_reach;
use super::tags::{INT, REAL, Tags};
use super::{MIXED_DIVISION, OTHER_FORM_READS};
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
    let live = design.live_exprs();
    for &e in &live {
        if let ir::Expr::Binary(ir::BinOp::DivTyped, x, y) = design.expr(e) {
            let (real, int) = decided(&mut tags, design, x, y);
            ints &= real || int;
        }
    }
    let decided_all = ints;
    // an Int quotient written Real is read wrong by an Int-by-Int typed division of the other
    // form, which this row leaves: then none is written (the other row runs, and the fixed
    // point returns; a static root never reads a dynamic one)
    let readers = other_form_operands(design, dynamic, &mut tags);
    let read = tag_reach(design, readers);
    let quotient =
        |e: ir::ExprId| matches!(design.expr(e), ir::Expr::Binary(ir::BinOp::DivTyped, ..));
    ints &= !live.iter().any(|&e| read[e.index()] && quotient(e));
    ir::rewrite_exprs_in(design, form(dynamic), &mut |b, e| {
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
    let code = match decided_all {
        true => OTHER_FORM_READS,
        false => MIXED_DIVISION,
    };
    note_left(design, diags, own, code);
}

/// The operands of every typed division in a root of the other form (or of both: a function
/// body two forms call) that no Real operand decides: the divisions whose value an operand's
/// tag changes.
fn other_form_operands(design: &mut ir::Design, dynamic: bool, tags: &mut Tags) -> Vec<ir::ExprId> {
    let other = form(!dynamic);
    let items: Vec<ir::ItemRef> = design.reachable().into_iter().map(|(i, _)| i).collect();
    let mut roots = Vec::new();
    {
        let mut editor = design.editor();
        for item in items {
            editor.map_exprs(item, &mut |e, use_| roots.push((*e, use_)));
        }
    }
    let mut out = Vec::new();
    let mut seen = vec![false; design.expr_count_all()];
    let mut stack = Vec::new();
    for (root, use_) in roots {
        if root.is_none() || ![other, ir::Form::Both].contains(&ir::root_form(design, root, use_)) {
            continue;
        }
        stack.push(root);
        // bounded: each row once
        while let Some(e) = stack.pop() {
            if e.is_none() || seen[e.index()] {
                continue;
            }
            seen[e.index()] = true;
            if let ir::Expr::Binary(ir::BinOp::DivTyped, x, y) = design.expr(e)
                && tags.of(design, x) != REAL
                && tags.of(design, y) != REAL
            {
                out.extend([x, y]);
            }
            kids(design, e, &mut stack);
        }
    }
    out
}
