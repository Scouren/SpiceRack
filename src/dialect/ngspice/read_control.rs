//! Control blocks (sub-slice S3.4, a stub until it lands): a `.control` … `.endc` block (`Tag::Control`, its body as `Line`s and analysis commands) → run items in run order, `Loop`/`Alter` rows, `Inert` display commands, `Script` for data-dependent flow, `Load` for `pre_osdi`.
//!
//! **Contract for S3.4.** `pub(super) fn control(lw: &mut Lower<'_, '_>, st: Stmt)`: commands that are run items become them; `alterparam` needs `reset`, a dot-card analysis runs after the script and at each `run` (INV 13.01); loops and alters as `Loop`/`Alter`; `plot`, `print`, `set`, `quit` are `Inert`; data-dependent control flow is one `Script` opaque.
//! Every statement handed here becomes IR items in `st.scope`, or one `Opaque` with a coded note
//! (`Lower::opaque`); none is dropped (law L1, asserted by `read.rs`). The function validates the
//! whole statement before it pushes anything. It binds references by ngspice's rules (ruling R3)
//! and never learns a target. Codes: `P1400–P1499, R1400–R1499` (`codes.rs`).
//!
//! Stub: each statement is one `Opaque` of class `Unread` with `P1001`.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §2, §3; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn control(lw: &mut Lower<'_, '_>, st: Stmt)`

use super::codes::Row;
use super::read::{Lower, Stmt};
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its reader produces and its writer spells).
/// Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `P1400–P1499, R1400–R1499`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// A `.control` block (stub: one `Unread` for the whole block).
pub(super) fn control(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.unread(st);
}
