//! Element cards (sub-slice S3.1, a stub until it lands): an `Element` item → its ngspice card.
//!
//! **Contract for S3.1.** `pub(super) fn element(w: &mut Out<'_>, item: ir::ItemRef)`: the IR name as it is, letter included (R23; a name not starting with its kind's letter is `CircuitExtra::NameWithoutLetter`, absent from FEATURES: A13); nodes in ngspice's order; values with `expr_write::value` (or the B-source form with no braces); a `VSource`/`ISource` goes to `write_sources::source`.
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
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn element(w: &mut Out<'_>, item: ir::ItemRef)`

use super::codes::Row;
use super::write::Out;
use super::write_sources;
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its writer spells). Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `W1100–W1199`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// Appends a refusal for this area's items ngspice cannot spell for a reason no feature states
/// (R21, `Write::admits`). Stub: nothing (no item of this area is legal yet).
pub(super) fn admits(_design: &ir::Design, _item: ir::ItemRef, _diags: &mut ir::Diags) {}

/// An `Element` (stub: sources go to `write_sources::source`, the rest are unreachable).
pub(super) fn element(w: &mut Out<'_>, item: ir::ItemRef) {
    let e: ir::Element = w.design.row(item.index);
    if matches!(e.kind, ir::ElemKind::VSource | ir::ElemKind::ISource) {
        return write_sources::source(w, item);
    }
    let feature = ir::Feature::Elem(e.kind);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: element is a stub but FEATURES has {:?}",
        e.kind
    );
    unreachable!("ngspice write: FEATURES lacks Item(Element) until S3.1 lands")
}
