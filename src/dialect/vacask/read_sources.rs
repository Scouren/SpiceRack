//! Independent sources (`vsource`, `isource`: `type=` dc sine pulse exp pwl am fm, `mag`, `phase`), controlled sources, `mutual` and `tline_ideal` → their `Element` detail rows and waves, each field filled with VACASK's default.
//!
//! Sub-slice S4.2 writes this file; until then every statement it owns is an `Unread` opaque
//! (`read::unread`), `FEATURES` is empty and `CODES` has no row. The contract of each function
//! is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (sources, waves, controlled, mutual, tline_ideal); VD dev-builtin-src, dev-builtin-vcvs/vccs/ccvs/cccs, dev-builtin-mutual.
//!
//! Contents: `FEATURES`, `CODES`, `source` (crate-private)

use super::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use (S4.2 fills it).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's rows (S4.2 fills it).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];

/// The detail of a source, controlled source, coupling or line instance.
///
/// Stub (S4.2): an `Unread` opaque of the statement.
#[expect(dead_code, reason = "read_elements (S4.1) calls it")]
pub(super) fn source(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}
