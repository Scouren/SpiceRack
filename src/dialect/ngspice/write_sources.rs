//! Independent source cards (sub-slice S3.1, a stub until it lands): an `Element` item of kind `VSource` or `ISource` → its `V`/`I` card.
//!
//! **Contract for S3.1.** `pub(super) fn source(w: &mut Out<'_>, item: ir::ItemRef)`, called by `write_elements::element`: `dc`, `ac`, the waveform in ngspice's argument order (SFFM `VO VA FM MDI FC`), a `TStep`/`TStop` leaf as ngspice's given zero.
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. It spells exactly the rows its area adds to
//! `FEATURES` and asserts it is not handed another. Codes: `W1100–W1199` (`codes.rs`).
//!
//! Stub: its features are absent from `FEATURES`, so the spine never hands it an item; it
//! asserts so.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §4; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn source(w: &mut Out<'_>, item: ir::ItemRef)`

use super::codes::Row;
use super::write::Out;
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its writer spells). Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `W1100–W1199`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// A `V` or `I` element (stub: unreachable).
pub(super) fn source(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Element);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: source is a stub but FEATURES has Element"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Element) until S3.1 lands")
}
