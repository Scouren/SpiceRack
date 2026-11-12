//! Passes group `structure` (subcircuits, models, names, libraries, decks): a design in, the
//! design without one feature out, one pass per file in this folder.
//!
//! Owns the passes over scopes, definitions and names (`deck_title`, `hoist_nested_subckts`,
//! `formal_defaults_to_instances`, `explicit_family_models`, `device_libraries`,
//! `splice_elaborate`, `name_with_letter`, … in `rewrite/DIALECT-VACASK.md` §6). It runs first:
//! every other group walks the scopes and names it settles.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - one pass per file in this folder, a helper shared by this group's passes here too;
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` lists the passes in the order `legalize` tries them within the group;
//! - `CODES` holds the notes the passes put at items they leave, codes `U0300..=U0399`, each
//!   named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/structure/<pass>/<case>.ir` and
//!   `.out.ir`, and meets the laws of `tests/pass_laws.rs` (P1–P5).
//!
//! Cites: `rewrite/SPINE.md` §4; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub mod deck_title`, `pub mod device_libraries`, `pub mod explicit_family_models`,
//! `pub(super) const PASSES: &[Pass]`, `pub(super) const CODES: &[(ir::Code, ir::Info)]`

pub mod deck_title;
pub mod device_libraries;
pub mod explicit_family_models;
mod libraries;

use super::Pass;
use crate::ir;

/// The group's passes, in the order `legalize` tries them.
pub(super) const PASSES: &[Pass] = &[
    Pass {
        name: "deck_title",
        removes: ir::Feature::Extra(ir::Extra::DeckWithoutTitle),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Title)),
        notes: &[],
        run: deck_title::deck_title,
    },
    // before `device_libraries`: the models it adds may need libraries (it adds them itself)
    Pass {
        name: "explicit_family_models",
        removes: ir::Feature::Circuit(ir::CircuitExtra::ElementWithoutModel),
        requires: explicit_family_models::REQUIRES,
        notes: &[
            explicit_family_models::NO_FAMILY,
            explicit_family_models::NO_PLACE,
        ],
        run: explicit_family_models::explicit_family_models,
    },
    Pass {
        name: "device_libraries",
        removes: ir::Feature::Circuit(ir::CircuitExtra::ModelWithoutLibrary),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::DeviceLibrary)),
        notes: &[],
        run: device_libraries::device_libraries,
    },
];

/// The group's catalogue rows (`U0300..=U0399`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        explicit_family_models::NO_FAMILY,
        ir::Info {
            title: "an element of a kind that names no model",
            cite: "rewrite/IR-CIRCUIT.md §4a; rewrite/AMEND-A12.md C5",
            explain: "The target instantiates every device through a model, and this element's kind (a behavioural, table, Laplace, frequency-table, delay or polynomial source, a port, a charge-defined capacitor or a flux-defined inductor) has no model family a model could name, so no model can stand for it and the element is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        explicit_family_models::NO_PLACE,
        ir::Info {
            title: "an element in a library section the design places no model in",
            cite: "rewrite/IR-CIRCUIT.md §4a; rewrite/AMEND-A12.md A15 K1, C5",
            explain: "The target instantiates every device through a model. A section is read alone, so the element's model must stand in the section; the design places no model in a section, so the target may lack that placement, and the element is refused.",
            kind: ir::Kind::Note,
        },
    ),
];
