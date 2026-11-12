//! Control scripts (sub-slice S3.4, a stub until it lands): `Load`, `Loop`, `Alter` items → a `.control` script (ngspice's native form of a run plan with loops or alters; `pre_osdi` for an OSDI load).
//!
//! **Contract for S3.4.** `pub(super) fn load`, `loop_`, `alter` (each `(w: &mut Out<'_>, item: ir::ItemRef)`): the writer's script form of a run plan; `reset` before `alterparam`.
//! Each function writes one item of a design legal for `FEATURES` (the spine checked it), sees
//! only the IR, and never learns a source. It spells exactly the rows its area adds to
//! `FEATURES` and asserts it is not handed another. Codes: `W1400–W1499` (`codes.rs`).
//!
//! Stub: its features are absent from `FEATURES`, so the spine never hands it an item; it
//! asserts so.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §4; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn load`, `loop_`, `alter`

use super::codes::Row;
use super::write::Out;
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its writer spells). Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `W1400–W1499`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// A `Load` (stub: unreachable).
pub(super) fn load(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Load);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: load is a stub but FEATURES has Load"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Load) until S3.4 lands")
}

/// A `Loop` and its body (stub: unreachable).
pub(super) fn loop_(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Loop);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: loop_ is a stub but FEATURES has Loop"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Loop) until S3.4 lands")
}

/// An `Alter` (stub: unreachable).
pub(super) fn alter(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Alter);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: alter is a stub but FEATURES has Alter"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Alter) until S3.4 lands")
}
