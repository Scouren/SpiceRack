//! The neutral linker: a design with unresolved references in, the same design with each bound
//! to the definition its exact name finds from the item's scope outward (Addendum A4). Every
//! `Ref` a row holds is reached through `RowFacts::map_refs` (one exhaustive dispatch over
//! `ItemKind`), and every `Param` and `UserCall` expression through the expression rows.
//!
//! Who calls it: the Python and Rust builders when they are done, and `text::parse` after the
//! whole text (so a hand-written fixture may name a later definition). A dialect reader never
//! calls it: it binds by its own simulator's rule (`Builder::resolve`, a resolved `Ref` in the
//! row). An already-bound reference is never changed; one no definition matches stays
//! unresolved, which is legal (the kit may be absent).
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (A4); `REWRITE-PLAN.md` §3.1 principle 7.
//!
//! Contents: `pub fn link(&mut Design, &mut Diags)`, `pub struct UnboundRead`,
//! `pub fn unbound_reads(&Design) -> Vec<UnboundRead>`

use crate::design::Design;
use crate::diag::Diags;
#[cfg(doc)]
use crate::edit::Editor;
use crate::expr::Expr;
use crate::facts::RowFacts;
use crate::feature::{ExprWalk, expr_roots};
use crate::id::{DefId, ExprId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, ItemKind, Subckt};

/// A parameter read no definition is bound to (A38, R63), at the item whose expressions hold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnboundRead {
    /// The item that holds the read.
    pub item: ItemRef,
    /// The `Expr::Param` row.
    pub expr: ExprId,
    /// The name it reads.
    pub name: NameId,
    /// Whether it is held on purpose: a `Subckt` head default's read, looked up from each
    /// instance's scope (A29; IR text `name?`). A strict read refuses the others only.
    pub held: bool,
}

/// Binds every still-unresolved reference by `Design::lookup` from the scope of the item (or the
/// expression's item) that holds it, innermost scope first, exact names, with the kind the field
/// expects. A `Param`-tag expression reached from a `Measure` item's handles tries
/// `DefKind::Param` first, then `DefKind::Measure` (A7). A reference it cannot bind stays unresolved and gets no diagnostic: unresolved is
/// legal. A bound reference and a held one (IR text `name?`, held by a crate-private sentinel) are never changed.
/// Works through an [`Editor`] (`Editor::map_refs`, `Builder::resolve`). Deterministic: items in
/// scope order, fields in `map_refs` order. O(references × scope depth). `diags` is not appended
/// to today; it is the spine's uniform signature.
pub fn link(design: &mut Design, diags: &mut Diags) {
    let mut walk = ExprWalk::new(design);
    let mut handles = Vec::new();
    for (item, scope) in design.reachable() {
        // rows: bind on a copy against the design as it is, then store the copy back
        handles.clear();
        for_kind!(item.kind, R => {
            let row: R = design.row(item.index);
            row.handles(design, &mut handles);
        });
        // rows: record every reference in `map_refs` order, look the unresolved ones up through
        // `&Design`, then write the k-th answer into the k-th reference
        let mut slots: Vec<(NameId, DefKind, DefId)> = Vec::new();
        design
            .editor()
            .map_refs(item, &mut |r, kind| slots.push((r.name, kind, r.def)));
        let found: Vec<DefId> = slots
            .iter()
            .map(|&(name, kind, def)| {
                if def.is_none() {
                    design.lookup(scope, kind, name)
                } else {
                    def
                }
            })
            .collect();
        if found.iter().zip(&slots).any(|(f, s)| *f != s.2) {
            let mut k = 0;
            design.editor().map_refs(item, &mut |r, _| {
                r.def = found[k];
                k += 1;
            });
        }
        // expressions: a subcircuit's head defaults are looked up from its body (A10: a default
        // reads its sibling head parameters first); every other row's from its listing scope
        // A29 (R53): a default's read of its own formal is never the formal itself; it is what
        // is visible from where the subcircuit is defined (the listing scope), or unbound
        let listing = scope;
        let (scope, roots): (ScopeId, Vec<(ExprId, NameId)>) = if item.kind == ItemKind::Subckt {
            let row = design.row::<Subckt>(item.index);
            let defaults = design
                .params(row.params)
                .iter()
                .map(|&(n, e)| (e, n))
                .collect();
            (row.body, defaults)
        } else {
            let roots = expr_roots(design, &handles);
            (
                scope,
                roots.into_iter().map(|e| (e, NameId::NONE)).collect(),
            )
        };
        let mut binds = Vec::new();
        for (root, own) in roots {
            for &e in walk.tree(design, root) {
                let def = match design.expr(e) {
                    Expr::Param(r) if r.def.is_none() && r.name == own => {
                        design.lookup(listing, DefKind::Param, r.name)
                    }
                    Expr::Param(r) if r.def.is_none() => {
                        let found = design.lookup(scope, DefKind::Param, r.name);
                        if found.is_none() && item.kind == ItemKind::Measure {
                            design.lookup(scope, DefKind::Measure, r.name)
                        } else {
                            found
                        }
                    }
                    Expr::UserCall(r, _) if r.def.is_none() => {
                        design.lookup(scope, DefKind::Func, r.name)
                    }
                    Expr::Num(_)
                    | Expr::Str(_)
                    | Expr::Param(_)
                    | Expr::Call(..)
                    | Expr::UserCall(..)
                    | Expr::Unary(..)
                    | Expr::Binary(..)
                    | Expr::Ternary(..)
                    | Expr::V(..)
                    | Expr::I(_)
                    | Expr::Time
                    | Expr::Temp
                    | Expr::Freq
                    | Expr::TStep
                    | Expr::TStop
                    | Expr::Tnom
                    | Expr::Draw(..)
                    | Expr::List(_) => DefId::NONE,
                };
                if !def.is_none() {
                    binds.push((e, def));
                }
            }
        }
        let mut b = design.builder();
        for (e, def) in binds {
            b.resolve(e, def);
        }
    }
    let _ = diags; // unresolved is legal: link appends nothing (A4)
}

/// Every `Expr::Param` row no definition is bound to, item by item in `Design::reachable` order,
/// root by root in `RowFacts::handles` order and left to right within a root (A38: the spine's
/// strict read refuses the ones not `held`). O(expressions).
pub fn unbound_reads(design: &Design) -> Vec<UnboundRead> {
    let mut out = Vec::new();
    let (mut handles, mut nodes) = (Vec::new(), Vec::new());
    for (item, _) in design.reachable() {
        handles.clear();
        design.item_handles(item, &mut handles);
        let mut seen = std::collections::HashSet::new();
        for root in expr_roots(design, &handles) {
            nodes.clear();
            crate::class::tree_nodes(design, root, &mut nodes);
            // children are built before parents, left to right: ascending ids are source order
            nodes.sort_unstable();
            for &e in &nodes {
                if let Expr::Param(r) = design.expr(e)
                    && !r.is_resolved()
                    && seen.insert(e)
                {
                    let held = item.kind == ItemKind::Subckt;
                    out.push(UnboundRead {
                        item,
                        expr: e,
                        name: r.name,
                        held,
                    });
                }
            }
        }
    }
    out
}
