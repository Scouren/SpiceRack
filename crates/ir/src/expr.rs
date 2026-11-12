//! Expressions: the neutral language as rows. Builder calls in, `{tag, sub, a, b}` SoA rows out,
//! decoded one at a time into the [`Expr`] view for writers, passes and the evaluator.
//!
//! One node per native meaning, the most specific one (Addendum A1): a reader maps each of its
//! simulator's spellings to the member its simulator means (`log` → [`Func::Ln`] or
//! [`Func::Log10`], ngspice `**` → [`Func::PowAbs`], Xyce `**` → [`Func::PowRe`]), and its writer
//! maps back. There is no `limit`, `pwr`, `pwrs`, `if`, `nint`, `log`, logical XOR or named
//! constant in the IR. Precedence and grouping are a reader's and a writer's concern: the IR holds
//! trees. Operators are binary rows; every named function, including the power laws, is a
//! [`Func`] call (so `x**y` and `pow(x, y)` of one simulator are one node).
//!
//! A number is a decimal literal as written (Addendum A3): a [`Literal`] in the literal side
//! table, never a double; each simulator computes its own double from the digits. Semantics
//! (the evaluator, the passes) compute with [`Literal::value`], the correctly rounded value, on
//! real IEEE-754 doubles with C's library functions. A result that is not finite,
//! or an argument outside the domain each member states, is a domain error (`eval::Domain`); a
//! constant expression is never folded by this crate.
//!
//! Row encoding (`sub` is the operator, function or distribution byte; unused words are 0):
//!
//! | tag | `sub` | `a` | `b` |
//! |---|---|---|---|
//! | `Num` | 0 | `NumId` of the literal | 0 |
//! | `Str` | 0 | `NameId` of the string | 0 |
//! | `Param` | 0 | `NameId` | `DefId` (`NONE`: unresolved) |
//! | `Call` | `Func` | `ExprList.start` | `ExprList.len` |
//! | `UserCall` | 0 | index into the user-call side table | 0 |
//! | `Unary` | `UnOp` | operand `ExprId` | 0 |
//! | `Binary` | `BinOp` | left `ExprId` | right `ExprId` |
//! | `Ternary` | 0 | condition `ExprId` | start in the expression pool of `[then, else]` |
//! | `V` | 0 | net `NameId` | reference net `NameId` (`NONE`: ground) |
//! | `I` | 0 | element `NameId` | 0 |
//! | `Time`, `Temp`, `Freq`, `TStep`, `TStop`, `Tnom` | 0 | 0 | 0 |
//! | `Draw` | `Dist` + `DrawWhen` + `DrawPer`, packed | `ExprList.start` | `ExprList.len` |
//! | `List` | 0 | `ExprList.start` | `ExprList.len` |
//!
//! A `Draw`'s `sub` byte packs three enums: bits 0–3 the `Dist`, bit 4 the `DrawWhen`, bits 5–6
//! the `DrawPer`, bit 7 zero.
//!
//! Cites: `rewrite/IR-KERNEL.md` §7, Addenda A1, A2 (draws) and A3 (literals);
//! `rewrite/INV-semantics.md` 1.08, 1.09; `rewrite/INV-semantics.md` 12.02,
//! 12.03; `DESIGN-neutral-expressions.md` §2;
//! `rewrite/INV-constructs.md` C8–C30, C51–C53, C61–C76.
//!
//! Contents:
//! - `pub enum ExprTag`, `pub enum Expr`, `pub enum BinOp`, `pub enum UnOp`, `pub enum Func`,
//!   `pub enum Dist`, `pub enum DrawWhen`, `pub enum DrawPer`, `pub enum NumForm`; on each enum
//!   but `Expr`: `ALL`, `COUNT`
//! - `pub struct Literal`, `Literal::ZERO`, `Literal::value(self) -> f64`,
//!   `Literal::from_f64(f64) -> Literal`, `Literal::eq_value(self, Literal) -> bool`,
//!   `Literal::is_valid(self) -> bool`, crate: `Literal::shape_ok(self) -> bool`,
//!   `Literal::finite(self) -> bool`
//! - `Func::arity(self) -> (u8, u8)`, `Func::reads_circuit(self) -> bool`,
//!   `Dist::arity(self) -> (u8, u8)`, `Expr::tag(self) -> ExprTag`
//! - `pub(crate) struct Exprs`, `pub(crate) struct UserCall`

