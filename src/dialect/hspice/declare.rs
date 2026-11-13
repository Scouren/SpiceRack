//! The reader's declare step: one file's CST → (1) which definition of each name ngspice keeps
//! in its block (the last `.param`, INV 3.01; the first of any other kind), and (2) each
//! definition's class (ruling R14): `Circuit` when its text, or a parameter or function it reads
//! (transitively, nearest block outward), holds `v(`, `i(`, `time`, `hertz` or `ddt(`; `Temp`
//! when one of them holds `temper`; else `Constant`. ngspice turns every parameter that depends
//! on `temper` into a function the B-source parser evaluates (`inpcom.c`
//! `inp_fix_temper_in_param`), so the class picks the evaluator: an expression of class `Temp`
//! or `Circuit` is the B-source parser's, wherever it stands.
//!
//! Cites: ngspice-45 `inpcom.c` (`inp_fix_temper_in_param`, `inp_sort_params`); `xpressn.c`
//! (numparam's lookup by block); `rewrite/RULINGS.md` R14; `rewrite/INV-semantics.md` 2.06, 3.01,
//! 3.06; IR-KERNEL A10 (a head default is read from the subcircuit's body).
//!
//! Contents: `pub(super) struct Declared`, `pub(super) enum Class`,
//! `pub(super) fn declare(&Ast, &Tokens) -> Declared`, `Declared::wins`, `Declared::redefault`, `Declared::head`, `Declared::top_head`, `Declared::arity`, `Declared::class`,
//! `pub(super) fn cycles(&[Declared], usize, Node, &str) -> bool`,
//! `pub(super) fn assigns(&Ast, &Tokens, Node) -> Vec<(String, String)>`

use std::collections::HashMap;

use super::cst::{body, joined, next, rest};
use super::expr_lex;
use super::name;
use super::read::Def;
use super::read_structure;
use crate::syntax::ast::{Ast, NONE, Node, Tag};
use crate::syntax::token::Tokens;

/// What the declare step knows of one file.
#[derive(Debug, Default)]
pub(super) struct Declared {
    /// `(block, kind, folded name)` → the winning `(statement, field)`.
    winners: HashMap<(u32, Def, String), (u32, usize)>,
    /// Each block's enclosing block (the root has none).
    parent: HashMap<u32, u32>,
    /// `(block, Param | Func, folded name)` → its class.
    classes: HashMap<(u32, Def, String), Class>,
    /// `(block, folded subcircuit name)` → its port count and its head parameters, folded (the
    /// first definition).
    heads: HashMap<(u32, String), (usize, Vec<String>)>,
    /// `(block, folded .func name)` → its formal count (the first definition, the winner).
    arity: HashMap<(u32, String), usize>,
    /// The file's root block.
    root: u32,
    /// `(block, folded subcircuit name)` → its `.subckt` node (the first definition).
    nodes: HashMap<(u32, String), u32>,
    /// Each `.subckt` node → the masters (folded) of the X cards directly in its body.
    masters: HashMap<u32, Vec<String>>,
    /// `(.subckt node, folded head parameter)` → the value of the last `.param` of its body
    /// naming it: ngspice's new default (settle `dup_formal.sp`).
    redefaults: HashMap<(u32, String), String>,
}

/// How much of a run an expression reads (R14), ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Class {
    /// Nothing that varies.
    Constant,
    /// The temperature, directly or through a definition.
    Temp,
    /// The circuit or the time, directly or through a definition.
    Circuit,
}

/// One definition's text, for the class fixpoint.
struct Text {
    block: u32,
    def: Def,
    name: String,
    text: String,
    formals: Vec<String>,
}

