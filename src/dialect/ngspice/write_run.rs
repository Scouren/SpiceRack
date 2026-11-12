//! Run cards (sub-slice S3.3, a stub until it lands): `Analysis`, `Option`, `Env`, `NodeSet`, `Statistics` items → dot cards.
//!
//! **Contract for S3.3.** `pub(super) fn analysis`, `option`, `env`, `node_set`, `statistics` (each `(w: &mut Out<'_>, item: ir::ItemRef)`): plain analyses as dot cards in the order that makes ngspice run them in IR order (it runs them in reverse card order, INV 8.04); a run plan with loops or alters is S3.4's `.control` script; ngspice has no statistics block (a citation row in `citations.toml`).
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. It spells exactly the rows its area adds to
//! `FEATURES` and asserts it is not handed another. Codes: `W1300–W1399` (`codes.rs`).
//!
//! Stub: its features are absent from `FEATURES`, so the spine never hands it an item; it
//! asserts so.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §4; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn analysis`, `option`, `env`, `node_set`, `statistics`

use super::codes::Row;
use super::write::Out;
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its writer spells). Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `W1300–W1399`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21, `Write::admits`). Stub: nothing (no item of this area is legal yet).
pub(super) fn admits(_design: &ir::Design, _item: ir::ItemRef, _diags: &mut ir::Diags) {}

/// An `Analysis` (stub: unreachable).
pub(super) fn analysis(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Analysis);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: analysis is a stub but FEATURES has Analysis"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Analysis) until S3.3 lands")
}

/// An `Option` (stub: unreachable).
pub(super) fn option(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Option);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: option is a stub but FEATURES has Option"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Option) until S3.3 lands")
}

/// An `Env` (stub: unreachable).
pub(super) fn env(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Env);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: env is a stub but FEATURES has Env"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Env) until S3.3 lands")
}

/// A `NodeSet` (stub: unreachable).
pub(super) fn node_set(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::NodeSet);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: node_set is a stub but FEATURES has NodeSet"
    );
    unreachable!("ngspice write: FEATURES lacks Item(NodeSet) until S3.3 lands")
}

/// A `Statistics` block (stub: unreachable).
pub(super) fn statistics(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Statistics);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: statistics is a stub but FEATURES has Statistics"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Statistics) until S3.3 lands")
}
