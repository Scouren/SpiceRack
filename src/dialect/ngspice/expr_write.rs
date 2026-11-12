//! An `ir::Expr` tree → ngspice expression text, in the evaluator ngspice will read it with (the
//! B-source parser when it holds a probe, a time-like leaf, the temperature or `ddt`; numparam
//! otherwise), with the fewest parentheses that read back (`expr_read.rs`) as the same tree.
//!
//! Parentheses follow `tables.rs`'s levels, plus each evaluator's placement rules: a numparam
//! negation is a leading `0 - …`, so it stands only at the start of a (sub)formula and takes the
//! first additive term; a numparam `!` takes an atom; a numparam then-branch never holds a bare
//! ternary; a negative literal is written bare only where `-` reads as its sign (numparam: after
//! a binary operator, or a whole additive term at a formula's start; B source: anywhere but as
//! a power's base); the negation of a literal is `-(…)`. Binary operators get a space each side
//! (the B-source lexer reads `a<b` as one name, `inpptree.c` `PTlex`); a value that is not a
//! number or a parameter name goes in `{…}`.
//!
//! Cites: ngspice-45 `xpressn.c`, `inpptree-parser.y`, `inpptree.c`; `rewrite/IR-KERNEL.md` §7.1;
//! `rewrite/DIALECT-NGSPICE.md` §4.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) fn eval_of(&ir::Design, ir::ExprId) -> Eval`,
//! `pub(super) fn expr(&ir::Design, ir::ExprId, &mut String)`,
//! `pub(super) fn value(&ir::Design, ir::ExprId, &mut String)`,
//! `pub(super) fn expr_in(&ir::Design, ir::ExprId, Eval, &mut String)`,
//! `pub(super) fn spells_in(&ir::Design, ir::ExprId, Eval) -> bool`,
//! `pub(super) fn holds_text(&ir::Design, ir::ExprId) -> bool`

use super::number;
use super::tables::{self, Eval};
use crate::ir::{self, BinOp, Dist, DrawPer, DrawWhen, ExprTag, Feature, Func, UnOp};

