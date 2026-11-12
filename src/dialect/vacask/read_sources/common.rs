//! The parts every source statement is read through: its `key=value` fields (`Params`), a
//! field's value or VACASK's default (`Val`), the checks VACASK makes on a literal, the element
//! header (whole name, terminals, the master bound by VACASK's rule, `$mfactor`), and the
//! module word of the master.
//!
//! Cites: VD cir-instance, cir-hier ("Scope"), cir-mfactor; RUN m1; A2 (check before push).
//!
//! Contents: `Fail`, `Params` (`get`, `val`, `only`, `keys`), `Val`, `COMMON`, `invalid`,
//! `private`, `params`, `module_of`, `type_keys`, `source_key`, `at_default`, `nonpositive`,
//! `string`, `check`, `build`, `head`, `mult`, `terminals`, `flow_node` (crate-private)

use super::{CTLNODE, INVALID, UNSTATED, default_of};
use crate::dialect::vacask::expr_read::{self, Context};
use crate::dialect::vacask::number;
use crate::dialect::vacask::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// Why a statement is not rows: its class and code.
pub(super) type Fail = (ir::OpaqueClass, ir::Code);

/// The parameters of a statement, `key → value node`, in order.
pub(super) struct Params(Vec<(String, u32)>);

/// A field's value: a node of the statement, or VACASK's default (a `DEFAULTS` literal).
#[derive(Clone, Copy)]
pub(super) enum Val {
    /// The value as written.
    Node(u32),
    /// The literal text VACASK means when the parameter is absent.
    Lit(&'static str),
}

/// The parameters every `vsource`/`isource` reads whatever its `type`.
pub(super) const COMMON: &[&str] = &["type", "mag", "phase", "$mfactor", "spur", "smag", "sphase"];

/// The parameters each `type` reads (VD dev-builtin-src).
const TYPES: &[(&str, &[&str])] = &[
    ("dc", &["dc"]),
    (
        "sine",
        &["sinedc", "ampl", "freq", "tdphase", "theta", "delay"],
    ),
    (
        "pulse",
        &["val0", "val1", "delay", "rise", "fall", "width", "period"],
    ),
    ("exp", &["val0", "val1", "delay", "td2", "tau1", "tau2"]),
    (
        "pwl",
        &[
            "wave",
            "file",
            "scale",
            "offset",
            "stretch",
            "tdphase",
            "period",
            "delay",
            "break",
            "slopetol",
            "sloperel",
            "slopeglob",
        ],
    ),
    ("am", AM_FM),
    ("fm", AM_FM),
];

/// The parameters `am` and `fm` read.
const AM_FM: &[&str] = &[
    "sinedc", "ampl", "freq", "tdphase", "delay", "modfreq", "modphase", "modindex",
];

/// The deepest scope chain the master lookup walks (as `read.rs`).
const SCOPE_DEPTH_MAX: usize = 1 << 16;

impl Params {
    /// The value node of `key`, if given.
    pub(super) fn get(&self, key: &str) -> Option<u32> {
        self.0.iter().find(|(k, _)| k == key).map(|&(_, v)| v)
    }

    /// `key`'s value, or its default.
    pub(super) fn val(&self, key: &str) -> Val {
        match self.get(key) {
            Some(v) => Val::Node(v),
            None => Val::Lit(default_of(key).expect("a parameter with a default")),
        }
    }

    /// Fails unless every key is one of `known`.
    pub(super) fn only(&self, known: &[&str]) -> Result<(), Fail> {
        match self.0.iter().all(|(k, _)| known.contains(&k.as_str())) {
            true => Ok(()),
            false => Err(invalid()),
        }
    }

    /// Every given key, in order.
    pub(super) fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }
}

/// An `InvalidInSource` failure.
pub(super) const fn invalid() -> Fail {
    (ir::OpaqueClass::InvalidInSource, INVALID)
}

/// A `Private` failure with `code`.
pub(super) const fn private(code: ir::Code) -> Fail {
    (ir::OpaqueClass::Private, code)
}

