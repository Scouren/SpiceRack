//! Handles and ranges: the typed `u32` (files: `u16`) indices every row uses instead of a
//! pointer, the reference to a definition, the typed range into a shared pool, and the named
//! capacities every table asserts at `push`.
//!
//! A handle is `#[repr(transparent)]` over its integer, `NONE` is the integer's maximum, and its
//! `Debug` is a letter and the index (`n12`, `e7`, `n-` for `NONE`). Ids are given in append
//! order, so the same appends give the same ids (determinism, L3).
//!
//! Cites: `rewrite/IR-KERNEL.md` C1, C5, C8, C10; `REWRITE-PLAN.md` §3.2 (counts: kits reach
//! 10⁶–10⁷ statements, the capacities leave ten times that).
//!
//! Contents:
//! - `pub struct NameId`, `NameId::GROUND`, `pub struct FileId`, `pub struct ScopeId`,
//!   `pub struct ExprId`, `pub struct DefId`, `pub struct TextId`; on each: `NONE`,
//!   `index(self) -> usize`, `is_none(self) -> bool`
//! - `pub struct ItemRef`, `ItemRef::NONE`, `ItemRef::new(kind, index) -> ItemRef`
//! - `pub struct Ref`, `Ref::unresolved(name) -> Ref`, `Ref::is_resolved(self) -> bool`
//! - `pub struct List<T>`, `List::EMPTY`, `List::len(self) -> usize`,
//!   `List::is_empty(self) -> bool`; `pub type NameList`, `pub type ExprList`,
//!   `pub type ParamList`, `pub type BranchList`
//! - capacities: `pub const FILES_MAX`, `pub const NAMES_MAX`, `pub const BYTES_MAX`,
//!   `pub const EXPRS_MAX`, `pub const ROWS_MAX`, `pub const SCOPES_MAX`, `pub const DEFS_MAX`,
//!   `pub const POOL_MAX`

use std::fmt;
use std::marker::PhantomData;

use crate::item::{Branch, ItemKind};

/// An item: a row in the table of its kind (C3). Existence-based: the kind names the table, the
/// index the row; there is no other tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemRef {
    /// The table.
    pub kind: ItemKind,
    /// The row in that table; `u32::MAX` only in [`ItemRef::NONE`].
    pub index: u32,
}

/// A reference to a definition by name (C8). `def == DefId::NONE` means unresolved, which is
/// legal (the kit that defines it may be absent); a resolved `def` names a definition of the
/// kind the referring field expects, with this `name` (checked by `check`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ref {
    /// The name as the source spelled it (one spelling per name, C6).
    pub name: NameId,
    /// The definition it binds to, or `NONE`.
    pub def: DefId,
}

/// A typed range `[start, start + len)` into one of the design's shared pools (C5). The pool is
/// fixed by `T`: `NameId` → the name pool, `ExprId` → the expression pool, `(NameId, ExprId)` →
/// the parameter pool, [`Branch`] → the branch pool. Lists are written once, whole, by the
/// builder; a list never grows in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct List<T> {
    /// First element's index in the pool.
    pub start: u32,
    /// Element count.
    pub len: u32,
    /// The pool the range indexes.
    pub pool: PhantomData<fn() -> T>,
}

/// An ordered list of names (ports, terminals, global nets, formal arguments).
pub type NameList = List<NameId>;
/// An ordered list of expressions (call arguments, waveform points, coefficient vectors).
pub type ExprList = List<ExprId>;
/// An ordered list of `name = value` pairs (instance and model parameters, subcircuit
/// defaults), in source order; a name appears at most once per list.
pub type ParamList = List<(NameId, ExprId)>;
/// The branches of an `If`, in source order; only the last may be the `else` branch.
pub type BranchList = List<Branch>;