/// The declare step over one file.
pub(super) fn declare(ast: &Ast<'_>, toks: &Tokens<'_>) -> Declared {
    let mut d = Declared {
        root: ast.root().0,
        ..Declared::default()
    };
    let mut texts: Vec<Text> = Vec::new();
    let mut stack: Vec<Node> = vec![ast.root()];
    // Bounded: each block is pushed once.
    while let Some(container) = stack.pop() {
        // nothing after a deck's first `.alter` defines a name: its blocks are reruns
        let items: &[u32] = if container == ast.root() {
            &ast.fields(ast.root())[..super::read_plan::first_alter(ast, toks)]
        } else {
            body(ast, container)
        };
        let block = container.0;
        let mut head: Vec<String> = Vec::new();
        if ast.tag(container) == Tag::Subckt {
            for (k, v) in assigns(ast, toks, container) {
                texts.push(text(block, Def::Param, &k, v, Vec::new()));
                head.push(k);
            }
        }
        for &s in items {
            let s = Node(s);
            let mut put = |def: Def, name: &str, k: usize, last: bool| {
                let key = (block, def, name::key(name));
                if last || !d.winners.contains_key(&key) {
                    d.winners.insert(key, (s.0, k));
                }
            };
            match ast.tag(s) {
                // `.param f(x)='…'` defines a function (settle param_func.sp)
                Tag::Param if read_structure::param_is_func(&rest(toks, ast, s)) => {
                    let line = rest(toks, ast, s);
                    if let Some((name, formals, body)) = read_structure::func_parts(&line) {
                        put(Def::Func, &name, 0, true);
                        d.arity.insert((block, name::key(&name)), formals.len());
                        texts.push(text(block, Def::Func, &name, body.to_string(), formals));
                    }
                }
                Tag::Param => {
                    for (k, (name, value)) in assigns(ast, toks, s).into_iter().enumerate() {
                        let clash = head.iter().any(|h| name::key(h) == name::key(&name));
                        if clash {
                            d.redefaults
                                .insert((block, name::key(&name)), value.clone());
                        }
                        put(Def::Param, &name, if clash { usize::MAX } else { k }, true);
                        texts.push(text(block, Def::Param, &name, value, Vec::new()));
                    }
                }
                Tag::Func => {
                    let line = rest(toks, ast, s);
                    if let Some((name, formals, body)) = read_structure::func_parts(&line) {
                        // the last .func of a name wins (settle dup_func.sp)
                        put(Def::Func, &name, 0, true);
                        d.arity.insert((block, name::key(&name)), formals.len());
                        texts.push(text(block, Def::Func, &name, body.to_string(), formals));
                    }
                }
                Tag::Subckt | Tag::Section => {
                    let def = if ast.tag(s) == Tag::Subckt {
                        Def::Subckt
                    } else {
                        Def::Section
                    };
                    if let Some(name) = read_structure::def_name(ast, toks, s) {
                        put(def, &name, 0, false);
                        if def == Def::Subckt {
                            let heads = assigns(ast, toks, s)
                                .into_iter()
                                .map(|(k, _)| name::key(&k));
                            d.heads
                                .entry((block, name::key(&name)))
                                .or_insert_with(|| (port_count(ast, toks, s), heads.collect()));
                            d.nodes.entry((block, name::key(&name))).or_insert(s.0);
                        }
                    }
                    d.parent.insert(s.0, block);
                    stack.push(s);
                }
                Tag::Instance if ast.tag(container) == Tag::Subckt => {
                    let card = toks.text(ast.main_token(s));
                    if card.starts_with(['x', 'X']) {
                        let line = rest(toks, ast, s);
                        let words = line
                            .split_whitespace()
                            .take_while(|w| !w.contains('=') && !w.eq_ignore_ascii_case("params:"));
                        if let Some(m) = words.last() {
                            d.masters.entry(block).or_default().push(name::key(m));
                        }
                    }
                }
                Tag::If => {
                    let mut b = Some(s);
                    // Bounded: a chain's branches are distinct nodes.
                    for _ in 0..ast.len() {
                        let Some(at) = b else { break };
                        d.parent.insert(at.0, block);
                        stack.push(at);
                        b = next(ast, at);
                    }
                }
                _ => {}
            }
        }
    }
    d.settle(&texts);
    d
}

