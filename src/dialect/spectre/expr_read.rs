//! A Spectre expression (a CST subtree) → an IR expression tree, by Spectre's meaning of each
//! operator and function as far as evidence reaches (`rewrite/DIALECT-SPECTRE.md` §3):
//!
//! - `+ - * /` (real division), the comparisons, `&& || !`, `?:`; `**` and `pow` are `Pow`
//!   (RUN-SP: `(-2)**2` = 4, `(-2)**3` = −8; a fractional power of a negative base is 0 there,
//!   outside `Pow`'s domain); a `-` directly before a nonzero number is that negative literal
//!   (`-0` stays a negation, so the writer's `-0` reads back), a unary `+` its operand;
//! - `int` is `Trunc`, `floor`, `fmod`, `min`, `max`, `abs`, `round` (`RoundHalfAway`, refused
//!   where the circuit is read: SFE-2319) by RUN-SP; `log` is `Ln`, and `exp sqrt log10 ceil sin
//!   cos tan asin acos atan sinh cosh tanh asinh acosh atanh hypot` by MAN only;
//! - `$time` the time and `$freq` the frequency (RUN-SP), `v(a)`, `v(a, b)` node voltages, where
//!   the circuit is read; `temp` and `tnom` the circuit temperatures (MAN, two manuals
//!   disagree: §10);
//! - any other call is a user function's (`real f(...)`), bound later.
//!
//! What has no IR meaning here, or no evidence (`atan2`, `ln`, `%`, `~^`, `i()`, `ddt`, `idt`,
//! a vector, a string), makes the whole statement `Unread`; a circuit read where the context
//! reads none makes it `InvalidInSource`. The tree is checked before any row is pushed, so a
//! refused statement leaves no orphan rows. Depth bounded by the parser's `DEPTH_MAX`.
//!
//! Contents: `Context`, `Refusal`, `FEATURES`, `expr`, `check_expr`, `literal_of`
//! (crate-private)

use super::codes;
use super::number;
use super::read::Lower;
use crate::ir;
use crate::syntax::ast::{Name, Node, Tag};

/// Where an expression stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    /// A parameter, a model or instance parameter, a condition, a formal's default.
    Param,
    /// A value that may read the circuit: a `bsource` output, a resistor's `r=`, a
    /// capacitor's `c=`.
    Circuit,
}

/// Why an expression is not read: the statement becomes an `Opaque` of `class`, with `code`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refusal {
    /// The diagnostic.
    pub(super) code: ir::Code,
    /// The statement's class.
    pub(super) class: ir::OpaqueClass,
}

/// What the expressions use: every tag, function and operator above, in both forms but
/// `round` (static only).
pub(super) const FEATURES: ir::FeatureSet = operators(functions(ir::FeatureSet::EMPTY))
    .with(ir::Feature::Expr(ir::ExprTag::Num))
    .with(ir::Feature::Expr(ir::ExprTag::Param))
    .with(ir::Feature::Expr(ir::ExprTag::Call))
    .with(ir::Feature::Expr(ir::ExprTag::UserCall))
    .with(ir::Feature::Expr(ir::ExprTag::Unary))
    .with(ir::Feature::Expr(ir::ExprTag::Binary))
    .with(ir::Feature::Expr(ir::ExprTag::Ternary))
    .with(ir::Feature::Expr(ir::ExprTag::Temp))
    .with(ir::Feature::Expr(ir::ExprTag::Tnom))
    .with(ir::Feature::Expr(ir::ExprTag::V))
    .with(ir::Feature::Expr(ir::ExprTag::Time))
    .with(ir::Feature::Expr(ir::ExprTag::Freq))
    .with(ir::Feature::Func(ir::Func::RoundHalfAway));

