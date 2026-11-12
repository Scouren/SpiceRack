//! Model cards (sub-slice S3.2, a stub until it lands): a `Model` item → its `.model` card.
//!
//! **Contract for S3.2.** `pub(super) fn model(w: &mut Out<'_>, item: ir::ItemRef)`: the (type word, level) of the family through `families::card`; parameters in IR order; bins as `name.k`.
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. It spells exactly the rows its area adds to
//! `FEATURES` and asserts it is not handed another. Codes: `W1200–W1299` (`codes.rs`).
//!
//! Stub: its features are absent from `FEATURES`, so the spine never hands it an item; it
//! asserts so.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §4; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn model(w: &mut Out<'_>, item: ir::ItemRef)`

use super::codes::Row;
use super::write::Out;
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its writer spells). Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `W1200–W1299`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// A `Model` (stub: unreachable).
pub(super) fn model(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Model);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: model is a stub but FEATURES has Model"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Model) until S3.2 lands")
}
