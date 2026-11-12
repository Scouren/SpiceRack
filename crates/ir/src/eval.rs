//! The reference evaluator: an expression and its bindings in, an `f64` or the domain error out.
//! It is the oracle for every expression rule (each `Func`, `BinOp`, `UnOp` member's doc is its
//! specification); writers never call it, and nothing folds constants with it.
//!
//! A `Num` evaluates to its `Literal::value()`, the correctly rounded double (not any
//! simulator's arithmetic, Addendum A3). C semantics on IEEE-754 doubles; the ternary, `And` and `Or` evaluate only what they select;
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
//! `pub fn eval(&Design, ExprId, &dyn Bindings) -> Result<f64, Domain>`,
//! `pub fn eval_typed(&Design, ExprId, &dyn Bindings) -> Result<Value, Domain>`, `pub struct Value`
//! (`Value::real(f64)`, `Value::int(f64)`), `Bindings::param_value`

use crate::design::Design;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, Expr, Func, NumForm, UnOp};
use crate::id::{DefId, ExprId, NameId, Ref};
use crate::item::{DefKind, Func as FuncRow};

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

    /// A parameter read's value with its type (A12 K1): an instance may pass an Int or a Real.
    /// Default: `param` as a Real.
    fn param_value(&self, param: Ref) -> Option<Value> {
        self.param(param).map(Value::real)
    }
}

/// A value and its type (A12 K1): an Int is a whole number within ±2⁵³ computed with integer
/// rules (`DivTyped` truncates); everything else is Real.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Value {
    /// The number.
    pub number: f64,
    /// Whether it is of Int type.
    pub int: bool,
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

/// The largest magnitude an Int value may have (2⁵³; R12: no integer overflow is carried).
const INT_MAX: f64 = 9_007_199_254_740_992.0;

/// The frame index of the outermost expression (no formal arguments bound).
const NO_FRAME: u32 = u32::MAX;

/// One step of the walk. Operands are evaluated first (their values pushed in order), then the
/// operator is applied to the top of the value stack.
#[derive(Clone, Copy)]
enum Task {
    /// Evaluate `e` in `frame`, pushing one value.
    Eval(ExprId, u32),
    /// Apply `e`'s operator to its evaluated operands.
    Apply(ExprId, u32),
    /// A ternary's condition is on the stack: evaluate the chosen branch.
    Choose(ExprId, u32),
    /// `And`/`Or`'s left side is on the stack: evaluate the right side only if needed.
    Logic(ExprId, u32),
    /// The right side of `And`/`Or` is on the stack: turn it into 0 or 1.
    Truth,
    /// A user call's body has been evaluated: drop its frame.
    Return,
}

/// One user call's bound formal arguments.
struct Frame {
    def: DefId,
    names: Vec<NameId>,
    values: Vec<Value>,
}

/// The evaluator's state: the value stack, the call frames and the task stack.
struct Machine<'a> {
    design: &'a Design,
    env: &'a dyn Bindings,
    values: Vec<Value>,
    frames: Vec<Frame>,
    tasks: Vec<Task>,
}

/// The value of `e` under `env`. Pure: the same design, id and bindings give the same result.
impl Value {
    /// A Real value.
    pub const fn real(number: f64) -> Value {
        Value { number, int: false }
    }

    /// An Int value.
    pub const fn int(number: f64) -> Value {
        Value { number, int: true }
    }
}

/// `Ddt` and `Idt` need a time history no `Bindings` gives: they are [`Domain::Unbound`].
pub fn eval_typed(design: &Design, e: ExprId, env: &dyn Bindings) -> Result<Value, Domain> {
    assert!(
        !e.is_none() && e.index() < design.expr_count(),
        "an expression id in range"
    );
    let mut m = Machine {
        design,
        env,
        values: Vec::new(),
        frames: Vec::new(),
        tasks: vec![Task::Eval(e, NO_FRAME)],
    };
    while let Some(task) = m.tasks.pop() {
        m.step(task)?;
    }
    assert!(m.values.len() == 1, "an expression leaves one value");
    Ok(m.values[0])
}

