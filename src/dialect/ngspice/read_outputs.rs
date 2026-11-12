//! Output cards (sub-slice S3.3, a stub until it lands): `.save`, `.probe` (`Tag::Save`), `.print`, `.plot` (`Tag::Print`), `.meas` (`Tag::Measure`) → `Save`, `Print`, `Measure` rows.
//!
//! **Contract for S3.3.** `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`: probes as `Probe` rows (a difference probe a pair of nets, INV 1.11); measures structured (9.x); a `.meas td=<measure>` is ngspice's error (14.08).
//! Every statement handed here becomes IR items in `st.scope`, or one `Opaque` with a coded note
//! (`Lower::opaque`); none is dropped (law L1, asserted by `read.rs`). The function validates the
//! whole statement before it pushes anything. It binds references by ngspice's rules (ruling R3)
//! and never learns a target. Codes: `P1300–P1399, R1300–R1399` (`codes.rs`).
//!
//! Stub: each statement is one `Opaque` of class `Unread` with `P1001`.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §3; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`

use super::codes::Row;
use super::read::{Lower, Stmt};
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its reader produces and its writer spells).
/// Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `P1300–P1399, R1300–R1399`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// An output card (stub: `Unread`).
pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.unread(st);
}
