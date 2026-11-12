//! Binding by VACASK's rule (RUN n1, m1; VD cir-hier "Scope", cir-masters): a name resolves in
//! its own subcircuit (its branches and sections), then the top of its file, then the top of every
//! other file (VACASK's include is textual); never in an enclosing subcircuit, and a name only an
//! enclosing subcircuit defines stays unbound and is refused (`R2003`). Parameter reads
//! (`reads`) and element masters (`masters`) are bound after every file is read, since a
//! forward reference is legal (RUN n5). And the instance head every element area reads
//! (`head`): the exact name, the terminals with the ground names as `0`, the master's name, the
//! argument fields.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.1; `rewrite/S4-AREAS.md` (shared helpers).
//!
//! Contents: `Master`, `Head`, `head`, `reads`, `masters` (crate-private)

use super::read::{self, Lower};
use super::read_structure;
use crate::ir;
use crate::syntax::ast::{NONE, Node};

/// An element whose master to bind: the element, the scope its master is read from and its
/// statement.
#[derive(Clone, Copy, Debug)]
pub(super) struct Master {
    /// The element.
    pub(super) item: ir::ItemRef,
    /// The scope the name is read from.
    pub(super) scope: ir::ScopeId,
    /// The statement.
    pub(super) span: ir::Span,
}

/// An instance statement's head: `name (nodes) master args`.
#[allow(
    dead_code,
    reason = "read_elements (S4.1) and read_sources (S4.2) read it"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Head {
    /// The whole name, exact bytes (R23, RUN n4).
    pub(super) name: String,
    /// The terminals in order, a ground name (`0`, a `ground` directive's) as `0`.
    pub(super) terminals: Vec<String>,
    /// The master's name; `None` for a behavioural source.
    pub(super) master: Option<String>,
    /// The argument fields after the master (positional values and `key=value`s).
    pub(super) args: Vec<u32>,
}

/// The deepest scope chain `region` walks (blocks nest at most this deep in a parsed file).
const SCOPE_DEPTH_MAX: usize = 1 << 16;

/// The head of instance statement `n`.
#[allow(
    dead_code,
    reason = "read_elements (S4.1) and read_sources (S4.2) call it"
)]
pub(super) fn head(l: &Lower<'_, '_, '_>, n: Node) -> Head {
    let ast = &l.cst.ast;
    let h = ast.header(n);
    let terminals = h
        .nodes
        .iter()
        .map(|&t| {
            let name = read::field_name(l.cst, t);
            match l.grounds.contains(&name) {
                true => super::name::GROUND.to_string(),
                false => name,
            }
        })
        .collect();
    Head {
        name: read::field_name(l.cst, h.name),
        terminals,
        master: (h.master != NONE).then(|| read::field_name(l.cst, h.master)),
        args: ast.fields(n).to_vec(),
    }
}

/// Binds every recorded master by VACASK's rule; one only an enclosing subcircuit defines is
/// refused (`R2003`).
pub(super) fn masters(b: &mut ir::Builder<'_>, masters: &[Master], diags: &mut ir::Diags) {
    for m in masters {
        let row: ir::Element = b.design().row(m.item.index);
        let (Some(kind), false) = (row.kind.master_kind(), row.model.name.is_none()) else {
            continue;
        };
        let name = row.model.name;
        let (def, enclosing) = find(b.design(), m.scope, kind, name);
        if enclosing {
            diags.push(read_structure::ENCLOSING, m.span, 0);
        }
        if !def.is_none() {
            b.bind_ref(m.item, 0, def);
        }
    }
}

/// Binds every recorded parameter read by VACASK's rule: the own subcircuit (its branches and
/// sections), then the top of its file, then the top of every other file (VACASK's include is
/// textual). A read only an enclosing subcircuit could bind stays unbound, refused (`R2003`).
pub(super) fn reads(
    b: &mut ir::Builder<'_>,
    reads: &[(ir::ExprId, ir::ScopeId, ir::Span)],
    diags: &mut ir::Diags,
) {
    for &(e, scope, span) in reads {
        let design = b.design();
        let ir::Expr::Param(r) = design.expr(e) else {
            unreachable!("a recorded read is a parameter read");
        };
        let (def, enclosing) = find(design, scope, ir::DefKind::Param, r.name);
        if enclosing {
            diags.push(read_structure::ENCLOSING, span, 0);
        }
        if !def.is_none() {
            b.resolve(e, def);
        }
    }
}

// ---- private ----

/// The definition of `kind` named `name` VACASK binds a read from `scope` to, and whether only an
/// enclosing subcircuit defines it.
fn find(
    design: &ir::Design,
    scope: ir::ScopeId,
    kind: ir::DefKind,
    name: ir::NameId,
) -> (ir::DefId, bool) {
    let (own, top) = region(design, scope);
    let found = design.lookup(scope, kind, name);
    let in_own = !found.is_none() && {
        let at = design.def(found).scope;
        own.contains(&at) || design.scope(at).kind == ir::ScopeKind::File
    };
    if in_own {
        return (found, false);
    }
    let at_top = design.lookup(top, kind, name);
    let def = match at_top.is_none() {
        false => at_top,
        true => design
            .files()
            .map(|f| design.file(f).top)
            .filter(|&t| t != top)
            .map(|t| design.lookup(t, kind, name))
            .find(|d| !d.is_none())
            .unwrap_or(ir::DefId::NONE),
    };
    (def, !found.is_none() && def.is_none())
}

/// The scopes of `scope`'s own region (it and its ancestors up to its subcircuit's body or its
/// file's top) and its file's top scope.
fn region(design: &ir::Design, scope: ir::ScopeId) -> (Vec<ir::ScopeId>, ir::ScopeId) {
    let mut own = Vec::new();
    let mut at = scope;
    // Bounded: each step moves to a parent, and the scope tree is acyclic (`ir::check`).
    for _ in 0..SCOPE_DEPTH_MAX {
        own.push(at);
        let s = design.scope(at);
        if matches!(s.kind, ir::ScopeKind::Subckt | ir::ScopeKind::File) || s.parent.is_none() {
            break;
        }
        at = s.parent;
    }
    let top = design.file(design.scope(scope).file).top;
    (own, top)
}
