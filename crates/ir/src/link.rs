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
//! Contents: `pub fn link(&mut Design, &mut Diags)`

use crate::design::Design;
use crate::diag::Diags;
#[cfg(doc)]
use crate::edit::Editor;
use crate::expr::Expr;
use crate::facts::RowFacts;
use crate::feature::{ExprWalk, expr_roots};
use crate::id::DefId;
use crate::item::{DefKind, ItemKind};

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
        let d: &Design = design;
        let mut changed = false;
        for_kind!(item.kind, R => {
            let mut row: R = d.row(item.index);
            row.handles(d, &mut handles);
            row.map_refs(&mut |r, kind| {
                if r.def.is_none() {
                    r.def = d.lookup(scope, kind, r.name);
                    changed |= !r.def.is_none();
                }
            });
            if changed {
                design.editor().set(item.index, row);
            }
        });
        // expressions: collect the bindings, then apply them through the builder
        let mut binds = Vec::new();
        for root in expr_roots(design, &handles) {
            for &e in walk.tree(design, root) {
                let def = match design.expr(e) {
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
