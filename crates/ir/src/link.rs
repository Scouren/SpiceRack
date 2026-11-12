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

/// Binds every still-unresolved reference by `Design::lookup` from the scope of the item (or the
/// expression's item) that holds it, innermost scope first, exact names, with the kind the field
/// expects. A reference it cannot bind stays unresolved and gets no diagnostic: unresolved is
/// legal. A bound reference and a held one (IR text `name?`, held by a crate-private sentinel) are never changed.
/// Works through an [`Editor`] (`Editor::map_refs`, `Builder::resolve`). Deterministic: items in
/// scope order, fields in `map_refs` order. O(references × scope depth). `diags` is not appended
/// to today; it is the spine's uniform signature.
pub fn link(design: &mut Design, diags: &mut Diags) {
    todo!()
}
