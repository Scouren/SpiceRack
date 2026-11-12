//! A VACASK expression (a CST subtree) → an IR expression tree, by VACASK's own meaning of each
//! operator and function (`rewrite/DIALECT-VACASK.md` §3.3; runs e1, i1, t3, f1–f3):
//! `**` and `pow` are `Pow` (signed); `/` is `DivTyped` (A12 K1; `a12.rs`); `^` is `BitXor`;
//! `log` and `ln` are `Ln`; `round` is `RoundHalfAway`; `integer` is `Trunc`, `int` is `ToInt`;
//! `sgn(x)` is the sugar `x >= 0 ? 1 : -1` and `sign(x, y)` the sugar `abs(x) * (y >= 0 ? 1 :
//! -1)` (zero counts as positive, R13); `agauss`, `gauss`, `aunif`, `unif` are draws that sample
//! only in an `mc` loop, once per call site (RUN g2); `M_*`/`P_*` are the literals VACASK
//! documents; `$temp`, `$tnom` the leaves; a `-` directly before a number is that negative
//! literal; a unary `+` is its operand. A construct with no IR meaning (vectors, indexing,
//! `$scale`, vector and type functions) makes the whole statement `Private`; what VACASK rejects
//! (`inf`, `nan`, a probe in a parameter) makes it `InvalidInSource`. The tree is checked before
//! any row is pushed, so a refused statement leaves no orphan rows. Explicit stacks (C11).
//!
//! Cites: VD expr-operators, expr-functions, expr-special, expr-constants,
//! dev-builtin-behavioral (Functions); R13 (sugar).
//!
//! Contents: `Context`, `Refusal`, `FEATURES`, `CODES`, `NOT_A_NUMBER`, `NOT_FINITE`,
//! `PROBE_IN_PARAM`, `check_expr`, `expr` (crate-private)

use super::number;
use super::read::Lower;
use super::tables;
use crate::ir;
use crate::syntax::ast::{Name, Node, Tag};

/// Where an expression stands: a behavioural source's may read the circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    /// A parameter, a model or instance parameter, a condition, a formal's default.
    Param,
    /// A behavioural source's `v=`/`i=` (S4.1).
    #[expect(dead_code, reason = "read_elements (S4.1) reads behavioural sources")]
    Behavioural,
}

/// Why an expression is not read: the statement becomes an `Opaque` of `class`, with `code`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Refusal {
    /// The diagnostic.
    pub(super) code: ir::Code,
    /// The statement's class.
    pub(super) class: ir::OpaqueClass,
}

/// A number token VACASK does not read.
pub(super) const NOT_A_NUMBER: ir::Code = ir::Code::new(b'P', 2001);
/// `inf` or `nan` as a value.
pub(super) const NOT_FINITE: ir::Code = ir::Code::new(b'P', 2002);
/// `v()` or `i()` outside a behavioural source.
pub(super) const PROBE_IN_PARAM: ir::Code = ir::Code::new(b'P', 2006);

