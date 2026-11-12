//! The masters every file declares (`declare`: one walk over every CST before lowering, so an
//! instance knows its master's kind when the master is defined later or in a library file),
//! the lookup by VACASK's rule (`find`: the own subcircuit, then the top; never an enclosing
//! subcircuit) and the binding of each element's master after the last file (`bind`).
//!
//! Cites: VD cir-masters ("Scope": the current scope, then the toplevel scope), cir-instance;
//! DIALECT-VACASK run m1 (an enclosing subckt's model: "Master 'res' not found."), run fwd (a
//! model defined after its instance); R3 (the reader binds), R33 (a branch's definitions are its
//! own).
//!
//! Contents: `Masters`, `Master`, `Pending`, `Found`, `declare`, `find`, `bind`
//! (crate-private to the dialect)

use std::collections::HashMap;

use super::super::read::{self, Cst, Lower};
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// Every master every file declares, by the scope that holds it and its name.
#[derive(Debug, Default)]
pub(in crate::dialect::vacask) struct Masters {
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

/// An element whose master binds after the last file is lowered.
#[derive(Clone, Copy, Debug)]
pub(in crate::dialect::vacask) struct Pending {
    /// The element.
    pub(in crate::dialect::vacask) item: ir::ItemRef,
    /// The scope it stands in.
    pub(in crate::dialect::vacask) scope: ir::ScopeId,
    /// What its master is.
    pub(in crate::dialect::vacask) kind: ir::DefKind,
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
pub(in crate::dialect::vacask) fn declare(csts: &[(Cst<'_>, ir::ScopeId)]) -> Masters {
    let mut out = Masters::default();
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
pub(in crate::dialect::vacask) fn find(l: &Lower<'_, '_, '_>, name: &str) -> Found {
    let own = holder(l);
    let at = |h: Holder| l.masters.defs.get(&(h, name.to_string())).cloned();
    if let Some(m) = at(own).or_else(|| at(Holder::Top)) {
        return Found::Here(m);
    }
    let mut up = l.masters.parent.get(&own).copied();
    // Bounded by the subcircuit count: each step moves to an enclosing one.
    for _ in 0..=l.masters.parent.len() {
        match up {
            Some(h @ Holder::Subckt(..)) if at(h).is_some() => return Found::Enclosing,
            Some(h @ Holder::Subckt(..)) => up = l.masters.parent.get(&h).copied(),
            _ => break,
        }
    }
    Found::Nowhere
}

/// Binds each pending element's master by VACASK's rule: defined in the element's own region
/// (its scope up to its subcircuit's body or its file's top), else in exactly one top-level
/// scope of any file (a file's top or a section at it). Two top-level candidates (sections of a
/// corner library) or none: left named and unbound (R33).
pub(in crate::dialect::vacask) fn bind(b: &mut ir::Builder<'_>, pending: &[Pending]) {
    for p in pending {
        let design = b.design();
        let row: ir::Element = design.row(p.item.index);
        let name = row.model.name;
        let here = |s: ir::ScopeId| {
            let d = design.lookup(s, p.kind, name);
            (!d.is_none() && design.def(d).scope == s).then_some(d)
        };
        let mut own = None;
        let mut at = p.scope;
        // Bounded: each step moves to a parent; the scope tree is acyclic (`ir::check`).
        for _ in 0..1 << 16 {
            own = own.or_else(|| here(at));
            let s = design.scope(at);
            if matches!(s.kind, ir::ScopeKind::Subckt | ir::ScopeKind::File) || s.parent.is_none()
            {
                break;
            }
            at = s.parent;
        }
        let def = own.or_else(|| {
            let tops: Vec<ir::DefId> = design
                .definitions()
                .iter()
                .enumerate()
                .filter(|(_, d)| d.kind == p.kind && d.name == name && top_level(design, d.scope))
                .map(|(k, _)| ir::DefId(u32::try_from(k).expect("fewer than 2^32 defs")))
                .collect();
            match tops.as_slice() {
                [one] => Some(*one),
                _ => None,
            }
        });
        if let Some(def) = def {
            b.bind_ref(p.item, 0, def);
        }
    }
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

/// Whether `scope` is a file's top or a section (of sections) at it.
fn top_level(design: &ir::Design, scope: ir::ScopeId) -> bool {
    let mut at = scope;
    // Bounded: each step moves to a parent; the scope tree is acyclic (`ir::check`).
    for _ in 0..1 << 16 {
        let s = design.scope(at);
        match s.kind {
            ir::ScopeKind::File => return true,
            ir::ScopeKind::Section => at = s.parent,
            _ => return false,
        }
    }
    false
}
