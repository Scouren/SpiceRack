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
//! And each subcircuit instance on a cycle of the instance graph (`Declared::recursion`: VACASK's
//! "Hierarchical recursion detected.", settle b_cycle_self, b_cycle_two; one nothing reaches
//! from the top level runs, b_cycle_unused).
//!
//! Contents: `Declared`, `Master`, `Found`, `declare`, `find`, `Aimed`, `Declared::aim`,
//! `Declared::by_section`, `Declared::recursion` (crate-private to the dialect)

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
    /// Each subcircuit instance on a cycle of the instance graph → whether the top level (or an
    /// `elaborate`) reaches the cycle.
    recursive: HashMap<(Holder, String), bool>,
}

/// What a run statement's instance parameter aims at ([`Declared::aim`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::dialect::vacask) enum Aimed {
    /// No file declares the instance (settle run/nosuch_alter: "Instance not found."), or a
    /// shipped module's instance has no such parameter (dev_nosuch).
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

/// An edge of the instance graph: its holder's node, its body's node, the instance (`None`: an
/// `elaborate`'s edge from the top).
type Edge<'a> = (usize, usize, Option<&'a (Holder, String)>);

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
    out.recursive = recursive(&out);
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
        // A value parameter by any of its names (`sp_resistor`'s `r` is its `resistance`).
        let same = |a: &str| {
            super::super::families::canonical(word, param)
                == super::super::families::canonical(word, a)
        };
        let param = match () {
            _ if same("r") => "r",
            _ if same("c") => "c",
            _ if same("l") => "l",
            _ => param,
        };
        match (word, param, value) {
            ("vsource" | "isource", "dc", _) => match wave.as_deref() {
                None | Some("dc") => Aimed::SourceDc,
                Some(_) => Aimed::SourceWave,
            },
            (_, "r", Some(ir::ElemKind::Resistor))
            | (_, "c", Some(ir::ElemKind::Capacitor))
            | (_, "l", Some(ir::ElemKind::Inductor)) => Aimed::ElementValue,
            // A shipped module's instance parameters are known (`families/params.txt`): another
            // name is VACASK's error (settle run/dev_nosuch: "Parameter 'zz' not found."). A
            // loaded module's are not, so its names pass.
            _ if !matches!(
                super::super::families::instance_name_in(word, param),
                super::super::families::Name::Ir(_)
            ) =>
            {
                Aimed::Missing
            }
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

    /// Whether the subcircuit instance `name` of the current scope closes a cycle of the instance
    /// graph (`None`: it does not), and if so whether the top level or an `elaborate` reaches it.
    pub(in crate::dialect::vacask) fn recursion(
        &self,
        l: &Lower<'_, '_, '_>,
        name: &str,
    ) -> Option<bool> {
        self.recursive.get(&(holder(l), name.to_string())).copied()
    }

    /// The body `master` names from `at`: the holder's own subcircuit, then the top's.
    fn body(&self, at: Holder, master: &str) -> Option<Holder> {
        [at, Holder::Top]
            .iter()
            .find_map(|&h| self.bodies.get(&(h, master.to_string())).copied())
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

/// Each instance on a cycle of `d`'s instance graph (holders as nodes, an instance an edge from
/// its holder to its master's body; the top level's edges to each `elaborate`d body too) → whether
/// the top reaches its holder. An edge is on a cycle when both ends share a strongly connected
/// component (Kosaraju, two explicit-stack walks).
fn recursive(d: &Declared) -> HashMap<(Holder, String), bool> {
    let mut index: HashMap<Holder, usize> = HashMap::from([(Holder::Top, 0)]);
    let mut nodes = vec![Holder::Top];
    let mut id = |h: Holder| {
        *index.entry(h).or_insert_with(|| {
            nodes.push(h);
            nodes.len() - 1
        })
    };
    // (from, to, the instance; `None` for an `elaborate`'s edge)
    let mut edges: Vec<Edge<'_>> = Vec::new();
    for (key, (master, _)) in &d.instances {
        if let Some(body) = master.as_deref().and_then(|m| d.body(key.0, m)) {
            edges.push((id(key.0), id(body), Some(key)));
        }
    }
    let elaborated = d.elaborated.iter().flat_map(|(_, _, subs)| subs);
    for body in elaborated.filter_map(|s| d.body(Holder::Top, s)) {
        edges.push((0, id(body), None));
    }
    let n = nodes.len();
    let (mut adj, mut radj) = (vec![Vec::new(); n], vec![Vec::new(); n]);
    for &(u, w, _) in &edges {
        adj[u].push(w);
        radj[w].push(u);
    }
    // First walk: the finish order. Bounded: each node is entered once, each edge read once.
    let (mut seen, mut order) = (vec![false; n], Vec::with_capacity(n));
    for s in 0..n {
        if std::mem::replace(&mut seen[s], true) {
            continue;
        }
        let mut stack = vec![(s, 0usize)];
        while let Some((v, k)) = stack.pop() {
            match adj[v].get(k) {
                Some(&w) => {
                    stack.push((v, k + 1));
                    if !std::mem::replace(&mut seen[w], true) {
                        stack.push((w, 0));
                    }
                }
                None => order.push(v),
            }
        }
    }
    assert!(order.len() == n, "every node finished once");
    // Second walk, on the reversed graph in reverse finish order: the components.
    let mut comp = vec![usize::MAX; n];
    for (c, &s) in order.iter().rev().enumerate() {
        if comp[s] != usize::MAX {
            continue;
        }
        comp[s] = c;
        let mut stack = vec![s];
        while let Some(v) = stack.pop() {
            for &w in &radj[v] {
                if comp[w] == usize::MAX {
                    comp[w] = c;
                    stack.push(w);
                }
            }
        }
    }
    // What the top reaches. Bounded: each node is pushed once.
    let mut reached = vec![false; n];
    reached[0] = true;
    let mut stack = vec![0];
    while let Some(v) = stack.pop() {
        for &w in &adj[v] {
            if !std::mem::replace(&mut reached[w], true) {
                stack.push(w);
            }
        }
    }
    edges
        .into_iter()
        .filter(|&(u, w, _)| comp[u] == comp[w])
        .filter_map(|(u, _, key)| key.map(|k| (k.clone(), reached[u])))
        .collect()
}

/// The last part of `path` (after the last `/`).
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
