//! `@if c` … `@elseif c` … `@else` … `@end` → one `If`, a branch per condition, each body read
//! into its branch's scope. Part of `read_structure`.
//!
//! Cites: VD cir-conditional; DIALECT-VACASK §3.1.
//!
//! Contents: `if_chain` (crate-private to the dialect)

use super::super::codes;
use super::super::expr_read::{self, Context};
use super::super::read::{self, Lower};
use super::closer_start;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// `@if c` … `@elseif c` … `@else` … `@end`: one `If` with a branch per condition.
pub(in crate::dialect::vacask) fn if_chain(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    let mut chain = vec![n];
    // Bounded by the node count: each step moves to a later branch.
    for _ in 0..ast.len() {
        match ast.next(*chain.last().expect("non-empty")) {
            Some(next) => chain.push(next),
            None => break,
        }
    }
    for &branch in &chain {
        if ast.tag(branch) != Tag::If {
            continue;
        }
        let Some(&cond) = ast.fields(branch).first() else {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(codes::PRIVATE));
        };
        if let Err(r) = expr_read::check_expr(l, cond, Context::Param) {
            return read::opaque(l, n, r.class, Some(r.code));
        }
    }
    let mut conds = Vec::with_capacity(chain.len());
    for &branch in &chain {
        conds.push(match l.cst.ast.tag(branch) {
            Tag::If => {
                let cond = l.cst.ast.fields(branch)[0];
                expr_read::expr(l, cond, Context::Param).expect("checked")
            }
            _ => ir::ExprId::NONE,
        });
    }
    let span = l.span(n);
    let (_, branches) = l.b.if_(l.scope, &conds, span);
    let scopes: Vec<ir::ScopeId> =
        l.b.design()
            .branches(branches)
            .iter()
            .map(|b| b.body)
            .collect();
    for (k, &branch) in chain.iter().enumerate() {
        let until = match chain.get(k + 1) {
            Some(&next) => Some(l.start(next)),
            None => closer_start(l, branch),
        };
        let body = l.cst.ast.body(branch).to_vec();
        l.body(scopes[k], &body, until);
    }
}