/// The `key=value` fields of `n`; a field that is no assignment, or a key given twice, fails.
pub(super) fn params(l: &Lower<'_, '_, '_>, n: Node) -> Result<Params, Fail> {
    let ast = &l.cst.ast;
    let mut out: Vec<(String, u32)> = Vec::new();
    for &f in ast.fields(n) {
        let x = Node(f);
        if ast.tag(x) != Tag::Assign {
            return Err(invalid());
        }
        let key = read::field_name(l.cst, ast.lhs(x));
        if out.iter().any(|(k, _)| *k == key) {
            return Err(private(UNSTATED));
        }
        out.push((key, ast.rhs(x)));
    }
    Ok(Params(out))
}

/// The module word `master` is declared with: the first `model master <module>` of the file.
/// ponytail: one file only, until the spine hands over its name → module map (asked of
/// vacask-plan); a model from an include is missed.
pub(super) fn module_of(l: &Lower<'_, '_, '_>, master: &str) -> Option<String> {
    let ast = &l.cst.ast;
    let mut todo: Vec<u32> = ast.fields(ast.root()).to_vec();
    // Bounded: each statement node is taken once (the tree is acyclic).
    while let Some(s) = todo.pop() {
        let n = Node(s);
        match ast.tag(n) {
            Tag::Model => {
                if let [name, module, ..] = ast.fields(n)
                    && read::field_name(l.cst, *name) == master
                {
                    return Some(read::field_name(l.cst, *module));
                }
            }
            Tag::Subckt | Tag::Section | Tag::If | Tag::Else => {
                todo.extend_from_slice(ast.body(n));
                if let Some(next) = ast.next(n) {
                    todo.push(next.0);
                }
            }
            _ => {}
        }
    }
    None
}

/// The active and the known parameters of a `vsource`/`isource` of `type`, or `None` for a
/// type VACASK has not.
pub(super) fn type_keys(typ: &str) -> Option<&'static [&'static str]> {
    TYPES.iter().find(|(t, _)| *t == typ).map(|&(_, k)| k)
}

/// Whether `key` is a parameter of `vsource`/`isource` (VD dev-builtin-src).
pub(super) fn source_key(key: &str) -> bool {
    COMMON.contains(&key) || TYPES.iter().any(|(_, ks)| ks.contains(&key))
}

/// Whether the value node `v` is a literal (or a string) equal to `default` by value.
pub(super) fn at_default(l: &Lower<'_, '_, '_>, v: u32, default: &str) -> bool {
    let ast = &l.cst.ast;
    let n = Node(v);
    if let Some(s) = default.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return ast.tag(n) == Tag::Str && read::field_name(l.cst, v) == s;
    }
    let want = number::lex(default).expect("a default literal").literal;
    // VACASK reads an integer given to a real parameter as that real (run t1).
    let real = |x: ir::Literal| match x.form {
        ir::NumForm::Int if x.mantissa == 0 => ir::Literal::ZERO,
        ir::NumForm::Int => ir::Literal {
            form: ir::NumForm::Plain,
            ..x
        },
        _ => x,
    };
    ast.tag(n) == Tag::Num
        && number::lex(l.cst.tokens.text(ast.main_token(n)))
            .is_ok_and(|got| real(got.literal).eq_value(want))
}

/// Whether the value node `v` is a literal at or below 0 (`0`, `-1n`).
pub(super) fn nonpositive(l: &Lower<'_, '_, '_>, v: u32) -> bool {
    let ast = &l.cst.ast;
    let n = Node(v);
    let lit = |x: Node| {
        ast.tag(x) == Tag::Num
            && number::lex(l.cst.tokens.text(ast.main_token(x)))
                .is_ok_and(|g| g.literal.mantissa >= 0)
    };
    match ast.tag(n) {
        Tag::Num => {
            number::lex(l.cst.tokens.text(ast.main_token(n))).is_ok_and(|g| g.literal.mantissa <= 0)
        }
        Tag::Unary => l.cst.tokens.text(ast.main_token(n)) == "-" && lit(Node(ast.lhs(n))),
        _ => false,
    }
}

/// The string literal at `v` (quotes off), or the `UNSTATED` failure.
pub(super) fn string(l: &Lower<'_, '_, '_>, v: u32) -> Result<String, Fail> {
    match l.cst.ast.tag(Node(v)) == Tag::Str {
        true => Ok(read::field_name(l.cst, v)),
        false => Err(private(UNSTATED)),
    }
}

