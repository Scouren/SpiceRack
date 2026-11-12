//! Model cards (sub-slice S3.2, a stub until it lands): a `.model name type (params)` card → `Model` rows (family by `families.rs`, version, polarity, parameters, bins).
//!
//! **Contract for S3.2.** `pub(super) fn model(lw: &mut Lower<'_, '_>, st: Stmt)`: (type word, `level`) → `ModelFamily` through `families::family`; a level ngspice would not run as written is `InvalidInSource` (INV 7.01, 7.02: above 99 ngspice silently runs level 1); bins `name.k`; a model inside a subcircuit is local (`Extra::LocalModel`), visible in its subcircuit and the ones inside it.
//! Every statement handed here becomes IR items in `st.scope`, or one `Opaque` with a coded note
//! (`Lower::opaque`); none is dropped (law L1, asserted by `read.rs`). The function validates the
//! whole statement before it pushes anything. It binds references by ngspice's rules (ruling R3)
//! and never learns a target. Codes: `P1200–P1299, R1200–R1299` (`codes.rs`).
//!
//! Stub: each statement is one `Opaque` of class `Unread` with `P1001`.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §3; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn model(lw: &mut Lower<'_, '_>, st: Stmt)`

use super::codes::Row;
use super::read::{Lower, Stmt};
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its reader produces and its writer spells).
/// Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `P1200–P1299, R1200–R1299`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// A `.model` card (stub: `Unread`).
pub(super) fn model(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.unread(st);
}