/// The functions read, by name and argument count, each with its evidence above.
const FUNCS: &[(&str, ir::Func, usize)] = &[
    ("abs", ir::Func::Abs, 1),
    ("sqrt", ir::Func::Sqrt, 1),
    ("exp", ir::Func::Exp, 1),
    ("log", ir::Func::Ln, 1),
    ("log10", ir::Func::Log10, 1),
    ("pow", ir::Func::Pow, 2),
    ("min", ir::Func::Min, 2),
    ("max", ir::Func::Max, 2),
    ("floor", ir::Func::Floor, 1),
    ("ceil", ir::Func::Ceil, 1),
    ("int", ir::Func::Trunc, 1),
    ("round", ir::Func::RoundHalfAway, 1),
    ("fmod", ir::Func::Fmod, 2),
    ("hypot", ir::Func::Hypot, 2),
    ("sin", ir::Func::Sin, 1),
    ("cos", ir::Func::Cos, 1),
    ("tan", ir::Func::Tan, 1),
    ("asin", ir::Func::Asin, 1),
    ("acos", ir::Func::Acos, 1),
    ("atan", ir::Func::Atan, 1),
    ("sinh", ir::Func::Sinh, 1),
    ("cosh", ir::Func::Cosh, 1),
    ("tanh", ir::Func::Tanh, 1),
    ("asinh", ir::Func::Asinh, 1),
    ("acosh", ir::Func::Acosh, 1),
    ("atanh", ir::Func::Atanh, 1),
];

/// The binary operators read (`**` is `Pow`, a call).
const BINARY: &[(&str, ir::BinOp)] = &[
    ("+", ir::BinOp::Add),
    ("-", ir::BinOp::Sub),
    ("*", ir::BinOp::Mul),
    ("/", ir::BinOp::Div),
    ("==", ir::BinOp::Eq),
    ("!=", ir::BinOp::Ne),
    ("<", ir::BinOp::Lt),
    ("<=", ir::BinOp::Le),
    (">", ir::BinOp::Gt),
    (">=", ir::BinOp::Ge),
    ("&&", ir::BinOp::And),
    ("||", ir::BinOp::Or),
];

/// The expression at CST node `node`, read in `ctx` into rows, each parameter read and user
/// call queued for binding from `l.scope`.
///
/// # Errors
/// The statement's class and code when any part has no IR meaning here.
pub(super) fn expr(
    l: &mut Lower<'_, '_, '_>,
    node: u32,
    ctx: Context,
) -> Result<ir::ExprId, Refusal> {
    check_expr(l, node, ctx)?;
    Ok(build(l, node))
}

/// Checks the expression at CST node `node` in `ctx` without pushing a row: a statement of
/// several values checks them all before it reads any.
///
/// # Errors
/// As [`expr`].
pub(super) fn check_expr(l: &Lower<'_, '_, '_>, node: u32, ctx: Context) -> Result<(), Refusal> {
    check(l, node, ctx, 0)?;
    // `round` where the circuit is read (SFE-2319); a value that reads none is a parameter's
    if ctx == Context::Circuit && reads_circuit(l, node) && calls_round(l, node) {
        return Err(refuse(
            codes::ROUND_IN_CIRCUIT,
            ir::OpaqueClass::InvalidInSource,
        ));
    }
    Ok(())
}

/// The literal a `Num` node writes, or `None` for any other node or a number not read.
pub(super) fn literal_of(l: &Lower<'_, '_, '_>, node: u32) -> Option<ir::Literal> {
    let n = Node(node);
    (node != crate::syntax::ast::NONE && l.cst.ast.tag(n) == Tag::Num)
        .then(|| number::lex(l.token(n)).ok())
        .flatten()
}

// ---- private ----

/// `set` with every function's feature, static and dynamic (`round`: static, added above).
const fn functions(set: ir::FeatureSet) -> ir::FeatureSet {
    let mut out = set;
    let mut i = 0;
    while i < FUNCS.len() {
        let f = FUNCS[i].1;
        out = out.with(ir::Feature::Func(f));
        if !matches!(f, ir::Func::RoundHalfAway) {
            out = out.with(ir::Feature::FuncDyn(f));
        }
        i += 1;
    }
    out
}

/// `set` with every operator's feature, static and dynamic.
const fn operators(set: ir::FeatureSet) -> ir::FeatureSet {
    let mut out = set
        .with(ir::Feature::Unary(ir::UnOp::Neg))
        .with(ir::Feature::Unary(ir::UnOp::Not))
        .with(ir::Feature::UnaryDyn(ir::UnOp::Neg))
        .with(ir::Feature::UnaryDyn(ir::UnOp::Not));
    let mut i = 0;
    while i < BINARY.len() {
        out = out
            .with(ir::Feature::Binary(BINARY[i].1))
            .with(ir::Feature::BinaryDyn(BINARY[i].1));
        i += 1;
    }
    out
}

