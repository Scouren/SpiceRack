//! Instance statements → `Element` rows: `name (nodes) master params` of a model, a subcircuit or a loaded module (the kind from the master), `$mfactor` → `Element.mult` on a device (a parameter on a subckt instance), and the behavioural source `name (p n) v=|i=|potential=|flow=<expr>` → `Behav`.
//!
//! Sub-slice S4.1 writes this file; until then every statement it owns is an `Unread` opaque
//! (`read::unread`), `FEATURES` is empty and `CODES` has no row. The contract of each function
//! is its doc and `rewrite/S4-AREAS.md`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (instance, `$mfactor`, R C L, behavioural); VD cir-instance, cir-mfactor, dev-builtin-behavioral.
//!
//! Contents: `FEATURES`, `CODES`, `Declared`, `declare`, `instance` (crate-private)

use super::read::{self, Cst, Lower};
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use (S4.1 fills it).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's rows (S4.1 fills it).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];

/// Every master every file declares, by the scope that holds it (S4.1 fills it).
#[derive(Debug, Default)]
pub(super) struct Declared;

/// The masters of every file, one walk over every CST before lowering (an instance's kind
/// comes from its master, which may be defined later or in a library file: run fwd).
///
/// Stub (S4.1): nothing declared.
pub(super) fn declare(csts: &[(Cst<'_>, ir::ScopeId)]) -> Declared {
    let _ = csts;
    Declared
}

/// `name (nodes) master params`, and a behavioural source (no master).
///
/// Stub (S4.1): an `Unread` opaque of the statement.
pub(super) fn instance(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}
