//! ngspice's model table (sub-slice S3.2, a stub until it lands): a `.model` card's type word
//! and `level` ↔ `ir::ModelFamily`, both ways, as data.
//!
//! **Contract for S3.2.** `pub(super) fn family(word: &str, level: Option<&str>) ->
//! Option<ir::ModelFamily>`: ngspice's own table (`devhelp`, the manual's model chapter); `None`
//! for a pair ngspice would not run as written (above level 99 it silently runs level 1, INV
//! 7.02: the reader makes that card `InvalidInSource`). `pub(super) fn card(family:
//! ir::ModelFamily) -> Option<(&'static str, Option<u32>)>`: the type word and level ngspice
//! spells the family with, `None` for a family it lacks (a citation row). The two are inverse on
//! every family `card` spells (law L5).
//!
//! Stub: no family is known; `read_models` and `write_models` do not call these yet.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §3 (model levels); `rewrite/INV-semantics.md` 7.01,
//! 7.02; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) fn family(&str, Option<&str>) -> Option<ir::ModelFamily>`,
//! `pub(super) fn card(ir::ModelFamily) -> Option<(&'static str, Option<u32>)>`

use crate::ir;

/// The family of a `.model` card's type word and `level` (stub: none).
#[expect(dead_code, reason = "a stub: S3.2's read_models calls it")]
pub(super) fn family(word: &str, level: Option<&str>) -> Option<ir::ModelFamily> {
    let _ = (word, level);
    None
}

/// The type word and level ngspice spells `family` with (stub: none).
#[expect(dead_code, reason = "a stub: S3.2's write_models calls it")]
pub(super) fn card(family: ir::ModelFamily) -> Option<(&'static str, Option<u32>)> {
    let _ = family;
    None
}
