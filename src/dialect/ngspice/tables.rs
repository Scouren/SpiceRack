//! The facts of ngspice's two expression evaluators as data: which evaluator reads an
//! expression, each function name's meaning per evaluator, the binding level of each operator.
//! `expr_read.rs` and `expr_write.rs` read the same rows, so the reader and the writer are inverse
//! by construction (law L5).
//!
//! ngspice reads an expression with numparam (`frontend/numparam/xpressn.c`: braces, `.param`,
//! `.func`, `.if`) unless it holds `v(`, `i(`, `time`, `temper` or `hertz`; then the B-source
//! parser reads it (`spicelib/parser/inpptree-parser.y`, functions `inpptree.c` `funcs[]`,
//! `ptfuncs.c`), wherever it stands (INV-semantics 2.06).
//!
//! Levels: a smaller number binds tighter; every binary level is left-associative; the ternary is
//! the loosest and groups to the right. numparam (`fetchoperator`): power 2, `* / % \` 3,
//! `+ -` 4, every comparison 5 (one level: `{0==1<2}` = 1, run), `&&` 6, `||` 7, `?:` 9; `!`
//! binds its atom; a leading `-` is `0 - …` at level 4. B source (the grammar's `%left` rows):
//! `^` 1, unary `-` and `!` 2, `* /` 3, `+ -` 4, `< <= > >=` 5, `== !=` 6, `&&` 7, `||` 8, `?:` 9.
//!
//! Cites: ngspice-45 source (files above); runs recorded in `expr_read.rs`'s tests;
//! `rewrite/INV-semantics.md` 2.02–2.14, 2.21; `rewrite/IR-KERNEL.md` §7.1 (A1).
//!
//! Contents: `pub(super) enum Eval`, `Eval::grammar(self) -> number::Grammar`,
//! `pub(super) enum Named`, `pub(super) const FUNCS: &[(&str, Named, Eval)]`,
//! `pub(super) const NO_MEMBER: &[(&str, Eval)]`, `pub(super) const LEAVES: &[&str]`,
//! `pub(super) const TERNARY: u8`,
//! `pub(super) fn named(Eval, &str) -> Option<Named>`, `pub(super) fn no_member(Eval, &str) -> bool`,
//! `pub(super) fn func_spelling(Eval, ir::Func) -> Option<&'static str>`,
//! `pub(super) fn dist_spelling(ir::Dist) -> &'static str`,
//! `pub(super) const fn level(Eval, ir::BinOp) -> Option<u8>`, `pub(super) const fn power(Eval) -> u8`,
//! `pub(super) const fn feature_set(&[Feature]) -> FeatureSet`

use super::number::Grammar;
use crate::ir::{BinOp, Dist, Feature, FeatureSet, Func};

/// Which of ngspice's evaluators reads an expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Eval {
    /// numparam: braces, `.param`, `.func`, `.if`.
    Numparam,
    /// The B-source parser: any expression holding a probe or `time`, `temper`, `hertz`.
    Behavioural,
}

/// What a function name means to one evaluator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Named {
    /// A built-in function.
    Func(Func),
    /// A statistical draw (numparam: a fresh sample at every reference, INV 12.01–12.03).
    Draw(Dist),
    /// `ternary_fcn(c, a, b)`: the conditional.
    Ternary,
}