impl Declared {
    /// Whether `(def, name)` of field `k` of statement `node` wins in `block`.
    pub(super) fn wins(&self, block: Node, def: Def, name: &str, node: Node, k: usize) -> bool {
        let key = (block.0, def, name::key(name));
        self.winners.get(&key) == Some(&(node.0, k))
    }

    /// The new default a body `.param` gives head parameter `name` of the `.subckt` `node`, when
    /// moving it to the default is exact: its value reads no name, no temperature and no circuit
    /// (a default is read where the instance stands, a body `.param` in the body, R53).
    pub(super) fn redefault(&self, node: Node, name: &str) -> Option<&str> {
        let text = self.redefaults.get(&(node.0, name::key(name)))?;
        let leaves = expr_lex::leaves(text);
        (leaves.names.is_empty() && !leaves.circuit && !leaves.temp).then_some(text.as_str())
    }

    /// The port count and the head parameter names (folded) of the `.subckt name` visible from
    /// `block`: the nearest block outward that defines it; `None` when this file has none there.
    pub(super) fn head(&self, block: Node, name: &str) -> Option<(usize, &[String])> {
        let key = name::key(name);
        let mut at = Some(block.0);
        // Bounded by the nesting: each step moves to an enclosing block.
        for _ in 0..=self.parent.len() {
            let b = at?;
            if let Some((ports, f)) = self.heads.get(&(b, key.clone())) {
                return Some((*ports, f));
            }
            at = self.parent.get(&b).copied();
        }
        None
    }

    /// The port count and head parameter names of a `.subckt name` at this file's top level.
    pub(super) fn top_head(&self, name: &str) -> Option<(usize, &[String])> {
        let (ports, f) = self.heads.get(&(self.root, name::key(name)))?;
        Some((*ports, f))
    }

    /// The formal count of the `.func name` visible from `block`: the nearest block outward that
    /// defines it; `None` when this file has none there.
    pub(super) fn arity(&self, block: Node, name: &str) -> Option<usize> {
        let key = name::key(name);
        let mut at = Some(block.0);
        // Bounded by the nesting: each step moves to an enclosing block.
        for _ in 0..=self.parent.len() {
            let b = at?;
            if let Some(&n) = self.arity.get(&(b, key.clone())) {
                return Some(n);
            }
            at = self.parent.get(&b).copied();
        }
        None
    }

    /// The class of `text` read in `block`, `formals` shadowing the definitions they name.
    pub(super) fn class(&self, block: Node, text: &str, formals: &[String]) -> Class {
        let leaves = expr_lex::leaves(text);
        let mut class = if leaves.circuit {
            Class::Circuit
        } else if leaves.temp {
            Class::Temp
        } else {
            Class::Constant
        };
        for (name, call) in &leaves.names {
            if !call && formals.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                continue;
            }
            let def = if *call { Def::Func } else { Def::Param };
            class = class.max(self.lookup(block.0, def, name));
        }
        class
    }
}

/// Whether an X card in `block` of file `here` naming `name` closes a cycle of instances
/// (X0024): the subcircuit it names is one that encloses `block`, or reaches one through the X
/// cards of its body and theirs (a worklist, each subcircuit once). A name binds in its own file
/// outward, else to the first top-level `.subckt` of any file of the read (ngspice inlines every
/// include first, as `Lower::subckt_head`; run: a cycle through an included file is "unknown
/// subckt: x1.xb.xa.xb…").
pub(super) fn cycles(files: &[Declared], here: usize, block: Node, name: &str) -> bool {
    let d = &files[here];
    let mut enclosing = Vec::new();
    let mut at = Some(block.0);
    // Bounded by the nesting.
    for _ in 0..=d.parent.len() {
        let Some(b) = at else { break };
        if d.nodes.values().any(|&n| n == b) {
            enclosing.push((here, b));
        }
        at = d.parent.get(&b).copied();
    }
    let bind = |f: usize, from: u32, name: &str| {
        files[f].resolve(from, name).map(|n| (f, n)).or_else(|| {
            let key = name::key(name);
            files
                .iter()
                .enumerate()
                .find_map(|(g, o)| o.nodes.get(&(o.root, key.clone())).map(|&n| (g, n)))
        })
    };
    let Some(first) = bind(here, block.0, name) else {
        return false;
    };
    let mut seen = vec![first];
    let mut todo = vec![first];
    // Bounded: each subcircuit of each file is pushed once.
    while let Some((f, sub)) = todo.pop() {
        if enclosing.contains(&(f, sub)) {
            return true;
        }
        for m in files[f].masters.get(&sub).into_iter().flatten() {
            if let Some(next) = bind(f, sub, m)
                && !seen.contains(&next)
            {
                seen.push(next);
                todo.push(next);
            }
        }
    }
    false
}

