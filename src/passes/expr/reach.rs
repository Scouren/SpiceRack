//! Where an expression's tag (Int or Real) may be read: the tag table's flow (`tags.rs`)
//! followed backwards from the operands of the nodes that read a tag. Through `+ - *`, `/:`,
//! `-x`, both branches of a ternary, `min`, `max`, `sgn_zero_pos`; from a parameter read to
//! every value of its definition (its value, a head default, each instance's actual, a
//! function body); from a function's formal read to every call's argument; from a parameter a
//! loop or an alter sets to every expression of that item. Every other node reads a value
//! only. Shared by `int_literal_to_plain` and `div_typed_to_div`.
//!
//! Cites: `rewrite/AMEND-A12.md` K1, A21; `crates/ir/src/eval.rs` (`apply`'s tags).
//!
//! Contents: `pub(super) fn tag_reach(&ir::Design, Vec<ir::ExprId>) -> Vec<bool>`

use super::nodes::def_values;
use crate::ir;

/// Per expression row: whether its tag may reach one of `seeds` (the operands of the nodes
/// that read a tag), following the tag table's flow backwards. A worklist over rows and
/// definitions; each row and each definition enters it once.
pub(super) fn tag_reach(design: &ir::Design, seeds: Vec<ir::ExprId>) -> Vec<bool> {
    let n = design.expr_count();
    let defs = design.definitions();
    let values = def_values(design);
    let mut read = vec![false; n];
    let mut def_read = vec![false; defs.len()];
    let mut stack = seeds;
    // the calls of each function, for its formals
    let mut calls: Vec<Vec<ir::ExprList>> = vec![Vec::new(); defs.len()];
    for i in 0..n {
        if let ir::Expr::UserCall(r, args) = design.expr(ir::ExprId(i as u32))
            && r.is_resolved()
        {
            calls[r.def.index()].push(args);
        }
    }
    let setters = setters(design);
    // bounded: a round after the first starts only from a setter's row not yet read
    for _round in 0..=n + 1 {
        while let Some(e) = stack.pop() {
            if e.is_none() || read[e.index()] {
                continue;
            }
            read[e.index()] = true;
            let mut mark_def = |d: ir::DefId, stack: &mut Vec<ir::ExprId>| {
                if !def_read[d.index()] {
                    def_read[d.index()] = true;
                    stack.extend_from_slice(&values[d.index()]);
                }
            };
            match design.expr(e) {
                ir::Expr::Binary(
                    ir::BinOp::Add | ir::BinOp::Sub | ir::BinOp::Mul | ir::BinOp::DivTyped,
                    a,
                    b,
                )
                | ir::Expr::Ternary(_, a, b) => stack.extend([a, b]),
                ir::Expr::Unary(ir::UnOp::Neg, a) => stack.push(a),
                ir::Expr::Call(ir::Func::Min | ir::Func::Max | ir::Func::SgnZeroPos, l) => {
                    stack.extend_from_slice(design.exprs(l))
                }
                ir::Expr::UserCall(r, _) if r.is_resolved() => mark_def(r.def, &mut stack),
                ir::Expr::Param(r) if r.is_resolved() => {
                    let def = design.def(r.def);
                    match def.kind {
                        ir::DefKind::Func => {
                            let row: ir::item::Func = design.row(def.item.index);
                            let k = design.names(row.args).iter().position(|m| *m == r.name);
                            if let Some(k) = k {
                                for &args in &calls[r.def.index()] {
                                    stack.extend(design.exprs(args).get(k).copied());
                                }
                            }
                        }
                        _ => mark_def(r.def, &mut stack),
                    }
                }
                // a value only: the node's own tag does not follow this operand's
                _ => {}
            }
        }
        // a parameter a loop or an alter sets takes that item's values too
        for (item, roots) in &setters {
            let sets_read = item_defs(design, *item).iter().any(|d| def_read[d.index()]);
            if sets_read {
                stack.extend(
                    roots
                        .iter()
                        .copied()
                        .filter(|e| !e.is_none() && !read[e.index()]),
                );
            }
        }
        if stack.is_empty() {
            return read;
        }
    }
    unreachable!("each round marks a row or ends")
}

// ---- private ----

/// Every reachable loop and alter with its expression roots.
fn setters(design: &ir::Design) -> Vec<(ir::ItemRef, Vec<ir::ExprId>)> {
    let mut handles = Vec::new();
    let mut out = Vec::new();
    for (item, _) in design.reachable() {
        if !matches!(item.kind, ir::ItemKind::Loop | ir::ItemKind::Alter) {
            continue;
        }
        handles.clear();
        design.item_handles(item, &mut handles);
        let mut roots = Vec::new();
        for h in &handles {
            match *h {
                ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => roots.push(e),
                ir::Handle::Exprs(l) => roots.extend_from_slice(design.exprs(l)),
                ir::Handle::Params(l) => roots.extend(design.params(l).iter().map(|p| p.1)),
                ir::Handle::Branches(l) => roots.extend(design.branches(l).iter().map(|b| b.cond)),
                ir::Handle::Name(_)
                | ir::Handle::Ref(..)
                | ir::Handle::Names(_)
                | ir::Handle::Scope(_)
                | ir::Handle::Text(_) => {}
            }
        }
        out.push((item, roots));
    }
    out
}

/// The parameter definitions `item` refers to.
fn item_defs(design: &ir::Design, item: ir::ItemRef) -> Vec<ir::DefId> {
    let mut refs = Vec::new();
    design.item_refs(item, &mut refs);
    refs.into_iter()
        .filter(|(r, k)| *k == ir::DefKind::Param && r.is_resolved())
        .map(|(r, _)| r.def)
        .collect()
}
