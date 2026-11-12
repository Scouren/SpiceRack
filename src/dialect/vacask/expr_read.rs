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
//! `PROBE_IN_PARAM`, `expr` (crate-private)

use super::read::Lower;
use crate::ir;

/// Where an expression stands: a behavioural source's may read the circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Context {
    /// A parameter, a model or instance parameter, a condition, a formal's default.
    Param,
    /// A behavioural source's `v=`/`i=` (S4.1).
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
    let _ = (l, node, ctx);
    todo!("S4.0: expr_read::expr")
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
