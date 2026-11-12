//! `save` directives (`default`, `full`, `v()`, `i()`, `p(inst, var)`) → `Save` rows, and `ic=`/`nodeset=` lists of an analysis → `NodeSet` rows.
//!
//! Sub-slice S4.3 writes this file; until then every statement it owns is an `Unread` opaque
//! (`read::unread`), `FEATURES` is empty and `CODES` has no row. The contract of each function
//! is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.4 (save, ic, nodeset); VD cmd-save, cmd-analysis-op.
//!
//! Contents: `FEATURES`, `CODES`, `save` (crate-private)

use super::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use (S4.3 fills it).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's rows (S4.3 fills it).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];

/// A `save` statement.
///
/// Stub (S4.3): an `Unread` opaque of the statement.
#[allow(dead_code, reason = "read_run (S4.3) calls it")]
pub(super) fn save(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}
