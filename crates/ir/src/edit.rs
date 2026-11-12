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
//! `Editor::set_detail<D: DetailRow>(&mut self, u32, D)`,
//! `Editor::map_exprs(&mut self, ItemRef, &mut dyn FnMut(&mut ExprId, ExprUse))`,
//! `Editor::map_refs(&mut self, ItemRef, &mut dyn FnMut(&mut Ref, DefKind))`

use crate::build::Builder;
use crate::design::{Design, DetailRow, ItemRow};
use crate::facts::{ExprUse, RowFacts};
use crate::id::{ExprId, ItemRef, Ref, ScopeId};
use crate::item::DefKind;

/// Rewrites one design. Holds it mutably for its lifetime.
#[derive(Debug)]
pub struct Editor<'d> {
    pub(crate) design: &'d mut Design,
}

impl<'d> Editor<'d> {
    /// The design as it stands, read only.
    pub fn design(&self) -> &Design {
        self.design
    }

    /// An append door for new rows, expressions, scopes and definitions.
    pub fn builder(&mut self) -> Builder<'_> {
        Builder {
            design: self.design,
        }
    }

    /// The ordered item list of `scope`. An item taken out of every list is dead (its row stays,
    /// unreachable); an item must not appear in two lists.
    pub fn items_mut(&mut self, scope: ScopeId) -> &mut Vec<ItemRef> {
        self.design.classes.take();
        assert!(
            scope.index() < self.design.scope_items.len(),
            "a scope id in range"
        );
        &mut self.design.scope_items[scope.index()]
    }

    /// Replaces the row at `index` of `R`'s table; the span is kept. Asserts the index is in
    /// range and the row's handles are.
    pub fn set<R: ItemRow>(&mut self, index: u32, row: R) {
        self.design.classes.take();
        R::table_mut(self.design).set(index, row)
    }

    /// Replaces the row at `index` of `D`'s table (a pass rewriting a typed field). Asserts the
    /// index is in range and the row's handles are. A detail row shared by two headers is a bug
    /// of the pass that shared it: the builder gives each header its own.
    pub fn set_detail<D: DetailRow>(&mut self, index: u32, row: D) {
        self.design.classes.take();
        let rows = D::rows_mut(self.design);
        assert!((index as usize) < rows.len(), "a detail index in range");
        rows[index as usize] = row;
    }

    /// Calls `f` on every expression root `item` owns, with how its field reads, in `handles`'
    /// order (one exhaustive dispatch to `RowFacts::map_exprs`, which stores them back): the
    /// door a pass uses to point a row at a new expression (A23). Asserts `item` is in range.
    pub fn map_exprs(&mut self, item: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        self.design.classes.take();
        let rows = for_kind!(item.kind, R => R::table(self.design).len());
        assert!((item.index as usize) < rows, "an item in range");
        for_kind!(item.kind, R => R::map_exprs(self.design, item, f))
    }

    /// Calls `f` on every `Ref` `item` owns (header, detail rows, pools) with the kind its field
    /// expects (one exhaustive dispatch over `ItemKind` to `RowFacts::map_refs`, which stores
    /// them back). For the linker and for passes that renumber definitions. Asserts `item` is in
    /// range.
    pub fn map_refs(&mut self, item: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        self.design.classes.take();
        let rows = for_kind!(item.kind, R => R::table(self.design).len());
        assert!((item.index as usize) < rows, "an item in range");
        for_kind!(item.kind, R => R::map_refs(self.design, item, f))
    }
}