/// The `(name, value text)` of each `key = value` field of a statement.
pub(super) fn assigns(ast: &Ast<'_>, toks: &Tokens<'_>, n: Node) -> Vec<(String, String)> {
    ast.fields(n)
        .iter()
        .map(|&f| Node(f))
        .filter(|&f| ast.tag(f) == Tag::Assign && ast.rhs(f) != NONE)
        .map(|f| {
            let key = joined(toks, ast, Node(ast.lhs(f)));
            (key, joined(toks, ast, Node(ast.rhs(f))))
        })
        .collect()
}

// ---- private ----

impl Declared {
    /// The class of the definition `name` of `def` seen from `block`: the nearest block outward
    /// that defines it; `Constant` when none does (an unresolved name reads nothing that varies).
    fn lookup(&self, block: u32, def: Def, name: &str) -> Class {
        let key = name::key(name);
        let mut at = Some(block);
        // Bounded by the nesting: each step moves to an enclosing block.
        for _ in 0..=self.parent.len() {
            let Some(b) = at else { break };
            if let Some(&c) = self.classes.get(&(b, def, key.clone())) {
                return c;
            }
            at = self.parent.get(&b).copied();
        }
        Class::Constant
    }

    /// Each definition's class, to the fixpoint (monotone: at most two raises per definition).
    fn settle(&mut self, texts: &[Text]) {
        // Bounded: a round that changes nothing ends it; each change raises a class.
        for _ in 0..=2 * texts.len() + 1 {
            let mut changed = false;
            for t in texts {
                let c = self.class(Node(t.block), &t.text, &t.formals);
                let key = (t.block, t.def, name::key(&t.name));
                let old = self.classes.get(&key).copied().unwrap_or(Class::Constant);
                if c > old {
                    self.classes.insert(key, c);
                    changed = true;
                }
            }
            if !changed {
                return;
            }
        }
    }
}

/// A definition's text record.
fn text(block: u32, def: Def, name: &str, text: String, formals: Vec<String>) -> Text {
    Text {
        block,
        def,
        name: name.to_string(),
        text,
        formals,
    }
}

impl Declared {
    /// The `.subckt` node `name` names from `block`: the nearest block outward that defines it.
    fn resolve(&self, block: u32, name: &str) -> Option<u32> {
        let key = name::key(name);
        let mut at = Some(block);
        // Bounded by the nesting.
        for _ in 0..=self.parent.len() {
            let b = at?;
            if let Some(&n) = self.nodes.get(&(b, key.clone())) {
                return Some(n);
            }
            at = self.parent.get(&b).copied();
        }
        None
    }
}

/// How many ports a `.subckt` node's head names: the words after its name, before `params:` or
/// the first assignment (as `read_structure`'s head reads them).
fn port_count(ast: &Ast<'_>, toks: &Tokens<'_>, n: Node) -> usize {
    ast.fields(n)
        .iter()
        .skip(1)
        .map(|&f| Node(f))
        .take_while(|&f| {
            ast.tag(f) == Tag::Word && !joined(toks, ast, f).eq_ignore_ascii_case("params:")
        })
        .count()
}
