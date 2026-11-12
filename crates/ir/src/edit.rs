//! The passes' door: a design in, the same design rewritten out. A pass replaces rows in place,
//! rewrites a scope's ordered item list (insert, drop, reorder, hoist) and appends new rows and
//! expressions through a [`Builder`]. Expression rows are never replaced: a pass builds the new
//! tree and points the row at it, so ids stay acyclic (children below parents).
//!
//! `check` runs after every pass; a violation is that pass's bug. `link` is a client: it rewrites
//! row references through [`Editor::map_refs`] and expression references through
//! `Builder::resolve`.
//!
//! Cites: `rewrite/IR-KERNEL.md` C13; `REWRITE-PLAN.md` §4.4.
//!
//! Contents: `pub struct Editor<'d>`, `Editor::design(&self) -> &Design`,
//! `Editor::builder(&mut self) -> Builder<'_>`,
//! `Editor::items_mut(&mut self, ScopeId) -> &mut Vec<ItemRef>`,
//! `Editor::set<R: ItemRow>(&mut self, u32, R)`,
//! `Editor::map_refs(&mut self, ItemRef, &mut dyn FnMut(&mut Ref, DefKind))`

use crate::build::Builder;
use crate::design::{Design, ItemRow};
use crate::id::{ItemRef, Ref, ScopeId};
use crate::item::DefKind;

/// Rewrites one design. Holds it mutably for its lifetime.
#[derive(Debug)]
pub struct Editor<'d> {
    pub(crate) design: &'d mut Design,
}

impl<'d> Editor<'d> {
    /// The design as it stands, read only.
    pub fn design(&self) -> &Design {
        todo!()
    }

    /// An append door for new rows, expressions, scopes and definitions.
    pub fn builder(&mut self) -> Builder<'_> {
        todo!()
    }

    /// The ordered item list of `scope`. An item taken out of every list is dead (its row stays,
    /// unreachable); an item must not appear in two lists.
    pub fn items_mut(&mut self, scope: ScopeId) -> &mut Vec<ItemRef> {
        todo!()
    }

    /// Replaces the row at `index` of `R`'s table; the span is kept. Asserts the index is in
    /// range and the row's handles are.
    pub fn set<R: ItemRow>(&mut self, index: u32, row: R) {
        todo!()
    }

    /// Calls `f` on each `Ref` of `item`'s row with the kind its field expects, and stores the
    /// row back (one exhaustive dispatch over `ItemKind` to `RowFacts::map_refs`). For the
    /// linker and for passes that renumber definitions. Asserts `item` is in range.
    pub fn map_refs(&mut self, item: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        todo!()
    }
}
