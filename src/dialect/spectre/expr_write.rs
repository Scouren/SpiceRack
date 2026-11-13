//! An IR expression → Spectre text, the reader's image read back: a blank around each binary
//! operator (a name may end in `!`, so `a!=b` reads as the name `a!`), a compound operand in
//! parentheses (a parenthesis reads as nothing; the writer parenthesizes every compound
//! parameter value, inside which blanks do not end the parameter), `Pow` as `pow(a,b)`, `Ln` as `log`, `Trunc` as `int`,
//! `RoundHalfAway` as `round`; `Temp`, `Tnom`, `Time`, `Freq` as `temp`, `tnom`, `$time`,
//! `$freq`; `v(a)`, `v(a,b)`. A negative literal is `-` and its digits, a `Neg` of a literal
//! `-(…)` (the reader folds `-` before a nonzero number into the literal).
//!
//! Contents: `write_expr`, `write_item` (crate-private)

use super::number;
use crate::ir;

/// Appends `e`'s Spectre text.
pub(super) fn write_expr(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    assert!(!e.is_none(), "write_expr: an expression");
    match design.expr(e) {
        ir::Expr::Num(n) => number::spell(design.num(n), out),
        ir::Expr::Param(r) => out.push_str(design.str(r.name)),
        ir::Expr::Temp => out.push_str("temp"),
        ir::Expr::Tnom => out.push_str("tnom"),
        ir::Expr::Time => out.push_str("$time"),
        ir::Expr::Freq => out.push_str("$freq"),
        ir::Expr::V(a, b) => {
            out.push_str("v(");
            out.push_str(design.str(a));
            if !b.is_none() {
                out.push(',');
                out.push_str(design.str(b));
            }
            out.push(')');
        }
        ir::Expr::Call(f, args) => call(design, func(f), design.exprs(args), out),
        ir::Expr::UserCall(r, args) => call(design, design.str(r.name), design.exprs(args), out),
        ir::Expr::Unary(op, x) => {
            out.push(match op {
                ir::UnOp::Neg => '-',
                ir::UnOp::Not => '!',
                ir::UnOp::BitNot => unreachable!("FEATURES lacks BitNot"),
            });
            let literal = matches!(design.expr(x), ir::Expr::Num(_));
            operand(design, x, literal, out);
        }
        ir::Expr::Binary(op, a, b) => {
            operand(design, a, false, out);
            out.push(' ');
            out.push_str(binary(op));
            out.push(' ');
            operand(design, b, false, out);
        }
        ir::Expr::Ternary(c, t, f) => {
            operand(design, c, false, out);
            out.push_str(" ? ");
            operand(design, t, false, out);
            out.push_str(" : ");
            operand(design, f, false, out);
        }
        other => unreachable!("FEATURES lacks {other:?}"),
    }
}

/// Appends `e` as a vector item: a nonnegative number as itself, anything else in parentheses,
/// so that a blank-separated list reads back item by item (`[a (b)]` would read `a(b)` as a
/// call).
pub(super) fn write_item(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    match design.expr(e) {
        ir::Expr::Num(n) if design.num(n).mantissa >= 0 => write_expr(design, e, out),
        _ => {
            out.push('(');
            write_expr(design, e, out);
            out.push(')');
        }
    }
}

// ---- private ----

/// An operand: an atom as itself, anything compound (or a literal under `-`) in parentheses.
fn operand(design: &ir::Design, x: ir::ExprId, paren: bool, out: &mut String) {
    let compound = match design.expr(x) {
        ir::Expr::Unary(..) | ir::Expr::Binary(..) | ir::Expr::Ternary(..) => true,
        ir::Expr::Num(n) => paren || design.num(n).mantissa < 0,
        _ => false,
    };
    if compound {
        out.push('(');
    }
    write_expr(design, x, out);
    if compound {
        out.push(')');
    }
}

/// `name(a,b,…)`.
fn call(design: &ir::Design, name: &str, args: &[ir::ExprId], out: &mut String) {
    out.push_str(name);
    out.push('(');
    for (k, &a) in args.iter().enumerate() {
        if k > 0 {
            out.push(',');
        }
        write_expr(design, a, out);
    }
    out.push(')');
}

/// Spectre's name of `f` (the reader's table read backwards).
fn func(f: ir::Func) -> &'static str {
    use ir::Func as F;
    match f {
        F::Abs => "abs",
        F::Sqrt => "sqrt",
        F::Exp => "exp",
        F::Ln => "log",
        F::Log10 => "log10",
        F::Pow => "pow",
        F::Min => "min",
        F::Max => "max",
        F::Floor => "floor",
        F::Ceil => "ceil",
        F::Trunc => "int",
        F::RoundHalfAway => "round",
        F::Fmod => "fmod",
        F::Hypot => "hypot",
        F::Sin => "sin",
        F::Cos => "cos",
        F::Tan => "tan",
        F::Asin => "asin",
        F::Acos => "acos",
        F::Atan => "atan",
        F::Sinh => "sinh",
        F::Cosh => "cosh",
        F::Tanh => "tanh",
        F::Asinh => "asinh",
        F::Acosh => "acosh",
        F::Atanh => "atanh",
        other => unreachable!("FEATURES lacks {other:?}"),
    }
}

/// Spectre's spelling of `op`.
fn binary(op: ir::BinOp) -> &'static str {
    use ir::BinOp as B;
    match op {
        B::Add => "+",
        B::Sub => "-",
        B::Mul => "*",
        B::Div => "/",
        B::Eq => "==",
        B::Ne => "!=",
        B::Lt => "<",
        B::Le => "<=",
        B::Gt => ">",
        B::Ge => ">=",
        B::And => "&&",
        B::Or => "||",
        other => unreachable!("FEATURES lacks {other:?}"),
    }
}
