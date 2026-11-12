//! Rule `loop_end`: what a loop's targets hold after it (`Loop::after`, `LoopEnd`), made the
//! other way by an `Alter` of each target right after the loop. Two rows, for loops over
//! parameters (`TargetKind::Param` bound to a `.param`), sweep or table:
//! - `loop_restore_to_keep_last` removes `Run(LoopRestore)`: the loop keeps its last value and
//!   an alter of each target to the design's own value (the parameter's expression) follows.
//!   Exact when nothing before the loop has changed the target: no alter of it earlier in the
//!   walk, no enclosing loop over it (the value in force before the loop is then the design's).
//!   Requires `Run(LoopKeepLast)`, `Item(Alter)`, `Run(AlterParam)`, `Run(AlterParamAtOnce)`.
//! - `loop_keep_last_to_restore` removes `Run(LoopKeepLast)`: the loop restores and an alter
//!   of each target to its last value follows: a list sweep's last value, a table's last row.
//!   Requires `Run(LoopRestore)`, `Item(Alter)`, `Run(AlterParam)`, `Run(AlterParamAtOnce)`.
//!
//! Both copy an operand into the alter, so a value holding a draw leaves the loop with
//! `LOOP_END_KEPT`, as does any other target kind, a Monte Carlo loop, a range sweep (its last
//! value is not a stated expression), a loop not listed in its walk's top, and one an earlier
//! alter or an enclosing loop changes the target of.
//!
//! Cites: `rewrite/AMEND-A12.md` A22, A27; `INV-semantics.md` 13.03; `crates/ir/src/plan.rs`.
//!
//! Contents: `pub fn loop_restore_to_keep_last(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn loop_keep_last_to_restore(&mut ir::Design, &mut ir::Diags)`,
//! `pub const LOOP_END_KEPT: ir::Code`

use crate::ir;
use ir::plan::{Loop, LoopEnd, LoopKind, SweepLoop, TableLoop, Target, TargetKind};

/// The note at a loop the rule leaves as it is.
pub const LOOP_END_KEPT: ir::Code = ir::Code::new(b'U', 409);

/// One alter to add: its target and value.
type Set = (Target, ir::ExprId);

/// Restoring loops made keep-last, each target altered back to its own value after.
pub fn loop_restore_to_keep_last(design: &mut ir::Design, diags: &mut ir::Diags) {
    convert(design, diags, LoopEnd::Restore);
}

/// Keep-last loops made restoring, each target altered to its last value after.
pub fn loop_keep_last_to_restore(design: &mut ir::Design, diags: &mut ir::Diags) {
    convert(design, diags, LoopEnd::KeepLast);
}

// ---- private ----

fn convert(design: &mut ir::Design, diags: &mut ir::Diags, from: LoopEnd) {
    let mut work: Vec<(ir::ItemRef, ir::ScopeId, Vec<Set>)> = Vec::new();
    for (top, order) in ir::run_walks(design) {
        for (k, &(item, scope)) in order.iter().enumerate() {
            if item.kind != ir::ItemKind::Loop || design.row::<Loop>(item.index).after != from {
                continue;
            }
            match alters(design, &order, k, from).filter(|_| scope == top) {
                Some(a) => work.push((item, scope, a)),
                None => diags.push_at(LOOP_END_KEPT, design, item, 0),
            }
        }
    }
    let mut editor = design.editor();
    for (item, scope, alters) in work {
        let mut l = editor.design().row::<Loop>(item.index);
        l.after = match from {
            LoopEnd::Restore => LoopEnd::KeepLast,
            LoopEnd::KeepLast => LoopEnd::Restore,
        };
        editor.set(item.index, l);
        let built: Vec<ir::ItemRef> = alters
            .into_iter()
            .map(|(t, v)| {
                editor.builder().push(
                    scope,
                    // the value from here on, where it stands: the loop's own end rule
                    ir::plan::Alter {
                        target: t,
                        value: v,
                        effect: ir::plan::AlterEffect::AtOnce,
                    },
                    ir::Span::NONE,
                )
            })
            .collect();
        let items = editor.items_mut(scope);
        items.retain(|i| !built.contains(i));
        let at = items.iter().position(|&i| i == item).expect("listed") + 1;
        items.splice(at..at, built);
    }
}