/// The expression nodes ngspice spells: in numparam (plain features) and in the B-source parser
/// (the `*Dyn` features), each the image of `tables.rs`'s rows. Not: strings, lists, the
/// `TStep`/`TStop`/`Tnom` leaves (a pass substitutes them), `IntDiv` (numparam's `\` is
/// `trunc(|a/b|)`), bitwise operators, and the members no ngspice name means (`Pow` in a B
/// source is `PowAbs`; `LnAbs`, `SqrtAbs`, `ExpLin14` are not ngspice-45's, `ptfuncs.c`).
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    Feature::Expr(ExprTag::Num),
    Feature::Expr(ExprTag::Param),
    Feature::Expr(ExprTag::Call),
    Feature::Expr(ExprTag::UserCall),
    Feature::Expr(ExprTag::Unary),
    Feature::Expr(ExprTag::Binary),
    Feature::Expr(ExprTag::Ternary),
    Feature::Expr(ExprTag::V),
    Feature::Expr(ExprTag::I),
    Feature::Expr(ExprTag::Time),
    Feature::Expr(ExprTag::Temp),
    Feature::Expr(ExprTag::Freq),
    Feature::Expr(ExprTag::Draw),
    Feature::Func(Func::Abs),
    Feature::Func(Func::Sqrt),
    Feature::Func(Func::Exp),
    Feature::Func(Func::Ln),
    Feature::Func(Func::Log10),
    Feature::Func(Func::Sin),
    Feature::Func(Func::Cos),
    Feature::Func(Func::Tan),
    Feature::Func(Func::Asin),
    Feature::Func(Func::Acos),
    Feature::Func(Func::Atan),
    Feature::Func(Func::Sinh),
    Feature::Func(Func::Cosh),
    Feature::Func(Func::Tanh),
    Feature::Func(Func::Asinh),
    Feature::Func(Func::Acosh),
    Feature::Func(Func::Atanh),
    Feature::Func(Func::Pow),
    Feature::Func(Func::PowAbs),
    Feature::Func(Func::Min),
    Feature::Func(Func::Max),
    Feature::Func(Func::Trunc),
    Feature::Func(Func::RoundHalfEven),
    Feature::Func(Func::Sgn),
    Feature::Func(Func::Ceil),
    Feature::Func(Func::Floor),
    Feature::Func(Func::Fmod),
    Feature::Func(Func::Sqr),
    Feature::FuncDyn(Func::Abs),
    Feature::FuncDyn(Func::Sqrt),
    Feature::FuncDyn(Func::Exp),
    Feature::FuncDyn(Func::Ln),
    Feature::FuncDyn(Func::Log10),
    Feature::FuncDyn(Func::Sin),
    Feature::FuncDyn(Func::Cos),
    Feature::FuncDyn(Func::Tan),
    Feature::FuncDyn(Func::Asin),
    Feature::FuncDyn(Func::Acos),
    Feature::FuncDyn(Func::Atan),
    Feature::FuncDyn(Func::Sinh),
    Feature::FuncDyn(Func::Cosh),
    Feature::FuncDyn(Func::Tanh),
    Feature::FuncDyn(Func::Asinh),
    Feature::FuncDyn(Func::Acosh),
    Feature::FuncDyn(Func::Atanh),
    Feature::FuncDyn(Func::PowAbs),
    Feature::FuncDyn(Func::Min),
    Feature::FuncDyn(Func::Max),
    Feature::FuncDyn(Func::RoundHalfEven),
    Feature::FuncDyn(Func::Sgn),
    Feature::FuncDyn(Func::Ceil),
    Feature::FuncDyn(Func::Floor),
    Feature::FuncDyn(Func::Ramp),
    Feature::FuncDyn(Func::Ddt),
    Feature::FuncDyn(Func::StepHalf),
    Feature::FuncDyn(Func::PowSigned),
    Feature::Binary(BinOp::Add),
    Feature::Binary(BinOp::Sub),
    Feature::Binary(BinOp::Mul),
    Feature::Binary(BinOp::Div),
    Feature::Binary(BinOp::Eq),
    Feature::Binary(BinOp::Ne),
    Feature::Binary(BinOp::Lt),
    Feature::Binary(BinOp::Le),
    Feature::Binary(BinOp::Gt),
    Feature::Binary(BinOp::Ge),
    Feature::Binary(BinOp::And),
    Feature::Binary(BinOp::Or),
    Feature::BinaryDyn(BinOp::Add),
    Feature::BinaryDyn(BinOp::Sub),
    Feature::BinaryDyn(BinOp::Mul),
    Feature::BinaryDyn(BinOp::Div),
    Feature::BinaryDyn(BinOp::Eq),
    Feature::BinaryDyn(BinOp::Ne),
    Feature::BinaryDyn(BinOp::Lt),
    Feature::BinaryDyn(BinOp::Le),
    Feature::BinaryDyn(BinOp::Gt),
    Feature::BinaryDyn(BinOp::Ge),
    Feature::BinaryDyn(BinOp::And),
    Feature::BinaryDyn(BinOp::Or),
    Feature::Unary(UnOp::Neg),
    Feature::Unary(UnOp::Not),
    Feature::UnaryDyn(UnOp::Neg),
    Feature::UnaryDyn(UnOp::Not),
    Feature::Dist(Dist::AbsGauss),
    Feature::Dist(Dist::RelGauss),
    Feature::Dist(Dist::AbsUnif),
    Feature::Dist(Dist::RelUnif),
    Feature::Dist(Dist::Limit),
    Feature::DrawWhen(DrawWhen::Always),
    Feature::DrawPer(DrawPer::Reference),
]);

/// One step of the writer.
#[derive(Clone, Copy)]
enum Task {
    Text(&'static str),
    Expr(ir::ExprId, Role, Pos),
}

/// What a node is to its parent.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Root,
    /// Left operand of an operator binding at the level.
    Left(u8),
    /// Right operand of an operator binding at the level.
    Right(u8),
    /// Operand of a prefix operator.
    Operand(ir::UnOp),
    Cond,
    Then,
    Else,
    Arg,
}

/// Where a node's first token stands in its formula.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Pos {
    /// The first token of a (sub)formula.
    start: bool,
    /// Right after a binary operator (numparam reads a `-` there as a number's sign).
    after_op: bool,
}

/// The evaluator ngspice reads `e` with: `Behavioural` when its class (`ir::expr_class`: it, or
/// a parameter or function it reads, transitively) is `Temp` or `Circuit` (R14, A11).
pub(super) fn eval_of(design: &ir::Design, e: ir::ExprId) -> Eval {
    if ir::expr_class(design, e) >= ir::ExprClass::Temp {
        Eval::Behavioural
    } else {
        Eval::Numparam
    }
}

/// Appends `e` as an expression of its evaluator. Asserts every node is one ngspice spells
/// (the spine wrote only a design legal for `FEATURES`).
pub(super) fn expr(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    expr_in(design, e, eval_of(design, e), out);
}