/// Fails unless every expression `vals` holds reads into the IR (A2: before any row).
pub(super) fn check(l: &Lower<'_, '_, '_>, vals: &[Val]) -> Result<(), Fail> {
    for v in vals {
        if let Val::Node(x) = *v {
            expr_read::check_expr(l, x, Context::Param).map_err(|r| (r.class, r.code))?;
        }
    }
    Ok(())
}

/// The expression of a checked value.
pub(super) fn build(l: &mut Lower<'_, '_, '_>, v: Val) -> ir::ExprId {
    match v {
        Val::Node(x) => expr_read::expr(l, x, Context::Param).expect("checked"),
        Val::Lit(text) => {
            let literal = number::lex(text).expect("a default literal").literal;
            l.b.literal(literal)
        }
    }
}

/// The element header of `n`: its whole name, its terminals (a ground name is `0`), its
/// master bound by VACASK's rule (the own subcircuit, then the top; RUN m1), `mult`.
pub(super) fn head(l: &mut Lower<'_, '_, '_>, n: Node, mult: ir::ExprId) -> ir::ElemHead {
    let h = l.cst.ast.header(n);
    let name = read::field_name(l.cst, h.name);
    let nodes: Vec<String> = h
        .nodes
        .iter()
        .map(|&x| {
            let node = read::field_name(l.cst, x);
            match l.grounds.contains(&node) {
                true => crate::dialect::vacask::name::GROUND.to_string(),
                false => node,
            }
        })
        .collect();
    let master = read::field_name(l.cst, h.master);
    let model = master_ref(l, &master);
    let name = l.b.name(&name);
    let terminals = match nodes.is_empty() {
        true => ir::List::EMPTY,
        false => {
            l.b.names(&nodes.iter().map(String::as_str).collect::<Vec<_>>())
        }
    };
    ir::ElemHead {
        name,
        terminals,
        model,
        params: ir::List::EMPTY,
        mult,
        mult_law: ir::MultLaw::Parallel,
    }
}

/// `$mfactor`'s expression, or `NONE`.
pub(super) fn mult(l: &mut Lower<'_, '_, '_>, p: &Params) -> ir::ExprId {
    match p.get("$mfactor") {
        Some(v) => build(l, Val::Node(v)),
        None => ir::ExprId::NONE,
    }
}

/// Fails unless `n` has exactly `count` terminals.
pub(super) fn terminals(l: &Lower<'_, '_, '_>, n: Node, count: usize) -> Result<(), Fail> {
    match l.cst.ast.header(n).nodes.len() == count {
        true => Ok(()),
        false => Err(invalid()),
    }
}

/// Fails `Private` unless `key` is absent or the branch flow.
pub(super) fn flow_node(l: &Lower<'_, '_, '_>, p: &Params, key: &str) -> Result<(), Fail> {
    match p.get(key) {
        Some(v) if !at_default(l, v, "\"flow(br)\"") => Err(private(CTLNODE)),
        _ => Ok(()),
    }
}

// ---- private ----

/// The model `master`, bound by VACASK's rule: a definition in the own region (the scope up to
/// its subcircuit's body or file's top), else the file's top; never an enclosing subcircuit.
/// ponytail: bound at push time, so a model defined later stays unbound until the spine's
/// deferred master binding lands (asked of vacask-plan).
fn master_ref(l: &mut Lower<'_, '_, '_>, master: &str) -> ir::Ref {
    let name = l.b.name(master);
    let d = l.b.design();
    let found = d.lookup(l.scope, ir::DefKind::Model, name);
    let mut own = vec![];
    let mut at = l.scope;
    // Bounded: each step moves to a parent (the scope tree is acyclic, `ir::check`).
    for _ in 0..SCOPE_DEPTH_MAX {
        own.push(at);
        let s = d.scope(at);
        if matches!(s.kind, ir::ScopeKind::Subckt | ir::ScopeKind::File) || s.parent.is_none() {
            break;
        }
        at = s.parent;
    }
    let ok = |def: ir::DefId| {
        !def.is_none() && {
            let at = d.def(def).scope;
            own.contains(&at) || d.scope(at).kind == ir::ScopeKind::File
        }
    };
    let def = match ok(found) {
        true => found,
        false => {
            let top = d.file(d.scope(l.scope).file).top;
            d.lookup(top, ir::DefKind::Model, name)
        }
    };
    ir::Ref { name, def }
}