use crate::id::{ExprId, ExprList, NameId, NumId, Ref};

/// An expression row's tag. Each tag's `sub`, `a` and `b` are read as the table in the module
/// header says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExprTag {
    /// A decimal literal ([`Literal`]); SI base units where the field states one.
    Num,
    /// A string value (a model `version="4.8"`, a file name).
    Str,
    /// A parameter read by name.
    Param,
    /// A built-in function call ([`Func`]); argument count within `Func::arity`.
    Call,
    /// A call of a user function (`item::Func`).
    UserCall,
    /// A unary operator ([`UnOp`]).
    Unary,
    /// A binary operator ([`BinOp`]).
    Binary,
    /// `cond ? then : else`; only the chosen branch is evaluated.
    Ternary,
    /// The voltage of a net, or between two nets. Reads the circuit.
    V,
    /// The branch current of an element, positive into its first terminal. Reads the circuit.
    I,
    /// The simulation time, s. Reads the circuit.
    Time,
    /// The circuit temperature, °C.
    Temp,
    /// The analysis frequency, Hz. Reads the circuit.
    Freq,
    /// The transient analysis's step (Addendum A1 leaf; a pass substitutes the number).
    TStep,
    /// The transient analysis's stop time (Addendum A1 leaf).
    TStop,
    /// The nominal temperature, °C (Addendum A1 leaf).
    Tnom,
    /// A statistical draw ([`Dist`]).
    Draw,
    /// A vector of values (Spectre `[a b]`, VACASK `[a, b]`), elements of one type.
    List,
}

/// One expression row, decoded. `Copy`, borrows nothing; lists are read back with
/// `Design::exprs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Expr {
    /// A decimal literal (`Design::num`).
    Num(NumId),
    /// A string.
    Str(NameId),
    /// A parameter read: name and binding.
    Param(Ref),
    /// A built-in function and its arguments.
    Call(Func, ExprList),
    /// A user function (binding to a `DefKind::Func`) and its arguments.
    UserCall(Ref, ExprList),
    /// An operator and its operand.
    Unary(UnOp, ExprId),
    /// An operator and its left and right operands.
    Binary(BinOp, ExprId, ExprId),
    /// Condition, then, else.
    Ternary(ExprId, ExprId, ExprId),
    /// Net and reference net (`NONE`: ground).
    V(NameId, NameId),
    /// The element whose branch current is read.
    I(NameId),
    /// The simulation time.
    Time,
    /// The circuit temperature.
    Temp,
    /// The analysis frequency.
    Freq,
    /// The transient step.
    TStep,
    /// The transient stop time.
    TStop,
    /// The nominal temperature.
    Tnom,
    /// A distribution, when it is drawn, how often, and its arguments.
    Draw(Dist, DrawWhen, DrawPer, ExprList),
    /// The elements.
    List(ExprList),
}

