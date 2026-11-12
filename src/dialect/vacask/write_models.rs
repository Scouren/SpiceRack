//! `Model` rows → `model name module params`, the module word from the family (`families.rs`).
//!
//! Sub-slice S4.1 writes this file; until then `FEATURES` lacks every row it writes. The
//! contract of each function is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cir-masters.
//!
//! Contents: `model` (crate-private)

use super::write::Writer;
use crate::ir;

/// One `Model` row as its `model` line.
///
/// Stub (S4.1): `FEATURES` lacks every row this writes, so `legalize` refused the design first.
pub(super) fn model(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let _ = w;
    unreachable!("S4.1: no VACASK writer yet for {at:?} (FEATURES lacks it)");
}