/// Every function name and its meaning, per evaluator. The first row of a member for an
/// evaluator is its spelling there. Sources: numparam `fmathS` and `mathfunction`
/// (`xpressn.c`: `pow` is C's `pow`, `pwr` is `pow(fabs(x), y)`, `int` truncates, `nint` is
/// `nearbyint`, `log` is natural); B source `funcs[]` (`pow` is `pow(fabs(x), y)`, `ln`/`log`
/// are `log` with a domain error below 0, `sqrt` the same, `exp` exact below 227.96).
pub(super) const FUNCS: &[(&str, Named, Eval)] = &[
    ("abs", Named::Func(Func::Abs), Eval::Numparam),
    ("abs", Named::Func(Func::Abs), Eval::Behavioural),
    ("sqrt", Named::Func(Func::Sqrt), Eval::Numparam),
    ("sqrt", Named::Func(Func::Sqrt), Eval::Behavioural),
    ("exp", Named::Func(Func::Exp), Eval::Numparam),
    ("exp", Named::Func(Func::Exp), Eval::Behavioural),
    ("ln", Named::Func(Func::Ln), Eval::Numparam),
    ("ln", Named::Func(Func::Ln), Eval::Behavioural),
    ("log", Named::Func(Func::Ln), Eval::Numparam),
    ("log", Named::Func(Func::Ln), Eval::Behavioural),
    ("log10", Named::Func(Func::Log10), Eval::Numparam),
    ("log10", Named::Func(Func::Log10), Eval::Behavioural),
    ("sin", Named::Func(Func::Sin), Eval::Numparam),
    ("sin", Named::Func(Func::Sin), Eval::Behavioural),
    ("cos", Named::Func(Func::Cos), Eval::Numparam),
    ("cos", Named::Func(Func::Cos), Eval::Behavioural),
    ("tan", Named::Func(Func::Tan), Eval::Numparam),
    ("tan", Named::Func(Func::Tan), Eval::Behavioural),
    ("asin", Named::Func(Func::Asin), Eval::Numparam),
    ("asin", Named::Func(Func::Asin), Eval::Behavioural),
    ("acos", Named::Func(Func::Acos), Eval::Numparam),
    ("acos", Named::Func(Func::Acos), Eval::Behavioural),
    ("atan", Named::Func(Func::Atan), Eval::Numparam),
    ("atan", Named::Func(Func::Atan), Eval::Behavioural),
    ("arctan", Named::Func(Func::Atan), Eval::Numparam),
    ("sinh", Named::Func(Func::Sinh), Eval::Numparam),
    ("sinh", Named::Func(Func::Sinh), Eval::Behavioural),
    ("cosh", Named::Func(Func::Cosh), Eval::Numparam),
    ("cosh", Named::Func(Func::Cosh), Eval::Behavioural),
    ("tanh", Named::Func(Func::Tanh), Eval::Numparam),
    ("tanh", Named::Func(Func::Tanh), Eval::Behavioural),
    ("asinh", Named::Func(Func::Asinh), Eval::Numparam),
    ("asinh", Named::Func(Func::Asinh), Eval::Behavioural),
    ("acosh", Named::Func(Func::Acosh), Eval::Numparam),
    ("acosh", Named::Func(Func::Acosh), Eval::Behavioural),
    ("atanh", Named::Func(Func::Atanh), Eval::Numparam),
    ("atanh", Named::Func(Func::Atanh), Eval::Behavioural),
    ("pow", Named::Func(Func::Pow), Eval::Numparam),
    ("pow", Named::Func(Func::PowAbs), Eval::Behavioural),
    ("pwr", Named::Func(Func::PowAbs), Eval::Numparam),
    ("min", Named::Func(Func::Min), Eval::Numparam),
    ("min", Named::Func(Func::Min), Eval::Behavioural),
    ("max", Named::Func(Func::Max), Eval::Numparam),
    ("max", Named::Func(Func::Max), Eval::Behavioural),
    ("int", Named::Func(Func::Trunc), Eval::Numparam),
    ("nint", Named::Func(Func::RoundHalfEven), Eval::Numparam),
    ("nint", Named::Func(Func::RoundHalfEven), Eval::Behavioural),
    ("sgn", Named::Func(Func::Sgn), Eval::Numparam),
    ("sgn", Named::Func(Func::Sgn), Eval::Behavioural),
    ("ceil", Named::Func(Func::Ceil), Eval::Numparam),
    ("ceil", Named::Func(Func::Ceil), Eval::Behavioural),
    ("floor", Named::Func(Func::Floor), Eval::Numparam),
    ("floor", Named::Func(Func::Floor), Eval::Behavioural),
    ("uramp", Named::Func(Func::Ramp), Eval::Behavioural),
    ("ddt", Named::Func(Func::Ddt), Eval::Behavioural),
    ("ternary_fcn", Named::Ternary, Eval::Numparam),
    ("ternary_fcn", Named::Ternary, Eval::Behavioural),
    ("agauss", Named::Draw(Dist::AbsGauss), Eval::Numparam),
    ("gauss", Named::Draw(Dist::RelGauss), Eval::Numparam),
    ("aunif", Named::Draw(Dist::AbsUnif), Eval::Numparam),
    ("unif", Named::Draw(Dist::RelUnif), Eval::Numparam),
    ("limit", Named::Draw(Dist::Limit), Eval::Numparam),
];

