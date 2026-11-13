//! The read spine's bind step (ruling R3): every row reference and every parameter read or user
//! call the areas queued, bound once every file is lowered, by ngspice's scoping: the nearest
//! definition outward, includes expanded (`visible::Visible`, R71). Split from `read.rs`.
//!
//! Contents: `pub(super) struct RowRef`, `pub(super) fn bind`, `pub(super) fn bind_rows`,
//! `pub(super) fn bind_reads`

use super::visible::Visible;
use crate::ir;

/// A row's reference waiting for the bind step ([`Lower::refer`]): its item, its slot in
/// `RowFacts::map_refs` order, the scope it is read from, the kind the slot expects, the name.
#[derive(Clone, Copy, Debug)]
pub(super) struct RowRef {
    /// The pushed item.
    pub(super) item: ir::ItemRef,
    /// Its reference's slot.
    pub(super) k: u32,
    /// Where the name is read.
    pub(super) scope: ir::ScopeId,
    /// The definition kind the slot expects.
    pub(super) kind: ir::DefKind,
    /// The name, as pushed.
    pub(super) name: ir::NameId,
}

/// The bind step: each queued row reference ([`bind_rows`]), then each pending parameter read
/// and user call ([`bind_reads`]).
pub(super) fn bind(
    b: &mut ir::Builder<'_>,
    pending: &[(ir::ExprId, ir::ScopeId)],
    measured: &[(usize, usize)],
    rows: &[RowRef],
) {
    let seen = Visible::new(b.design());
    bind_rows(b, &seen, rows);
    bind_reads(b, &seen, pending, measured);
}

/// The bind step's row half: each queued reference bound to the `kind` definition of its name
/// nearest its scope outward (`Design::lookup`), through `Builder::bind_ref`; left unresolved
/// when there is none (R4).
pub(super) fn bind_rows(b: &mut ir::Builder<'_>, seen: &Visible, rows: &[RowRef]) {
    for r in rows {
        let def = seen.lookup(b.design(), r.scope, r.kind, r.name);
        if !def.is_none() {
            b.bind_ref(r.item, r.k, def);
        }
    }
}

/// The bind step's read half: each pending parameter read and user call, still unresolved,
/// bound to the definition ngspice reads from its scope, the nearest enclosing one
/// (`Design::lookup`); a read inside a `measured` range (`Lower::measure_reads`) binds to a
/// measure of its name first, then to a parameter.
pub(super) fn bind_reads(
    b: &mut ir::Builder<'_>,
    seen: &Visible,
    pending: &[(ir::ExprId, ir::ScopeId)],
    measured: &[(usize, usize)],
) {
    for (k, &(e, scope)) in pending.iter().enumerate() {
        let (kind, r) = match b.design().expr(e) {
            ir::Expr::Param(r) => (ir::DefKind::Param, r),
            ir::Expr::UserCall(r, _) => (ir::DefKind::Func, r),
            other => unreachable!("only reads and calls are pending: {other:?}"),
        };
        if !r.def.is_none() {
            continue;
        }
        let in_measure = measured.iter().any(|&(a, z)| (a..z).contains(&k));
        let mut def = ir::DefId::NONE;
        if in_measure && kind == ir::DefKind::Param {
            def = seen.lookup(b.design(), scope, ir::DefKind::Measure, r.name);
        }
        if def.is_none() {
            def = seen.lookup(b.design(), scope, kind, r.name);
        }
        if !def.is_none() {
            b.resolve(e, def);
        }
    }
}
