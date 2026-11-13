//! Rule `alter_param_at_once_to_at_reset`: a parameter alter that takes effect where it stands
//! (`AlterEffect::AtOnce`, VACASK's) stated as one that takes effect at the next reset
//! (`AtReset`, ngspice's `alterparam`) followed at once by a `Reset` of every class (ngspice's
//! `reset`, the only one it spells). It removes `Run(AlterParamAtOnce)` and requires
//! `Run(AlterParamAtReset)`, the reset and its five class members (R70, finding F7).
//!
//! Exact where the reset undoes nothing else in force: no alter of a source, element, device or
//! model value, no option, temperature or save that holds from its place, and no loop that
//! leaves such a target at its last value, stands in the walk before the alter since the last
//! reset of every class (a parameter alter is never restored: `Reset`'s doc). The evidence:
//! ngspice-45 `alterparam a=2` with no `reset` leaves v(n) at 1, with one gives 2
//! (`AlterEffect`'s doc, `step2-ngspice` d8221c3e); VACASK's alter gives the new value at the
//! next operating point (`alter_toplevel.sim`, `pair-test` b213ada2). An alter inside a loop's
//! body or a branch is left too: a reset there undoes every earlier pass's items.
//!
//! Left items carry `ALTER_KEPT`.
//!
//! Cites: R70, AMEND-A12 A43; `crates/ir/src/{plan,reset}.rs`.
//!
//! Contents: `pub fn alter_param_at_once_to_at_reset(&mut ir::Design, &mut ir::Diags)`,
//! `pub const ALTER_KEPT: ir::Code`

use crate::ir;
use ir::plan::{Alter, AlterEffect, Loop, LoopEnd, LoopKind, SweepLoop, TableLoop, Target};

/// The note at a parameter alter whose reset would undo something else in force.
pub const ALTER_KEPT: ir::Code = ir::Code::new(b'U', 424);

/// Each at-once parameter alter made at-reset with a reset after it, where exact; noted else.
pub fn alter_param_at_once_to_at_reset(design: &mut ir::Design, diags: &mut ir::Diags) {
    let mut work: Vec<(ir::ItemRef, ir::ScopeId)> = Vec::new();
    for (top, order) in ir::run_walks(design) {
        for (k, &(item, scope)) in order.iter().enumerate() {
            if item.kind != ir::ItemKind::Alter {
                continue;
            }
            let a = design.row::<Alter>(item.index);
            if a.effect != AlterEffect::AtOnce || !a.target.kind.is_parameter() {
                continue;
            }
            if scope == top && !restorable_in_force(design, &order[..k]) {
                work.push((item, scope));
            } else {
                diags.push_at(ALTER_KEPT, design, item, 0);
            }
        }
    }
    for (item, scope) in work {
        let mut a = design.row::<Alter>(item.index);
        a.effect = AlterEffect::AtReset;
        let mut editor = design.editor();
        editor.set(item.index, a);
        let reset = editor
            .builder()
            .reset(scope, ir::ResetSet::ALL, ir::Span::NONE);
        let items = editor.items_mut(scope);
        assert!(items.last() == Some(&reset), "a new reset is pushed last");
        items.pop();
        let at = items.iter().position(|&i| i == item).expect("listed") + 1;
        items.insert(at, reset);
    }
}

// ---- private ----

/// Whether something a reset of every class undoes is in force after `before` (a walk's items
/// before the alter): counted from the last reset of every class.
fn restorable_in_force(design: &ir::Design, before: &[(ir::ItemRef, ir::ScopeId)]) -> bool {
    let from = before
        .iter()
        .rposition(|&(i, _)| {
            i.kind == ir::ItemKind::Reset
                && design.row::<ir::Reset>(i.index).restores == ir::ResetSet::ALL
        })
        .map_or(0, |p| p + 1);
    before[from..].iter().any(|&(i, _)| match i.kind {
        ir::ItemKind::Alter => !design.row::<Alter>(i.index).target.kind.is_parameter(),
        ir::ItemKind::Option => design.row::<ir::OptionRow>(i.index).holds == ir::Applies::FromHere,
        ir::ItemKind::Env => design.row::<ir::Env>(i.index).holds == ir::Applies::FromHere,
        ir::ItemKind::Save => design.row::<ir::Save>(i.index).holds == ir::Applies::FromHere,
        ir::ItemKind::Loop => {
            let l = design.row::<Loop>(i.index);
            l.after == LoopEnd::KeepLast
                && targets(design, l).iter().any(|t| !t.kind.is_parameter())
        }
        _ => false,
    })
}

/// A loop's targets (none for a Monte Carlo loop: it alters nothing by name).
fn targets(design: &ir::Design, l: Loop) -> Vec<Target> {
    match l.kind {
        LoopKind::Sweep => vec![design.detail::<SweepLoop>(l.detail).target],
        LoopKind::Table => design
            .targets(design.detail::<TableLoop>(l.detail).columns)
            .to_vec(),
        LoopKind::MonteCarlo => Vec::new(),
    }
}
