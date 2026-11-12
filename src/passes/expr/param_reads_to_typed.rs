//! Pass `param_reads_to_typed`: a design in, the same design out with every untyped parameter
//! read of a behavioural source's expression (a `Param` under neither `ToReal` nor `ToInt`,
//! A33, R56) wrapped by its tag (`tags.rs`): surely Real `to_real(p)`, surely Int `to_int(p)`;
//! each is the read itself, in value and in tag (a Real read is its value, an Int one is whole,
//! so its truncation is itself). A read whose tag the design does not fix (an instance passes
//! an Int and another a Real, a looped parameter, an unresolved read) leaves the source with
//! note `U0115`. Behavioural sources only (a resistor's value is no such expression, R52), each
//! through the per-root door `ir::rewrite_tree`. A row shared by a typed and an untyped place
//! is wrapped in both: `to_real(to_real(p))` and `to_int(to_real(p))` are still the read.
//!
//! Removes `Circuit(BehavUntypedParam)`; requires `FuncDyn(ToReal)`, `FuncDyn(ToInt)`,
//! `Expr(Call)` (a behavioural expression counts the dynamic form). One row.
//!
//! Cites: `rewrite/AMEND-A12.md` A33, A21, K1; `rewrite/RULINGS.md` R56 (runs ip1 to ip3).
//!
//! Contents: `pub fn param_reads_to_typed(&mut ir::Design, &mut ir::Diags)`

use std::collections::HashSet;

use super::UNTYPED_READ;
use super::nodes::{kids, note_left};
use super::tags::{INT, REAL, Tags};
use crate::ir;

/// Wraps every untyped parameter read of every behavioural source by its tag; notes a source
/// left with one.
pub fn param_reads_to_typed(design: &mut ir::Design, diags: &mut ir::Diags) {
    let mut tags = Tags::new(design);
    let behavs: Vec<ir::Element> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Element)
        .map(|(i, _)| design.row::<ir::Element>(i.index))
        .filter(|e| e.kind == ir::ElemKind::Behav)
        .collect();
    for el in behavs {
        let row: ir::controlled::Behav = design.detail(el.detail);
        let untyped = untyped_reads(design, row.expr);
        if untyped.is_empty() {
            continue;
        }
        let mut editor = design.editor();
        let expr = ir::rewrite_tree(&mut editor.builder(), row.expr, &mut |b, e| {
            if !untyped.contains(&e) {
                return None;
            }
            let wrap = match tags.of(b.design(), e) {
                REAL => ir::Func::ToReal,
                INT => ir::Func::ToInt,
                _ => return None,
            };
            Some(b.call(wrap, &[e]))
        });
        editor.set_detail(el.detail, ir::controlled::Behav { expr, ..row });
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::BehavUntypedParam);
    note_left(design, diags, feature, UNTYPED_READ);
}

// ---- private ----

/// The `Param` rows of the tree under `root` reached outside every `ToReal` and `ToInt`.
/// Each (row, typed) pair visited once.
fn untyped_reads(design: &ir::Design, root: ir::ExprId) -> HashSet<ir::ExprId> {
    let mut out = HashSet::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(root, false)];
    let mut below = Vec::new();
    while let Some((e, typed)) = stack.pop() {
        if e.is_none() || !seen.insert((e, typed)) {
            continue;
        }
        let x = design.expr(e);
        if matches!(x, ir::Expr::Param(_)) && !typed {
            out.insert(e);
        }
        let wraps = typed || matches!(x, ir::Expr::Call(ir::Func::ToReal | ir::Func::ToInt, _));
        below.clear();
        kids(design, e, &mut below);
        stack.extend(below.iter().map(|&k| (k, wraps)));
    }
    out
}
