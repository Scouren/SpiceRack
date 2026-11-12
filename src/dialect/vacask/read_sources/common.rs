//! The parts every source statement is read through: its `key=value` fields (`Params`), a
//! field's value or VACASK's default (`Val`), the checks VACASK makes on a literal, the element
//! header (whole name, terminals, the master bound by VACASK's rule, `$mfactor`), and the
//! module word of the master.
//!
//! Cites: VD cir-instance, cir-hier ("Scope"), cir-mfactor; RUN m1; A2 (check before push).
//!
//! Contents: `Fail`, `Params` (`get`, `val`, `only`, `keys`), `Val`, `COMMON`, `invalid`,
//! `private`, `params`, `type_keys`, `source_key`, `at_default`, `nonpositive`,
//! `string`, `check`, `build`, `head`, `mult`, `terminals`, `flow_node` (crate-private)

use super::{CTLNODE, INVALID, UNSTATED, default_of};
use crate::dialect::vacask::bind;
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

/// The element header of `n` (`bind::head`: the whole name, the terminals with a ground name
/// as `0`), its master by name, unbound: the caller records the pushed element for
/// `bind::masters` (`Lower::bind_master_later`, VACASK's rule: own subcircuit, then the top).
pub(super) fn head(l: &mut Lower<'_, '_, '_>, n: Node, mult: ir::ExprId) -> ir::ElemHead {
    let h = bind::head(l, n);
    let master = h.master.expect("an instance routed here names its master");
    let name = l.b.name(&h.name);
    let terminals = match h.terminals.is_empty() {
        true => ir::List::EMPTY,
        false => {
            l.b.names(&h.terminals.iter().map(String::as_str).collect::<Vec<_>>())
        }
    };
    let model = ir::Ref::unresolved(l.b.name(&master));
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