/// Appends `e` as an expression of `eval`, whatever its class: a field ngspice always hands one
/// evaluator (a B source's expression is the B-source parser's even when it is constant,
/// `inpcom.c` `inp_bsource_compat`). Asserts every node is one `eval` spells ([`spells_in`]).
pub(super) fn expr_in(design: &ir::Design, e: ir::ExprId, eval: Eval, out: &mut String) {
    let mut tasks = vec![Task::Expr(e, Role::Root, Pos::START)];
    // Bounded: each task pops once and pushes only the tasks of a node's children.
    while let Some(t) = tasks.pop() {
        match t {
            Task::Text(s) => out.push_str(s),
            Task::Expr(x, role, pos) => node(design, eval, x, role, pos, out, &mut tasks),
        }
    }
}

/// Appends `e` as a card value: a non-negative number or a parameter name bare, a negative
/// number bare (a leading `-` before a number is its sign), anything else in `{…}`.
pub(super) fn value(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    let bare = matches!(design.expr(e), ir::Expr::Num(_) | ir::Expr::Param(_));
    if !bare {
        out.push('{');
    }
    expr(design, e, out);
    if !bare {
        out.push('}');
    }
}

/// Whether `eval` spells every node of `e`: its functions and operators (numparam has no probe,
/// time-like leaf or `temper`, and the B-source parser no `%`, `int`, `pow`, `sqr` or draw).
/// Every literal spells (R28).
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the element area's admits calls it (S3.1a)")
)]
pub(super) fn spells_in(design: &ir::Design, e: ir::ExprId, eval: Eval) -> bool {
    let numparam = eval == Eval::Numparam;
    let mut stack = vec![e];
    let mut kids = Vec::new();
    let mut seen = vec![false; design.expr_count()];
    // Bounded: each row is checked once.
    while let Some(x) = stack.pop() {
        if std::mem::replace(&mut seen[x.index()], true) {
            continue;
        }
        let row = design.expr(x);
        let ok = match row {
            // every literal has an exact spelling (R28: `number::spell` respells a form ngspice
            // has no letter for, and `mil` under numparam, in exponent form)
            ir::Expr::Num(_)
            | ir::Expr::Param(_)
            | ir::Expr::UserCall(..)
            | ir::Expr::Ternary(..) => true,
            ir::Expr::Call(ir::Func::PowAbs, _) => true,
            ir::Expr::Call(ir::Func::Fmod, _) => numparam,
            ir::Expr::Call(f, _) => tables::func_spelling(eval, f).is_some(),
            ir::Expr::Draw(_, ir::DrawWhen::Always, ir::DrawPer::Reference, _) => numparam,
            ir::Expr::Unary(op, _) => op != ir::UnOp::BitNot,
            ir::Expr::Binary(op, ..) => tables::level(eval, op).is_some(),
            ir::Expr::V(..) | ir::Expr::I(_) | ir::Expr::Time | ir::Expr::Temp | ir::Expr::Freq => {
                !numparam
            }
            ir::Expr::Draw(..)
            | ir::Expr::Str(_)
            | ir::Expr::List(_)
            | ir::Expr::TStep
            | ir::Expr::TStop
            | ir::Expr::Tnom => false,
        };
        if !ok {
            return false;
        }
        kids.clear();
        children(design, row, &mut kids);
        stack.extend_from_slice(&kids);
    }
    true
}

/// Whether `e` holds a string or a list anywhere (no ngspice expression has either).
pub(super) fn holds_text(design: &ir::Design, e: ir::ExprId) -> bool {
    let mut stack = vec![e];
    let mut kids = Vec::new();
    let mut seen = vec![false; design.expr_count()];
    // Bounded: each row is visited once.
    while let Some(x) = stack.pop() {
        if std::mem::replace(&mut seen[x.index()], true) {
            continue;
        }
        let row = design.expr(x);
        if matches!(row, ir::Expr::Str(_) | ir::Expr::List(_)) {
            return true;
        }
        kids.clear();
        children(design, row, &mut kids);
        stack.extend_from_slice(&kids);
    }
    false
}

// ---- private ----

impl Pos {
    const START: Pos = Pos {
        start: true,
        after_op: false,
    };
    const AFTER_OP: Pos = Pos {
        start: false,
        after_op: true,
    };
    const INSIDE: Pos = Pos {
        start: false,
        after_op: false,
    };
}

