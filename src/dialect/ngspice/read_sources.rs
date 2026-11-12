//! Independent source cards (sub-slice S3.1, a stub until it lands): a `V` or `I` card → `Element` rows of kind `VSource`/`ISource` with their `Source` detail and waveform rows.
//!
//! **Contract for S3.1.** `pub(super) fn source(lw: &mut Lower<'_, '_>, st: Stmt)`: `dc`, `ac` (magnitude, phase in degrees) and the waveform (`PULSE`, `SIN`, `EXP`, `PWL`, `SFFM`, `AM`, `TRNOISE`, `TRRANDOM`) by named fields in ngspice's argument order; a given zero rise, fall or tau is the `TStep` leaf, a given zero width or period `TStop` (INV 5.05); SFFM and AM `SffmLaw::Ngspice`, `PreDelay::Zero` (5.01–5.03); a source with no DC gets the waveform's value at t = 0 (5.13).
//! Every statement handed here becomes IR items in `st.scope`, or one `Opaque` with a coded note
//! (`Lower::opaque`); none is dropped (law L1, asserted by `read.rs`). The function validates the
//! whole statement before it pushes anything. It binds references by ngspice's rules (ruling R3)
//! and never learns a target. Codes: `P1100–P1199, R1100–R1199` (`codes.rs`).
//!
//! Stub: each statement is one `Opaque` of class `Unread` with `P1001`.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §3; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn source(lw: &mut Lower<'_, '_>, st: Stmt)`

use super::codes::Row;
use super::read::{Lower, Stmt};
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its reader produces and its writer spells).
/// Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `P1100–P1199, R1100–R1199`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// A `V` or `I` card (stub: `Unread`).
pub(super) fn source(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.unread(st);
}
