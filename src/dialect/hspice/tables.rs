//! The facts of HSPICE's expression language as data: each function name's meaning and the
//! binding level of each operator. `expr_read.rs` and `expr_write.rs` read the same rows, so the
//! reader and the writer are inverse by construction (law L5). HSPICE has one expression
//! language wherever an expression stands (a `.param`, an element value, an `E … vol=`).
//!
//! Levels: a smaller number binds tighter; every binary level is left-associative, the power
//! too (`2**3**2` = 64, `2^3^2` = 64: settle `expr_power.sp`, ESPice only); the ternary is the
//! loosest and groups as C does (`1?0?4:8:16` = 8: `expr_logic.sp`). `**` and `^` 1, unary `-`
//! 2 (`-2**2` = −4), `* /` 3, `+ -` 4, `< <= > >=` 5, `== !=` 6, `&&` 7, `||` 8, `?:` 9. No `%`
//! and no unary `!` (`expr_percent.sp`, `expr_not.sp`: ESPice refuses both, as the VAMS
//! conformance suite's `invalid/` decks expect).
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §3 (branch `differences`); settle decks
//! `tests/fixtures/dialect/hspice/settle/expr_*.sp` (ESPice 0.3.0 in HSPICE mode: ESPice is not
//! HSPICE) and the recorded HSPICE pin `nint.pin` (F68).
//!
//! Contents: `pub(super) enum Named`, `pub(super) const FUNCS: &[(&str, Named)]`,
//! `pub(super) const NO_MEMBER: &[&str]`, `pub(super) const TERNARY: u8`,
//! `pub(super) const POWER: u8`, `pub(super) const UNARY: u8`,
//! `pub(super) const DRAW: (DrawWhen, DrawPer)`,
//! `pub(super) fn named(&str) -> Option<Named>`, `pub(super) fn no_member(&str) -> bool`,
//! `pub(super) fn func_spelling(ir::Func) -> Option<&'static str>`,
//! `pub(super) fn dist_spelling(ir::Dist) -> &'static str`,
//! `pub(super) const fn level(ir::BinOp) -> Option<u8>`,
//! `pub(super) const fn feature_set(&[Feature]) -> FeatureSet`

use crate::ir::{BinOp, Dist, DrawPer, DrawWhen, Feature, FeatureSet, Func};

/// What a function name means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Named {
    /// A built-in function.
    Func(Func),
    /// A statistical draw: its nominal outside Monte Carlo ([`DRAW`]).
    Draw(Dist),
}

/// Every function name and its meaning; the first row of a member is its spelling. `log` is
/// natural and `ln` the same (`expr_logs.sp`, `expr_ln.sp`); `pow` is C's `pow` (`pow(-2,3)` =
/// −8, `expr_power.sp`); `int` truncates, `floor`, `ceil` (`expr_round.sp`); `nint` rounds half
/// away from zero (recorded HSPICE `nint.pin`; ESPice rounds half to even, F68).
pub(super) const FUNCS: &[(&str, Named)] = &[
    ("abs", Named::Func(Func::Abs)),
    ("sqrt", Named::Func(Func::Sqrt)),
    ("exp", Named::Func(Func::Exp)),
    ("log", Named::Func(Func::Ln)),
    ("ln", Named::Func(Func::Ln)),
    ("log10", Named::Func(Func::Log10)),
    ("sin", Named::Func(Func::Sin)),
    ("cos", Named::Func(Func::Cos)),
    ("tan", Named::Func(Func::Tan)),
    ("asin", Named::Func(Func::Asin)),
    ("acos", Named::Func(Func::Acos)),
    ("atan", Named::Func(Func::Atan)),
    ("sinh", Named::Func(Func::Sinh)),
    ("cosh", Named::Func(Func::Cosh)),
    ("tanh", Named::Func(Func::Tanh)),
    ("pow", Named::Func(Func::Pow)),
    ("min", Named::Func(Func::Min)),
    ("max", Named::Func(Func::Max)),
    ("int", Named::Func(Func::Trunc)),
    ("nint", Named::Func(Func::RoundHalfAway)),
    ("sgn", Named::Func(Func::Sgn)),
    ("ceil", Named::Func(Func::Ceil)),
    ("floor", Named::Func(Func::Floor)),
    ("agauss", Named::Draw(Dist::AbsGauss)),
    ("gauss", Named::Draw(Dist::RelGauss)),
    ("aunif", Named::Draw(Dist::AbsUnif)),
    ("unif", Named::Draw(Dist::RelUnif)),
    ("limit", Named::Draw(Dist::Limit)),
];

/// Function names HSPICE has whose meaning is not settled: `pwr` (ESPice `pwr(-2,3)` = 8, the
/// manual lead sign·|x|^y gives −8) and `sign` (ESPice `sign(-3,2)` = 0, the manual lead 3). An
/// expression using one is `Unread` until a run on HSPICE settles it.
pub(super) const NO_MEMBER: &[&str] = &["pwr", "sign"];

/// The ternary's level: the loosest.
pub(super) const TERNARY: u8 = 9;

/// The power operators' level (`**`, `^`): the tightest.
pub(super) const POWER: u8 = 1;

/// A prefix `-` or `+`: looser than the power, tighter than every other binary operator.
pub(super) const UNARY: u8 = 2;

/// How a draw draws: its nominal outside a Monte Carlo run (`expr_random.sp`: `agauss`, `gauss`,
/// `aunif`, `unif`, `limit` give the nominal 1; ESPice only), the same as Xyce. The per-sample
/// rule inside Monte Carlo is unrun: `Reference` is the old notes' lead.
pub(super) const DRAW: (DrawWhen, DrawPer) = (DrawWhen::Sampling, DrawPer::Reference);

/// What the function `name` (any case) means.
pub(super) fn named(name: &str) -> Option<Named> {
    FUNCS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, named)| *named)
}

/// Whether `name` (any case) is a function HSPICE has but the IR has no settled member for.
pub(super) fn no_member(name: &str) -> bool {
    NO_MEMBER.iter().any(|n| n.eq_ignore_ascii_case(name))
}

/// The name HSPICE spells `func` with, or `None` when it has none (`Pow` is the operator `**`,
/// spelled by the writer).
pub(super) fn func_spelling(func: Func) -> Option<&'static str> {
    FUNCS
        .iter()
        .find(|(_, named)| *named == Named::Func(func))
        .map(|(n, _)| *n)
}

/// HSPICE's name for `dist`.
pub(super) fn dist_spelling(dist: Dist) -> &'static str {
    FUNCS
        .iter()
        .find(|(_, named)| *named == Named::Draw(dist))
        .map(|(n, _)| *n)
        .expect("every Dist has an HSPICE name")
}

/// The level `op` binds at, or `None` when HSPICE has no such operator.
pub(super) const fn level(op: BinOp) -> Option<u8> {
    use BinOp as B;
    Some(match op {
        B::Mul | B::Div => 3,
        B::Add | B::Sub => 4,
        B::Lt | B::Le | B::Gt | B::Ge => 5,
        B::Eq | B::Ne => 6,
        B::And => 7,
        B::Or => 8,
        B::IntDiv | B::DivTyped | B::BitAnd | B::BitOr | B::BitXor | B::Shl | B::Shr => {
            return None;
        }
    })
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
