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
//! `Editor::remove_item(&mut self, ScopeId, ItemRef)`, `Editor::move_item(&mut self, ScopeId, ItemRef, ScopeId, usize)`,
//! `Editor::redefine(&mut self, DefId, ItemRef)`, `Editor::rename_def(&mut self, DefId, NameId)`,
//! `Editor::map_exprs(&mut self, ItemRef, &mut dyn FnMut(&mut ExprId, ExprUse))`,
//! `Editor::map_refs(&mut self, ItemRef, &mut dyn FnMut(&mut Ref, DefKind))`

use crate::build::Builder;
use crate::design::{Design, DetailRow, ItemRow};
use crate::facts::{ExprUse, RowFacts};
use crate::id::{DefId, ExprId, ItemRef, NameId, Ref, ScopeId};
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

    /// Takes `item` out of `scope`'s list and every definition of `item` out of the index, so no
    /// lookup finds it; its body scopes are then dead (A28). Asserts `item` is listed in `scope`.
    pub fn remove_item(&mut self, scope: ScopeId, item: ItemRef) {
        let d = &mut *self.design;
        d.derived_changed();
        d.element_names.take();
        let list = &mut d.scope_items[scope.index()];
        let at = list
            .iter()
            .position(|&i| i == item)
            .expect("the item is listed in the scope");
        list.remove(at);
        for (k, def) in d.defs.iter().enumerate() {
            let key = (def.scope, def.kind, def.name);
            if def.item == item && d.def_index.get(&key) == Some(&DefId(k as u32)) {
                d.def_index.remove(&key);
            }
        }
    }

    /// Moves `item` from `from`'s list to position `at` of `to`'s (a hoist): its definitions in
    /// `from` are re-keyed to `to`, its body scopes' parent becomes `to`, and references already
    /// bound stay bound. Asserts it is listed in `from`, `at <= ` the list's length, both scopes
    /// in one file, and no definition of the same kind and name in `to`.
    pub fn move_item(&mut self, from: ScopeId, item: ItemRef, to: ScopeId, at: usize) {
        let d = &mut *self.design;
        d.derived_changed();
        d.element_names.take();
        assert!(
            d.scopes[from.index()].file == d.scopes[to.index()].file,
            "a move stays in one file"
        );
        let list = &mut d.scope_items[from.index()];
        let pos = list
            .iter()
            .position(|&i| i == item)
            .expect("the item is listed in `from`");
        list.remove(pos);
        let target = &mut d.scope_items[to.index()];
        assert!(at <= target.len(), "a position in `to`'s list");
        target.insert(at, item);
        for k in 0..d.defs.len() {
            let def = d.defs[k];
            if def.item != item || def.scope != from {
                continue;
            }
            let old = (from, def.kind, def.name);
            let new = (to, def.kind, def.name);
            assert!(
                !d.def_index.contains_key(&new),
                "no definition of that kind and name in `to`"
            );
            if d.def_index.get(&old) == Some(&DefId(k as u32)) {
                d.def_index.remove(&old);
            }
            d.def_index.insert(new, DefId(k as u32));
            d.defs[k].scope = to;
        }
        for s in &mut d.scopes {
            if s.owner == item && s.parent == from {
                s.parent = to;
            }
        }
    }

    /// Makes `item` the defining item of `def` (a local parameter made a formal); every
    /// reference bound to `def` stays bound. Asserts both in range.
    pub fn redefine(&mut self, def: DefId, item: ItemRef) {
        let d = &mut *self.design;
        d.derived_changed();
        let rows = for_kind!(item.kind, R => R::table(d).len());
        assert!((item.index as usize) < rows, "an item in range");
        assert!(def.index() < d.defs.len(), "a definition id in range");
        d.defs[def.index()].item = item;
    }

    /// Re-keys `def` under `name` in its scope. References bound to it keep the `DefId`; their
    /// names are the pass's to rename (`map_refs`, `rewrite_exprs`), else `check` reports
    /// `RefTarget`. Asserts no definition of its kind is named `name` in its scope.
    pub fn rename_def(&mut self, def: DefId, name: NameId) {
        let d = &mut *self.design;
        d.derived_changed();
        assert!(def.index() < d.defs.len(), "a definition id in range");
        let row = d.defs[def.index()];
        let new = (row.scope, row.kind, name);
        assert!(!d.def_index.contains_key(&new), "the new key is free");
        let old = (row.scope, row.kind, row.name);
        if d.def_index.get(&old) == Some(&def) {
            d.def_index.remove(&old);
        }
        d.def_index.insert(new, def);
        d.defs[def.index()].name = name;
    }

    /// The ordered item list of `scope`, raw: it does not maintain the definition index (use
    /// `remove_item` and `move_item`; a stale entry is `check`'s `StaleDefinition`). An item
    /// taken out of every list is dead (its row stays,
    /// unreachable); an item must not appear in two lists.
    pub fn items_mut(&mut self, scope: ScopeId) -> &mut Vec<ItemRef> {
        self.design.derived_changed();
        self.design.element_names.take();
        assert!(
            scope.index() < self.design.scope_items.len(),
            "a scope id in range"
        );
        &mut self.design.scope_items[scope.index()]
    }

    /// Replaces the row at `index` of `R`'s table; the span is kept. Asserts the index is in
    /// range and the row's handles are.
    pub fn set<R: ItemRow>(&mut self, index: u32, row: R) {
        self.design.derived_changed();
        self.design.element_names.take();
        R::table_mut(self.design).set(index, row)
    }

    /// Replaces the row at `index` of `D`'s table (a pass rewriting a typed field). Asserts the
    /// index is in range and the row's handles are. A detail row shared by two headers is a bug
    /// of the pass that shared it: the builder gives each header its own.
    pub fn set_detail<D: DetailRow>(&mut self, index: u32, row: D) {
        self.design.derived_changed();
        let rows = D::rows_mut(self.design);
        assert!((index as usize) < rows.len(), "a detail index in range");
        rows[index as usize] = row;
    }

    /// Calls `f` on every expression root `item` owns, with how its field reads, in `handles`'
    /// order (one exhaustive dispatch to `RowFacts::map_exprs`, which stores them back): the
    /// door a pass uses to point a row at a new expression (A23). Asserts `item` is in range.
    pub fn map_exprs(&mut self, item: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        self.design.derived_changed();
        let rows = for_kind!(item.kind, R => R::table(self.design).len());
        assert!((item.index as usize) < rows, "an item in range");
        for_kind!(item.kind, R => R::map_exprs(self.design, item, f))
    }

    /// Calls `f` on every `Ref` `item` owns (header, detail rows, pools) with the kind its field
    /// expects (one exhaustive dispatch over `ItemKind` to `RowFacts::map_refs`, which stores
    /// them back). For the linker and for passes that renumber definitions. Asserts `item` is in
    /// range.
    pub fn map_refs(&mut self, item: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        self.design.derived_changed();
        let rows = for_kind!(item.kind, R => R::table(self.design).len());
        assert!((item.index as usize) < rows, "an item in range");
        for_kind!(item.kind, R => R::map_refs(self.design, item, f))
    }
}
