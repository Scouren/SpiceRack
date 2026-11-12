//! `model name module params` → `Model{family, polarity, params, module}` by the module table (`tables.rs`, `families.rs`), `model_<p>`/`instance_<p>` → the family's names; a builtin module → a parameterless model of the builtin's family (§10 Q15).
//!
//! Sub-slice S4.1 writes this file; until then every statement it owns is an `Unread` opaque
//! (`read::unread`), `FEATURES` is empty and `CODES` has no row. The contract of each function
//! is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (model); VD cir-masters, dev-spice.
//!
//! Contents: `FEATURES`, `CODES`, `model` (crate-private)

use super::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use (S4.1 fills it).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's rows (S4.1 fills it).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];

/// `model name module params`.
///
/// Stub (S4.1): an `Unread` opaque of the statement.
pub(super) fn model(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}
