//! The `control` block → run items in order: `options` (typed keys, one `Hint` opaque for solver knobs), `var`, `sweep` + `analysis` (a `Dc` over `op`, else a `Loop`), `alter`, `elaborate`, `clear`, `mc … endmc`, `print`/`abort`/`postprocess` (`Inert`).
//!
//! Sub-slice S4.3 writes this file; until then every statement it owns is an `Unread` opaque
//! (`read::unread`), `FEATURES` is empty and `CODES` has no row. The contract of each function
//! is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.4; VD cmd-overview, cmd-sweep, cmd-alter, cmd-elaboration, cmd-analysis-*.
//!
//! Contents: `FEATURES`, `CODES`, `control` (crate-private)

use super::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use (S4.3 fills it).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's rows (S4.3 fills it).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];

/// `control` … `endc`: every statement of the block.
///
/// Stub (S4.3): an `Unread` opaque of the statement.
pub(super) fn control(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}
