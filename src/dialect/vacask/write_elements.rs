//! `Element` rows → instance statements (`name (nodes) master params`, `$mfactor`), and `Behav` → the behavioural source line.
//!
//! Sub-slice S4.1 writes this file; until then `FEATURES` lacks every row it writes. The
//! contract of each function is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cir-instance, dev-builtin-behavioral.
//!
//! Contents: `element` (crate-private)

use super::write::Writer;
use crate::ir;

/// One `Element` row as its instance statement.
///
/// Stub (S4.1): `FEATURES` lacks every row this writes, so `legalize` refused the design first.
pub(super) fn element(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let _ = w;
    unreachable!("S4.1: no VACASK writer yet for {at:?} (FEATURES lacks it)");
}
