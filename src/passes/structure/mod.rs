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
//! Contents: `pub mod deck_title`, `pub(super) const PASSES: &[Pass]`,
//! `pub(super) const CODES: &[(ir::Code, ir::Info)]`

pub mod deck_title;

use super::Pass;
use crate::ir;

/// The group's passes, in the order `legalize` tries them.
pub(super) const PASSES: &[Pass] = &[Pass {
    name: "deck_title",
    removes: ir::Feature::Extra(ir::Extra::DeckWithoutTitle),
    requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Title)),
    notes: &[],
    run: deck_title::deck_title,
}];

/// The group's catalogue rows (`U0300..=U0399`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];