/// Function names ngspice has whose meaning no IR member states: B `u` (0.5 at 0, `PTustep`),
/// `u2` (a clamp to [0, 1]), `pwr` (`sgn(x)·|x|^y`, `PTpwr`), `eq0`…`le0`, `pwl`,
/// `pwl_derivative`; numparam `sqr`. An expression using one is `Unread`.
pub(super) const NO_MEMBER: &[(&str, Eval)] = &[
    ("u", Eval::Behavioural),
    ("u2", Eval::Behavioural),
    ("pwr", Eval::Behavioural),
    ("eq0", Eval::Behavioural),
    ("ne0", Eval::Behavioural),
    ("gt0", Eval::Behavioural),
    ("lt0", Eval::Behavioural),
    ("ge0", Eval::Behavioural),
    ("le0", Eval::Behavioural),
    ("pwl", Eval::Behavioural),
    ("pwl_derivative", Eval::Behavioural),
    ("sqr", Eval::Numparam),
];

/// The names that make the B-source parser read an expression (with a `v(` or `i(` probe).
pub(super) const LEAVES: &[&str] = &["time", "temper", "hertz"];

/// The ternary's level in both evaluators: the loosest.
pub(super) const TERNARY: u8 = 9;

impl Eval {
    /// The number grammar of this evaluator's lexer.
    pub(super) fn grammar(self) -> Grammar {
        match self {
            Eval::Numparam => Grammar::Numparam,
            Eval::Behavioural => Grammar::Inp,
        }
    }
}

/// What the function `name` (any case) means to `eval`.
pub(super) fn named(eval: Eval, name: &str) -> Option<Named> {
    FUNCS
        .iter()
        .find(|(n, _, e)| *e == eval && n.eq_ignore_ascii_case(name))
        .map(|(_, named, _)| *named)
}

/// Whether `name` (any case) is a function `eval` has but the IR has no member for.
pub(super) fn no_member(eval: Eval, name: &str) -> bool {
    NO_MEMBER
        .iter()
        .any(|(n, e)| *e == eval && n.eq_ignore_ascii_case(name))
}

/// The name `eval` spells `func` with, or `None` when it has none (`PowAbs` and numparam's
/// `Fmod` are operators, spelled by the writer).
pub(super) fn func_spelling(eval: Eval, func: Func) -> Option<&'static str> {
    FUNCS
        .iter()
        .find(|(_, named, e)| *e == eval && *named == Named::Func(func))
        .map(|(n, _, _)| *n)
}

/// numparam's name for `dist`.
pub(super) fn dist_spelling(dist: Dist) -> &'static str {
    FUNCS
        .iter()
        .find(|(_, named, _)| *named == Named::Draw(dist))
        .map(|(n, _, _)| *n)
        .expect("every Dist has a numparam name")
}

/// The level `op` binds at in `eval`, or `None` when `eval` has no such operator.
pub(super) const fn level(eval: Eval, op: BinOp) -> Option<u8> {
    use BinOp as B;
    let numparam = matches!(eval, Eval::Numparam);
    Some(match op {
        B::Mul | B::Div => 3,
        B::Add | B::Sub => 4,
        B::Lt | B::Le | B::Gt | B::Ge => 5,
        B::Eq | B::Ne if numparam => 5,
        B::Eq | B::Ne => 6,
        B::And if numparam => 6,
        B::And => 7,
        B::Or if numparam => 7,
        B::Or => 8,
        B::IntDiv | B::BitAnd | B::BitOr | B::BitXor | B::Shl | B::Shr => return None,
    })
}

/// The level of the power operator (`**`, `^`) in `eval`.
pub(super) const fn power(eval: Eval) -> u8 {
    match eval {
        Eval::Numparam => 2,
        Eval::Behavioural => 1,
    }
}

/// The set of `features` (a `const` builder for the areas' `FEATURES`).
pub(super) const fn feature_set(features: &[Feature]) -> FeatureSet {
    let mut s = FeatureSet::EMPTY;
    let mut k = 0;
    // Bounded by the list's length.
    while k < features.len() {
        s = s.with(features[k]);
        k += 1;
    }
    s
}
