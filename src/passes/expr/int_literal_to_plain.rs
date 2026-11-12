//! Pass `int_literal_to_plain`: a design in, the same design out with every integer literal
//! (`NumForm::Int`, A12 K1) whose tag nothing reads written as the plain literal of the same
//! digits. The value is the same double; only the tag goes from Int to Real, and with it the
//! tag of every node the literal's tag flows into.
//!
//! A tag is read by: the typed division `DivTyped` (Int operands truncate), and the rules that
//! judge their operands by the tag table: `IntDiv`, the bit operators, `ToInt`, `ToReal`,
//! `SgnZeroPos` (A21, A25; their own rules run first, and `legalize`'s fixed point comes back
//! here once they are gone). A literal is left when its tag can reach an operand of one of
//! them, following the tag table's flow backwards: through `+ - *`, `/:`, `-x`, both branches
//! of a ternary, `min`, `max`, `sgn_zero_pos`; from a parameter read to every value of its
//! definition (its value, a head default, each instance's actual, a function body); from a
//! function's formal read to every call's argument; from a parameter a loop or an alter sets to
//! every expression of that item. Every other node (a comparison, a call, a field) reads a
//! value only. Each item left holding an integer literal gets `U0118`.
//!
//! Exact in value: the literal's double is its digits' either way. One widening: an Int result
//! beyond 2⁵³ is out of the domain (R12) where its Real twin is a value.
//!
//! Removes `Extra(IntLiteral)`; requires nothing (a plain literal is no feature). One row: a
//! literal has no form.
//!
//! Cites: `rewrite/AMEND-A12.md` K1, A21, A25; `crates/ir/src/eval.rs` (`apply`'s tags).
//!
//! Contents: `pub fn int_literal_to_plain(&mut ir::Design, &mut ir::Diags)`

use super::INT_TAG_READ;
use super::nodes::{def_values, kids, note_left};
use crate::ir;

/// Writes every integer literal whose tag nothing reads as a plain literal; notes the items
/// left holding one.
pub fn int_literal_to_plain(design: &mut ir::Design, diags: &mut ir::Diags) {
    let read = tag_read(design);
    assert_eq!(read.len(), design.expr_count(), "one mark per row");
    ir::rewrite_exprs(design, &mut |b, e| {
        let ir::Expr::Num(n) = b.design().expr(e) else {
            return None;
        };
        let l = b.design().num(n);
        if l.form != ir::NumForm::Int || read[e.index()] {
            return None;
        }
        assert!(
            l.exp10 == 0 && l.frac == 0,
            "an Int literal is whole digits"
        );
        Some(b.literal(ir::Literal {
            form: ir::NumForm::Plain,
            ..l
        }))
    });
    note_left(
        design,
        diags,
        ir::Feature::Extra(ir::Extra::IntLiteral),
        INT_TAG_READ,
    );
}

// ---- private ----

/// Per expression row: whether its tag may reach an operand of a node that reads a tag.
/// A worklist over rows and definitions; each row and each definition enters it once.
fn tag_read(design: &ir::Design) -> Vec<bool> {
    let n = design.expr_count();
    let defs = design.definitions();
    let values = def_values(design);
    let mut read = vec![false; n];
    let mut def_read = vec![false; defs.len()];
    let mut stack: Vec<ir::ExprId> = Vec::new();
    // the operands of every reader
    let mut out = Vec::new();
    for i in 0..n {
        let e = ir::ExprId(i as u32);
        if reads_tag(design.expr(e)) {
            out.clear();
            kids(design, e, &mut out);
            stack.extend_from_slice(&out);
        }
    }
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

/// Whether `e` reads its operands' tags.
fn reads_tag(e: ir::Expr) -> bool {
    matches!(
        e,
        ir::Expr::Binary(
            ir::BinOp::DivTyped
                | ir::BinOp::IntDiv
                | ir::BinOp::BitAnd
                | ir::BinOp::BitOr
                | ir::BinOp::BitXor
                | ir::BinOp::Shl
                | ir::BinOp::Shr,
            ..
        ) | ir::Expr::Unary(ir::UnOp::BitNot, _)
            | ir::Expr::Call(ir::Func::ToInt | ir::Func::ToReal | ir::Func::SgnZeroPos, _)
    )
}

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
                | ir::Handle::Ref(_)
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