/// A binary operator. Truth is "non-zero"; comparisons and logical operators yield 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BinOp {
    /// `a + b`.
    Add,
    /// `a - b`.
    Sub,
    /// `a * b`.
    Mul,
    /// `a / b`; `b == 0` is a domain error.
    Div,
    /// `trunc(a / b)`, toward zero; `b == 0` is a domain error. Not ngspice's `\` (which is
    /// `trunc(|a / b|)`, read as `Trunc(Abs(Div))`, A11); no speaker yet: removed if no reader
    /// produces it.
    IntDiv,
    /// `a == b` (exact).
    Eq,
    /// `a != b` (exact).
    Ne,
    /// `a < b`.
    Lt,
    /// `a <= b`.
    Le,
    /// `a > b`.
    Gt,
    /// `a >= b`.
    Ge,
    /// Both non-zero. `b` is evaluated only when `a` is non-zero (C).
    And,
    /// Either non-zero. `b` is evaluated only when `a` is zero (C).
    Or,
    /// Bitwise AND of integer-valued operands (VACASK `&`); a fractional operand is a domain
    /// error.
    BitAnd,
    /// Bitwise OR of integer-valued operands (VACASK `|`).
    BitOr,
    /// Bitwise XOR of integer-valued operands (VACASK `^`).
    BitXor,
    /// `a << b`, integer-valued, `0 <= b < 64`.
    Shl,
    /// `a >> b` (arithmetic), integer-valued, `0 <= b < 64`.
    Shr,
}

/// A unary operator. Unary plus is no node (the reader drops it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnOp {
    /// `-a`.
    Neg,
    /// 1 when `a` is zero, else 0.
    Not,
    /// Bitwise complement of an integer-valued operand (VACASK `~`).
    BitNot,
}

/// A built-in function: one meaning each, its domain stated (Addendum A1). Arguments are reals;
/// angles in radians.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Func {
    /// `|x|`.
    Abs,
    /// `√x`; `x < 0` is a domain error.
    Sqrt,
    /// `√|x|`. Not ngspice-45's B-source `sqrt`, which errors below zero (A11); speaker to be
    /// confirmed by the HSPICE and LTspice slices: removed if no reader produces it.
    SqrtAbs,
    /// `eˣ`, exact; a non-finite result is a domain error.
    Exp,
    /// `eˣ` for `x <= 14`, `e¹⁴·(1 + x − 14)` beyond (ngspice's B-source `exp` under
    /// `ngbehavior=ps` only, A11).
    ExpLin14,
    /// Natural logarithm; `x <= 0` is a domain error.
    Ln,
    /// `ln|x|`; `x == 0` is a domain error. Not ngspice-45's B-source `ln`/`log`, which error
    /// below zero (A11); speaker to be confirmed by the HSPICE and LTspice slices: removed if no
    /// reader produces it.
    LnAbs,
    /// Base-10 logarithm; `x <= 0` is a domain error.
    Log10,
    /// `pow(x, y)`, the signed real power: `x < 0` with a whole `y` is `(−1)ʸ·|x|ʸ`; `x < 0`
    /// with a fractional `y`, and `0` to a negative power, are domain errors (HSPICE, LTspice,
    /// Spectre, VACASK `**`/`pow`).
    Pow,
    /// `|x|ʸ` (ngspice B-source `**`, `^`, `pow`); `x == 0, y < 0` is a domain error.
    PowAbs,
    /// The real part of the complex power: `Pow` where `Pow` is defined, `|x|ʸ·cos(πy)` for
    /// `x < 0` and a fractional `y` (Xyce `**`, `pow`, `pwr`).
    PowRe,
    /// The smaller of two arguments.
    Min,
    /// The larger of two arguments.
    Max,
    /// The largest whole number `<= x`.
    Floor,
    /// The smallest whole number `>= x`.
    Ceil,
    /// `x` rounded toward zero (`int`).
    Trunc,
    /// `x` rounded to the nearest whole number, halves away from zero.
    RoundHalfAway,
    /// `x` rounded to the nearest whole number, halves to even (ngspice `nint`).
    RoundHalfEven,
    /// −1, 0 or 1 by the sign of `x`.
    Sgn,
    /// The half-maximum step: 0 for `x < 0`, 0.5 at `x == 0`, 1 for `x > 0` (ngspice B-source
    /// `u`, `ptfuncs.c` `PTustep`).
    StepHalf,
    /// The signed power: `−pow(−x, y)` for `x < 0`, else `pow(x, y)`; `x == 0, y < 0` is a
    /// domain error (ngspice B-source `pwr`, `PTpwr`).
    PowSigned,
    /// `x·x`, one multiplication (ngspice numparam `sqr`, `xpressn.c`).
    Sqr,
    /// Sine.
    Sin,
    /// Cosine.
    Cos,
    /// Tangent; a non-finite result is a domain error.
    Tan,
    /// Arc sine; `|x| > 1` is a domain error.
    Asin,
    /// Arc cosine; `|x| > 1` is a domain error.
    Acos,
    /// Arc tangent, in `(−π/2, π/2)`.
    Atan,
    /// `atan2(y, x)` (C), in `[−π, π]`; `atan2(0, 0)` is 0.
    Atan2,
    /// Hyperbolic sine.
    Sinh,
    /// Hyperbolic cosine.
    Cosh,
    /// Hyperbolic tangent.
    Tanh,
    /// Inverse hyperbolic sine.
    Asinh,
    /// Inverse hyperbolic cosine; `x < 1` is a domain error.
    Acosh,
    /// Inverse hyperbolic tangent; `|x| >= 1` is a domain error.
    Atanh,
    /// `√(x² + y²)` without intermediate overflow.
    Hypot,
    /// `fmod(x, y)` (C): the remainder of `x / y` truncated, with the sign of `x` (also `%`);
    /// `y == 0` is a domain error.
    Fmod,
    /// `clamp(x, lo, hi)` = `min(max(x, lo), hi)`; `lo > hi` is a domain error.
    Clamp,
    /// The unit step: 1 for `x > 0`, else 0. Not ngspice's `u` (that is `StepHalf`, A11);
    /// speaker to be confirmed by the LTspice and Xyce slices.
    Step,
    /// The unit ramp: `x` for `x > 0`, else 0 (ngspice `uramp`).
    Ramp,
    /// `d x / dt`, the time derivative. Reads the circuit (only in an expression that may).
    Ddt,
    /// `∫₀ᵗ x dt`, from 0 at `t = 0`. Reads the circuit.
    Idt,
    /// `pwl(x, x₁, y₁, …, xₙ, yₙ)`: linear interpolation in `x` through the points (`xᵢ`
    /// strictly increasing, `n >= 1`), held constant beyond the ends (the neutral `table`).
    Pwl,
}

