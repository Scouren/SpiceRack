//! Run cards (sub-slice S3.3, a stub until it lands): `.op .dc .ac .tran .noise .tf .sens .pz .four .sp` (`Tag::Analysis`), `.options`, `.temp`, `.ic`, `.nodeset`, `.step`, and every other dot card no area claims (`Tag::Card`, through `read_structure::card`) → `Analysis`, `Option`, `Env`, `NodeSet` rows.
//!
//! **Contract for S3.3.** `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`: analyses become run items in the order ngspice runs them (batch mode runs dot-card analyses in reverse card order, INV 8.04); `.temp` with a list runs 27 °C and says so (4.02); device `ic=` applies only under UIC (8.21); inert cards (`.width`, `.csparam`) are `Inert`; ngspice's private cards `Private`.
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

/// A run card or an unclaimed dot card (stub: `Unread`).
pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt) {
    lw.unread(st);
}
