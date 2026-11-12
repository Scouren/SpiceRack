//! Listed order, by the IR's docs (`Applies`, `state_order.rs`): per deck file its items depth
//! first, each body right after its item, an `Include` read as the file it names in place; a
//! file no deck includes walked alone. Written apart from any pass.
//!
//! Contents: `pub fn walks(&ir::Design) -> Vec<Vec<ir::ItemRef>>`

use std::collections::{HashMap, HashSet};

use spicerack::ir;

/// Each walk's items in listed order.
pub fn walks(design: &ir::Design) -> Vec<Vec<ir::ItemRef>> {
    let mut bodies: HashMap<ir::ItemRef, Vec<ir::ScopeId>> = HashMap::new();
    let mut seen = HashSet::new();
    for (_, s) in design.reachable() {
        let owner = design.scope(s).owner;
        if owner != ir::ItemRef::NONE && seen.insert(s) {
            bodies.entry(owner).or_default().push(s);
        }
    }
    let files: Vec<ir::FileId> = design.files().collect();
    let deck = |f: ir::FileId| design.file(f).role == ir::Role::Deck;
    let mut reached = HashSet::new();
    let mut out = Vec::new();
    let starts: Vec<ir::FileId> = files
        .iter()
        .copied()
        .filter(|&f| deck(f))
        .chain(files.iter().copied().filter(|&f| !deck(f)))
        .collect();
    for start in starts {
        if !deck(start) && reached.contains(&start) {
            continue;
        }
        reached.insert(start);
        let mut order = Vec::new();
        let top = design.file(start).top;
        let mut entered = HashSet::from([top]);
        let mut stack = vec![(top, 0usize)];
        while let Some((scope, k)) = stack.pop() {
            let Some(&item) = design.items(scope).get(k) else {
                continue;
            };
            stack.push((scope, k + 1));
            order.push(item);
            let mut next = bodies.get(&item).cloned().unwrap_or_default();
            if item.kind == ir::ItemKind::Include {
                let path = design.row::<ir::item::Include>(item.index).path;
                let f = design.find_file(design.str(path));
                if !f.is_none() {
                    reached.insert(f);
                    next.push(design.file(f).top);
                }
            }
            for s in next.into_iter().rev() {
                if entered.insert(s) {
                    stack.push((s, 0));
                }
            }
        }
        out.push(order);
    }
    out
}
