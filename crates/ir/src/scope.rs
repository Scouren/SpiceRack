//! The design's structure rows: files, scopes and definitions, and the name lookup over the
//! scope tree. Files and scopes come from `Builder::file` and from the push of an item that opens
//! a scope; definitions from the push of a defining item or `Builder::define`.
//!
//! Lookup is neutral and lexical: a scope, then its parent, up to the file's top scope. Anything
//! a dialect adds (include scope, section visibility, kit-wide search) is its reader's rule,
//! applied when it resolves (`Builder::resolve`); `link` applies the neutral rule.
//!
//! Cites: `rewrite/IR-KERNEL.md` §4 (`files`, `scopes`, `defs`); `REWRITE-PLAN.md` §3.2;
//! `rewrite/INV-constructs.md` A2, A28, B21.
//!
//! Contents: `pub struct File`, `pub struct Scope`, `pub struct Def`,
//! `Design::lookup(&self, ScopeId, DefKind, NameId) -> DefId`,
//! `Design::find_file(&self, &str) -> FileId`,
//! `Design::reachable(&self) -> Vec<(ItemRef, ScopeId)>`; crate:
//! `Design::bodies(&self) -> HashMap<ItemRef, Vec<ScopeId>>`

use std::collections::HashMap;

use crate::design::Design;
use crate::id::{DefId, FileId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, Role, ScopeKind};

/// A file of the design.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct File {
    /// The path, unique in the design (kit-relative as the reader gave it).
    pub path: NameId,
    /// What the file is.
    pub role: Role,
    /// Its top scope ([`ScopeKind::File`]).
    pub top: ScopeId,
}

/// A scope: an ordered item list (kept beside the table, `Design::items`) and its place in the
/// tree. Invariant (checked): `kind` matches `owner.kind`, the owner's row names this scope as
/// its body, `parent` is the scope holding `owner`, and every scope reaches a `File` scope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scope {
    /// What the scope is.
    pub kind: ScopeKind,
    /// The enclosing scope; `NONE` for a file's top scope.
    pub parent: ScopeId,
    /// The item that opens it; `ItemRef::NONE` for a file's top scope.
    pub owner: ItemRef,
    /// The file it is in.
    pub file: FileId,
}

/// A definition: something a [`crate::Ref`] can bind to. At most one per (scope, kind, name)
/// (checked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Def {
    /// What it defines.
    pub kind: DefKind,
    /// The defined name.
    pub name: NameId,
    /// The scope it is visible from (and from that scope's descendants).
    pub scope: ScopeId,
    /// The item that defines it (see [`DefKind`] for which kind per definition kind).
    pub item: ItemRef,
}

const _: () = assert!(size_of::<File>() == 12);
const _: () = assert!(size_of::<Scope>() == 16);
const _: () = assert!(size_of::<Def>() == 20);

impl Design {
    /// The definition of `kind` named `name` visible from `scope`: the innermost along the
    /// parent chain, or `NONE`. O(scope depth) hash lookups.
    pub fn lookup(&self, scope: ScopeId, kind: DefKind, name: NameId) -> DefId {
        assert!(scope.index() < self.scopes.len(), "a scope id in range");
        let mut at = scope;
        // bounded by the scope count: each step moves to a parent
        for _ in 0..=self.scopes.len() {
            if at.is_none() {
                return DefId::NONE;
            }
            if let Some(&def) = self.def_index.get(&(at, kind, name)) {
                return def;
            }
            at = self.scopes[at.index()].parent;
        }
        unreachable!("the scope tree is acyclic")
    }

    /// The file whose path is `path`, or `NONE`. O(files), at most [`crate::id::FILES_MAX`].
    pub fn find_file(&self, path: &str) -> FileId {
        let name = self.names.get(path);
        if name.is_none() {
            return FileId::NONE;
        }
        let at = self.files.iter().position(|f| f.path == name);
        at.map_or(FileId::NONE, |i| FileId(i as u16))
    }

    /// Every item reachable from a file's top scope, with the scope that lists it: files in id
    /// order, each scope's items in order, an item's body scopes right after the item (depth
    /// first: the order of `text::dump`). The order is part of the contract: callers attribute
    /// an item to its file by its listing scope (`design.scope(scope).file`, R10), never by its
    /// span. Total over any table contents: each scope is entered once, out-of-range scopes are
    /// skipped. Explicit stack (C11). O(items + scopes).
    pub fn reachable(&self) -> Vec<(ItemRef, ScopeId)> {
        let bodies = self.bodies();
        let mut entered = vec![false; self.scopes.len()];
        let mut out = Vec::new();
        let mut stack: Vec<(ScopeId, usize)> = Vec::new();
        for file in &self.files {
            if file.top.index() >= self.scopes.len() {
                continue;
            }
            entered[file.top.index()] = true;
            stack.push((file.top, 0));
            while let Some((scope, k)) = stack.pop() {
                let Some(&item) = self.scope_items[scope.index()].get(k) else {
                    continue;
                };
                stack.push((scope, k + 1));
                out.push((item, scope));
                for &body in bodies
                    .get(&item)
                    .map_or(&[][..], |v| v.as_slice())
                    .iter()
                    .rev()
                {
                    if !std::mem::replace(&mut entered[body.index()], true) {
                        stack.push((body, 0));
                    }
                }
            }
        }
        out
    }

    /// Each item's body scopes in creation order (branch order for an `If`), read from the scope
    /// table's owners, so no row has to be decoded. Lookup only, never iterated for output.
    pub(crate) fn bodies(&self) -> HashMap<ItemRef, Vec<ScopeId>> {
        let mut bodies: HashMap<ItemRef, Vec<ScopeId>> = HashMap::new();
        for (k, s) in self.scopes.iter().enumerate() {
            if s.kind != ScopeKind::File {
                bodies.entry(s.owner).or_default().push(ScopeId(k as u32));
            }
        }
        bodies
    }
}