/// The value of `e` under `env`, as a number (`eval_typed` without the type).
pub fn eval(design: &Design, e: ExprId, env: &dyn Bindings) -> Result<f64, Domain> {
    eval_typed(design, e, env).map(|v| v.number)
}

// ---- private ----

impl Machine<'_> {
    fn step(&mut self, task: Task) -> Result<(), Domain> {
        match task {
            Task::Eval(e, frame) => self.eval(e, frame),
            Task::Apply(e, frame) => self.apply(e, frame),
            Task::Choose(e, frame) => {
                let Expr::Ternary(_, then, other) = self.design.expr(e) else {
                    unreachable!()
                };
                let cond = self.pop().number;
                self.tasks
                    .push(Task::Eval(if cond != 0.0 { then } else { other }, frame));
                Ok(())
            }
            Task::Logic(e, frame) => {
                let Expr::Binary(op, _, right) = self.design.expr(e) else {
                    unreachable!()
                };
                let left = self.pop().number != 0.0;
                let decided = match op {
                    BinOp::And => (!left).then_some(0.0),
                    BinOp::Or => left.then_some(1.0),
                    _ => unreachable!("Logic is pushed for And and Or only"),
                };
                match decided {
                    Some(v) => self.values.push(Value::int(v)),
                    None => self.tasks.extend([Task::Truth, Task::Eval(right, frame)]),
                }
                Ok(())
            }
            Task::Truth => {
                let v = self.pop().number;
                self.values
                    .push(Value::int(if v != 0.0 { 1.0 } else { 0.0 }));
                Ok(())
            }
            Task::Return => {
                self.frames.pop().expect("a call frame to return from");
                Ok(())
            }
        }
    }

    /// Pushes the operands' tasks (or the value of a leaf).
    fn eval(&mut self, e: ExprId, frame: u32) -> Result<(), Domain> {
        let d = self.design;
        let operands = |l| d.exprs(l).iter().rev().map(move |&a| Task::Eval(a, frame));
        match d.expr(e) {
            Expr::Num(n) => {
                let l = d.num(n);
                self.values.push(Value {
                    number: l.value(),
                    int: l.form == NumForm::Int,
                });
            }
            Expr::Str(_) | Expr::List(_) => return Err(Domain::NotANumber(e)),
            Expr::Param(r) => {
                let v = self.formal(r, frame).or_else(|| self.env.param_value(r));
                self.values.push(v.ok_or(Domain::Unbound(e))?);
            }
            Expr::Call(Func::Ddt | Func::Idt, _) => return Err(Domain::Unbound(e)),
            Expr::Call(_, args) | Expr::Draw(_, _, _, args) | Expr::UserCall(_, args) => {
                self.tasks.push(Task::Apply(e, frame));
                self.tasks.extend(operands(args));
            }
            Expr::Unary(_, a) => self
                .tasks
                .extend([Task::Apply(e, frame), Task::Eval(a, frame)]),
            Expr::Binary(BinOp::And | BinOp::Or, a, _) => self
                .tasks
                .extend([Task::Logic(e, frame), Task::Eval(a, frame)]),
            Expr::Binary(_, a, b) => self.tasks.extend([
                Task::Apply(e, frame),
                Task::Eval(b, frame),
                Task::Eval(a, frame),
            ]),
            Expr::Ternary(c, _, _) => self
                .tasks
                .extend([Task::Choose(e, frame), Task::Eval(c, frame)]),
            Expr::V(net, reference) => self.leaf(e, self.env.voltage(net, reference))?,
            Expr::I(element) => self.leaf(e, self.env.current(element))?,
            Expr::Time => self.leaf(e, self.env.time())?,
            Expr::Temp => self.leaf(e, self.env.temp())?,
            Expr::Freq => self.leaf(e, self.env.freq())?,
            Expr::TStep => self.leaf(e, self.env.tstep())?,
            Expr::TStop => self.leaf(e, self.env.tstop())?,
            Expr::Tnom => self.leaf(e, self.env.tnom())?,
        }
        Ok(())
    }

    /// Applies `e`'s operator to the operand values on the stack.
    fn apply(&mut self, e: ExprId, frame: u32) -> Result<(), Domain> {
        let d = self.design;
        // the number by the untyped rule, then its type by the tag table (A12 K1)
        let v = match d.expr(e) {
            Expr::Unary(op, _) => {
                let a = self.pop();
                let n = unary(op, a.number).ok_or(Domain::OutOfDomain(e))?;
                Value {
                    number: n,
                    int: op != UnOp::Neg || a.int,
                }
            }
            Expr::Binary(BinOp::DivTyped, _, _) => {
                let b = self.pop();
                let a = self.pop();
                if b.number == 0.0 {
                    return Err(Domain::DivideByZero(e));
                }
                if a.int && b.int {
                    Value::int((a.number / b.number).trunc())
                } else {
                    Value::real(a.number / b.number)
                }
            }
            Expr::Binary(op, _, _) => {
                let b = self.pop();
                let a = self.pop();
                let n = binary(e, op, a.number, b.number)?;
                Value {
                    number: n,
                    int: binary_int(op, a.int && b.int),
                }
            }
            Expr::Call(f, args) => {
                let xs = self.pop_n(args.len());
                let numbers: Vec<f64> = xs.iter().map(|v| v.number).collect();
                let n = call(e, f, &numbers)?;
                let all_int = xs.iter().all(|v| v.int);
                let int = match f {
                    Func::Min | Func::Max => all_int,
                    Func::ToInt => true,
                    Func::Sgn => all_int, // run f2: `sgn(-3)/2` printed 0
                    Func::SgnZeroPos => todo!(),
                    _ => false,
                };
                Value { number: n, int }
            }
            Expr::Draw(dist, when, per, args) => {
                let xs = self.pop_n(args.len());
                let numbers: Vec<f64> = xs.iter().map(|v| v.number).collect();
                Value::real(
                    self.env
                        .draw(dist, when, per, &numbers)
                        .ok_or(Domain::Unbound(e))?,
                )
            }
            Expr::UserCall(r, args) => {
                let xs = self.pop_n(args.len());
                return self.enter(e, r, xs, frame);
            }
            Expr::Num(_)
            | Expr::Str(_)
            | Expr::Param(_)
            | Expr::Ternary(..)
            | Expr::V(..)
            | Expr::I(_)
            | Expr::Time
            | Expr::Temp
            | Expr::Freq
            | Expr::TStep
            | Expr::TStop
            | Expr::Tnom
            | Expr::List(_) => unreachable!("only operators are applied"),
        };
        // R12: no integer overflow is carried
        if !v.number.is_finite() || (v.int && v.number.abs() > INT_MAX) {
            return Err(Domain::OutOfDomain(e));
        }
        self.values.push(v);
        Ok(())
    }

    /// Starts a user call: binds its formal arguments and evaluates its body in a new frame.
    fn enter(&mut self, e: ExprId, r: Ref, values: Vec<Value>, _caller: u32) -> Result<(), Domain> {
        let d = self.design;
        if !r.is_resolved() || d.def(r.def).kind != DefKind::Func {
            return Err(Domain::Unbound(e));
        }
        if self.frames.len() >= CALLS_MAX {
            return Err(Domain::CallDepth(e));
        }
        let row: FuncRow = d.row(d.def(r.def).item.index);
        let names = d.names(row.args).to_vec();
        if names.len() != values.len() {
            return Err(Domain::Unbound(e));
        }
        self.frames.push(Frame {
            def: r.def,
            names,
            values,
        });
        let frame = (self.frames.len() - 1) as u32;
        self.tasks
            .extend([Task::Return, Task::Eval(row.body, frame)]);
        Ok(())
    }

    /// The value of a formal argument read in the current call frame, if `r` is one.
    fn formal(&self, r: Ref, frame: u32) -> Option<Value> {
        let f = self.frames.get(frame as usize)?;
        if f.def != r.def {
            return None;
        }
        f.names
            .iter()
            .position(|n| *n == r.name)
            .map(|k| f.values[k])
    }

    fn leaf(&mut self, e: ExprId, v: Option<f64>) -> Result<(), Domain> {
        self.values.push(Value::real(v.ok_or(Domain::Unbound(e))?));
        Ok(())
    }

    fn pop(&mut self) -> Value {
        self.values.pop().expect("an operand on the value stack")
    }

    fn pop_n(&mut self, n: usize) -> Vec<Value> {
        assert!(
            self.values.len() >= n,
            "every operand is on the value stack"
        );
        self.values.split_off(self.values.len() - n)
    }
}

