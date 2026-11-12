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
//! ElementUse)>`; crate: `pub(crate) fn walk_path(&Design, ScopeId, &[NameId], &mut dyn FnMut(ElementUse)
//! -> Option<NameId>) -> Vec<NameId>`

use crate::design::{Design, ItemRow};
use crate::edit::Editor;
use crate::element::Element;
use crate::expr::Expr;
use crate::facts::RowFacts;
use crate::feature::{ExprWalk, expr_roots};
use crate::id::{ExprId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, ItemKind, Subckt};

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
        let d = &mut *self.design;
        let rows = for_kind!(item.kind, R => R::table(d).len());
        assert!((item.index as usize) < rows, "an item in range");
        // the row's paths, copied (the row's walk holds the design)
        let mut paths: Vec<Vec<NameId>> = Vec::new();
        for_kind!(item.kind, R => R::map_element_names(d, item, &mut |p| paths.push(p.to_vec())));
        let currents = current_reads(d, item);
        // the pass's answers, segment by segment, with the scopes resolved on the design as is
        let read: &Design = d;
        let mut renamed: Vec<Vec<NameId>> = Vec::with_capacity(paths.len());
        for path in &paths {
            renamed.push(walk_path(read, scope, path, f));
        }
        let current_names: Vec<NameId> = currents
            .iter()
            .map(|&(_, old)| f(ElementUse { scope, name: old }).unwrap_or(old))
            .collect();
        // store back, in the same order
        let mut k = 0;
        for_kind!(item.kind, R => R::map_element_names(d, item, &mut |p| {
            p.copy_from_slice(&renamed[k]);
            k += 1;
        }));
        for (&(e, _), &name) in currents.iter().zip(&current_names) {
            d.exprs.a[e.index()] = name.0;
        }
    }
}

impl Design {
    /// Every live item's element uses, in `Editor::map_element_uses`'s order, item by item in
    /// `reachable` order: the read-only twin a pass plans with. O(design) (one copy of the
    /// design, read).
    pub fn element_uses(&self) -> Vec<(ItemRef, ElementUse)> {
        let mut copy = self.clone();
        let items = self.reachable();
        let mut out = Vec::new();
        let mut editor = copy.editor();
        for (item, scope) in items {
            editor.map_element_uses(scope, item, &mut |u| {
                out.push((item, u));
                None
            });
        }
        out
    }
}

/// Calls `f` on each segment of `path` read from `scope` and returns the names after it: segment
/// 0 is read in `scope`, each next in the body of the master of the instance the segment before
/// names (found by its old name, else by the name `f` gave it); `NONE` after a non-instance, an
/// unknown name or an unresolved master.
pub(crate) fn walk_path(
    design: &Design,
    scope: ScopeId,
    path: &[NameId],
    f: &mut dyn FnMut(ElementUse) -> Option<NameId>,
) -> Vec<NameId> {
    let mut at = scope;
    let mut names = Vec::with_capacity(path.len());
    for &old in path {
        let new = f(ElementUse {
            scope: at,
            name: old,
        });
        names.push(new.unwrap_or(old));
        at = next_scope(design, at, old)
            .or_else(|| new.and_then(|n| next_scope(design, at, n)))
            .unwrap_or(ScopeId::NONE);
    }
    names
}

// ---- private ----

/// The body of the master of the instance `name` names in `scope`'s element list, if it is an
/// instance of a resolved subcircuit.
fn next_scope(design: &Design, scope: ScopeId, name: NameId) -> Option<ScopeId> {
    if scope.is_none() {
        return None;
    }
    let index = design
        .element_names
        .get_or_init(|| crate::build::element_index(design));
    let item = *index.get(&(scope, name))?;
    let element: Element = design.row(item.index);
    let def = element.model.def.index_checked(design.defs.len())?;
    let def = design.defs[def];
    (def.kind == DefKind::Subckt && def.item.kind == ItemKind::Subckt)
        .then(|| design.row::<Subckt>(def.item.index).body)
}

/// The `Expr::I` rows of `item`'s expressions with the names they read, each row once, root by
/// root in `handles`' order and left to right within a root.
fn current_reads(design: &Design, item: ItemRef) -> Vec<(ExprId, NameId)> {
    let mut handles = Vec::new();
    design.item_handles(item, &mut handles);
    let mut walk = ExprWalk::new(design);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for root in expr_roots(design, &handles) {
        // children are built before parents, left to right: ascending ids are source order
        let mut nodes = walk.tree(design, root).to_vec();
        nodes.sort_unstable();
        for &e in &nodes {
            if let Expr::I(name) = design.expr(e)
                && seen.insert(e)
            {
                out.push((e, name));
            }
        }
    }
    out
}
