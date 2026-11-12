//! Pass `run_items_together`: a design in, the same design out with every circuit item a file's
//! top lists between two run items moved, in its order, to just before the first of them. It
//! removes `Run(RunInterleaved)` (VACASK has one `control` block per file, A22) and requires
//! nothing. Exact: where a circuit item stands among run items is no meaning (the circuit is the
//! whole of it, bound by definitions; an alter changes the circuit for the run items after it,
//! wherever the circuit's rows are listed). A comment or opaque stays where it is.
//!
//! Left with a `HOLDS_RUN_ITEMS` note: a circuit item between run items that holds run items in
//! its body or includes a file that does (moving it would move them in listed order).
//!
//! Cites: `rewrite/AMEND-A12.md` A22; `crates/ir/src/state_order.rs` (`interleaved_features`,
//! `listed`: the classes the feature counts by).
//!
//! Contents: `pub fn run_items_together(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const HOLDS_RUN_ITEMS: ir::Code`

use std::collections::HashSet;

use crate::ir;
use crate::ir::{Listed, listed};

/// The note at a circuit item between run items that holds or includes run items.
pub const HOLDS_RUN_ITEMS: ir::Code = ir::Code::new(b'U', 404);

/// Moves each file top's circuit items out from between its run items.
pub fn run_items_together(design: &mut ir::Design, diags: &mut ir::Diags) {
    let holders = holders(design);
    let tops: Vec<ir::ScopeId> = design.files().map(|f| design.file(f).top).collect();
    for top in tops {
        let items = design.items(top);
        let run: Vec<usize> = (0..items.len())
            .filter(|&k| listed(items[k].kind) == Listed::Run)
            .collect();
        let (Some(&first), Some(&last)) = (run.first(), run.last()) else {
            continue;
        };
        let mut moved = Vec::new();
        for &item in &items[first..last] {
            if listed(item.kind) != Listed::Other {
                continue;
            }
            match holders.contains(&item) {
                true => diags.push_at(HOLDS_RUN_ITEMS, design, item, 0),
                false => moved.push(item),
            }
        }
        if moved.is_empty() {
            continue;
        }
        let mut editor = design.editor();
        let items = editor.items_mut(top);
        let before = items.len();
        items.retain(|i| !moved.contains(i));
        let at = first.min(items.len());
        items.splice(at..at, moved);
        assert!(items.len() == before, "a move keeps every item");
    }
}

// ---- private ----

/// The items that hold a run item in a body (any depth) or include a file whose items do.
fn holders(design: &ir::Design) -> HashSet<ir::ItemRef> {
    let reachable = design.reachable();
    let mut out = HashSet::new();
    let mut files_with_run = HashSet::new();
    for &(item, scope) in &reachable {
        if listed(item.kind) != Listed::Run {
            continue;
        }
        files_with_run.insert(design.scope(scope).file);
        let mut at = scope;
        // bounded by the scope tree's depth: each step moves to a parent
        while !at.is_none() {
            let s = design.scope(at);
            if s.owner != ir::ItemRef::NONE {
                out.insert(s.owner);
            }
            at = s.parent;
        }
    }
    for &(item, _) in &reachable {
        if item.kind == ir::ItemKind::Include {
            let path = design.row::<ir::item::Include>(item.index).path;
            let target = design.find_file(design.str(path));
            if !target.is_none() && files_with_run.contains(&target) {
                out.insert(item);
            }
        }
    }
    out
}