/// Whether a binary operator's result is Int, given whether both operands are (A12 K1's table).
fn binary_int(op: BinOp, both_int: bool) -> bool {
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul => both_int,
        BinOp::Div | BinOp::IntDiv => false,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => true,
        BinOp::And | BinOp::Or => true,
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => true,
        BinOp::DivTyped => both_int,
    }
}

/// A unary operator; `None` outside its domain.
fn unary(op: UnOp, a: f64) -> Option<f64> {
    match op {
        UnOp::Neg => Some(-a),
        UnOp::Not => Some(if a == 0.0 { 1.0 } else { 0.0 }),
        UnOp::BitNot => int(a).map(|a| !a as f64),
    }
}

/// A binary operator other than `And` and `Or` (those short-circuit in `step`).
fn binary(e: ExprId, op: BinOp, a: f64, b: f64) -> Result<f64, Domain> {
    let bool = |c: bool| if c { 1.0 } else { 0.0 };
    let bits = |f: fn(i64, i64) -> i64| match (int(a), int(b)) {
        (Some(a), Some(b)) => Ok(f(a, b) as f64),
        _ => Err(Domain::OutOfDomain(e)),
    };
    let shift = |left: bool| match (int(a), int(b)) {
        (Some(a), Some(s)) if (0..64).contains(&s) => Ok(if left { a << s } else { a >> s } as f64),
        _ => Err(Domain::OutOfDomain(e)),
    };
    match op {
        BinOp::Add => Ok(a + b),
        BinOp::Sub => Ok(a - b),
        BinOp::Mul => Ok(a * b),
        BinOp::Div if b == 0.0 => Err(Domain::DivideByZero(e)),
        BinOp::Div => Ok(a / b),
        BinOp::IntDiv if b == 0.0 => Err(Domain::DivideByZero(e)),
        BinOp::IntDiv => Ok((a / b).trunc()),
        BinOp::Eq => Ok(bool(a == b)),
        BinOp::Ne => Ok(bool(a != b)),
        BinOp::Lt => Ok(bool(a < b)),
        BinOp::Le => Ok(bool(a <= b)),
        BinOp::Gt => Ok(bool(a > b)),
        BinOp::Ge => Ok(bool(a >= b)),
        BinOp::And | BinOp::Or => unreachable!("And and Or short-circuit"),
        BinOp::BitAnd => bits(|a, b| a & b),
        BinOp::BitOr => bits(|a, b| a | b),
        BinOp::BitXor => bits(|a, b| a ^ b),
        BinOp::Shl => shift(true),
        BinOp::Shr => shift(false),
        BinOp::DivTyped => unreachable!("typed division is applied with its operands' tags"),
    }
}

