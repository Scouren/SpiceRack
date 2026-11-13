//! A design in, the items that bring a second ASCII-case spelling of a name into one namespace
//! out (`Extra::CaseSensitiveNames`): a case-folding target would merge the two. A namespace is
//! what a simulator's own lookup separates: the definitions of one kind in a scope; the elements
//! of a scope; the nets of a scope (element terminals, a subcircuit's ports in its body, and the
//! global nets, visible in every scope); the keys of one parameter list (a subcircuit's formals
//! included). Live items only: a name the pool holds and no listed item uses clashes with none.
//!
//! Cites: `rewrite/AMEND-A12.md` A28 (the lead's namespace ruling); `rewrite/IR-KERNEL.md` §8.
//!
//! Contents: crate: `pub(crate) fn case_clashes(&Design) -> HashSet<ItemRef>`

use std::collections::{HashMap, HashSet};

use crate::design::Design;
use crate::element::Element;
use crate::facts::Handle;
use crate::id::{ItemRef, NameId, ScopeId};
use crate::item::{Global, ItemKind, Subckt};

/// A namespace's kind (with the scope it is in, a key of `Spaces`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Space {
    /// The definitions of one `DefKind` (its index).
    Def(u8),
    /// The elements of a scope.
    Element,
    /// The nets of a scope.
    Net,
}

/// Each namespace's first spelling of each case-folded name.
type Spaces = HashMap<(Space, ScopeId), HashMap<String, NameId>>;

/// Every live item that brings a name's second case spelling into one of its namespaces, the
/// first spelling's item not included (the clash is reported at the second). O(names used).
pub(crate) fn case_clashes(design: &Design) -> HashSet<ItemRef> {
    let mut clashes = HashSet::new();
    let items = design.reachable();
    let live: HashSet<ItemRef> = items.iter().map(|&(i, _)| i).collect();
    // the nets every scope sees: the global ones, first spelling of each
    let mut globals: HashMap<String, NameId> = HashMap::new();
    for &(item, _) in &items {
        if item.kind == ItemKind::Global {
            let row: Global = design.row(item.index);
            for &n in design.names(row.nets) {
                if spelled_again(&mut globals, design, n) {
                    clashes.insert(item);
                }
            }
        }
    }
    let mut seen: Spaces = HashMap::new();
    // definitions of one kind in a scope
    for def in design.definitions_all() {
        if live.contains(&def.item) {
            let space = (Space::Def(def.kind as u8), def.scope);
            if spelled_again(seen.entry(space).or_default(), design, def.name) {
                clashes.insert(def.item);
            }
        }
    }
    let mut handles = Vec::new();
    for &(item, scope) in &items {
        let mut add = |space: Space, at: ScopeId, n: NameId, clashes: &mut HashSet<ItemRef>| {
            let local = spelled_again(seen.entry((space, at)).or_default(), design, n);
            let global = space == Space::Net && globals_clash(&globals, design, n);
            if local || global {
                clashes.insert(item);
            }
        };
        match item.kind {
            ItemKind::Element => {
                let row: Element = design.row(item.index);
                add(Space::Element, scope, row.name, &mut clashes);
                for &n in design.names(row.terminals) {
                    add(Space::Net, scope, n, &mut clashes);
                }
            }
            ItemKind::Subckt => {
                let row: Subckt = design.row(item.index);
                for &n in design.names(row.ports) {
                    add(Space::Net, row.body, n, &mut clashes);
                }
            }
            _ => {} // else: the other kinds name no element and no net here
        }
        // the keys of each parameter list the item holds
        handles.clear();
        design.item_handles(item, &mut handles);
        for h in &handles {
            if let Handle::Params(l) = *h {
                let mut keys: HashMap<String, NameId> = HashMap::new();
                for &(k, _) in design.params(l) {
                    if spelled_again(&mut keys, design, k) {
                        clashes.insert(item);
                    }
                }
            }
        }
    }
    clashes
}

// ---- private ----

/// Records `n` in `space`, and whether another spelling of it is there already.
fn spelled_again(space: &mut HashMap<String, NameId>, design: &Design, n: NameId) -> bool {
    let first = *space.entry(design.str(n).to_ascii_lowercase()).or_insert(n);
    first != n
}

/// Whether a net `n` is another spelling of a global net.
fn globals_clash(globals: &HashMap<String, NameId>, design: &Design, n: NameId) -> bool {
    globals
        .get(&design.str(n).to_ascii_lowercase())
        .is_some_and(|&g| g != n)
}
