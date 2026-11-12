//! The state in force at each analysis, by the IR's docs (`Applies`, `Restorable`, `Save`,
//! `state_order.rs`), written apart from any pass: the model the run passes are checked
//! against. Listed order: per deck file its items depth first, each body right after its item,
//! an `Include` read as the file it names in place; a file no deck includes walked alone.
//! A `WholeRun` item holds for every analysis of its walk, the last of one thing winning; a
//! `FromHere` item holds for the analyses after it until a later one of the same thing or a
//! `Reset` of its class; where both set one thing, the `FromHere` one in force wins (the model's
//! choice: the passes never convert such an item, so either choice compares the same).
//! Saves add up. Loops are not unrolled: an analysis in a loop body is one execution.
//!
//! Contents: `pub enum Thing`, `pub type State`, `pub fn states(&ir::Design) -> Vec<(ir::ItemRef, State)>`

use std::collections::BTreeMap;

use spicerack::ir;

use crate::walk::walks;

/// One thing a state item sets.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Thing {
    /// An option: key index, private name, dialect.
    Option(String, u32, u32),
    /// The temperature.
    Temp,
    /// The nominal temperature.
    Tnom,
    /// A node condition: kind, scope, nodes, reference.
    Node(String, Vec<u32>, Vec<u32>),
}

/// The state at one analysis: each thing's value root, and the saves in force (sorted).
pub type State = (BTreeMap<Thing, u32>, Vec<u32>);

/// Every analysis of every walk, in listed order, with the state in force there.
pub fn states(design: &ir::Design) -> Vec<(ir::ItemRef, State)> {
    let mut out = Vec::new();
    for order in walks(design) {
        let mut whole: BTreeMap<Thing, u32> = BTreeMap::new();
        let mut whole_saves = Vec::new();
        for &item in &order {
            match holds(design, item) {
                Some(ir::Applies::WholeRun) if item.kind == ir::ItemKind::Save => {
                    whole_saves.push(item.index)
                }
                Some(ir::Applies::WholeRun) => {
                    for (t, v) in things(design, item) {
                        whole.insert(t, v);
                    }
                }
                _ => {}
            }
        }
        let mut here: BTreeMap<Thing, u32> = BTreeMap::new();
        let mut here_saves: Vec<u32> = Vec::new();
        for &item in &order {
            match (item.kind, holds(design, item)) {
                (ir::ItemKind::Save, Some(ir::Applies::FromHere)) => here_saves.push(item.index),
                (_, Some(ir::Applies::FromHere)) => {
                    for (t, v) in things(design, item) {
                        here.insert(t, v);
                    }
                }
                (ir::ItemKind::Reset, _) => {
                    let r = design.row::<ir::Reset>(item.index).restores;
                    here.retain(|t, _| !restored(t, r));
                    if r.contains(ir::Restorable::Saves) {
                        here_saves.clear();
                    }
                }
                (ir::ItemKind::Analysis, _) => {
                    let mut state = whole.clone();
                    state.extend(here.iter().map(|(t, v)| (t.clone(), *v)));
                    let mut saves: Vec<u32> =
                        whole_saves.iter().chain(&here_saves).copied().collect();
                    saves.sort_unstable();
                    out.push((item, (state, saves)));
                }
                _ => {}
            }
        }
    }
    out
}

// ---- private ----

fn restored(t: &Thing, r: ir::ResetSet) -> bool {
    match t {
        Thing::Option(..) => r.contains(ir::Restorable::Options),
        Thing::Temp => r.contains(ir::Restorable::Temp),
        Thing::Tnom => r.contains(ir::Restorable::Tnom),
        Thing::Node(..) => false,
    }
}

fn holds(design: &ir::Design, item: ir::ItemRef) -> Option<ir::Applies> {
    match item.kind {
        ir::ItemKind::Option => Some(design.row::<ir::OptionRow>(item.index).holds),
        ir::ItemKind::Env => Some(design.row::<ir::Env>(item.index).holds),
        ir::ItemKind::NodeSet => Some(design.row::<ir::NodeSet>(item.index).holds),
        ir::ItemKind::Save => Some(design.row::<ir::Save>(item.index).holds),
        _ => None,
    }
}

fn things(design: &ir::Design, item: ir::ItemRef) -> Vec<(Thing, u32)> {
    match item.kind {
        ir::ItemKind::Option => {
            let o = design.row::<ir::OptionRow>(item.index);
            vec![(
                Thing::Option(format!("{:?}", o.key), o.private.0, o.dialect.0),
                o.value.0,
            )]
        }
        ir::ItemKind::Env => {
            let e = design.row::<ir::Env>(item.index);
            let mut v = Vec::new();
            if !e.temp.is_none() {
                v.push((Thing::Temp, e.temp.0));
            }
            if !e.tnom.is_none() {
                v.push((Thing::Tnom, e.tnom.0));
            }
            v
        }
        ir::ItemKind::NodeSet => {
            let n = design.row::<ir::NodeSet>(item.index);
            let ids = |l| design.names(l).iter().map(|x: &ir::NameId| x.0).collect();
            let key = format!("{:?} {:?} {}", n.kind, n.applies, n.next_only);
            vec![(Thing::Node(key, ids(n.node), ids(n.reference)), n.value.0)]
        }
        _ => Vec::new(),
    }
}
