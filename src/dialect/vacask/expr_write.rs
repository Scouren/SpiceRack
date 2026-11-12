//! An IR expression → VACASK text: VACASK's precedence table (VD expr-operators: `? :`
//! loosest and right-associative, then `|| && | ^ & == != < <= > >= << >> + - * /`
//! left-associative, `**` right-associative, unary `- ! ~` tighter than `**`), the fewest
//! parentheses that read back as the same tree, one space around a binary operator. `Pow` is
//! `**`, `Ln` is `ln`, `Trunc` is `integer`, `RoundHalfAway` is `round`; a draw is its call;
//! `Temp` is `$temp`, `Tnom` is `$tnom`. A negative literal is `-` and its digits, a `Neg` of a
//! literal `-(…)`. Explicit stack (C11). Total over the features of `FEATURES`.
//!
//! Cites: VD expr-operators, expr-functions, expr-special; `rewrite/DIALECT-VACASK.md` §4.
//!
//! Contents: `write_expr` (crate-private)

use super::name::{self, Place};
use super::number;
use crate::ir;

/// One step of the writer: text, or an expression (in parentheses or not).
enum Task {
    Text(&'static str),
    Expr(ir::ExprId, bool),
}

/// VACASK's precedence levels (VD expr-operators), loosest first.
const TERNARY: u8 = 1;
/// `**`.
const POWER: u8 = 12;
/// Unary `- ! ~`.
const UNARY: u8 = 13;
/// A leaf, a call, a literal.
const PRIMARY: u8 = 14;

/// Appends expression `e` of `design` as VACASK text.
pub(super) fn write_expr(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    let mut todo = vec![Task::Expr(e, false)];
    // Bounded: each expression row is pushed once per parent that reads it (a tree).
    while let Some(task) = todo.pop() {
        let (e, paren) = match task {
            Task::Text(t) => {
                out.push_str(t);
                continue;
            }
            Task::Expr(e, paren) => (e, paren),
        };
        if paren {
            out.push('(');
            todo.push(Task::Text(")"));
        }
        match design.expr(e) {
            ir::Expr::Num(id) => number::spell(design.num(id), out),
            ir::Expr::Str(s) => string(design.str(s), out),
            ir::Expr::Param(r) => name::spell(design.str(r.name), Place::Other, out),
            ir::Expr::Temp => out.push_str("$temp"),
            ir::Expr::Tnom => out.push_str("$tnom"),
            ir::Expr::Time => out.push_str("$abstime"),
            ir::Expr::Unary(op, x) => {
                out.push_str(match op {
                    ir::UnOp::Neg => "-",
                    ir::UnOp::Not => "!",
                    ir::UnOp::BitNot => "~",
                });
                let literal = matches!(design.expr(x), ir::Expr::Num(_));
                todo.push(Task::Expr(x, literal || prec(design, x) < UNARY));
            }
            ir::Expr::Binary(op, a, b) => {
                let (p, word) = binary(op);
                push_binary(design, &mut todo, p, word, a, b, false);
            }
            ir::Expr::Call(ir::Func::Pow, args) => {
                let [a, b] = design.exprs(args) else {
                    unreachable!("pow has two arguments (ir::check)");
                };
                push_binary(design, &mut todo, POWER, " ** ", *a, *b, true);
            }
            ir::Expr::Call(func, args) => {
                out.push_str(function(func));
                push_args(&mut todo, design.exprs(args));
            }
            ir::Expr::Draw(dist, _, _, args) => {
                out.push_str(match dist {
                    ir::Dist::AbsGauss => "agauss",
                    ir::Dist::RelGauss => "gauss",
                    ir::Dist::AbsUnif => "aunif",
                    ir::Dist::RelUnif => "unif",
                    ir::Dist::Limit => unreachable!("FEATURES lacks Dist::Limit"),
                });
                push_args(&mut todo, design.exprs(args));
            }
            ir::Expr::Ternary(c, t, f) => {
                todo.push(Task::Expr(f, false));
                todo.push(Task::Text(" : "));
                todo.push(Task::Expr(t, false));
                todo.push(Task::Text(" ? "));
                todo.push(Task::Expr(c, prec(design, c) <= TERNARY));
            }
            other => unreachable!("FEATURES lacks {other:?} (S4.1 writes probes)"),
        }
    }
}

// ---- private ----

/// A binary operator's level and spelling.
fn binary(op: ir::BinOp) -> (u8, &'static str) {
    use ir::BinOp as B;
    match op {
        B::Or => (2, " || "),
        B::And => (3, " && "),
        B::BitOr => (4, " | "),
        B::BitXor => (5, " ^ "),
        B::BitAnd => (6, " & "),
        B::Eq => (7, " == "),
        B::Ne => (7, " != "),
        B::Lt => (8, " < "),
        B::Le => (8, " <= "),
        B::Gt => (8, " > "),
        B::Ge => (8, " >= "),
        B::Shl => (9, " << "),
        B::Shr => (9, " >> "),
        B::Add => (10, " + "),
        B::Sub => (10, " - "),
        B::Mul => (11, " * "),
        B::DivTyped => (11, " / "),
        B::Div | B::IntDiv => unreachable!("FEATURES lacks {op:?}: a pass converts it"),
    }
}

/// The level of expression `e` as written.
fn prec(design: &ir::Design, e: ir::ExprId) -> u8 {
    match design.expr(e) {
        ir::Expr::Binary(op, _, _) => binary(op).0,
        ir::Expr::Call(ir::Func::Pow, _) => POWER,
        ir::Expr::Unary(..) => UNARY,
        ir::Expr::Ternary(..) => TERNARY,
        _ => PRIMARY,
    }
}

/// Pushes `a word b` at level `p`, each side in parentheses where VACASK would group it otherwise.
fn push_binary(
    design: &ir::Design,
    todo: &mut Vec<Task>,
    p: u8,
    word: &'static str,
    a: ir::ExprId,
    b: ir::ExprId,
    right_assoc: bool,
) {
    let (pa, pb) = (prec(design, a), prec(design, b));
    todo.push(Task::Expr(b, pb < p || (pb == p && !right_assoc)));
    todo.push(Task::Text(word));
    todo.push(Task::Expr(a, pa < p || (pa == p && right_assoc)));
}

/// Pushes `(a, b, …)`.
fn push_args(todo: &mut Vec<Task>, args: &[ir::ExprId]) {
    todo.push(Task::Text(")"));
    for (k, &a) in args.iter().enumerate().rev() {
        todo.push(Task::Expr(a, false));
        if k > 0 {
            todo.push(Task::Text(", "));
        }
    }
    todo.push(Task::Text("("));
}

/// VACASK's name of a function (every member `FEATURES` holds).
fn function(func: ir::Func) -> &'static str {
    use ir::Func as F;
    match func {
        F::Abs => "abs",
        F::Sqrt => "sqrt",
        F::Exp => "exp",
        F::Ln => "ln",
        F::Log10 => "log10",
        F::Min => "min",
        F::Max => "max",
        F::Floor => "floor",
        F::Ceil => "ceil",
        F::Trunc => "integer",
        F::ToInt => "int",
        F::RoundHalfAway => "round",
        F::Sin => "sin",
        F::Cos => "cos",
        F::Tan => "tan",
        F::Asin => "asin",
        F::Acos => "acos",
        F::Atan => "atan",
        F::Atan2 => "atan2",
        F::Sinh => "sinh",
        F::Cosh => "cosh",
        F::Tanh => "tanh",
        F::Asinh => "asinh",
        F::Acosh => "acosh",
        F::Atanh => "atanh",
        F::Hypot => "hypot",
        F::Fmod => "fmod",
        other => unreachable!("FEATURES lacks {other:?}"),
    }
}

/// `"text"`, with `"`, `\\` and newlines escaped (VD input-strings).
fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
}
