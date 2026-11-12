//! A design, a feature context and a rule in, the design with the rule applied to the expression
//! roots of that context only out. A function or operator is `Func(f)` in a root that does not
//! read the circuit and `FuncDyn(f)` in one that does (`ir::features_used`: a root in a
//! `Handle::ExprDyn` field, or of class `Temp` or above, R14, A19), so a pass removes one of the
//! two and leaves the other's roots as they were (law P5).
//!
//! The rule runs through `ir::rewrite_exprs` on every root; the roots of the other context are
//! then pointed back at their old trees (rows are appended, never replaced, so the old trees are
//! still there, shared subtrees included).
//!
//! Cites: `rewrite/AMEND-A12.md` A23 (`rewrite_exprs`, `map_exprs`); `rewrite/SPINE.md` §4 (P5).
//!
//! Contents: `pub(super) fn rewrite_in(&mut ir::Design, bool, &mut dyn FnMut(&mut
//! ir::Builder<'_>, ir::ExprId) -> Option<ir::ExprId>)`

use crate::ir;

/// Rewrites by `rule` (as `ir::rewrite_exprs` does) every expression root whose features are the
/// dynamic form when `dynamic`, the plain form otherwise; every other root keeps its tree.
/// O(expressions + items).
pub(super) fn rewrite_in(
    design: &mut ir::Design,
    dynamic: bool,
    rule: &mut dyn FnMut(&mut ir::Builder<'_>, ir::ExprId) -> Option<ir::ExprId>,
) {
    let items: Vec<ir::ItemRef> = design.reachable().into_iter().map(|(i, _)| i).collect();
    // every root in `map_exprs` order, and whether its item holds it in a circuit field
    let mut roots: Vec<(ir::ExprId, bool)> = Vec::new();
    let mut ends = Vec::with_capacity(items.len());
    {
        let mut editor = design.editor();
        for &item in &items {
            let start = roots.len();
            editor.map_exprs(item, &mut |e, u| {
                roots.push((*e, u == ir::ExprUse::Circuit));
            });
            ends.push((start, roots.len()));
        }
    }
    // A19: a root an item holds in a circuit field counts the dynamic form in all its fields
    let mut keep: Vec<bool> = Vec::with_capacity(roots.len());
    for &(start, end) in &ends {
        let item = &roots[start..end];
        for &(e, _) in item {
            let in_circuit = item.iter().any(|&(f, c)| c && f == e);
            let dyn_form =
                !e.is_none() && (in_circuit || ir::expr_class(design, e) >= ir::ExprClass::Temp);
            keep.push(dyn_form != dynamic);
        }
    }
    assert!(keep.len() == roots.len(), "one verdict per root");
    if keep.iter().all(|&k| k) {
        return;
    }
    ir::rewrite_exprs(design, rule);
    let mut editor = design.editor();
    let mut k = 0;
    for &item in &items {
        editor.map_exprs(item, &mut |e, _| {
            if keep[k] {
                *e = roots[k].0;
            }
            k += 1;
        });
    }
    assert!(k == roots.len(), "map_exprs visits the same roots twice");
}
