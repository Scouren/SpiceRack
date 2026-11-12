//! A design's files, scopes and item lists in, `check`'s rows about them out: handle ranges,
//! the scope tree's shape (each scope reaches its file's top), each scope's owner, and each item
//! listed once; the tree's rules hold for the live scopes (A28: a body whose owner a pass took
//! out of every list is dead, not wrong). Split from `check.rs` at its size limit.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, C5; `rewrite/AMEND-A12.md` A28.
//!
//! Contents: crate: `pub(crate) fn files_and_scopes(&mut Out<'_>)`,
//! `pub(crate) fn live_scopes(&Design) -> Vec<bool>`, `pub(crate) fn stale_definitions(&mut Out<'_>)`

use std::collections::HashSet;

use crate::check::{Out, Violation, table_len};
use crate::design::Design;
use crate::facts::{Handle, RowFacts};
use crate::id::{ItemRef, ScopeId};
use crate::item::{ItemKind, ScopeKind};

/// Files, scopes and item lists: handle ranges, the tree's shape, each scope's owner.
pub(crate) fn files_and_scopes(out: &mut Out<'_>) {
    let d = out.design;
    let n_scopes = d.scopes.len();
    for (k, f) in d.files.iter().enumerate() {
        if f.path.index_checked(d.names.len()).is_none() {
            out.report(Violation::HandleRange, ItemRef::NONE, f.path.0);
        }
        let top_ok = f.top.0 < n_scopes as u32
            && d.scopes[f.top.index()].kind == ScopeKind::File
            && d.scopes[f.top.index()].file.0 as usize == k;
        if !top_ok {
            out.report(Violation::ScopeTree, ItemRef::NONE, f.top.0);
        }
    }
    let mut listed: HashSet<ItemRef> = HashSet::new();
    // the scope that first lists each item, for the owner test (one pass, not a search per scope)
    let mut listing: std::collections::HashMap<ItemRef, ScopeId> = std::collections::HashMap::new();
    for (k, items) in d.scope_items.iter().enumerate() {
        for &item in items {
            listing.entry(item).or_insert(ScopeId(k as u32));
        }
    }
    // A28: the tree's own rules hold for the scopes a file reaches through listed items; a
    // body whose owner a pass took out of every list is dead, not wrong
    let live = live_scopes(d);
    for (k, s) in d.scopes.iter().enumerate() {
        let id = k as u32;
        let parent_ok = s.parent.is_none() || s.parent.index() < n_scopes;
        let owner_ok =
            s.owner == ItemRef::NONE || (s.owner.index as usize) < table_len(d, s.owner.kind);
        if !parent_ok || (s.file.0 as usize) >= d.files.len() || !owner_ok {
            out.report(Violation::HandleRange, ItemRef::NONE, id);
            continue;
        }
        if !live[k] {
            continue;
        }
        if !reaches_a_file(d, ScopeId(id)) {
            out.report(Violation::ScopeTree, ItemRef::NONE, id);
            continue;
        }
        for &item in &d.scope_items[k] {
            if (item.index as usize) >= table_len(d, item.kind) {
                out.report(Violation::HandleRange, ItemRef::NONE, item.index);
            } else if !listed.insert(item) {
                out.report(Violation::ScopeTree, item, id);
            }
        }
        if s.kind != ScopeKind::File && !owner_matches(d, ScopeId(id), &listing) {
            out.report(Violation::ScopeOwner, ItemRef::NONE, id);
        }
    }
}

/// Which scopes are live: a file's top, and the body of an item listed in a live scope. Explicit
/// worklist bounded by the scope count; out-of-range handles are skipped (they are reported).
pub(crate) fn live_scopes(d: &Design) -> Vec<bool> {
    let n = d.scopes.len();
    let mut bodies: std::collections::HashMap<ItemRef, Vec<usize>> =
        std::collections::HashMap::new();
    for (k, s) in d.scopes.iter().enumerate() {
        if s.owner != ItemRef::NONE {
            bodies.entry(s.owner).or_default().push(k);
        }
    }
    let mut live = vec![false; n];
    let mut stack: Vec<usize> = d
        .files
        .iter()
        .map(|f| f.top.index())
        .filter(|&t| t < n)
        .collect();
    while let Some(k) = stack.pop() {
        if std::mem::replace(&mut live[k], true) {
            continue;
        }
        for item in d.scope_items.get(k).into_iter().flatten() {
            stack.extend(
                bodies
                    .get(item)
                    .into_iter()
                    .flatten()
                    .filter(|&&b| !live[b]),
            );
        }
    }
    live
}

/// `StaleDefinition` for each index entry in a live scope whose definition's item no live scope
/// lists (round 7b: a pass that bypassed `Editor::remove_item` or `move_item`), in definition
/// order.
pub(crate) fn stale_definitions(out: &mut Out<'_>) {
    let d = out.design;
    let live = live_scopes(d);
    let listed: HashSet<ItemRef> = d
        .scope_items
        .iter()
        .enumerate()
        .filter(|(k, _)| live[*k])
        .flat_map(|(_, items)| items.iter().copied())
        .collect();
    let mut stale: Vec<u32> = d
        .def_index
        .iter()
        .filter(|((scope, ..), def)| {
            scope.index() < live.len()
                && live[scope.index()]
                && def.index() < d.defs.len()
                && !listed.contains(&d.defs[def.index()].item)
        })
        .map(|(_, def)| def.0)
        .collect();
    stale.sort_unstable();
    for k in stale {
        out.report(Violation::StaleDefinition, ItemRef::NONE, k);
    }
}

// ---- private ----

/// Whether the parent chain of a valid-ranged scope ends at a file scope with no parent, owner
/// `NONE` and its file's top, without a cycle.
fn reaches_a_file(d: &Design, scope: ScopeId) -> bool {
    let mut at = scope;
    for _ in 0..=d.scopes.len() {
        let s = d.scopes[at.index()];
        if s.kind == ScopeKind::File {
            let file = d.files.get(s.file.index());
            return s.parent.is_none()
                && s.owner == ItemRef::NONE
                && file.is_some_and(|f| f.top == at);
        }
        if s.parent.is_none() || s.parent.index() >= d.scopes.len() {
            return false;
        }
        at = s.parent;
    }
    false
}

/// Whether a non-file scope's owner has the scope's kind, names it as a body, and is listed in
/// the scope's parent.
fn owner_matches(
    d: &Design,
    scope: ScopeId,
    listing: &std::collections::HashMap<ItemRef, ScopeId>,
) -> bool {
    let s = d.scopes[scope.index()];
    let kind_ok = match s.kind {
        ScopeKind::Subckt => s.owner.kind == ItemKind::Subckt,
        ScopeKind::Section => s.owner.kind == ItemKind::Section,
        ScopeKind::Branch => s.owner.kind == ItemKind::If,
        ScopeKind::Loop => s.owner.kind == ItemKind::Loop,
        ScopeKind::File => false,
    };
    if !kind_ok || listing.get(&s.owner) != Some(&s.parent) {
        return false;
    }
    let mut handles = Vec::new();
    for_kind!(s.owner.kind, R => d.row::<R>(s.owner.index).handles(d, &mut handles));
    handles.contains(&Handle::Scope(scope))
}
