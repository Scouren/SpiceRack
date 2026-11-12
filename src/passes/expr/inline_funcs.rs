//! Passes `inline_funcs` and `drop_unused_funcs`: a design in, the same design out with every
//! call of a user function replaced by its body, each formal argument's read replaced by the
//! call's argument (ngspice's textual rule: an argument is evaluated where the body reads it),
//! then every function whose calls were all replaced taken out of its scope (its body would
//! otherwise count static, A28: a function nothing calls). A body counts in its
//! callers' forms (A28), so a copy in its caller adds no feature the design did not count.
//!
//! The value is the call's wherever the call has one: the evaluator binds each argument once
//! and the copy reads it at each use, which is the same number unless the argument holds a
//! draw. An argument that may hold a draw and that the body reads more than once keeps the call
//! (each read would draw anew), and so does a call of a function on or behind a call cycle (a
//! recursive `.func` has no finite copy), an unresolved one, or one whose argument of class `Temp` or above the body never reads
//! (the caller counts the dynamic form through it, R14; the copy would not): note `U0113`. Where the call
//! has no value (an argument the body never reads, or reads on a branch not taken, raises a
//! domain error) the copy may have one: R12, not carried. A body calling another function is
//! inlined over rounds, one call depth each, bounded by the function count.
//!
//! `inline_funcs` removes `Expr(UserCall)`; `drop_unused_funcs` removes `Item(Func)` (every function
//! nothing calls, transitively), noting with `U0114` a function something still calls. Neither requires a feature. One row each:
//! neither is a function or operator of a form.
//!
//! Cites: `rewrite/RULINGS.md` R51 (A28); `rewrite/DIALECT-VACASK.md` §6.1; ngspice's `.func`
//! (`INV-semantics.md` 2.26).
//!
//! Contents: `pub fn inline_funcs(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn drop_unused_funcs(&mut ir::Design, &mut ir::Diags)`

use std::collections::HashMap;

use super::nodes::{Draws, kids, live, note_left};
use super::{CALL_LEFT, FUNC_CALLED};
use crate::ir;

/// Replaces every call of an acyclic, resolved user function by its body with the arguments
/// substituted, over rounds; notes each item left holding a call.
pub fn inline_funcs(design: &mut ir::Design, diags: &mut ir::Diags) {
    let funcs = design
        .definitions()
        .iter()
        .filter(|d| d.kind == ir::DefKind::Func)
        .count();
    let was = called(design);
    for _ in 0..=funcs {
        let draws = Draws::new(design);
        let ok = acyclic(design);
        let changed = ir::rewrite_exprs(design, &mut |b, e| {
            let ir::Expr::UserCall(r, args) = b.design().expr(e) else {
                return None;
            };
            if !r.is_resolved() || !ok[r.def.index()] {
                return None;
            }
            let args = b.design().exprs(args).to_vec();
            inline(b, r.def, &args, &draws)
        });
        if changed == 0 {
            break;
        }
    }
    // a function whose every call was replaced goes with its calls: its body would otherwise
    // count static (A28: uncalled), a form no caller gave it
    let now = called(design);
    let gone: Vec<bool> = was.iter().zip(&now).map(|(&a, &b)| a && !b).collect();
    drop_uncalled(design, &gone);
    note_left(
        design,
        diags,
        ir::Feature::Expr(ir::ExprTag::UserCall),
        CALL_LEFT,
    );
}

/// Takes every function no live expression calls out of its scope's item list (until none is
/// left: a function only a dropped one called goes too).
pub fn drop_unused_funcs(design: &mut ir::Design, diags: &mut ir::Diags) {
    let all = vec![true; design.definitions().len()];
    drop_uncalled(design, &all);
    note_left(
        design,
        diags,
        ir::Feature::Item(ir::ItemKind::Func),
        FUNC_CALLED,
    );
}

// ---- private ----

/// By `DefId`: whether a live expression calls the definition.
fn called(design: &ir::Design) -> Vec<bool> {
    let mut out = vec![false; design.definitions().len()];
    for e in live(design) {
        if let ir::Expr::UserCall(r, _) = design.expr(e)
            && r.is_resolved()
        {
            out[r.def.index()] = true;
        }
    }
    out
}

/// Takes out of its scope every reachable function that `may[def]` allows and nothing calls,
/// again while one goes (a function only a dropped one called), bounded by the function count.
fn drop_uncalled(design: &mut ir::Design, may: &[bool]) {
    for _ in 0..=design.definitions().len() {
        let calls = called(design);
        let defs = design.definitions();
        let unused: Vec<(ir::ItemRef, ir::ScopeId)> = design
            .reachable()
            .into_iter()
            .filter(|(i, _)| i.kind == ir::ItemKind::Func)
            .filter(|&(i, _)| {
                let def = defs
                    .iter()
                    .position(|d| d.kind == ir::DefKind::Func && d.item == i);
                def.is_some_and(|d| may[d] && !calls[d])
            })
            .collect();
        if unused.is_empty() {
            return;
        }
        let mut editor = design.editor();
        for (item, scope) in unused {
            let items = editor.items_mut(scope);
            let before = items.len();
            items.retain(|&i| i != item);
            assert!(
                items.len() + 1 == before,
                "the function stood once in its scope"
            );
        }
    }
    unreachable!("each round drops a function or ends")
}