/// Queues `x`'s text: its parentheses if its role needs them, its tokens, its children.
fn node(
    d: &ir::Design,
    eval: Eval,
    x: ir::ExprId,
    role: Role,
    pos: Pos,
    out: &mut String,
    tasks: &mut Vec<Task>,
) {
    let wrapped = wraps(d, eval, x, role, pos);
    let pos = if wrapped { Pos::START } else { pos };
    let mut seq: Vec<Task> = Vec::new();
    if wrapped {
        out.push('(');
        seq.push(Task::Text(")"));
    }
    let numparam = eval == Eval::Numparam;
    match d.expr(x) {
        ir::Expr::Num(n) => number::spell(d.num(n), eval.grammar(), out),
        ir::Expr::Param(r) => out.push_str(d.str(r.name)),
        ir::Expr::V(n, m) => {
            out.push_str("v(");
            out.push_str(d.str(n));
            if !m.is_none() {
                out.push(',');
                out.push_str(d.str(m));
            }
            out.push(')');
        }
        ir::Expr::I(e) => {
            out.push_str("i(");
            out.push_str(d.str(e));
            out.push(')');
        }
        ir::Expr::Time => out.push_str("time"),
        ir::Expr::Temp => out.push_str("temper"),
        ir::Expr::Freq => out.push_str("hertz"),
        ir::Expr::Call(ir::Func::PowAbs, args) => {
            let (l, r) = pair(d, args);
            let p = tables::power(eval);
            seq.push(Task::Expr(r, Role::Right(p), Pos::AFTER_OP));
            seq.push(Task::Text(" ** "));
            seq.push(Task::Expr(l, Role::Left(p), pos));
        }
        ir::Expr::Call(ir::Func::Fmod, args) if numparam => {
            let (l, r) = pair(d, args);
            seq.push(Task::Expr(r, Role::Right(3), Pos::AFTER_OP));
            seq.push(Task::Text(" % "));
            seq.push(Task::Expr(l, Role::Left(3), pos));
        }
        ir::Expr::Call(f, args) => {
            let name = tables::func_spelling(eval, f)
                .unwrap_or_else(|| panic!("ngspice spells no {f:?} here (FEATURES lacks it)"));
            out.push_str(name);
            call_args(d, args, &mut seq);
        }
        ir::Expr::UserCall(r, args) => {
            out.push_str(d.str(r.name));
            call_args(d, args, &mut seq);
        }
        ir::Expr::Draw(dist, ir::DrawWhen::Always, ir::DrawPer::Reference, args) if numparam => {
            out.push_str(tables::dist_spelling(dist));
            call_args(d, args, &mut seq);
        }
        ir::Expr::Unary(op, a) => {
            out.push_str(match op {
                ir::UnOp::Neg => "-",
                ir::UnOp::Not => "!",
                ir::UnOp::BitNot => panic!("ngspice has no `~` (FEATURES lacks it)"),
            });
            let inner = match (numparam, op) {
                (true, ir::UnOp::Neg) => Pos::AFTER_OP,
                _ => Pos::INSIDE,
            };
            seq.push(Task::Expr(a, Role::Operand(op), inner));
        }
        ir::Expr::Binary(op, l, r) => {
            let level = tables::level(eval, op)
                .unwrap_or_else(|| panic!("ngspice spells no {op:?} (FEATURES lacks it)"));
            seq.push(Task::Expr(r, Role::Right(level), Pos::AFTER_OP));
            seq.push(Task::Text(binary_text(op)));
            seq.push(Task::Expr(l, Role::Left(level), pos));
        }
        ir::Expr::Ternary(c, t, f) => {
            seq.push(Task::Expr(f, Role::Else, Pos::AFTER_OP));
            seq.push(Task::Text(" : "));
            seq.push(Task::Expr(t, Role::Then, Pos::AFTER_OP));
            seq.push(Task::Text(" ? "));
            seq.push(Task::Expr(c, Role::Cond, pos));
        }
        other @ (ir::Expr::Str(_)
        | ir::Expr::List(_)
        | ir::Expr::Draw(..)
        | ir::Expr::TStep
        | ir::Expr::TStop
        | ir::Expr::Tnom) => panic!(
            "ngspice spells no {:?} here (FEATURES lacks it)",
            other.tag()
        ),
    }
    tasks.extend(seq);
}

