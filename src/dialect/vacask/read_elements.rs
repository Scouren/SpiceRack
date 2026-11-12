//! Instance statements → `Element` rows: `name (nodes) master params` of a model, a subcircuit
//! or a loaded module (the kind from the master), `$mfactor` → `Element.mult` on a device (a
//! parameter on a subckt instance), and the behavioural source `name (p n) v=|i=<expr>` →
//! `Behav`. The master table every file declares (`declare`) and the master binding after the
//! last file (`bind`) live here too; the spine calls them.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (instance, `$mfactor`, R C L, behavioural); VD
//! cir-instance, cir-mfactor, dev-builtin-behavioral.
//!
//! Contents: `FEATURES`, `CODES`, `Masters`, `Pending`, `declare`, `bind`, `instance`
//! (crate-private)

use super::read::{self, Cst, Lower};
use super::write_models;
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY;

/// This area's rows, and the two the model and element writers push.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        write_models::UNSPELLED,
        ir::Info {
            title: "a name VACASK cannot spell",
            cite: "VD input-identifiers (a quoted name holds no white space)",
            explain: "VACASK reads a name as one token; a name holding white space or a line break has no spelling.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        write_models::LIBRARIES,
        ir::Info {
            title: "two device libraries of one family from files of different modules",
            cite: "RULINGS R17, R41; DIALECT-VACASK run mod (one module per shipped file)",
            explain: "A VACASK model line names one module; when the design loads a family from files of two modules, no module word states the family's model.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        write_models::LACKED,
        ir::Info {
            title: "a parameter the VACASK module does not have",
            cite: "INV-semantics 7.11 (VACASK refuses an unknown parameter); families/params.txt",
            explain: "The model or instance gives a parameter the shipped module VACASK would instantiate does not have; VACASK refuses the deck.",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// Every master every file declares, for an instance's kind before its master is lowered.
#[derive(Debug, Default)]
pub(super) struct Masters;

/// An element whose master binds after the last file is lowered.
#[derive(Clone, Copy, Debug)]
pub(super) struct Pending;

/// The masters of every file (S4.1 fills it).
#[allow(dead_code, reason = "the spine calls it once vacask-plan's hook lands")]
pub(super) fn declare(csts: &[(Cst<'_>, ir::ScopeId)]) -> Masters {
    let _ = csts;
    Masters
}

/// Binds every pending master (S4.1 fills it).
#[allow(dead_code, reason = "the spine calls it once vacask-plan's hook lands")]
pub(super) fn bind(b: &mut ir::Builder<'_>, pending: &[Pending], diags: &mut ir::Diags) {
    let _ = (b, pending, diags);
}

/// `name (nodes) master params`, and a behavioural source (no master).
///
/// Stub (S4.1): an `Unread` opaque of the statement.
pub(super) fn instance(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}
