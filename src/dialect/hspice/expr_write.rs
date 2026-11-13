//! An `ir::Expr` tree → HSPICE expression text, with the fewest parentheses that read back
//! (`expr_read.rs`) as the same tree: `tables.rs`'s levels, every binary operator
//! left-associative (the power too), a negative literal bare except as a power's base, the
//! negation of a literal `-(…)`. Binary operators get a space each side; a value that is not a
//! number or a parameter name goes in `'…'` (HSPICE's expression quotes: `expr_quotes.sp`;
//! braces are no expression, `expr_braces.sp`).
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §3 (branch `differences`); settle `expr_*.sp` (ESPice
//! 0.3.0 in HSPICE mode: ESPice is not HSPICE); `rewrite/IR-KERNEL.md` §7.1.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`,
//! `pub(super) fn expr(&ir::Design, ir::ExprId, &mut String)`,
//! `pub(super) fn value(&ir::Design, ir::ExprId, &mut String)`,
//! `pub(super) fn spells(&ir::Design, ir::ExprId) -> bool`,
//! `pub(super) fn reads_unbound(&ir::Design, ir::ExprId) -> bool`

use super::number;
use super::tables;
use crate::ir::{self, BinOp, Dist, ExprTag, Feature, Func, UnOp};

/// The expression nodes HSPICE spells: the image of `tables.rs`'s rows and the leaves its
/// lexer reads (`time`, `temper`, `hertz`, `v(…)`, `i(…)`). Not: strings, lists, the
/// `TStep`/`TStop`/`Tnom` leaves, `!`, `%`, bitwise operators, the members no HSPICE name is
/// settled for.
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    Feature::Expr(ExprTag::Num),
    Feature::Extra(ir::Extra::LongLiteral),
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
    Feature::UnaryDyn(UnOp::Neg),
    Feature::Dist(Dist::AbsGauss),
    Feature::Dist(Dist::RelGauss),
    Feature::Dist(Dist::AbsUnif),
    Feature::Dist(Dist::RelUnif),
    Feature::Dist(Dist::Limit),
    Feature::DrawWhen(tables::DRAW.0),
    Feature::DrawPer(tables::DRAW.1),
    // the functions, in a constant expression and a circuit one alike (one language)
    Feature::Func(Func::Abs),
    Feature::FuncDyn(Func::Abs),
    Feature::Func(Func::Sqrt),
    Feature::FuncDyn(Func::Sqrt),
    Feature::Func(Func::Exp),
    Feature::FuncDyn(Func::Exp),
    Feature::Func(Func::Ln),
    Feature::FuncDyn(Func::Ln),
    Feature::Func(Func::Log10),
    Feature::FuncDyn(Func::Log10),
    Feature::Func(Func::Sin),
    Feature::FuncDyn(Func::Sin),
    Feature::Func(Func::Cos),
    Feature::FuncDyn(Func::Cos),
    Feature::Func(Func::Tan),
    Feature::FuncDyn(Func::Tan),
    Feature::Func(Func::Asin),
    Feature::FuncDyn(Func::Asin),
    Feature::Func(Func::Acos),
    Feature::FuncDyn(Func::Acos),
    Feature::Func(Func::Atan),
    Feature::FuncDyn(Func::Atan),
    Feature::Func(Func::Sinh),
    Feature::FuncDyn(Func::Sinh),
    Feature::Func(Func::Cosh),
    Feature::FuncDyn(Func::Cosh),
    Feature::Func(Func::Tanh),
    Feature::FuncDyn(Func::Tanh),
    Feature::Func(Func::Pow),
    Feature::FuncDyn(Func::Pow),
    Feature::Func(Func::Min),
    Feature::FuncDyn(Func::Min),
    Feature::Func(Func::Max),
    Feature::FuncDyn(Func::Max),
    Feature::Func(Func::Trunc),
    Feature::FuncDyn(Func::Trunc),
    Feature::Func(Func::RoundHalfAway),
    Feature::FuncDyn(Func::RoundHalfAway),
    Feature::Func(Func::Sgn),
    Feature::FuncDyn(Func::Sgn),
    Feature::Func(Func::Ceil),
    Feature::FuncDyn(Func::Ceil),
    Feature::Func(Func::Floor),
    Feature::FuncDyn(Func::Floor),
]);

