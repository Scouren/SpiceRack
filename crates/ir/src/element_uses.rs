//! Element names used as references (A32): a design and an item in, each by-name element
//! reference the item holds out, each segment of a path one use with the scope whose element
//! list the name is read in; a pass's renames stored back. Rows report their references
//! (`RowFacts::map_element_names`); the kernel adds `Expr::I` in the item's expressions and
//! resolves every path: segment 0 is read in the item's listing scope, each next one in the body
//! of the master of the instance the segment before names, and a segment after a non-instance
//! or an unresolved master is read in `ScopeId::NONE` (unresolved: never guessed).
//!
//! Cites: `rewrite/AMEND-A12.md` A23 (Later), A32; `rewrite/RULINGS.md` R23.
//!
//! Contents: `pub struct ElementUse`, `Editor::map_element_uses(&mut self, ScopeId, ItemRef,
//! &mut dyn FnMut(ElementUse) -> Option<NameId>)`, `Design::element_uses(&self) -> Vec<(ItemRef,
//! ElementUse)>`; crate: `pub(crate) fn path_scopes(&Design, ScopeId, &[NameId]) -> Vec<ScopeId>`

use crate::design::Design;
use crate::edit::Editor;
use crate::id::{ItemRef, NameId, ScopeId};

/// One element name read by name: `name` is looked up in `scope`'s element list
/// (`ScopeId::NONE`: the scope cannot be resolved, so the name names no known element).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementUse {
    /// The scope whose element list `name` is read in.
    pub scope: ScopeId,
    /// The element name as written (the whole name, A13).
    pub name: NameId,
}

impl Editor<'_> {
    /// Calls `f` on every by-name element reference of `item`, listed in `scope`: the row's
    /// (`RowFacts::map_element_names`, in field order), then each `Expr::I` of its expressions
    /// (in `RowFacts::handles`' root order); a path gives one use per segment. `Some(new)`
    /// replaces that name. A segment's scope is resolved by its old name, else by the new one
    /// (so a pass may rename the elements first). Asserts `item` is in range.
    pub fn map_element_uses(
        &mut self,
        scope: ScopeId,
        item: ItemRef,
        f: &mut dyn FnMut(ElementUse) -> Option<NameId>,
    ) {
        todo!()
    }
}

impl Design {
    /// Every live item's element uses, in `Editor::map_element_uses`'s order, item by item in
    /// `reachable` order: the read-only twin a pass plans with. O(design) (one copy of the
    /// design, read).
    pub fn element_uses(&self) -> Vec<(ItemRef, ElementUse)> {
        todo!()
    }
}

/// The scope each segment of `path` is read in, from `scope`: segment 0 in `scope`, each next
/// in the body of the master of the instance the segment before names; `NONE` after a
/// non-instance, an unknown name or an unresolved master.
pub(crate) fn path_scopes(design: &Design, scope: ScopeId, path: &[NameId]) -> Vec<ScopeId> {
    todo!()
}