/// A refusal of `class` with `code`.
const fn refuse(code: ir::Code, class: ir::OpaqueClass) -> Refusal {
    Refusal { code, class }
}

/// A call's arguments (`extra [close, n, args…]`).
fn call_args(l: &Lower<'_, '_, '_>, n: Node) -> Vec<u32> {
    let ast = &l.cst.ast;
    let at = ast.rhs(n);
    let count = ast.extra(at + 1);
    (0..count).map(|k| ast.extra(at + 2 + k)).collect()
}

/// The name a call, an identifier or a word holds.
fn name_of(l: &Lower<'_, '_, '_>, n: Node) -> String {
    let ast = &l.cst.ast;
    match ast.tag(n) {
        Tag::Word => ast.name(Name(ast.rhs(n))).to_string(),
        Tag::Ident | Tag::Call | Tag::Probe => ast.name(Name(ast.lhs(n))).to_string(),
        _ => l.token(n).to_string(),
    }
}

/// Whether `node` is a nonzero number node (a negative literal's operand).
fn nonzero_number(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    literal_of(l, node).is_some_and(|x| x.mantissa != 0)
}

/// The nodes of the subtree at `node` (checked: a bounded tree).
fn nodes(l: &Lower<'_, '_, '_>, node: u32) -> Vec<Node> {
    let ast = &l.cst.ast;
    let mut out = Vec::new();
    let mut todo = vec![node];
    // Bounded: the checked tree is acyclic and at most DEPTH_MAX deep.
    while let Some(x) = todo.pop() {
        let n = Node(x);
        out.push(n);
        match ast.tag(n) {
            Tag::Unary | Tag::Paren => todo.push(ast.lhs(n)),
            Tag::Binary => todo.extend([ast.lhs(n), ast.rhs(n)]),
            Tag::Ternary => {
                let at = ast.rhs(n);
                todo.extend([ast.lhs(n), ast.extra(at), ast.extra(at + 1)]);
            }
            Tag::Call | Tag::Probe => todo.extend(call_args(l, n)),
            _ => {}
        }
    }
    out
}

/// Whether the checked subtree at `node` reads the circuit (`v()`, `$time`, `$freq`).
fn reads_circuit(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    nodes(l, node).into_iter().any(|n| match l.cst.ast.tag(n) {
        Tag::Call | Tag::Probe => name_of(l, n) == "v",
        Tag::Ident | Tag::Word => matches!(name_of(l, n).as_str(), "$time" | "$freq"),
        _ => false,
    })
}

/// Whether the checked subtree at `node` calls `round`.
fn calls_round(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    nodes(l, node)
        .into_iter()
        .any(|n| matches!(l.cst.ast.tag(n), Tag::Call) && name_of(l, n) == "round")
}