/// One step of the writer.
#[derive(Clone, Copy)]
enum Task {
    Text(&'static str),
    Expr(ir::ExprId, Role),
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
    Operand,
    Cond,
    Then,
    Else,
    Arg,
}

/// Appends `e` as an HSPICE expression. Asserts every node is one HSPICE spells (the spine
/// wrote only a design legal for `FEATURES`).
pub(super) fn expr(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    let mut tasks = vec![Task::Expr(e, Role::Root)];
    // Bounded: each task pops once and pushes only the tasks of a node's children.
    while let Some(t) = tasks.pop() {
        match t {
            Task::Text(s) => out.push_str(s),
            Task::Expr(x, role) => node(design, x, role, out, &mut tasks),
        }
    }
}

/// Appends `e` as a card value: a number or a parameter name bare, anything else in `'…'`.
pub(super) fn value(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    let bare = matches!(design.expr(e), ir::Expr::Num(_) | ir::Expr::Param(_));
    if !bare {
        out.push('\'');
    }
    expr(design, e, out);
    if !bare {
        out.push('\'');
    }
}

/// Whether HSPICE spells every node of `e`: its functions, operators and draws. Every literal
/// spells (R28).
pub(super) fn spells(design: &ir::Design, e: ir::ExprId) -> bool {
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
            ir::Expr::Num(_)
            | ir::Expr::Param(_)
            | ir::Expr::UserCall(..)
            | ir::Expr::Ternary(..)
            | ir::Expr::V(..)
            | ir::Expr::I(_)
            | ir::Expr::Time
            | ir::Expr::Temp
            | ir::Expr::Freq => true,
            ir::Expr::Call(ir::Func::Pow, _) => true,
            ir::Expr::Call(f, _) => tables::func_spelling(f).is_some(),
            ir::Expr::Draw(_, w, p, _) => (w, p) == tables::DRAW,
            ir::Expr::Unary(op, _) => op == ir::UnOp::Neg,
            ir::Expr::Binary(op, ..) => tables::level(op).is_some(),
            ir::Expr::Str(_)
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

/// Whether `e` reads a parameter that binds to no definition (an unbound name).
pub(super) fn reads_unbound(design: &ir::Design, e: ir::ExprId) -> bool {
    let mut stack = vec![e];
    let mut kids = Vec::new();
    // Bounded: every child id is lower than its parent's, so the walk ends.
    while let Some(x) = stack.pop() {
        let row = design.expr(x);
        if let ir::Expr::Param(r) = row
            && !r.is_resolved()
        {
            return true;
        }
        kids.clear();
        children(design, row, &mut kids);
        stack.extend_from_slice(&kids);
    }
    false
}

// ---- private ----

/// Queues `x`'s text: its parentheses if its role needs them, its tokens, its children.
fn node(d: &ir::Design, x: ir::ExprId, role: Role, out: &mut String, tasks: &mut Vec<Task>) {
    let mut seq: Vec<Task> = Vec::new();
    if wraps(d, x, role) {
        out.push('(');
        seq.push(Task::Text(")"));
    }
    match d.expr(x) {
        ir::Expr::Num(n) => number::spell(d, d.num(n), out),
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
        ir::Expr::Call(ir::Func::Pow, args) => {
            let (l, r) = pair(d, args);
            seq.push(Task::Expr(r, Role::Right(tables::POWER)));
            seq.push(Task::Text(" ** "));
            seq.push(Task::Expr(l, Role::Left(tables::POWER)));
        }
        ir::Expr::Call(f, args) => {
            let name = tables::func_spelling(f)
                .unwrap_or_else(|| panic!("HSPICE spells no {f:?} (FEATURES lacks it)"));
            out.push_str(name);
            call_args(d, args, &mut seq);
        }
        ir::Expr::UserCall(r, args) => {
            out.push_str(d.str(r.name));
            call_args(d, args, &mut seq);
        }
        ir::Expr::Draw(dist, w, p, args) if (w, p) == tables::DRAW => {
            out.push_str(tables::dist_spelling(dist));
            call_args(d, args, &mut seq);
        }
        ir::Expr::Unary(ir::UnOp::Neg, a) => {
            out.push('-');
            seq.push(Task::Expr(a, Role::Operand));
        }
        ir::Expr::Binary(op, l, r) => {
            let level = tables::level(op)
                .unwrap_or_else(|| panic!("HSPICE spells no {op:?} (FEATURES lacks it)"));
            seq.push(Task::Expr(r, Role::Right(level)));
            seq.push(Task::Text(binary_text(op)));
            seq.push(Task::Expr(l, Role::Left(level)));
        }
        ir::Expr::Ternary(c, t, f) => {
            seq.push(Task::Expr(f, Role::Else));
            seq.push(Task::Text(" : "));
            seq.push(Task::Expr(t, Role::Then));
            seq.push(Task::Text(" ? "));
            seq.push(Task::Expr(c, Role::Cond));
        }
        other @ (ir::Expr::Str(_)
        | ir::Expr::List(_)
        | ir::Expr::Draw(..)
        | ir::Expr::Unary(..)
        | ir::Expr::TStep
        | ir::Expr::TStop
        | ir::Expr::Tnom) => panic!("HSPICE spells no {:?} (FEATURES lacks it)", other.tag()),
    }
    tasks.extend(seq);
}

/// Whether `x` in `role` needs parentheses to read back as itself.
fn wraps(d: &ir::Design, x: ir::ExprId, role: Role) -> bool {
    let e = d.expr(x);
    if let ir::Expr::Num(n) = e {
        let negative = match d.long_digits(d.num(n)) {
            Some((significand, _)) => significand.starts_with('-'),
            None => d.num(n).mantissa < 0,
        };
        return match role {
            // `-(2)` is the negation of 2; `-2` would read as the literal
            Role::Operand => true,
            // `-2 ** 2` is −(2 ** 2)
            Role::Left(l) => negative && l == tables::POWER,
            _ => false,
        };
    }
    let own = level(e);
    match role {
        Role::Root | Role::Arg | Role::Else | Role::Then => false,
        Role::Left(l) => own > l,
        Role::Right(l) => own > 0 && own >= l,
        Role::Operand => own > tables::UNARY,
        Role::Cond => own >= tables::TERNARY,
    }
}

/// The level a node binds at as written bare (0: an atom).
fn level(e: ir::Expr) -> u8 {
    match e {
        ir::Expr::Call(ir::Func::Pow, _) => tables::POWER,
        ir::Expr::Binary(op, ..) => tables::level(op).unwrap_or(0),
        ir::Expr::Unary(..) => tables::UNARY,
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
        B::IntDiv | B::DivTyped | B::BitAnd | B::BitOr | B::BitXor | B::Shl | B::Shr => {
            panic!("HSPICE has no {op:?} (FEATURES lacks it)")
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
        tail.push(Task::Expr(a, Role::Arg));
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