/// A statistical distribution of a draw. Outside sampling a draw's value is a simulator rule;
/// the evaluator asks its `Bindings` for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Dist {
    /// `(nom, abs_var, n_sigma)`: normal, mean `nom`, deviation `abs_var / n_sigma` (`agauss`).
    AbsGauss,
    /// `(nom, rel_var, n_sigma)`: normal, mean `nom`, deviation `nom·rel_var / n_sigma`
    /// (`gauss`).
    RelGauss,
    /// `(nom, abs_var)`: uniform on `[nom − abs_var, nom + abs_var]` (`aunif`).
    AbsUnif,
    /// `(nom, rel_var)`: uniform on `[nom·(1 − rel_var), nom·(1 + rel_var)]` (`unif`).
    RelUnif,
    /// `(nom, abs_var)`: `nom + abs_var` or `nom − abs_var`, equally likely (ngspice `limit`).
    Limit,
}

/// When a draw yields a sample rather than its nominal (`INV-semantics.md` 12.02).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrawWhen {
    /// A sample in every run, a plain one too (ngspice: `agauss` draws even outside Monte Carlo).
    Always,
    /// A sample only in a sampling run (Monte Carlo, Xyce `.SAMPLING`); otherwise its nominal,
    /// the distribution's mean (Xyce and HSPICE `agauss`).
    Sampling,
}