/// The alters that follow loop `order[k]` once converted, or `None` when it cannot be.
fn alters(
    design: &ir::Design,
    order: &[(ir::ItemRef, ir::ScopeId)],
    k: usize,
    from: LoopEnd,
) -> Option<Vec<Set>> {
    let l = design.row::<Loop>(order[k].0.index);
    let (targets, last): (Vec<Target>, Vec<ir::ExprId>) = match l.kind {
        LoopKind::Sweep => {
            let s = design.detail::<SweepLoop>(l.detail);
            let last = match s.sweep.kind {
                ir::sweep::SweepKind::List => design.exprs(s.sweep.values).last().copied(),
                _ => None,
            };
            (vec![s.target], vec![last.unwrap_or(ir::ExprId::NONE)])
        }
        LoopKind::Table => {
            let t = design.detail::<TableLoop>(l.detail);
            let columns = design.targets(t.columns).to_vec();
            let values = design.exprs(t.values);
            let row = &values[values.len().checked_sub(columns.len())?..];
            (columns, row.to_vec())
        }
        LoopKind::MonteCarlo => return None,
    };
    let mut out = Vec::new();
    for (t, last) in targets.into_iter().zip(last) {
        if t.kind != TargetKind::Param || !t.a.is_resolved() {
            return None;
        }
        let def = design.def(t.a.def);
        if def.item.kind != ir::ItemKind::Param {
            return None;
        }
        let changed = order[..k].iter().any(|&(i, _)| changes(design, i, t.a.def));
        let enclosing = order[..k].iter().any(|&(i, s)| {
            i.kind == ir::ItemKind::Loop
                && inside(design, order[k].1, design.row::<Loop>(i.index).body)
                && s != order[k].1
                && changes(design, i, t.a.def)
        });
        if changed || enclosing {
            return None;
        }
        let value = match from {
            LoopEnd::Restore => design.row::<ir::item::Param>(def.item.index).value,
            LoopEnd::KeepLast => last,
        };
        if value.is_none() || holds_draw(design, value) {
            return None;
        }
        out.push((t, value));
    }
    Some(out)
}

/// Whether an alter or loop sets the parameter `def`.
fn changes(design: &ir::Design, item: ir::ItemRef, def: ir::DefId) -> bool {
    let sets = |t: &Target| t.kind == TargetKind::Param && t.a.def == def;
    match item.kind {
        ir::ItemKind::Alter => sets(&design.row::<ir::plan::Alter>(item.index).target),
        ir::ItemKind::Loop => {
            let l = design.row::<Loop>(item.index);
            match l.kind {
                LoopKind::Sweep => sets(&design.detail::<SweepLoop>(l.detail).target),
                LoopKind::Table => {
                    let t = design.detail::<TableLoop>(l.detail);
                    design.targets(t.columns).iter().any(sets)
                }
                LoopKind::MonteCarlo => false,
            }
        }
        _ => false,
    }
}

/// Whether `scope` is `body` or lies inside it.
fn inside(design: &ir::Design, scope: ir::ScopeId, body: ir::ScopeId) -> bool {
    let mut at = scope;
    // bounded by the scope tree's depth
    while !at.is_none() {
        if at == body {
            return true;
        }
        at = design.scope(at).parent;
    }
    false
}

/// Whether the tree under `root` holds a draw.
fn holds_draw(design: &ir::Design, root: ir::ExprId) -> bool {
    let mut stack = vec![root];
    // bounded: each pop reads one row of the tree (a shared row once per path)
    while let Some(e) = stack.pop() {
        if e.is_none() {
            continue;
        }
        match design.expr(e) {
            ir::Expr::Draw(..) => return true,
            ir::Expr::Call(_, l) | ir::Expr::UserCall(_, l) | ir::Expr::List(l) => {
                stack.extend_from_slice(design.exprs(l))
            }
            ir::Expr::Unary(_, a) => stack.push(a),
            ir::Expr::Binary(_, a, c) => stack.extend([a, c]),
            ir::Expr::Ternary(c, a, d) => stack.extend([c, a, d]),
            _ => {}
        }
    }
    false
}
