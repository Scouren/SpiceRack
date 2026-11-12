//! Element items → ngspice cards: each kind of `read_elements::FEATURES` spelled the way
//! `read_elements` reads it back to the same rows (law L5), the name as it is (R23), nodes in
//! ngspice's order, a card number in `INPevaluate`'s spelling, any other value in `{…}`, a
//! B-source field (`v=`, `i=`, `vol=`, `cur=`, a table's input, an R/C/L value ngspice runs as a
//! B source) in the B-source grammar. A `VSource`, `ISource` or `Port` (ngspice's `V … portnum`) goes to `write_sources::source`.
//!
//! What a feature cannot state and ngspice cannot spell is refused by [`admits`] (R21, code
//! `W1100`): a name whose letter is not its kind's card (until A13's `NameWithoutLetter`), `m`
//! where the device has none (E, H, a voltage B), fields ngspice's B-source rewrite of an R/C/L
//! drops, an instance parameter the reader would read as a typed field.
//!
//! Cites: `read_elements.rs` and its children (the forms); `rewrite/DIALECT-NGSPICE.md` §4;
//! `rewrite/RULINGS.md` R21, R23.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) const UNSPELLABLE: ir::Code`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn element(w: &mut Out<'_>, item: ir::ItemRef)`

mod admit;
mod cards;

use super::codes::Row;
use super::write::Out;
use super::{read_elements, write_sources};
use crate::ir::{self, ElemKind as K};

/// The writer spells exactly the reader's image (L5).
pub(super) const FEATURES: ir::FeatureSet = read_elements::FEATURES;

/// An element ngspice has no card for though every feature it uses is in `FEATURES`.
pub(super) const UNSPELLABLE: ir::Code = ir::Code::new(b'W', 1100);

/// This area's codes.
pub(super) const CODES: &[Row] = &[(
    UNSPELLABLE,
    ir::Info {
        title: "an element ngspice has no card for",
        cite: "rewrite/RULINGS.md R21; ngspice-45 spicelib/parser/inp2*.c, frontend/inpcom.c",
        explain: "a name whose letter is not its kind's, m where the device takes none, a field ngspice's B-source rewrite drops, or a parameter that reads back as a typed field.",
        kind: ir::Kind::Refusal,
    },
)];

/// Appends a refusal for an element ngspice cannot spell for a reason no feature states (R21).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let e: ir::Element = design.row(item.index);
    if matches!(e.kind, K::VSource | K::ISource | K::Port) {
        return write_sources::admits(design, item, diags);
    }
    if !admit::spellable(design, &e) {
        diags.push_at(UNSPELLABLE, design, item, 0);
    }
}

/// An `Element`: its card.
pub(super) fn element(w: &mut Out<'_>, item: ir::ItemRef) {
    let e: ir::Element = w.design.row(item.index);
    if matches!(e.kind, K::VSource | K::ISource | K::Port) {
        return write_sources::source(w, item);
    }
    assert!(
        super::FEATURES.contains(ir::Feature::Elem(e.kind)),
        "ngspice write: FEATURES lacks {:?} (the spine checked the design)",
        e.kind
    );
    let text = cards::card(w.design, &e);
    w.line(&text);
}