/// A built-in function at its evaluated arguments (counts already within its arity).
fn call(e: ExprId, f: Func, xs: &[f64]) -> Result<f64, Domain> {
    let out = Domain::OutOfDomain(e);
    let x = xs[0];
    let y = || xs[1];
    let v = match f {
        Func::Abs => x.abs(),
        Func::Sqrt if x < 0.0 => return Err(out),
        Func::Sqrt => x.sqrt(),
        Func::SqrtAbs => x.abs().sqrt(),
        Func::Exp => x.exp(),
        Func::ExpLin14 if x <= 14.0 => x.exp(),
        Func::ExpLin14 => 14f64.exp() * (1.0 + x - 14.0),
        Func::Ln | Func::Log10 if x <= 0.0 => return Err(out),
        Func::Ln => x.ln(),
        Func::Log10 => x.log10(),
        Func::LnAbs if x == 0.0 => return Err(out),
        Func::LnAbs => x.abs().ln(),
        Func::Pow | Func::PowAbs | Func::PowRe if x == 0.0 && y() < 0.0 => return Err(out),
        Func::Pow if x < 0.0 && y().fract() != 0.0 => return Err(out),
        Func::Pow => x.powf(y()),
        Func::PowAbs => x.abs().powf(y()),
        Func::PowRe if x < 0.0 && y().fract() != 0.0 => {
            x.abs().powf(y()) * (std::f64::consts::PI * y()).cos()
        }
        Func::PowRe => x.powf(y()),
        Func::Min => x.min(y()),
        Func::Max => x.max(y()),
        Func::Floor => x.floor(),
        Func::Ceil => x.ceil(),
        Func::Trunc => x.trunc(),
        Func::RoundHalfAway => x.round(),
        Func::RoundHalfEven => x.round_ties_even(),
        Func::StepHalf => {
            if x < 0.0 {
                0.0
            } else if x == 0.0 {
                0.5
            } else {
                1.0
            }
        }
        Func::PowSigned if x == 0.0 && y() < 0.0 => return Err(out),
        Func::PowSigned if x < 0.0 => -(-x).powf(y()),
        Func::PowSigned => x.powf(y()),
        Func::Sqr => x * x,
        Func::Sgn => {
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        Func::Sin => x.sin(),
        Func::Cos => x.cos(),
        Func::Tan => x.tan(),
        Func::Asin | Func::Acos if x.abs() > 1.0 => return Err(out),
        Func::Asin => x.asin(),
        Func::Acos => x.acos(),
        Func::Atan => x.atan(),
        Func::Atan2 => x.atan2(y()),
        Func::Sinh => x.sinh(),
        Func::Cosh => x.cosh(),
        Func::Tanh => x.tanh(),
        Func::Asinh => x.asinh(),
        Func::Acosh if x < 1.0 => return Err(out),
        Func::Acosh => x.acosh(),
        Func::Atanh if x.abs() >= 1.0 => return Err(out),
        Func::Atanh => x.atanh(),
        Func::Hypot => x.hypot(y()),
        Func::Fmod if y() == 0.0 => return Err(Domain::DivideByZero(e)),
        Func::Fmod => x % y(),
        Func::Clamp if xs[1] > xs[2] => return Err(out),
        Func::Clamp => x.max(xs[1]).min(xs[2]),
        Func::Step => {
            if x > 0.0 {
                1.0
            } else {
                0.0
            }
        }
        Func::Ramp => {
            if x > 0.0 {
                x
            } else {
                0.0
            }
        }
        Func::Ddt | Func::Idt => unreachable!("refused before their arguments are evaluated"),
        Func::Pwl => return pwl(xs).ok_or(out),
        Func::ToInt => x.trunc(),
        Func::SgnZeroPos => todo!(),
    };
    Ok(v)
}

/// `pwl(x, x1, y1, …)`: linear between points with strictly increasing `x`, ends held.
fn pwl(xs: &[f64]) -> Option<f64> {
    let (x, pts) = xs.split_first()?;
    if pts.len() % 2 != 0 || pts.is_empty() {
        return None;
    }
    let points: Vec<(f64, f64)> = pts.chunks_exact(2).map(|p| (p[0], p[1])).collect();
    if points.windows(2).any(|w| w[1].0 <= w[0].0) {
        return None;
    }
    let (first, last) = (points[0], points[points.len() - 1]);
    if *x <= first.0 {
        return Some(first.1);
    }
    if *x >= last.0 {
        return Some(last.1);
    }
    let k = points.windows(2).position(|w| *x < w[1].0)?;
    let ((x0, y0), (x1, y1)) = (points[k], points[k + 1]);
    Some(y0 + (y1 - y0) * (x - x0) / (x1 - x0))
}

/// The integer value of `a` for the bitwise operators; `None` if fractional or out of `i64`.
fn int(a: f64) -> Option<i64> {
    (a.fract() == 0.0 && a.abs() < 9.2e18).then_some(a as i64)
}