/// How often a draw yields a new sample, when it samples (`INV-semantics.md` 12.03).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DrawPer {
    /// At every reference: each evaluation of the expression draws anew (ngspice: a one-layer
    /// `.param` redraws wherever it is read, hence the two-layer idiom).
    Reference,
    /// Once per call site per scope (VACASK).
    CallSite,
    /// Once per element instance (Xyce `.SAMPLING useExpr=true` in an instance value; Spectre's
    /// `mismatch`).
    Instance,
    /// Once per run, shared by every reader (Spectre's `process`).
    Run,
}

/// A decimal literal, as written: `mantissa × 10^(exp10 − frac)` (`Mil`: that times 25.4e-6).
/// The IR states the number; the double is each simulator's own arithmetic on these digits.
///
/// Invariants (asserted by `Builder::literal`, checked by `check`): `|mantissa| < 10¹⁸`;
/// `frac <= 18`; `Plain` and `Mil` have `exp10 == 0`; a `Scale` has an `exp10` some scale letter
/// states (−18, −15, −12, −9, −6, −3, −2, 3, 6, 9, 12, 15); zero is the one literal
/// [`Literal::ZERO`]; `value()` is finite. Equality (`==`) is field by field (law L5): `1k` and
/// `1e3` differ; [`Literal::eq_value`] compares meanings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Literal {
    /// Every digit as one integer, sign included (`-0.050000`: −50000).
    pub mantissa: i64,
    /// Digits after the decimal point, trailing zeros counted (`0.050000`: 6).
    pub frac: u8,
    /// The exponent, or the scale suffix's power of ten (`100u`: −6).
    pub exp10: i16,
    /// How the source wrote the power of ten.
    pub form: NumForm,
}

/// How a literal wrote its power of ten. A writer keeps the form where its dialect has it
/// (its own letter for a `Scale`), else spells `e<exp10>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NumForm {
    /// No exponent and no suffix (`0.05`); `exp10` is 0.
    Plain,
    /// An exponent (`1e-4`, `2.5E3`); also a suffix combined with an exponent (`1e3k`, folded by
    /// the reader).
    Exponent,
    /// A scale suffix (`100u`, `1meg`); `exp10` is the suffix's power of ten.
    Scale,
    /// The `mil` suffix: the digits times 25.4e-6 m; `exp10` is 0.
    Mil,
}

/// The expression rows (SoA). Invariant: the four columns have equal length; every child id is
/// lower than its parent's.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Exprs {
    pub(crate) tags: Vec<ExprTag>,
    pub(crate) subs: Vec<u8>,
    pub(crate) a: Vec<u32>,
    pub(crate) b: Vec<u32>,
    /// The `UserCall` side table.
    pub(crate) calls: Vec<UserCall>,
    /// The literal side table (`NumId`).
    pub(crate) nums: Vec<Literal>,
}

/// A user-function call's payload (two words do not hold a reference and a list).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UserCall {
    pub(crate) func: Ref,
    pub(crate) args: ExprList,
}

/// The powers of ten a scale letter states (A3): a `Scale` literal's `exp10` is one of these.
const SCALES: [i16; 12] = [-18, -15, -12, -9, -6, -3, -2, 3, 6, 9, 12, 15];
/// `|mantissa|` is below this (10^18).
const MANTISSA_BOUND: u64 = 1_000_000_000_000_000_000;

const _: () = assert!(size_of::<ExprTag>() == 1);
const _: () = assert!(size_of::<ExprTag>() + 1 + 2 * size_of::<u32>() == 10);
const _: () = assert!(size_of::<UserCall>() == 16);
const _: () = assert!(size_of::<Literal>() == 16);
const _: () = assert!(Func::COUNT <= u8::MAX as usize);
const _: () = assert!(Dist::COUNT <= 16 && DrawWhen::COUNT <= 2 && DrawPer::COUNT <= 4);

