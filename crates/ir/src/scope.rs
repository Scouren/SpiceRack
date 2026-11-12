//! The design's structure rows: files, scopes and definitions, and the name lookup over the
//! scope tree. Files and scopes come from `Builder::file` and from the push of an item that opens
//! a scope; definitions from the push of a defining item or `Builder::define`.
//!
//! Lookup is neutral and lexical: a scope, then its parent, up to the file's top scope. Anything
//! a dialect adds (include scope, section visibility, kit-wide search) is its reader's rule,
//! applied when it resolves (`Builder::resolve`).
//!
//! Cites: `rewrite/IR-KERNEL.md` §4 (`files`, `scopes`, `defs`); `REWRITE-PLAN.md` §3.2;
//! `rewrite/INV-constructs.md` A2, A28, B21.
//!
//! Contents: `pub struct File`, `pub struct Scope`, `pub struct Def`,
//! `Design::lookup(&self, ScopeId, DefKind, NameId) -> DefId`,
//! `Design::find_file(&self, &str) -> FileId`

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
        todo!()
    }

    /// The file whose path is `path`, or `NONE`. O(files), at most [`crate::id::FILES_MAX`].
    pub fn find_file(&self, path: &str) -> FileId {
        todo!()
    }
}