/// Most files one design holds (today's kit cap, `res::FILES_MAX`); a `FileId` is `u16`.
pub const FILES_MAX: usize = 4096;
/// Most distinct names one design interns.
pub const NAMES_MAX: usize = 1 << 26;
/// Most bytes of interned names, and separately of `Opaque`/`Comment`/`Title` text (offsets are
/// `u32`).
pub const BYTES_MAX: usize = 1 << 31;
/// Most expression rows one design holds.
pub const EXPRS_MAX: usize = 1 << 28;
/// Most rows one item table (or one detail table) holds.
pub const ROWS_MAX: usize = 1 << 27;
/// Most scopes one design holds (files, subcircuit bodies, sections, branches, loops).
pub const SCOPES_MAX: usize = 1 << 24;
/// Most definitions one design holds.
pub const DEFS_MAX: usize = 1 << 26;
/// Most elements in each shared pool (names, expressions, parameters, branches).
pub const POOL_MAX: usize = 1 << 28;

const _: () = assert!(size_of::<ItemRef>() == 8);
const _: () = assert!(size_of::<Ref>() == 8);
const _: () = assert!(size_of::<List<NameId>>() == 8);
const _: () = assert!(FILES_MAX < u16::MAX as usize);
const _: () = assert!(BYTES_MAX < u32::MAX as usize);

macro_rules! handle {
    ($(#[$doc:meta])* $name:ident, $int:ty, $letter:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(pub $int);

        impl $name {
            /// The sentinel: no row. What "no row" means is stated on each field that holds one.
            pub const NONE: Self = Self(<$int>::MAX);

            /// The row index in the handle's table. Asserts the handle is not `NONE`.
            pub const fn index(self) -> usize {
                todo!()
            }

            /// Whether this is the `NONE` sentinel.
            pub const fn is_none(self) -> bool {
                todo!()
            }
        }

        // `Debug` is the letter and the index (`n12`), or the letter and `-` for `NONE`.
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                todo!()
            }
        }

        const _: () = assert!(size_of::<$name>() == size_of::<$int>());
    };
}

handle!(
    /// An interned name: exact bytes, first-seen order (C6). The IR never folds case.
    NameId,
    u32,
    "n"
);
handle!(
    /// A file of the design (a deck, a library, a Verilog-A source, or a referenced file that was
    /// not read). At most [`FILES_MAX`].
    FileId,
    u16,
    "f"
);
handle!(
    /// A scope: a file's top, a subcircuit body, a section body, an `If` branch, a loop body.
    ScopeId,
    u32,
    "s"
);
handle!(
    /// An expression row. A tree's children always have lower ids than their parent, so trees are
    /// acyclic by construction. `NONE` in a value field means "absent" with the meaning that
    /// field's doc states.
    ExprId,
    u32,
    "e"
);
handle!(
    /// A definition (subcircuit, model, parameter, function, section, module).
    DefId,
    u32,
    "d"
);
handle!(
    /// A run of uninterned text (`Opaque`, `Comment` and `Title` bodies), appended once.
    TextId,
    u32,
    "t"
);

impl NameId {
    /// The name `0`, ground, interned first in every design so it is always id 0.
    pub const GROUND: NameId = NameId(0);
}

impl ItemRef {
    /// No item: the owner of a file's top scope.
    pub const NONE: ItemRef = ItemRef {
        kind: ItemKind::Title,
        index: u32::MAX,
    };

    /// The item at `index` of `kind`'s table. Does not check that the row exists.
    pub const fn new(kind: ItemKind, index: u32) -> ItemRef {
        todo!()
    }
}

impl Ref {
    /// A reference to `name` that binds to nothing yet.
    pub const fn unresolved(name: NameId) -> Ref {
        todo!()
    }

    /// Whether `def` is set.
    pub const fn is_resolved(self) -> bool {
        todo!()
    }
}

impl<T> List<T> {
    /// The empty list (`start` 0, `len` 0); valid in every pool.
    pub const EMPTY: List<T> = List {
        start: 0,
        len: 0,
        pool: PhantomData,
    };

    /// Element count.
    pub const fn len(self) -> usize {
        todo!()
    }

    /// Whether the list has no element.
    pub const fn is_empty(self) -> bool {
        todo!()
    }
}
