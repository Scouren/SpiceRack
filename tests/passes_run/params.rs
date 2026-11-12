//! The value each swept or altered parameter holds at each analysis, by the IR's docs (`Alter`:
//! everything after it runs on the changed circuit; `LoopEnd`: after the loop its targets hold
//! the value before it, or the last swept value), written apart from any pass. A value is its
//! number under the reference evaluator (parameters read by their definitions), or a token
//! when no expression states it (a range sweep's last value, a Monte Carlo loop's, a draw);
//! inside a loop body the swept parameter is the loop's own token.
//!
//! Contents: `pub type Values`, `pub fn values(&ir::Design) -> Vec<(ir::ItemRef, Values)>`

use std::collections::BTreeMap;

use spicerack::ir;
use spicerack::ir::plan::{Alter, Loop, LoopEnd, LoopKind, SweepLoop, TableLoop, TargetKind};

use crate::bindings::ByDefinition;
use crate::walk::walks;

/// Per parameter definition, its value's bits or a token.
pub type Values = BTreeMap<u32, (bool, u64)>;

/// Every analysis of every walk, in listed order, with the values in force there.
pub fn values(design: &ir::Design) -> Vec<(ir::ItemRef, Values)> {
    let env = ByDefinition(design);
    let num = |e: ir::ExprId| -> (bool, u64) {
        match ir::eval(design, e, &env) {
            Ok(v) => (true, v.to_bits()),
            Err(_) => (false, u64::from(e.0)),
        }
    };
    let own = |def: ir::DefId| {
        let item = design.def(def).item;
        match item.kind {
            ir::ItemKind::Param => num(design.row::<ir::item::Param>(item.index).value),
            _ => (false, u64::from(def.0) | 1 << 41), // a formal: its own value is per instance
        }
    };
    let mut out = Vec::new();
    for order in walks(design) {
        let mut now: Values = BTreeMap::new();
        // loops entered and not yet left: (loop, its body, the values before it)
        let mut open: Vec<(ir::ItemRef, ir::ScopeId, Values)> = Vec::new();
        for &item in &order {
            let scope = listing(design, item);
            // leave every open loop whose body does not hold this item
            while let Some((l, body, before)) = open.last().cloned() {
                if within(design, scope, body) {
                    break;
                }
                open.pop();
                let row = design.row::<Loop>(l.index);
                for (def, last) in targets(design, row) {
                    let v = match row.after {
                        LoopEnd::Restore => before.get(&def.0).copied().unwrap_or_else(|| own(def)),
                        LoopEnd::KeepLast => last.map_or((false, u64::from(l.index)), &num),
                    };
                    now.insert(def.0, v);
                }
            }
            match item.kind {
                ir::ItemKind::Alter => {
                    let a = design.row::<Alter>(item.index);
                    if a.target.kind == TargetKind::Param && a.target.a.is_resolved() {
                        now.insert(a.target.a.def.0, num(a.value));
                    }
                }
                ir::ItemKind::Loop => {
                    let row = design.row::<Loop>(item.index);
                    open.push((item, row.body, now.clone()));
                    for (def, _) in targets(design, row) {
                        now.insert(def.0, (false, u64::from(item.index) | 1 << 40));
                    }
                }
                ir::ItemKind::Analysis => {
                    let mut v = now.clone();
                    // a parameter no alter or loop touched yet holds its own value: omitted
                    v.retain(|&d, x| *x != own(ir::DefId(d)));
                    out.push((item, v));
                }
                _ => {}
            }
        }
    }
    out
}

// ---- private ----

/// A loop's parameter targets with their last value's expression when one states it.
fn targets(design: &ir::Design, l: Loop) -> Vec<(ir::DefId, Option<ir::ExprId>)> {
    let param = |t: &ir::plan::Target| {
        (t.kind == TargetKind::Param
            && t.a.is_resolved()
            && design.def(t.a.def).item.kind == ir::ItemKind::Param)
            .then_some(t.a.def)
    };
    match l.kind {
        LoopKind::Sweep => {
            let s = design.detail::<SweepLoop>(l.detail);
            let last = (s.sweep.kind == ir::SweepKind::List)
                .then(|| design.exprs(s.sweep.values).last().copied())
                .flatten();
            param(&s.target).map(|d| (d, last)).into_iter().collect()
        }
        LoopKind::Table => {
            let t = design.detail::<TableLoop>(l.detail);
            let columns = design.targets(t.columns);
            let values = design.exprs(t.values);
            let row = values.len().saturating_sub(columns.len());
            columns
                .iter()
                .enumerate()
                .filter_map(|(k, c)| param(c).map(|d| (d, values.get(row + k).copied())))
                .collect()
        }
        LoopKind::MonteCarlo => Vec::new(),
    }
}

fn listing(design: &ir::Design, item: ir::ItemRef) -> ir::ScopeId {
    design
        .reachable()
        .into_iter()
        .find(|p| p.0 == item)
        .map_or(ir::ScopeId::NONE, |p| p.1)
}

fn within(design: &ir::Design, scope: ir::ScopeId, body: ir::ScopeId) -> bool {
    let mut at = scope;
    while !at.is_none() {
        if at == body {
            return true;
        }
        at = design.scope(at).parent;
    }
    false
}
