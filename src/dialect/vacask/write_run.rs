//! Run items → one `control` … `endc` block: `options`, saves, `sweep` lines before each analysis of a loop's body, `analysis name kind k=v`, `alter`, `var`, `mc … endmc`.
//!
//! Sub-slice S4.3 writes this file; until then `FEATURES` lacks every row it writes. The
//! contract of each function is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-overview, cmd-sweep.
//!
//! Contents: `analysis` (crate-private)

use super::write::Writer;
use crate::ir;

/// One run item of the top scope, in the `control` block.
///
/// Stub (S4.3): `FEATURES` lacks every row this writes, so `legalize` refused the design first.
pub(super) fn analysis(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let _ = w;
    unreachable!("S4.3: no VACASK writer yet for {at:?} (FEATURES lacks it)");
}
