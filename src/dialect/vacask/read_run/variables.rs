//! Circuit variables (R43, R63): a `var` statement is Private (P2307), so a statement whose
//! expression reads a name only a `var` of the file's control block assigns (and no top-level
//! parameter defines) is Private with it, under `READS_VARIABLE` (P2315). VACASK runs such a
//! statement (corpus demo/bsim3-ptm/characteristic.sim `var a=2 b=2`, then `options …
//! op_itl=(a+b*a)*b`); the IR has no definition for the name it reads.
//!
//! Contents: `check_value` (crate-private)

use super::{READS_VARIABLE, assign, head};
use crate::dialect::vacask::expr_read;
use crate::dialect::vacask::read::Lower;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// Checks the expression at `node` (`expr_read::check_expr`), and keeps the statement as text
/// (`READS_VARIABLE`) when it reads a circuit variable: a name a `var` of this file's control
/// block assigns and no top-level parameter defines (R63: what reads a Private statement's
/// name is Private with it).
pub(super) fn check_value(l: &Lower<'_, '_, '_>, node: u32) -> Result<(), expr_read::Refusal> {
    expr_read::check_expr(l, node, expr_read::Context::Param)?;
    let vars = variables(l);
    if vars.is_empty() {
        return Ok(());
    }
    let design = l.b.design();
    let top = design.file(l.cst.file).top;
    let param = |name: &str| {
        let id = design.find_name(name);
        !id.is_none() && !design.lookup(top, ir::DefKind::Param, id).is_none()
    };
    let read = expr_read::names_read(l, node);
    match read.iter().any(|n| vars.contains(n) && !param(n)) {
        true => Err(expr_read::Refusal {
            code: READS_VARIABLE,
            class: ir::OpaqueClass::Private,
        }),
        false => Ok(()),
    }
}

// ---- private ----
/// The names the `var` statements of this file's control block assign.
fn variables(l: &Lower<'_, '_, '_>) -> Vec<String> {
    let ast = &l.cst.ast;
    let Some(&block) = ast
        .fields(ast.root())
        .iter()
        .find(|&&s| ast.tag(Node(s)) == Tag::Control)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for &s in ast.body(Node(block)) {
        if head(l, Node(s)) == "var" {
            let names = ast.fields(Node(s)).iter().filter_map(|&f| assign(l, f));
            out.extend(names.map(|(k, _)| k));
        }
    }
    out
}
