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
//! Semantics are real IEEE-754 doubles with C's library functions. A result that is not finite,
//! or an argument outside the domain each member states, is a domain error (`eval::Domain`); a
//! constant expression is never folded by this crate.
//!
//! Row encoding (`sub` is the operator, function or distribution byte; unused words are 0):
//!
//! | tag | `sub` | `a` | `b` |
//! |---|---|---|---|
//! | `Num` | 0 | low 32 bits of the `f64` | high 32 bits |
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
//! | `Draw` | `Dist` | `ExprList.start` | `ExprList.len` |
//! | `List` | 0 | `ExprList.start` | `ExprList.len` |
//!
//! Cites: `rewrite/IR-KERNEL.md` §7 and Addendum A1; `DESIGN-neutral-expressions.md` §2;
//! `rewrite/INV-constructs.md` C8–C30, C51–C53, C61–C76.
//!
//! Contents:
//! - `pub enum ExprTag`, `pub enum Expr`, `pub enum BinOp`, `pub enum UnOp`, `pub enum Func`,
//!   `pub enum Dist`; on each enum but `Expr`: `ALL`, `COUNT`
//! - `Func::arity(self) -> (u8, u8)`, `Func::reads_circuit(self) -> bool`,
//!   `Dist::arity(self) -> (u8, u8)`, `Expr::tag(self) -> ExprTag`
//! - `pub(crate) struct Exprs`, `pub(crate) struct UserCall`

use crate::id::{ExprId, ExprList, NameId, Ref};

/// An expression row's tag. Each tag's `sub`, `a` and `b` are read as the table in the module
/// header says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExprTag {
    /// A finite number (C9). SI base units where the field states one.
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
    /// A finite number.
    Num(f64),
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
    /// A distribution and its arguments.
    Draw(Dist, ExprList),
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
    /// `trunc(a / b)`, toward zero (ngspice `\`); `b == 0` is a domain error.
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
    /// `√|x|` (ngspice B-source `sqrt`).
    SqrtAbs,
    /// `eˣ`, exact; a non-finite result is a domain error.
    Exp,
    /// `eˣ` for `x <= 14`, `e¹⁴·(1 + x − 14)` beyond (ngspice B-source `exp`).
    ExpLin14,
    /// Natural logarithm; `x <= 0` is a domain error.
    Ln,
    /// `ln|x|` (ngspice B-source `ln`, `log`); `x == 0` is a domain error.
    LnAbs,
    /// Base-10 logarithm; `x <= 0` is a domain error.
    Log10,
    /// `pow(x, y)`, the signed real power: `x < 0` with a whole `y` is `(−1)ʸ·|x|ʸ`; `x < 0`
    /// with a fractional `y`, and `0` to a negative power, are domain errors (HSPICE, LTspice,
    /// Spectre, VACASK `**`/`pow`).
    Pow,
    /// `|x|ʸ` (ngspice `**`, `^`, B-source `pow`); `x == 0, y < 0` is a domain error.
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
    /// The unit step: 1 for `x > 0`, else 0 (ngspice `u`).
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
}

/// A user-function call's payload (two words do not hold a reference and a list).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UserCall {
    pub(crate) func: Ref,
    pub(crate) args: ExprList,
}

const _: () = assert!(size_of::<ExprTag>() == 1);
const _: () = assert!(size_of::<ExprTag>() + 1 + 2 * size_of::<u32>() == 10);
const _: () = assert!(size_of::<UserCall>() == 16);
const _: () = assert!(Func::COUNT <= u8::MAX as usize);

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
        todo!()
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
    pub const ALL: [Func; 40] = [
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
        todo!()
    }

    /// Whether the function reads the circuit's time history (`Ddt`, `Idt`), so it is legal only
    /// where an expression may read the circuit.
    pub const fn reads_circuit(self) -> bool {
        todo!()
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
        todo!()
    }
}
