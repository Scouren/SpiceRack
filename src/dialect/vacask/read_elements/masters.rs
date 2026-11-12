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
//! And each instance's master and `type` word, so a `sweep` can tell an independent source's DC
//! value from a device parameter (R64: `Aimed`, `Declared::aim`, `Declared::by_section`).
//!
//! Contents: `Declared`, `Master`, `Found`, `declare`, `find`, `Aimed`, `Declared::aim`, `Declared::by_section`
//! (crate-private to the dialect)

use std::collections::{HashMap, HashSet};

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
    /// `(holder, name)` → a subcircuit's own holder; the first declaration wins.
    bodies: HashMap<(Holder, String), Holder>,
    /// `(holder, instance name)` → its master's name (`None`: a behavioural source) and its
    /// `type="…"` word, if given.
    instances: HashMap<(Holder, String), Inst>,
    /// Each `elaborate circuit("…")` of a control block, in order: its file, its start byte and
    /// the subcircuits it names, whose instances stand at the top level from there to the next
    /// one (VD cir-elaboration; corpus demo/bsim3-ptm/amp.sim sweeps `vin` of `namp`).
    elaborated: Vec<(ir::FileId, u32, Vec<String>)>,
    /// The file names (the path's last part) some `include "…" section=…` names: such a file is
    /// read by section, so its text outside every section is skipped (settle
    /// star_outside_section); a file included whole reads it all (settle plain_include).
    section_included: HashSet<String>,
}

/// What a run statement's instance parameter aims at ([`Declared::aim`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::dialect::vacask) enum Aimed {
    /// No file declares the instance (settle run/nosuch_alter: "Instance not found.").
    Missing,
    /// The `dc` of a source of type dc: its DC value (R64; settle run/sweep_source).
    SourceDc,
    /// The `dc` of a waveform source, which VACASK ignores (the same deck's negative row).
    SourceWave,
    /// The value parameter of a resistor, capacitor or inductor (R64b).
    ElementValue,
    /// Any other parameter of a device.
    Device,
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