/// The copy of `def`'s body for one call, or `None` when the call must stay (an argument count
/// that does not match, or a draw-holding argument read more than once).
fn inline(
    b: &mut ir::Builder<'_>,
    def: ir::DefId,
    args: &[ir::ExprId],
    draws: &Draws,
) -> Option<ir::ExprId> {
    let d = b.design();
    let row: ir::item::Func = d.row(d.def(def).item.index);
    let formals = d.names(row.args).to_vec();
    if formals.len() != args.len() {
        return None;
    }
    let formal = |d: &ir::Design, e: ir::ExprId| match d.expr(e) {
        ir::Expr::Param(r) if r.def == def => formals.iter().position(|n| *n == r.name),
        _ => None,
    };
    // reads of each formal along every path of the body (a shared row counts at each use)
    let mut reads = vec![0usize; formals.len()];
    let mut stack = vec![row.body];
    let mut out = Vec::new();
    let bound = 64 * d.expr_count() + 64;
    for _ in 0..bound {
        let Some(e) = stack.pop() else { break };
        if let Some(k) = formal(d, e) {
            reads[k] += 1;
        }
        out.clear();
        kids(d, e, &mut out);
        stack.extend_from_slice(&out);
    }
    assert!(stack.is_empty(), "a body's paths are bounded");
    if (0..args.len()).any(|k| reads[k] > 1 && draws.holds(d, args[k])) {
        return None;
    }
    // an argument the body never reads drops out of the copy; one of class Temp or above made
    // its caller count the dynamic form (R14), which the copy would no longer
    let dynamic = |k: usize| ir::expr_class(d, args[k]) >= ir::ExprClass::Temp;
    if (0..args.len()).any(|k| reads[k] == 0 && dynamic(k)) {
        return None;
    }
    Some(copy(b, row.body, &|d, e| formal(d, e).map(|k| args[k])))
}

/// A copy of the tree under `root` with each row `swap` names replaced; leaves are shared, every
/// operator is built anew over its copied operands. Post-order, explicit stack, each row once.
fn copy(
    b: &mut ir::Builder<'_>,
    root: ir::ExprId,
    swap: &dyn Fn(&ir::Design, ir::ExprId) -> Option<ir::ExprId>,
) -> ir::ExprId {
    let mut done: HashMap<ir::ExprId, ir::ExprId> = HashMap::new();
    let mut stack = vec![(root, false)];
    let mut out = Vec::new();
    let bound = 2 * b.design().expr_count() + 2;
    for _ in 0..bound {
        let Some((e, ready)) = stack.pop() else { break };
        if done.contains_key(&e) {
            continue;
        }
        if let Some(arg) = swap(b.design(), e) {
            done.insert(e, arg);
            continue;
        }
        out.clear();
        kids(b.design(), e, &mut out);
        if !ready && !out.is_empty() {
            stack.push((e, true));
            stack.extend(out.iter().map(|&k| (k, false)));
            continue;
        }
        let new: Vec<ir::ExprId> = out.iter().map(|k| done[k]).collect();
        let built = rebuild(b, e, &new);
        done.insert(e, built);
    }
    assert!(stack.is_empty(), "the copy visits each row at most twice");
    done[&root]
}

/// `e` built anew over `new` operands (in `kids`' order); a leaf is itself.
fn rebuild(b: &mut ir::Builder<'_>, e: ir::ExprId, new: &[ir::ExprId]) -> ir::ExprId {
    match b.design().expr(e) {
        ir::Expr::Call(f, _) => b.call(f, new),
        ir::Expr::Draw(dist, when, per, _) => b.draw(dist, when, per, new),
        ir::Expr::List(_) => b.list(new),
        ir::Expr::Unary(op, _) => b.unary(op, new[0]),
        ir::Expr::Binary(op, ..) => b.binary(op, new[0], new[1]),
        ir::Expr::Ternary(..) => b.ternary(new[0], new[1], new[2]),
        ir::Expr::UserCall(r, _) => {
            let name = b.design().str(r.name).to_string();
            let call = b.user_call(&name, new);
            if r.is_resolved() {
                b.resolve(call, r.def);
            }
            call
        }
        ir::Expr::Num(_)
        | ir::Expr::Str(_)
        | ir::Expr::Param(_)
        | ir::Expr::V(..)
        | ir::Expr::I(_)
        | ir::Expr::Time
        | ir::Expr::Temp
        | ir::Expr::Freq
        | ir::Expr::TStep
        | ir::Expr::TStop
        | ir::Expr::Tnom => e,
    }
}

/// By `DefId`: whether a function definition is off every call cycle and calls none on one (its
/// copy is finite). Kahn's order over the bodies' calls, leaves first.
fn acyclic(design: &ir::Design) -> Vec<bool> {
    let defs = design.definitions();
    let mut calls: Vec<Vec<usize>> = vec![Vec::new(); defs.len()];
    let mut out = Vec::new();
    for (d, def) in defs.iter().enumerate() {
        if def.kind != ir::DefKind::Func || def.item.kind != ir::ItemKind::Func {
            continue;
        }
        let mut stack = vec![design.row::<ir::item::Func>(def.item.index).body];
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = stack.pop() {
            if e.is_none() || !seen.insert(e) {
                continue;
            }
            if let ir::Expr::UserCall(r, _) = design.expr(e) {
                calls[d].push(if r.is_resolved() { r.def.index() } else { d });
            }
            out.clear();
            kids(design, e, &mut out);
            stack.extend_from_slice(&out);
        }
    }
    let mut ok = vec![false; defs.len()];
    for _ in 0..=defs.len() {
        let mut grew = false;
        for d in 0..defs.len() {
            if !ok[d] && defs[d].kind == ir::DefKind::Func && calls[d].iter().all(|&c| ok[c]) {
                ok[d] = true;
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    ok
}
