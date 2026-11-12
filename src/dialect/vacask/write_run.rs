//! Run items → one `control` … `endc` block: `options`, saves, `sweep` lines before each analysis of a loop's body, `analysis name kind k=v`, `alter`, `var`, `mc … endmc`.
//!
//! Sub-slice S4.3 writes this file; until then `FEATURES` lacks every row it writes. The
//! contract of each function is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-overview, cmd-sweep.
//!
//! Contents: `admits`, `control_span`, `analysis` (crate-private)

use super::write::Writer;
use crate::ir;

/// The first and last index into `design.items(top)` of the items the one `control` … `endc`
/// block holds (the run items, and VACASK's own opaques of a control command), or `None` when the
/// file has none. The spine writes `control` before the first and `endc` after the last.
///
/// Stub (S4.3): `None` while `FEATURES` holds no run item.
pub(super) fn control_span(design: &ir::Design, top: ir::ScopeId) -> Option<(usize, usize)> {
    let _ = (design, top);
    None
}

/// One run item of the top scope, in the `control` block.
///
/// Stub (S4.3): `FEATURES` lacks every row this writes, so `legalize` refused the design first.
pub(super) fn analysis(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let _ = w;
    unreachable!("S4.3: no VACASK writer yet for {at:?} (FEATURES lacks it)");
}

/// Appends a refusal for each row of this area VACASK cannot spell for a reason no feature
/// states (R21: a parameter its module lacks, a name no quoting spells). `legalize` calls it
/// through `Vacask::admits`, always.
///
/// Stub (S4.3): nothing to refuse while `FEATURES` holds none of this area's rows.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    let _ = (design, diags);
}