/// Whether `x` in `role` at `pos` needs parentheses to read back as itself.
fn wraps(d: &ir::Design, eval: Eval, x: ir::ExprId, role: Role, pos: Pos) -> bool {
    let numparam = eval == Eval::Numparam;
    let e = d.expr(x);
    if let ir::Expr::Num(n) = e {
        let negative = d.num(n).mantissa < 0;
        return match role {
            // `-(2)` is the negation of 2; `-2` would read as the literal
            Role::Operand(_) => true,
            _ if !negative => false,
            Role::Left(l) if !numparam => l == tables::power(eval),
            _ if !numparam => false,
            _ if pos.after_op => false,
            Role::Root | Role::Cond | Role::Arg => !pos.start,
            Role::Left(l) => !pos.start || l < 4,
            Role::Right(_) | Role::Then | Role::Else => true,
        };
    }
    let own = level(eval, e);
    let neg = matches!(e, ir::Expr::Unary(ir::UnOp::Neg, _));
    if numparam && neg && (!pos.start || matches!(role, Role::Operand(_))) {
        return true;
    }
    match role {
        Role::Root | Role::Arg | Role::Else => false,
        Role::Left(l) => own > l,
        Role::Right(l) => own > 0 && own >= l,
        Role::Operand(op) => match (numparam, op) {
            (true, ir::UnOp::Neg) => own >= 4,
            (true, _) => own > 0,
            (false, _) => own > 2,
        },
        Role::Cond => own >= tables::TERNARY,
        Role::Then => numparam && own >= tables::TERNARY,
    }
}

/// The level a node binds at as written bare (0: an atom).
fn level(eval: Eval, e: ir::Expr) -> u8 {
    let numparam = eval == Eval::Numparam;
    match e {
        ir::Expr::Call(ir::Func::PowAbs, _) => tables::power(eval),
        ir::Expr::Call(ir::Func::Fmod, _) if numparam => 3,
        ir::Expr::Binary(op, ..) => tables::level(eval, op).unwrap_or(0),
        ir::Expr::Unary(ir::UnOp::Neg, _) if numparam => 4,
        ir::Expr::Unary(..) if numparam => 0,
        ir::Expr::Unary(..) => 2,
        ir::Expr::Ternary(..) => tables::TERNARY,
        _ => 0,
    }
}

/// ` op ` for a binary operator.
fn binary_text(op: ir::BinOp) -> &'static str {
    use ir::BinOp as B;
    match op {
        B::Add => " + ",
        B::Sub => " - ",
        B::Mul => " * ",
        B::Div => " / ",
        B::Eq => " == ",
        B::Ne => " != ",
        B::Lt => " < ",
        B::Le => " <= ",
        B::Gt => " > ",
        B::Ge => " >= ",
        B::And => " && ",
        B::Or => " || ",
        B::IntDiv | B::BitAnd | B::BitOr | B::BitXor | B::Shl | B::Shr => {
            panic!("ngspice has no {op:?} (FEATURES lacks it)")
        }
    }
}

/// Queues `(a, b, …)` (pushed in reverse onto `seq`, which is reversed into the task stack).
fn call_args(d: &ir::Design, args: ir::ExprList, seq: &mut Vec<Task>) {
    let args = d.exprs(args);
    let mut tail = vec![Task::Text("(")];
    for (k, &a) in args.iter().enumerate() {
        if k > 0 {
            tail.push(Task::Text(", "));
        }
        tail.push(Task::Expr(a, Role::Arg, Pos::START));
    }
    tail.push(Task::Text(")"));
    seq.extend(tail.into_iter().rev());
}

/// The two arguments of a binary function.
fn pair(d: &ir::Design, args: ir::ExprList) -> (ir::ExprId, ir::ExprId) {
    let a = d.exprs(args);
    assert_eq!(a.len(), 2, "a binary function has two arguments");
    (a[0], a[1])
}

/// The children of a decoded row.
fn children(d: &ir::Design, e: ir::Expr, out: &mut Vec<ir::ExprId>) {
    match e {
        ir::Expr::Call(_, l)
        | ir::Expr::UserCall(_, l)
        | ir::Expr::Draw(.., l)
        | ir::Expr::List(l) => {
            out.extend_from_slice(d.exprs(l));
        }
        ir::Expr::Unary(_, a) => out.push(a),
        ir::Expr::Binary(_, l, r) => out.extend([l, r]),
        ir::Expr::Ternary(c, t, f) => out.extend([c, t, f]),
        ir::Expr::Num(_)
        | ir::Expr::Str(_)
        | ir::Expr::Param(_)
        | ir::Expr::V(..)
        | ir::Expr::I(_)
        | ir::Expr::Time
        | ir::Expr::Temp
        | ir::Expr::Freq
        | ir::Expr::TStep
        | ir::Expr::TStop
        | ir::Expr::Tnom => {}
    }
}