/// Checks the subtree at `node`.
fn check(l: &Lower<'_, '_, '_>, node: u32, ctx: Context, depth: u32) -> Result<(), Refusal> {
    use ir::OpaqueClass::{InvalidInSource, Unread};
    let no_member = refuse(codes::EXPR_NO_MEMBER, Unread);
    let circuit = refuse(codes::READS_CIRCUIT, InvalidInSource);
    let ast = &l.cst.ast;
    let n = Node(node);
    if node == crate::syntax::ast::NONE || depth > crate::syntax::DEPTH_MAX {
        return Err(refuse(codes::NOT_A_NUMBER, InvalidInSource));
    }
    let down = |k: u32| check(l, k, ctx, depth + 1);
    match ast.tag(n) {
        Tag::Num => match number::lex(l.token(n)) {
            Ok(_) => Ok(()),
            Err(number::NotANumber::Grammar) => Err(refuse(codes::NOT_A_NUMBER, InvalidInSource)),
            Err(number::NotANumber::TooLong) => Err(refuse(codes::LONG_LITERAL, Unread)),
        },
        Tag::Ident | Tag::Word => match name_of(l, n).as_str() {
            "$time" | "$freq" if ctx == Context::Param => Err(circuit),
            s if s.starts_with('$') && !matches!(s, "$time" | "$freq") => Err(no_member),
            _ => Ok(()),
        },
        Tag::Unary => match l.token(n) {
            "-" | "+" | "!" => down(ast.lhs(n)),
            _ => Err(no_member),
        },
        Tag::Binary => {
            let op = l.token(n);
            if op != "**" && !BINARY.iter().any(|(s, _)| *s == op) {
                return Err(no_member);
            }
            down(ast.lhs(n))?;
            down(ast.rhs(n))
        }
        Tag::Ternary => {
            let at = ast.rhs(n);
            down(ast.lhs(n))?;
            down(ast.extra(at))?;
            down(ast.extra(at + 1))
        }
        Tag::Paren => down(ast.lhs(n)),
        Tag::Call | Tag::Probe => {
            let name = name_of(l, n);
            let args = call_args(l, n);
            if name == "v" {
                let words = args
                    .iter()
                    .all(|&a| matches!(ast.tag(Node(a)), Tag::Ident | Tag::Word));
                return match (ctx, args.len(), words) {
                    (Context::Param, _, _) => Err(circuit),
                    (_, 1 | 2, true) => Ok(()),
                    _ => Err(no_member),
                };
            }
            match FUNCS.iter().find(|(s, _, _)| *s == name) {
                Some((_, _, arity)) if *arity != args.len() => {
                    return Err(refuse(codes::CALL_ARITY, InvalidInSource));
                }
                Some(_) => {}
                // a function of no evidence, or Spectre's integrators: no rule here
                None if matches!(name.as_str(), "atan2" | "ln" | "i" | "ddt" | "idt") => {
                    return Err(no_member);
                }
                None => {}
            }
            args.iter().try_for_each(|&a| down(a))
        }
        _ => Err(no_member),
    }
}

/// Builds the checked subtree at `node`.
fn build(l: &mut Lower<'_, '_, '_>, node: u32) -> ir::ExprId {
    let n = Node(node);
    let ast = &l.cst.ast;
    match ast.tag(n) {
        Tag::Num => {
            let literal = literal_of(l, node).expect("checked");
            number::push(l.b, literal, false)
        }
        Tag::Ident | Tag::Word => match name_of(l, n).as_str() {
            "$time" => l.b.time(),
            "$freq" => l.b.freq(),
            "temp" => l.b.temp(),
            "tnom" => l.b.tnom(),
            name => {
                let e = l.b.param_ref(name);
                l.reads.push((e, l.scope));
                e
            }
        },
        Tag::Unary => {
            let operand = ast.lhs(n);
            match l.token(n) {
                "-" if nonzero_number(l, operand) => {
                    let literal = literal_of(l, operand).expect("checked");
                    number::push(l.b, literal, true)
                }
                "-" => {
                    let x = build(l, operand);
                    l.b.unary(ir::UnOp::Neg, x)
                }
                "!" => {
                    let x = build(l, operand);
                    l.b.unary(ir::UnOp::Not, x)
                }
                _ => build(l, operand),
            }
        }
        Tag::Binary => {
            let op = l.token(n).to_string();
            let (left, right) = (ast.lhs(n), ast.rhs(n));
            let a = build(l, left);
            let b = build(l, right);
            match BINARY.iter().find(|(s, _)| *s == op) {
                Some(&(_, op)) => l.b.binary(op, a, b),
                None => l.b.call(ir::Func::Pow, &[a, b]),
            }
        }
        Tag::Ternary => {
            let at = ast.rhs(n);
            let (c, t, e) = (ast.lhs(n), ast.extra(at), ast.extra(at + 1));
            let c = build(l, c);
            let t = build(l, t);
            let e = build(l, e);
            l.b.ternary(c, t, e)
        }
        Tag::Paren => build(l, ast.lhs(n)),
        _ => {
            let name = name_of(l, n);
            let raw = call_args(l, n);
            if name == "v" {
                let net = l.field(raw[0]);
                let reference = raw.get(1).map(|&r| l.field(r));
                let reference = reference.filter(|r| r != "0");
                return l.b.v(&net, reference.as_deref());
            }
            let args: Vec<ir::ExprId> = raw.into_iter().map(|a| build(l, a)).collect();
            match FUNCS.iter().find(|(s, _, _)| *s == name) {
                Some(&(_, func, _)) => l.b.call(func, &args),
                None => {
                    let e = l.b.user_call(&name, &args);
                    l.reads.push((e, l.scope));
                    e
                }
            }
        }
    }
}