impl ExprTag {
    /// Every tag, in declaration order.
    pub const ALL: [ExprTag; 18] = [
        ExprTag::Num,
        ExprTag::Str,
        ExprTag::Param,
        ExprTag::Call,
        ExprTag::UserCall,
        ExprTag::Unary,
        ExprTag::Binary,
        ExprTag::Ternary,
        ExprTag::V,
        ExprTag::I,
        ExprTag::Time,
        ExprTag::Temp,
        ExprTag::Freq,
        ExprTag::TStep,
        ExprTag::TStop,
        ExprTag::Tnom,
        ExprTag::Draw,
        ExprTag::List,
    ];
    /// Tag count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Expr {
    /// The row tag this view decodes.
    pub const fn tag(self) -> ExprTag {
        match self {
            Expr::Num(_) => ExprTag::Num,
            Expr::Str(_) => ExprTag::Str,
            Expr::Param(_) => ExprTag::Param,
            Expr::Call(..) => ExprTag::Call,
            Expr::UserCall(..) => ExprTag::UserCall,
            Expr::Unary(..) => ExprTag::Unary,
            Expr::Binary(..) => ExprTag::Binary,
            Expr::Ternary(..) => ExprTag::Ternary,
            Expr::V(..) => ExprTag::V,
            Expr::I(_) => ExprTag::I,
            Expr::Time => ExprTag::Time,
            Expr::Temp => ExprTag::Temp,
            Expr::Freq => ExprTag::Freq,
            Expr::TStep => ExprTag::TStep,
            Expr::TStop => ExprTag::TStop,
            Expr::Tnom => ExprTag::Tnom,
            Expr::Draw(..) => ExprTag::Draw,
            Expr::List(_) => ExprTag::List,
        }
    }
}

impl BinOp {
    /// Every operator, in declaration order.
    pub const ALL: [BinOp; 18] = [
        BinOp::Add,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::Div,
        BinOp::IntDiv,
        BinOp::Eq,
        BinOp::Ne,
        BinOp::Lt,
        BinOp::Le,
        BinOp::Gt,
        BinOp::Ge,
        BinOp::And,
        BinOp::Or,
        BinOp::BitAnd,
        BinOp::BitOr,
        BinOp::BitXor,
        BinOp::Shl,
        BinOp::Shr,
    ];
    /// Operator count.
    pub const COUNT: usize = Self::ALL.len();
}

impl UnOp {
    /// Every operator, in declaration order.
    pub const ALL: [UnOp; 3] = [UnOp::Neg, UnOp::Not, UnOp::BitNot];
    /// Operator count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Func {
    /// Every function, in declaration order.
    pub const ALL: [Func; 43] = [
        Func::Abs,
        Func::Sqrt,
        Func::SqrtAbs,
        Func::Exp,
        Func::ExpLin14,
        Func::Ln,
        Func::LnAbs,
        Func::Log10,
        Func::Pow,
        Func::PowAbs,
        Func::PowRe,
        Func::Min,
        Func::Max,
        Func::Floor,
        Func::Ceil,
        Func::Trunc,
        Func::RoundHalfAway,
        Func::RoundHalfEven,
        Func::Sgn,
        Func::StepHalf,
        Func::PowSigned,
        Func::Sqr,
        Func::Sin,
        Func::Cos,
        Func::Tan,
        Func::Asin,
        Func::Acos,
        Func::Atan,
        Func::Atan2,
        Func::Sinh,
        Func::Cosh,
        Func::Tanh,
        Func::Asinh,
        Func::Acosh,
        Func::Atanh,
        Func::Hypot,
        Func::Fmod,
        Func::Clamp,
        Func::Step,
        Func::Ramp,
        Func::Ddt,
        Func::Idt,
        Func::Pwl,
    ];
    /// Function count.
    pub const COUNT: usize = Self::ALL.len();

