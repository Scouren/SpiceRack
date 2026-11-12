//! The behavioural source `name (p n) v=|potential=|i=|flow=<expr>` → a `Behav` row (VD
//! dev-builtin-behavioral: `v=`/`potential=` a voltage, `i=`/`flow=` a current from p through
//! the source to n). A parameter read is `ToReal(p)` there (run b1: real-typed;
//! R40/A21, `expr_read`'s behavioural context). `expr=`, `declarations=`, `discipline=`, a second
//! parameter list: `Private` (§5 G14).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (behavioural); VD dev-builtin-behavioral.
//!
//! Contents: `source` (crate-private to the dialect)

use super::super::expr_read::{self, Context};
use super::super::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::Node;

/// `name (p n) v=|potential=|i=|flow=<expr>`.
pub(in crate::dialect::vacask) fn source(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    name: &str,
    nodes: &[String],
    entries: &[(String, u32)],
) {
    let output = match entries {
        [(key, _)] if key == "v" || key == "potential" => ir::controlled::OutKind::Voltage,
        [(key, _)] if key == "i" || key == "flow" => ir::controlled::OutKind::Current,
        _ => {
            return read::opaque(
                l,
                n,
                ir::OpaqueClass::Private,
                Some(super::super::codes::PRIVATE),
            );
        }
    };
    if nodes.len() != 2 {
        // "A behavioral source instance takes exactly two terminals" (VD).
        return read::opaque(
            l,
            n,
            ir::OpaqueClass::InvalidInSource,
            Some(super::super::codes::PRIVATE),
        );
    }
    let value = entries[0].1;
    let expr = match expr_read::expr(l, value, Context::Behavioural) {
        Ok(e) => e,
        Err(r) => return read::opaque(l, n, r.class, Some(r.code)),
    };
    let row = ir::controlled::Behav {
        expr,
        tc1: ir::ExprId::NONE,
        tc2: ir::ExprId::NONE,
        temp_abs: ir::ExprId::NONE,
        dtemp: ir::ExprId::NONE,
        rpar: ir::ExprId::NONE,
        output,
        tc_base: ir::passive::TcBase::Tnom,
        tc_law: ir::passive::TcLaw::Direct,
    };
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let head = l.b.head(name, &nodes, None, &[], ir::ExprId::NONE);
    let span = l.span(n);
    l.b.behav(l.scope, ir::ElemKind::Behav, head, row, span);
}
