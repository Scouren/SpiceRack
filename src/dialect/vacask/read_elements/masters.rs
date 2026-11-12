//! The masters every file declares (`declare`: one walk over every CST before lowering, so an
//! instance knows its master's kind when the master is defined later or in a library file),
//! the lookup by VACASK's rule (`find`: the own subcircuit, then the top; never an enclosing
//! subcircuit). The binding itself is the spine's (`bind::masters`).
//!
//! Cites: VD cir-masters ("Scope": the current scope, then the toplevel scope), cir-instance;
//! DIALECT-VACASK run m1 (an enclosing subckt's model: "Master 'res' not found."), run fwd (a
//! model defined after its instance); R3 (the reader binds), R33 (a branch's definitions are its
//! own).
//!
//! Contents: `Declared`, `Master`, `Found`, `declare`, `find`
//! (crate-private to the dialect)

use std::collections::HashMap;

use super::super::read::{self, Cst, Lower};
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// Every master every file declares, by the scope that holds it and its name.
#[derive(Debug, Default)]
pub(in crate::dialect::vacask) struct Declared {
    /// `(holder, name)` → the master; the first declaration wins.
    defs: HashMap<(Holder, String), Master>,
    /// Each subcircuit's enclosing holder (absent: the top).
    parent: HashMap<Holder, Holder>,
}

/// What a master is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::dialect::vacask) enum Master {
    /// A model of the module word.
    Model(String),
    /// A subcircuit: its ports and the names its `parameters` statements give.
    Subckt {
        /// Its terminal names.
        ports: usize,
        /// Every parameter name of its body's `parameters` statements.
        params: Vec<String>,
    },
}

/// A lookup's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::dialect::vacask) enum Found {
    /// Defined in the own subcircuit or at the top.
    Here(Master),
    /// Defined only in an enclosing subcircuit (VACASK does not look there: run m1).
    Enclosing,
    /// Defined in no read file.
    Nowhere,
}

/// Which scope holds a declaration: the top of every file (VACASK's include is textual), or a
/// subcircuit, by its file and its statement's start byte (the IR item's span start).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Holder {
    Top,
    Subckt(ir::FileId, u32),
}

/// The masters of every file.
pub(in crate::dialect::vacask) fn declare(csts: &[(Cst<'_>, ir::ScopeId)]) -> Declared {
    let mut out = Declared::default();
    for (cst, _) in csts {
        let ast = &cst.ast;
        let mut todo: Vec<(u32, Holder)> = ast
            .fields(ast.root())
            .iter()
            .map(|&s| (s, Holder::Top))
            .collect();
        // Bounded: each node is taken once (the statement tree is acyclic).
        while let Some((s, holder)) = todo.pop() {
            let n = Node(s);
            let fields = ast.fields(n);
            match ast.tag(n) {
                Tag::Model => {
                    if let [name, module, ..] = fields {
                        let key = (holder, read::field_name(cst, *name));
                        let master = Master::Model(read::field_name(cst, *module));
                        out.defs.entry(key).or_insert(master);
                    }
                }
                Tag::Subckt => {
                    let Some((&name, ports)) = fields.split_first() else {
                        continue;
                    };
                    let start = cst.tokens.start(ast.main_token(n)) - cst.shift;
                    let own = Holder::Subckt(cst.file, start);
                    out.parent.insert(own, holder);
                    let params = ast
                        .body(n)
                        .iter()
                        .filter(|&&b| ast.tag(Node(b)) == Tag::Param)
                        .flat_map(|&b| ast.fields(Node(b)).iter())
                        .filter(|&&f| ast.tag(Node(f)) == Tag::Assign)
                        .map(|&f| read::field_name(cst, ast.lhs(Node(f))))
                        .collect();
                    let master = Master::Subckt {
                        ports: ports.len(),
                        params,
                    };
                    let key = (holder, read::field_name(cst, name));
                    out.defs.entry(key).or_insert(master);
                    todo.extend(ast.body(n).iter().map(|&b| (b, own)));
                }
                Tag::Section | Tag::If | Tag::Else => {
                    todo.extend(ast.body(n).iter().map(|&b| (b, holder)));
                    if let Some(next) = ast.next(n) {
                        todo.push((next.0, holder));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The master `name` as seen from the current scope: the own subcircuit's, else the top's.
pub(in crate::dialect::vacask) fn find(
    l: &Lower<'_, '_, '_>,
    declared: &Declared,
    name: &str,
) -> Found {
    let own = holder(l);
    let at = |h: Holder| declared.defs.get(&(h, name.to_string())).cloned();
    if let Some(m) = at(own).or_else(|| at(Holder::Top)) {
        return Found::Here(m);
    }
    let mut up = declared.parent.get(&own).copied();
    // Bounded by the subcircuit count: each step moves to an enclosing one.
    for _ in 0..=declared.parent.len() {
        match up {
            Some(h @ Holder::Subckt(..)) if at(h).is_some() => return Found::Enclosing,
            Some(h @ Holder::Subckt(..)) => up = declared.parent.get(&h).copied(),
            _ => break,
        }
    }
    Found::Nowhere
}

// ---- private ----

/// The holder of the current scope: its subcircuit's, or the top.
fn holder(l: &Lower<'_, '_, '_>) -> Holder {
    let design = l.b.design();
    let mut at = l.scope;
    // Bounded: each step moves to a parent; the scope tree is acyclic (`ir::check`).
    for _ in 0..1 << 16 {
        let s = design.scope(at);
        match s.kind {
            ir::ScopeKind::Subckt => {
                let span = design.span_of(s.owner);
                return Holder::Subckt(span.file, span.start);
            }
            ir::ScopeKind::File => return Holder::Top,
            _ if s.parent.is_none() => return Holder::Top,
            _ => at = s.parent,
        }
    }
    Holder::Top
}
