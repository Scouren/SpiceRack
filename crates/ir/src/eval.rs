//! The reference evaluator: an expression and its bindings in, an `f64` or the domain error out.
//! It is the oracle for every expression rule (each `Func`, `BinOp`, `UnOp` member's doc is its
//! specification); writers never call it, and nothing folds constants with it.
//!
//! C semantics on IEEE-754 doubles; the ternary, `And` and `Or` evaluate only what they select;
//! a non-finite intermediate is a domain error. The walk is an explicit stack (C11): depth is
//! bounded by memory, not by the thread's stack. A user call binds its formal arguments (the
//! `Param` reads whose def is the function's) to the evaluated arguments; call nesting deeper
//! than [`CALLS_MAX`] is [`Domain::CallDepth`] (a recursive `.func`).
//!
//! Cites: `rewrite/IR-KERNEL.md` §7, C11; `DESIGN-neutral-expressions.md` §2 (semantics), §5 N1.
//!
//! Contents: `pub trait Bindings` (`Bindings::param`, `Bindings::voltage`, `Bindings::current`,
//! `Bindings::time`, `Bindings::temp`, `Bindings::freq`, `Bindings::tstep`, `Bindings::tstop`,
//! `Bindings::tnom`, `Bindings::draw`), `pub enum Domain`, `pub const CALLS_MAX`,
//! `pub fn eval(&Design, ExprId, &dyn Bindings) -> Result<f64, Domain>`

use crate::design::Design;
use crate::expr::{Dist, DrawPer, DrawWhen};
use crate::id::{ExprId, NameId, Ref};

/// What an expression reads from outside itself. Every method returns `None` when the value is
/// not bound in the caller's context (a probe in a parameter context, a draw outside sampling
/// when the caller wants that refused); `None` makes [`eval`] return [`Domain::Unbound`].
pub trait Bindings {
    /// The value of a parameter read (by its `Ref`; unresolved refs arrive with `def == NONE`).
    /// Never called for a user function's formal argument: `eval` binds those.
    fn param(&self, param: Ref) -> Option<f64>;
    /// `v(net, reference)`, V; `reference` `NONE` is ground.
    fn voltage(&self, net: NameId, reference: NameId) -> Option<f64>;
    /// `i(element)`, A, positive into the element's first terminal.
    fn current(&self, element: NameId) -> Option<f64>;
    /// The simulation time, s.
    fn time(&self) -> Option<f64>;
    /// The circuit temperature, °C.
    fn temp(&self) -> Option<f64>;
    /// The analysis frequency, Hz.
    fn freq(&self) -> Option<f64>;
    /// The transient step, s.
    fn tstep(&self) -> Option<f64>;
    /// The transient stop time, s.
    fn tstop(&self) -> Option<f64>;
    /// The nominal temperature, °C.
    fn tnom(&self) -> Option<f64>;
    /// A draw's value given its evaluated arguments: the nominal or a sample, by the caller's run
    /// context and the draw's `when` and `per`. `args.len()` is within `dist.arity()`.
    fn draw(&self, dist: Dist, when: DrawWhen, per: DrawPer, args: &[f64]) -> Option<f64>;
}

/// Why an expression has no value. Each carries the row where evaluation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// A division (`Div`, `IntDiv`, `Fmod`) by zero.
    DivideByZero(ExprId),
    /// An argument outside the domain its `Func` or operator states, or a non-finite result.
    OutOfDomain(ExprId),
    /// A read the bindings do not bind: parameter, probe, time-like leaf, draw, or a call of an
    /// unresolved user function.
    Unbound(ExprId),
    /// User calls nested deeper than [`CALLS_MAX`].
    CallDepth(ExprId),
    /// A value of the wrong type where a number is needed (a `Str` or `List` in arithmetic).
    NotANumber(ExprId),
}

/// Deepest user-function call nesting `eval` follows.
pub const CALLS_MAX: usize = 1024;

/// The value of `e` under `env`. Pure: the same design, id and bindings give the same result.
/// `Ddt` and `Idt` need a time history no `Bindings` gives: they are [`Domain::Unbound`].
pub fn eval(design: &Design, e: ExprId, env: &dyn Bindings) -> Result<f64, Domain> {
    todo!()
}
