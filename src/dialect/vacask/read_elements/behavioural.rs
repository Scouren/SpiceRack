//! The behavioural source `name (p n) v=|potential=|i=|flow=<expr>` → a `Behav` row (VD
//! dev-builtin-behavioral: `v=`/`potential=` a voltage, `i=`/`flow=` a current from p through
//! the source to n). One that reads a parameter waits for A21's `ToReal` (run b1: a parameter is
//! real-typed there; R41 Q1): `Unread`. `expr=`, `declarations=`, `discipline=`, a second
//! parameter list: `Private` (§5 G14).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (behavioural); VD dev-builtin-behavioral.
//!
//! Contents: `source` (crate-private to the dialect)

use super::super::expr_read::{self, Context};
use super::super::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::{NONE, Node, Tag};

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
    if nodes.len() != 2 || reads_name(l, entries[0].1) {
        // A parameter read is real-typed there (run b1): `Unread` until A21's `ToReal`.
        return read::unread(l, n);
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
    };
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let head = l.b.head(name, &nodes, None, &[], ir::ExprId::NONE);
    let span = l.span(n);
    l.b.behav(l.scope, ir::ElemKind::Behav, head, row, span);
}

// ---- private ----

/// Whether the expression at `node` names a parameter (an identifier that is no constant and
/// no special leaf).
fn reads_name(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    let ast = &l.cst.ast;
    let mut todo = vec![node];
    // Bounded: each node of the (acyclic) expression tree is taken once.
    while let Some(x) = todo.pop() {
        if x == NONE {
            continue;
        }
        let n = Node(x);
        match ast.tag(n) {
            Tag::Ident | Tag::Word => {
                let name = match ast.tag(n) {
                    Tag::Word => ast.name(crate::syntax::ast::Name(ast.rhs(n))),
                    _ => ast.name(crate::syntax::ast::Name(ast.lhs(n))),
                };
                let constant = super::super::tables::CONSTANTS
                    .iter()
                    .any(|(c, _)| *c == name);
                if !constant && !name.starts_with('$') {
                    return true;
                }
            }
            Tag::Unary | Tag::Paren | Tag::Delimited => todo.push(ast.lhs(n)),
            Tag::Binary => todo.extend([ast.lhs(n), ast.rhs(n)]),
            Tag::Ternary => {
                let at = ast.rhs(n);
                todo.extend([ast.lhs(n), ast.extra(at), ast.extra(at + 1)]);
            }
            Tag::Call | Tag::Probe => {
                // `v(a)`, `i(x)` name nodes and instances, not parameters.
                let callee = ast.name(crate::syntax::ast::Name(ast.lhs(n)));
                if callee != "v" && callee != "i" {
                    let at = ast.rhs(n);
                    todo.extend((0..ast.extra(at + 1)).map(|k| ast.extra(at + 2 + k)));
                }
            }
            _ => {}
        }
    }
    false
}