    /// The argument count range `(min, max)`; `max == u8::MAX` means unbounded (`Pwl`: 3, 5, 7,
    /// …). The one fact table `check` and the parsers use; exhaustive over members.
    pub const fn arity(self) -> (u8, u8) {
        match self {
            Func::Abs
            | Func::Sqrt
            | Func::SqrtAbs
            | Func::Exp
            | Func::ExpLin14
            | Func::Ln
            | Func::LnAbs
            | Func::Log10
            | Func::Floor
            | Func::Ceil
            | Func::Trunc
            | Func::RoundHalfAway
            | Func::RoundHalfEven
            | Func::Sgn
            | Func::Sin
            | Func::Cos
            | Func::Tan
            | Func::Asin
            | Func::Acos
            | Func::Atan
            | Func::Sinh
            | Func::Cosh
            | Func::Tanh
            | Func::Asinh
            | Func::Acosh
            | Func::Atanh
            | Func::Step
            | Func::StepHalf
            | Func::Sqr
            | Func::Ramp
            | Func::Ddt
            | Func::Idt => (1, 1),
            Func::Pow
            | Func::PowAbs
            | Func::PowRe
            | Func::PowSigned
            | Func::Min
            | Func::Max
            | Func::Atan2
            | Func::Hypot
            | Func::Fmod => (2, 2),
            Func::Clamp => (3, 3),
            Func::Pwl => (3, u8::MAX),
        }
    }

    /// Whether the function reads the circuit's time history (`Ddt`, `Idt`), so it is legal only
    /// where an expression may read the circuit.
    pub const fn reads_circuit(self) -> bool {
        matches!(self, Func::Ddt | Func::Idt)
    }
}

impl Dist {
    /// Every distribution, in declaration order.
    pub const ALL: [Dist; 5] = [
        Dist::AbsGauss,
        Dist::RelGauss,
        Dist::AbsUnif,
        Dist::RelUnif,
        Dist::Limit,
    ];
    /// Distribution count.
    pub const COUNT: usize = Self::ALL.len();

    /// The argument count range `(min, max)`, as each member's doc lists them.
    pub const fn arity(self) -> (u8, u8) {
        match self {
            Dist::AbsGauss | Dist::RelGauss => (3, 3),
            Dist::AbsUnif | Dist::RelUnif | Dist::Limit => (2, 2),
        }
    }
}