/// An instance's master name (`None`: a behavioural source) and its `type="…"` word.
type Inst = (Option<String>, Option<String>);

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
            match ast.tag(n) {
                Tag::Instance => {
                    let h = ast.header(n);
                    if h.name != crate::syntax::ast::NONE {
                        let key = (holder, read::field_name(cst, h.name));
                        let wave = ast
                            .fields(n)
                            .iter()
                            .map(|&f| Node(f))
                            .filter(|&f| ast.tag(f) == Tag::Assign)
                            .find(|&f| read::field_name(cst, ast.lhs(f)) == "type")
                            .map(|f| read::field_name(cst, ast.rhs(f)));
                        let master = (h.master != crate::syntax::ast::NONE)
                            .then(|| read::field_name(cst, h.master));
                        out.instances.entry(key).or_insert((master, wave));
                    }
                }
                Tag::Model => {
                    if let [name, module, ..] = ast.fields(n) {
                        let key = (holder, read::field_name(cst, *name));
                        let master = Master::Model(read::field_name(cst, *module));
                        out.defs.entry(key).or_insert(master);
                    }
                }
                Tag::Subckt => {
                    let Some((&name, ports)) = ast.fields(n).split_first() else {
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
                    out.bodies.entry(key.clone()).or_insert(own);
                    out.defs.entry(key).or_insert(master);
                    todo.extend(ast.body(n).iter().map(|&b| (b, own)));
                }
                Tag::Include => {
                    let fields = ast.fields(n);
                    let by_section = fields.iter().any(|&f| {
                        ast.tag(Node(f)) == Tag::Assign
                            && read::field_name(cst, ast.lhs(Node(f))) == "section"
                    });
                    if let (Some(&path), true) = (fields.first(), by_section) {
                        let path = read::field_name(cst, path);
                        out.section_included.insert(file_name(&path).to_string());
                    }
                }
                Tag::Control => {
                    for &b in ast.body(n) {
                        let b = Node(b);
                        let head = cst.tokens.text(ast.main_token(b));
                        if ast.tag(b) == Tag::Card && head == "elaborate" {
                            let names = ast.fields(b).iter().skip(1);
                            let strings = names.filter(|&&f| ast.tag(Node(f)) == Tag::Str);
                            let subs = strings.map(|&f| read::field_name(cst, f)).collect();
                            let start = cst.tokens.start(ast.main_token(b)) - cst.shift;
                            out.elaborated.push((cst.file, start, subs));
                        }
                    }
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

impl Declared {
    /// What a run statement's `instance="path:element" parameter="param"` at byte `pos` of
    /// `file` aims at (R64, R64b, R70): no instance any file declares; the DC value of a
    /// `vsource`/`isource` of type dc (one of a waveform type, whose `dc` VACASK ignores); the
    /// value of a resistor, capacitor or inductor (`r`, `c`, `l`); or another device parameter.
    /// Each master found by VACASK's rule (the holder's own, then the top's); at the top level
    /// the instances of the subcircuits the last `elaborate circuit(…)` before `pos` names too.
    pub(in crate::dialect::vacask) fn aim(
        &self,
        path: &[String],
        element: &str,
        param: &str,
        (file, pos): (ir::FileId, u32),
    ) -> Aimed {
        let Some((at, (master, wave))) = self.instance(path, element, (file, pos)) else {
            return Aimed::Missing;
        };
        let word = match master.as_deref().and_then(|m| self.master(at, m)) {
            Some(Master::Model(word)) => word.as_str(),
            _ => return Aimed::Device,
        };
        let value =
            super::super::families::module(word).map(|m| super::super::families::kind(m.family));
        match (word, param, value) {
            ("vsource" | "isource", "dc", _) => match wave.as_deref() {
                None | Some("dc") => Aimed::SourceDc,
                Some(_) => Aimed::SourceWave,
            },
            (_, "r", Some(ir::ElemKind::Resistor))
            | (_, "c", Some(ir::ElemKind::Capacitor))
            | (_, "l", Some(ir::ElemKind::Inductor)) => Aimed::ElementValue,
            _ => Aimed::Device,
        }
    }

    /// The holder and entry of instance `path:element`: each path step an instance whose master
    /// is a subcircuit, found by VACASK's rule; a top-level name also among the instances of the
    /// subcircuits the epoch's `elaborate` names (the last one before `pos` in `file`).
    fn instance(
        &self,
        path: &[String],
        element: &str,
        (file, pos): (ir::FileId, u32),
    ) -> Option<(Holder, &Inst)> {
        let mut at = Holder::Top;
        // Bounded by the path's length.
        for inst in path {
            let (m, _) = self.instances.get(&(at, inst.clone()))?;
            let m = m.as_deref()?;
            at = *[at, Holder::Top]
                .iter()
                .find_map(|&h| self.bodies.get(&(h, m.to_string())))?;
        }
        if let Some(entry) = self.instances.get(&(at, element.to_string())) {
            return Some((at, entry));
        }
        if !path.is_empty() || at != Holder::Top {
            return None;
        }
        let epoch = self
            .elaborated
            .iter()
            .rfind(|(f, start, _)| *f == file && *start < pos)?;
        epoch.2.iter().find_map(|sub| {
            let own = *self.bodies.get(&(Holder::Top, sub.clone()))?;
            self.instances
                .get(&(own, element.to_string()))
                .map(|e| (own, e))
        })
    }

    /// Whether some include names the file at `path` with `section=` (matched by its last part:
    /// ponytail: two libraries of one name in two directories count as one).
    pub(in crate::dialect::vacask) fn by_section(&self, path: &str) -> bool {
        self.section_included.contains(file_name(path))
    }

    /// The master `name` seen from `at`: the holder's own, then the top's.
    fn master(&self, at: Holder, name: &str) -> Option<&Master> {
        [at, Holder::Top]
            .iter()
            .find_map(|&h| self.defs.get(&(h, name.to_string())))
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

/// The last part of `path` (after the last `/`).
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
