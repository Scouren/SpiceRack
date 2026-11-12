//! Output cards (sub-slice S3.3, a stub until it lands): `Save`, `Print`, `Measure` items → `.save`, `.print`, `.meas` cards.
//!
//! **Contract for S3.3.** `pub(super) fn save`, `print`, `measure` (each `(w: &mut Out<'_>, item: ir::ItemRef)`): probes spelled with the element's letter; measures in ngspice's grammar.
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
//! `pub(super) fn save`, `print`, `measure`,
//! `pub(super) fn probe(design: &ir::Design, probe: &ir::Probe, out: &mut String)`

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

/// A `Save` (stub: unreachable).
pub(super) fn save(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Save);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: save is a stub but FEATURES has Save"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Save) until S3.3 lands")
}

/// A `Print` (stub: unreachable).
pub(super) fn print(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Print);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: print is a stub but FEATURES has Print"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Print) until S3.3 lands")
}

/// A `Measure` (stub: unreachable).
pub(super) fn measure(_w: &mut Out<'_>, _item: ir::ItemRef) {
    let feature = ir::Feature::Item(ir::ItemKind::Measure);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: measure is a stub but FEATURES has Measure"
    );
    unreachable!("ngspice write: FEATURES lacks Item(Measure) until S3.3 lands")
}

/// Appends one probe as ngspice spells it inside another card (`v(a)`, `v(a,b)`, `i(vin)`): the
/// run area calls it for the outputs of `.four`, `.tf`, `.noise`. Stub: unreachable (no probe
/// feature is in `FEATURES` yet).
#[expect(dead_code, reason = "the run area calls it (S3.3a)")]
pub(super) fn probe(design: &ir::Design, probe: &ir::Probe, out: &mut String) {
    let _ = (design, probe, out);
    unreachable!("ngspice write: no probe is spelled until S3.3b lands")
}
