//! Element cards (sub-slice S3.1, a stub until it lands): an element card (`Tag::Instance` whose letter is not `V` or `I`: R C L K E G F H B D Q M J Z S W T O U Y P A N X) → `Element` rows with their detail rows.
//!
//! **Contract for S3.1.** `pub(super) fn element(lw: &mut Lower<'_, '_>, st: Stmt)`: the card's letter picks the `ElemKind` (an `E`/`G`/`F`/`H`/`B` by its form: linear, `POLY`, `TABLE`, `VALUE`/`V=`/`I=`, `LAPLACE`); the IR name is the whole folded name, letter included (R23); nodes fold through `Folder::node` (`gnd` is `0`); values are card numbers (`number::card`) or expressions (`expr_read::parse`, which picks numparam or the B-source parser); an R/C/L whose value reads the circuit is what ngspice runs (INV 6.14); `m` on an `X` card is the multiplicity unless the subcircuit declares `m` (6.01); masters bind to the nearest definition (models and subcircuits visible from the scope).
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
//! `pub(super) fn element(lw: &mut Lower<'_, '_>, st: Stmt)`

use super::codes::Row;
use super::read::{Lower, Stmt};
use crate::ir;

/// What this area adds to `FEATURES` (the IR nodes its reader produces and its writer spells).
/// Empty in the stub.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's codes, `P1100–P1199, R1100–R1199`. Empty in the stub.
pub(super) const CODES: &[Row] = &[];

/// An element card (stub: `Unread`).
pub(super) fn element(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.unread(st);
}
