//! `Save` and `NodeSet` rows → `save` lines and the `ic=`/`nodeset=` lists of the analyses they apply to.
//!
//! Sub-slice S4.3 writes this file; until then `FEATURES` lacks every row it writes. The
//! contract of each function is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-save.
//!
//! Contents: `save` (crate-private)

use super::write::Writer;
use crate::ir;

/// One `Save` row as its `save` line.
///
/// Stub (S4.3): `FEATURES` lacks every row this writes, so `legalize` refused the design first.
#[expect(dead_code, reason = "write_run (S4.3) calls it")]
pub(super) fn save(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let _ = w;
    unreachable!("S4.3: no VACASK writer yet for {at:?} (FEATURES lacks it)");
}
