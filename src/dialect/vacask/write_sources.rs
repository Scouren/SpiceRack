//! Source, controlled-source, coupling and line detail rows → their instance parameters (`type=` and the wave's keys); a `DcRule::Given` DC on a waveform source → the `alter … type="dc"` pair around DC analyses (S4.3 writes it).
//!
//! Sub-slice S4.2 writes this file; until then `FEATURES` lacks every row it writes. The
//! contract of each function is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD dev-builtin-src.
//!
//! Contents: `source` (crate-private)

use super::write::Writer;
use crate::ir;

/// The parameters of one source-like `Element`.
///
/// Stub (S4.2): `FEATURES` lacks every row this writes, so `legalize` refused the design first.
pub(super) fn source(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let _ = w;
    unreachable!("S4.2: no VACASK writer yet for {at:?} (FEATURES lacks it)");
}
