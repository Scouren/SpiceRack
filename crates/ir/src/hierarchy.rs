//! The subcircuit instance graph: a design in, the instances that close a cycle of subcircuits
//! out (`check`'s `InstanceCycle`), and the generator's question whether a new instance would
//! close one. A node is a `Subckt` row; an edge runs from the subcircuit whose body (at any depth)
//! lists a bound instance to the subcircuit that instance is bound to. An instance outside every
//! subcircuit, or unbound, adds no edge.
//!
//! Every cycle is refused, a used or an unused one: a subcircuit that contains itself has no
//! finite elaboration. R58's evidence, two runs (`tests/fixtures/hierarchy/`, printed in `ngspice.out`, `vacask.out`): ngspice-45 stops
//! with "unknown subckt: x0.x1.x1…x1 n s" after 22 levels on a self instance and on a two-cycle;
//! VACASK refuses both with "Hierarchical recursion detected." at the instance closing the cycle.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` (instances), the lead's finding on `dialect_laws` l5 seed 19.
//!
//! Contents (crate-private): `pub(crate) fn cycle_instances(&Design) -> Vec<ItemRef>`,
//! `pub(crate) fn closes_cycle(&Design, ScopeId, DefId) -> bool`

use crate::check::table_len;
use crate::design::Design;
use crate::element::{ElemKind, Element};
use crate::id::{DefId, ItemRef, ScopeId};
use crate::item::{DefKind, ItemKind};

/// The bound instances whose edge lies on a cycle, in reach order. Assumes the design's handles
/// hold (`check` calls it once they do). O(items + scopes · depth).
pub(crate) fn cycle_instances(design: &Design) -> Vec<ItemRef> {
    let n = table_len(design, ItemKind::Subckt);
    let edges = edges(design);
    let mut adj = vec![Vec::new(); n];
    for &(_, from, to) in &edges {
        adj[from as usize].push(to);
    }
    let comp = components(&adj);
    let out: Vec<ItemRef> = edges
        .iter()
        .filter(|&&(_, from, to)| comp[from as usize] == comp[to as usize])
        .map(|&(at, _, _)| at)
        .collect();
    assert!(out.len() <= edges.len(), "one report per edge at most");
    out
}

/// Whether an instance listed in `scope` and bound to `master` would close a cycle: `master` is
/// the subcircuit enclosing `scope`, or reaches it. False outside every subcircuit and for a
/// master that is not a subcircuit. O(items + scopes · depth): the generator's, once per drawn
/// instance.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn closes_cycle(design: &Design, scope: ScopeId, master: DefId) -> bool {
    let (Some(from), Some(to)) = (enclosing(design, scope), subckt_of(design, master)) else {
        return false;
    };
    let mut adj = vec![Vec::new(); table_len(design, ItemKind::Subckt)];
    for (_, a, b) in edges(design) {
        adj[a as usize].push(b);
    }
    // a worklist from `to`: each subcircuit entered once
    let mut seen = vec![false; adj.len()];
    let mut work = vec![to];
    while let Some(s) = work.pop() {
        if s == from {
            return true;
        }
        if !std::mem::replace(&mut seen[s as usize], true) {
            work.extend(&adj[s as usize]);
        }
    }
    assert!(seen[to as usize], "the start was entered");
    false
}

// ---- private ----

/// Every bound instance inside a subcircuit: (the instance, its subcircuit, its master's).
fn edges(design: &Design) -> Vec<(ItemRef, u32, u32)> {
    let mut out = Vec::new();
    for (item, scope) in design.reachable() {
        if item.kind != ItemKind::Element {
            continue;
        }
        let e: Element = design.row(item.index);
        if e.kind != ElemKind::Instance {
            continue;
        }
        if let (Some(from), Some(to)) = (enclosing(design, scope), subckt_of(design, e.model.def)) {
            out.push((item, from, to));
        }
    }
    out
}

/// The `Subckt` row whose body holds `scope`, at any depth.
fn enclosing(design: &Design, scope: ScopeId) -> Option<u32> {
    let mut at = scope;
    // bounded by the scope count: each step moves to a parent
    for _ in 0..=design.scopes.len() {
        if at.is_none() || at.index() >= design.scopes.len() {
            return None;
        }
        let s = &design.scopes[at.index()];
        if s.owner.kind == ItemKind::Subckt && s.owner != ItemRef::NONE {
            return Some(s.owner.index);
        }
        at = s.parent;
    }
    None
}

/// The `Subckt` row `def` is bound to, if it is a subcircuit's definition.
fn subckt_of(design: &Design, def: DefId) -> Option<u32> {
    if def.is_none() || def.index() >= design.definitions().len() {
        return None;
    }
    let d = design.def(def);
    (d.kind == DefKind::Subckt && d.item.kind == ItemKind::Subckt).then_some(d.item.index)
}

/// The strongly connected component of each node (Tarjan, explicit stack: C11).
fn components(adj: &[Vec<u32>]) -> Vec<u32> {
    const UNSEEN: u32 = u32::MAX;
    let n = adj.len();
    let (mut index, mut low, mut comp) = (vec![UNSEEN; n], vec![0u32; n], vec![UNSEEN; n]);
    let mut on = vec![false; n];
    let (mut stack, mut call): (Vec<usize>, Vec<(usize, usize)>) = (Vec::new(), Vec::new());
    let (mut next, mut count) = (0u32, 0u32);
    for root in 0..n {
        if index[root] != UNSEEN {
            continue;
        }
        macro_rules! enter {
            ($v:expr) => {{
                let v = $v;
                index[v] = next;
                low[v] = next;
                next += 1;
                stack.push(v);
                on[v] = true;
                call.push((v, 0));
            }};
        }
        enter!(root);
        // each node entered once and each edge followed once
        while let Some(&(v, k)) = call.last() {
            if let Some(&w) = adj[v].get(k) {
                call.last_mut().expect("a frame").1 += 1;
                let w = w as usize;
                if index[w] == UNSEEN {
                    enter!(w);
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
                continue;
            }
            call.pop();
            if let Some(&(p, _)) = call.last() {
                low[p] = low[p].min(low[v]);
            }
            if low[v] == index[v] {
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    comp[w] = count;
                    if w == v {
                        break;
                    }
                }
                count += 1;
            }
        }
    }
    assert!(
        comp.iter().all(|&c| c != UNSEEN),
        "every node has a component"
    );
    comp
}