/// What a parameter expression uses: every node VACASK's parameter language reads into.
pub(super) const FEATURES: ir::FeatureSet = static_funcs(ir::FeatureSet::EMPTY)
    .with(ir::Feature::Expr(ir::ExprTag::Num))
    .with(ir::Feature::Expr(ir::ExprTag::Str))
    .with(ir::Feature::Expr(ir::ExprTag::Param))
    .with(ir::Feature::Expr(ir::ExprTag::Call))
    .with(ir::Feature::Expr(ir::ExprTag::Unary))
    .with(ir::Feature::Expr(ir::ExprTag::Binary))
    .with(ir::Feature::Expr(ir::ExprTag::Ternary))
    .with(ir::Feature::Expr(ir::ExprTag::Temp))
    .with(ir::Feature::Expr(ir::ExprTag::Tnom))
    .with(ir::Feature::Expr(ir::ExprTag::Draw))
    .with(ir::Feature::Unary(ir::UnOp::Neg))
    .with(ir::Feature::Unary(ir::UnOp::Not))
    .with(ir::Feature::Unary(ir::UnOp::BitNot))
    .with(ir::Feature::Binary(ir::BinOp::Add))
    .with(ir::Feature::Binary(ir::BinOp::Sub))
    .with(ir::Feature::Binary(ir::BinOp::Mul))
    .with(ir::Feature::Binary(super::a12::DIV_TYPED))
    .with(ir::Feature::Binary(ir::BinOp::Eq))
    .with(ir::Feature::Binary(ir::BinOp::Ne))
    .with(ir::Feature::Binary(ir::BinOp::Lt))
    .with(ir::Feature::Binary(ir::BinOp::Le))
    .with(ir::Feature::Binary(ir::BinOp::Gt))
    .with(ir::Feature::Binary(ir::BinOp::Ge))
    .with(ir::Feature::Binary(ir::BinOp::And))
    .with(ir::Feature::Binary(ir::BinOp::Or))
    .with(ir::Feature::Binary(ir::BinOp::BitAnd))
    .with(ir::Feature::Binary(ir::BinOp::BitOr))
    .with(ir::Feature::Binary(ir::BinOp::BitXor))
    .with(ir::Feature::Binary(ir::BinOp::Shl))
    .with(ir::Feature::Binary(ir::BinOp::Shr))
    .with(ir::Feature::Dist(ir::Dist::AbsGauss))
    .with(ir::Feature::Dist(ir::Dist::RelGauss))
    .with(ir::Feature::Dist(ir::Dist::AbsUnif))
    .with(ir::Feature::Dist(ir::Dist::RelUnif))
    .with(ir::Feature::DrawWhen(ir::DrawWhen::Sampling))
    .with(ir::Feature::DrawPer(ir::DrawPer::CallSite));

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        NOT_A_NUMBER,
        ir::Info {
            title: "a number VACASK does not read",
            cite: "VD input-numbers; INV-semantics 1.05-1.07",
            explain: "VACASK reads digits, a point, an exponent or one scale and letters after the scale only (`1k5`, `1e3k`, `1d3` are no numbers), and at most 18 significant digits are kept here. The statement is kept as text and the deck refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        NOT_FINITE,
        ir::Info {
            title: "an infinite or NaN value",
            cite: "VD input-numbers (special values); IR-KERNEL C9 (the NaN gate)",
            explain: "VACASK reads `inf` and `nan`; the IR holds only finite numbers. The statement is kept as text and the deck refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        PROBE_IN_PARAM,
        ir::Info {
            title: "a circuit probe outside a behavioural source",
            cite: "VD dev-builtin-behavioral (node and branch accessors)",
            explain: "`v()` and `i()` read the circuit only in a behavioural source's expression; elsewhere VACASK rejects them.",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// The functions VACASK has in a parameter expression (VD expr-functions).
const STATIC_FUNCS: &[ir::Func] = &[
    ir::Func::Abs,
    ir::Func::Sqrt,
    ir::Func::Exp,
    ir::Func::Ln,
    ir::Func::Log10,
    ir::Func::Pow,
    ir::Func::Min,
    ir::Func::Max,
    ir::Func::Floor,
    ir::Func::Ceil,
    ir::Func::Trunc,
    ir::Func::RoundHalfAway,
    ir::Func::Sin,
    ir::Func::Cos,
    ir::Func::Tan,
    ir::Func::Asin,
    ir::Func::Acos,
    ir::Func::Atan,
    ir::Func::Atan2,
    ir::Func::Sinh,
    ir::Func::Cosh,
    ir::Func::Tanh,
    ir::Func::Asinh,
    ir::Func::Acosh,
    ir::Func::Atanh,
    ir::Func::Hypot,
    ir::Func::Fmod,
];

/// Checks the expression at CST node `node` without pushing a row.
///
/// # Errors
/// As [`expr`].
pub(super) fn check_expr(l: &Lower<'_, '_, '_>, node: u32, ctx: Context) -> Result<(), Refusal> {
    check(l, node, ctx, 0)
}

/// The expression at CST node `node`, read in `ctx` into rows, each parameter read recorded
/// for binding from `l.scope`.
///
/// # Errors
/// The statement's class and code when any part has no IR meaning.
pub(super) fn expr(
    l: &mut Lower<'_, '_, '_>,
    node: u32,
    ctx: Context,
) -> Result<ir::ExprId, Refusal> {
    check(l, node, ctx, 0)?;
    Ok(build(l, node))
}

// ---- private ----

/// `set` with every static function's feature.
const fn static_funcs(set: ir::FeatureSet) -> ir::FeatureSet {
    let mut out = set;
    let mut i = 0;
    while i < STATIC_FUNCS.len() {
        out = out.with(ir::Feature::Func(STATIC_FUNCS[i]));
        i += 1;
    }
    out
}

/// A refusal of class `class` with code `code`.
const fn refuse(code: ir::Code, class: ir::OpaqueClass) -> Refusal {
    Refusal { code, class }
}

/// The function a VACASK call name reads as, with its argument count.
fn function(name: &str) -> Option<(ir::Func, usize)> {
    use ir::Func as F;
    Some(match name {
        "sin" => (F::Sin, 1),
        "cos" => (F::Cos, 1),
        "tan" => (F::Tan, 1),
        "asin" => (F::Asin, 1),
        "acos" => (F::Acos, 1),
        "atan" => (F::Atan, 1),
        "atan2" => (F::Atan2, 2),
        "sinh" => (F::Sinh, 1),
        "cosh" => (F::Cosh, 1),
        "tanh" => (F::Tanh, 1),
        "asinh" => (F::Asinh, 1),
        "acosh" => (F::Acosh, 1),
        "atanh" => (F::Atanh, 1),
        "exp" => (F::Exp, 1),
        "log" | "ln" => (F::Ln, 1),
        "log10" => (F::Log10, 1),
        "sqrt" => (F::Sqrt, 1),
        "pow" => (F::Pow, 2),
        "hypot" => (F::Hypot, 2),
        "abs" => (F::Abs, 1),
        "floor" => (F::Floor, 1),
        "ceil" => (F::Ceil, 1),
        "round" => (F::RoundHalfAway, 1),
        "integer" => (F::Trunc, 1),
        "int" => (super::a12::TO_INT, 1),
        "fmod" => (F::Fmod, 2),
        "min" => (F::Min, 2),
        "max" => (F::Max, 2),
        _ => return None,
    })
}

/// The draw a VACASK call name reads as.
fn draw(name: &str) -> Option<ir::Dist> {
    Some(match name {
        "agauss" => ir::Dist::AbsGauss,
        "gauss" => ir::Dist::RelGauss,
        "aunif" => ir::Dist::AbsUnif,
        "unif" => ir::Dist::RelUnif,
        _ => return None,
    })
}

/// The binary operator token `op` spells (`**` is `Pow`, handled by the caller).
fn binary(op: &str) -> Option<ir::BinOp> {
    use ir::BinOp as B;
    Some(match op {
        "+" => B::Add,
        "-" => B::Sub,
        "*" => B::Mul,
        "/" => super::a12::DIV_TYPED,
        "==" => B::Eq,
        "!=" => B::Ne,
        "<" => B::Lt,
        "<=" => B::Le,
        ">" => B::Gt,
        ">=" => B::Ge,
        "&&" => B::And,
        "||" => B::Or,
        "&" => B::BitAnd,
        "|" => B::BitOr,
        "^" => B::BitXor,
        "<<" => B::Shl,
        ">>" => B::Shr,
        _ => return None,
    })
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
        _ => l.cst.tokens.text(ast.main_token(n)).to_string(),
    }
}

/// The token text at node `n`'s main token.
fn token(l: &Lower<'_, '_, '_>, n: Node) -> String {
    l.cst.tokens.text(l.cst.ast.main_token(n)).to_string()
}

/// Whether `n` is a number node directly (a negative literal's operand).
fn is_number(l: &Lower<'_, '_, '_>, n: Node) -> bool {
    l.cst.ast.tag(n) == Tag::Num
}

/// Checks the subtree at `node` (depth bounded by the parser's `DEPTH_MAX`).
fn check(l: &Lower<'_, '_, '_>, node: u32, ctx: Context, depth: u32) -> Result<(), Refusal> {
    use ir::OpaqueClass::{InvalidInSource, Private};
    let private = refuse(super::codes::PRIVATE, Private);
    let ast = &l.cst.ast;
    let n = Node(node);
    if node == crate::syntax::ast::NONE || depth > crate::syntax::DEPTH_MAX {
        return Err(refuse(NOT_A_NUMBER, InvalidInSource));
    }
    let down = |k: u32| check(l, k, ctx, depth + 1);
    match ast.tag(n) {
        Tag::Num => number::lex(&token(l, n))
            .map(|_| ())
            .map_err(|_| refuse(NOT_A_NUMBER, InvalidInSource)),
        Tag::Str => Ok(()),
        Tag::Ident | Tag::Word => {
            let name = name_of(l, n);
            match name.as_str() {
                "$scale" => Err(private),
                "$abstime" if ctx == Context::Param => Err(refuse(PROBE_IN_PARAM, InvalidInSource)),
                s if s.eq_ignore_ascii_case("inf") || s.eq_ignore_ascii_case("nan") => {
                    Err(refuse(NOT_FINITE, InvalidInSource))
                }
                _ => Ok(()),
            }
        }
        Tag::Unary => match token(l, n).as_str() {
            "-" | "+" | "!" | "~" => down(ast.lhs(n)),
            _ => Err(private),
        },
        Tag::Binary => {
            let op = token(l, n);
            if op != "**" && binary(&op).is_none() {
                return Err(private);
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
        Tag::Paren | Tag::Delimited => down(ast.lhs(n)),
        Tag::Call | Tag::Probe => {
            let name = name_of(l, n);
            let args = call_args(l, n);
            if name == "v" || name == "i" {
                return match ctx {
                    Context::Param => Err(refuse(PROBE_IN_PARAM, InvalidInSource)),
                    Context::Behavioural => Err(private),
                };
            }
            let want = match (function(&name), draw(&name), name.as_str()) {
                (Some((_, k)), _, _) => k,
                (_, Some(d), _) => usize::from(d.arity().0),
                (_, _, "sgn") => 1,
                (_, _, "sign") => 2,
                _ => return Err(private),
            };
            if args.len() != want {
                return Err(private);
            }
            args.iter().try_for_each(|&a| down(a))
        }
        _ => Err(private),
    }
}

/// Builds the checked subtree at `node`.
fn build(l: &mut Lower<'_, '_, '_>, node: u32) -> ir::ExprId {
    let n = Node(node);
    let ast = &l.cst.ast;
    match ast.tag(n) {
        Tag::Num => {
            let literal = number::lex(&token(l, n)).expect("checked").literal;
            l.b.literal(literal)
        }
        Tag::Str => {
            let text = token(l, n);
            let inner = text
                .strip_prefix('"')
                .and_then(|t| t.strip_suffix('"'))
                .unwrap_or(&text);
            l.b.string(&unescape(inner))
        }
        Tag::Ident | Tag::Word => {
            let name = name_of(l, n);
            if let Some((_, value)) = tables::CONSTANTS.iter().find(|(c, _)| *c == name) {
                let literal = number::lex(value).expect("a documented constant").literal;
                return l.b.literal(literal);
            }
            match name.as_str() {
                "$temp" => l.b.temp(),
                "$tnom" => l.b.tnom(),
                "$abstime" => l.b.time(),
                _ => {
                    let e = l.b.param_ref(&super::name::unquote(&name));
                    l.reads.push((e, l.scope, l.stmt));
                    e
                }
            }
        }
        Tag::Unary => {
            let (op, operand) = (token(l, n), ast.lhs(n));
            if op == "-" && is_number(l, Node(operand)) {
                let mut literal = number::lex(&token(l, Node(operand)))
                    .expect("checked")
                    .literal;
                literal.mantissa = -literal.mantissa;
                return l.b.literal(literal);
            }
            let x = build(l, operand);
            match op.as_str() {
                "-" => l.b.unary(ir::UnOp::Neg, x),
                "!" => l.b.unary(ir::UnOp::Not, x),
                "~" => l.b.unary(ir::UnOp::BitNot, x),
                _ => x,
            }
        }
        Tag::Binary => {
            let (op, left, right) = (token(l, n), ast.lhs(n), ast.rhs(n));
            let a = build(l, left);
            let b = build(l, right);
            match binary(&op) {
                Some(op) => l.b.binary(op, a, b),
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
        Tag::Paren | Tag::Delimited => build(l, ast.lhs(n)),
        _ => {
            let name = name_of(l, n);
            let args: Vec<ir::ExprId> = call_args(l, n).into_iter().map(|a| build(l, a)).collect();
            if let Some((func, _)) = function(&name) {
                return l.b.call(func, &args);
            }
            if let Some(dist) = draw(&name) {
                return l
                    .b
                    .draw(dist, ir::DrawWhen::Sampling, ir::DrawPer::CallSite, &args);
            }
            // `sgn(x)` is `x >= 0 ? 1 : -1`; `sign(x, y)` is `abs(x) * (y >= 0 ? 1 : -1)`: zero
            // counts as positive (run e1, f1; R13 sugar: each operand once, read back the same).
            let last = *args.last().expect("checked arity");
            let sign = nonnegative_sign(l, last);
            match name.as_str() {
                "sgn" => sign,
                _ => {
                    let mag = l.b.call(ir::Func::Abs, &[args[0]]);
                    l.b.binary(ir::BinOp::Mul, mag, sign)
                }
            }
        }
    }
}

/// `x >= 0 ? 1 : -1`, the integers as VACASK types them.
fn nonnegative_sign(l: &mut Lower<'_, '_, '_>, x: ir::ExprId) -> ir::ExprId {
    let int = |b: &mut ir::Builder<'_>, v: i64| {
        b.literal(ir::Literal {
            mantissa: v,
            frac: 0,
            exp10: 0,
            form: super::a12::INT_FORM,
        })
    };
    let zero = int(l.b, 0);
    let cond = l.b.binary(ir::BinOp::Ge, x, zero);
    let one = int(l.b, 1);
    let minus = int(l.b, -1);
    l.b.ternary(cond, one, minus)
}

/// A VACASK string's text with `\"`, `\\` and `\n` read (VD input-strings).
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match (c, c == '\\') {
            (_, true) => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            },
            (c, false) => out.push(c),
        }
    }
    out
}