impl DrawWhen {
    /// Every member, in declaration order.
    pub const ALL: [DrawWhen; 2] = [DrawWhen::Always, DrawWhen::Sampling];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl DrawPer {
    /// Every member, in declaration order.
    pub const ALL: [DrawPer; 4] = [
        DrawPer::Reference,
        DrawPer::CallSite,
        DrawPer::Instance,
        DrawPer::Run,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl NumForm {
    /// Every form, in declaration order.
    pub const ALL: [NumForm; 4] = [
        NumForm::Plain,
        NumForm::Exponent,
        NumForm::Scale,
        NumForm::Mil,
    ];
    /// Form count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Literal {
    /// Zero: the only literal whose mantissa is 0.
    pub const ZERO: Literal = Literal {
        mantissa: 0,
        frac: 0,
        exp10: 0,
        form: NumForm::Plain,
    };

    /// The correctly rounded double of the exact decimal value (round half to even), what the
    /// reference evaluator and the passes compute with. Not any simulator's arithmetic.
    pub fn value(self) -> f64 {
        let (m, k) = self.exact();
        // Rust's decimal parser rounds correctly (round half to even), the IR's reference rule
        let v: f64 = format!("{m}e{k}")
            .parse()
            .expect("a decimal literal parses");
        debug_assert!(!v.is_nan(), "a decimal is never NaN");
        v
    }

    /// The shortest decimal that reads back to `value` under [`Literal::value`], in engineering
    /// form: a `Scale` when the power of ten is a multiple of 3 that a scale letter states
    /// (`1e-4` → mantissa 100, `Scale`, exp10 −6), `Plain` for powers 0, `Exponent` with an exp10
    /// that is a multiple of 3 otherwise; zero is [`Literal::ZERO`]. Asserts `value` is finite
    /// (the NaN gate, C9). For built numbers only: a reader passes the digits it read.
    pub fn from_f64(value: f64) -> Literal {
        assert!(value.is_finite(), "a built number is finite");
        if value == 0.0 {
            return Literal::ZERO;
        }
        // shortest round-trip digits: `d.ddde±x`
        let sci = format!("{:e}", value.abs());
        let (digits, x) = sci.split_once('e').expect("`{:e}` has an exponent");
        let x: i32 = x.parse().expect("`{:e}`'s exponent is an integer");
        let digits: String = digits.chars().filter(|c| *c != '.').collect();
        let n = digits.len() as i32;
        let m: i64 = digits.parse().expect("at most 17 significant digits");
        // value = m × 10^p; engineering exponent e3 = the multiple of 3 at or below x
        let p = x - (n - 1);
        let e3 = x.div_euclid(3) * 3;
        let (mantissa, frac) = if p >= e3 {
            (m * 10i64.pow((p - e3) as u32), 0)
        } else {
            (m, (e3 - p) as u8)
        };
        assert!(
            mantissa.unsigned_abs() < MANTISSA_BOUND,
            "17 digits and two zeros stay under 10^18"
        );
        let form = if e3 == 0 {
            NumForm::Plain
        } else if SCALES.contains(&(e3 as i16)) {
            NumForm::Scale
        } else {
            NumForm::Exponent
        };
        let sign = if value < 0.0 { -1 } else { 1 };
        Literal {
            mantissa: sign * mantissa,
            frac,
            exp10: e3 as i16,
            form,
        }
    }

    /// Whether the two literals state the same exact decimal number (`1k` and `1e3`; `Mil`
    /// compared after its factor). Exact: no rounding to a double.
    pub fn eq_value(self, other: Literal) -> bool {
        self.normal() == other.normal()
    }

    /// Whether the literal meets every invariant on [`Literal`].
    pub fn is_valid(self) -> bool {
        self.shape_ok() && self.finite()
    }

    /// Whether the value is finite, from the digit count and the power where that decides it
    /// (the largest double is about 1.8e308), and by `value()` only at the boundary.
    pub(crate) fn finite(self) -> bool {
        let (m, k) = self.exact();
        if m == 0 {
            return true;
        }
        let digits = m.unsigned_abs().ilog10() as i32 + 1;
        match digits + k {
            ..=308 => true,
            310.. => false,
            309 => self.value().is_finite(),
        }
    }

    /// Whether the literal meets every invariant but finiteness (`check` reports the two
    /// separately: `LiteralShape`, `NonFinite`).
    pub(crate) fn shape_ok(self) -> bool {
        self.mantissa.unsigned_abs() < MANTISSA_BOUND
            && self.frac <= 18
            && match self.form {
                NumForm::Plain | NumForm::Mil => self.exp10 == 0,
                NumForm::Scale => SCALES.contains(&self.exp10),
                NumForm::Exponent => true,
            }
            && (self.mantissa != 0 || self == Literal::ZERO)
    }
}

// ---- private ----

impl Literal {
    /// The exact value as `m × 10^k` (`Mil` folded in: 25.4e-6 = 254 × 10^-7).
    fn exact(self) -> (i128, i32) {
        let k = self.exp10 as i32 - self.frac as i32;
        match self.form {
            NumForm::Mil => (self.mantissa as i128 * 254, k - 7),
            NumForm::Plain | NumForm::Exponent | NumForm::Scale => (self.mantissa as i128, k),
        }
    }

    /// The exact value with trailing zeros moved into the power: equal decimals give equal pairs.
    fn normal(self) -> (i128, i32) {
        let (mut m, mut k) = self.exact();
        if m == 0 {
            return (0, 0);
        }
        // at most 21 digits (an 18-digit mantissa times 254)
        for _ in 0..24 {
            if m % 10 != 0 {
                break;
            }
            m /= 10;
            k += 1;
        }
        debug_assert!(m % 10 != 0, "every trailing zero moved");
        (m, k)
    }
}
