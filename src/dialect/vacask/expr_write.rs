//! An IR expression → VACASK text: VACASK's precedence table (VD expr-operators: `? :`
//! loosest and right-associative, then `|| && | ^ & == != < <= > >= << >> + - * /`
//! left-associative, `**` right-associative, unary `- ! ~` tighter than `**`), the fewest
//! parentheses that read back as the same tree, one space around a binary operator. `Pow` is
//! `**`, `Ln` is `ln`, `Trunc` is `integer`, `RoundHalfAway` is `round`; a draw is its call;
//! `Temp` is `$temp`, `Tnom` is `$tnom`; `V`, `I`, `Time` are `v()`, `i()`, `$abstime`, and
//! `to_real` of a parameter read in a circuit-reading field is the bare name (a behavioural
//! source reads every free identifier as a real: R40). A negative literal is `-` and its digits,
//! a `Neg` of a literal `-(…)`. Explicit stack (C11). Total over the features of `FEATURES` and
//! what `admits` lets through.
//!
//! Cites: VD expr-operators, expr-functions, expr-special; `rewrite/DIALECT-VACASK.md` §4.
//!
//! Contents: `NO_REAL`, `WIDE_INT`, `CODES`, `write_expr`, `write_behavioural`, `admits`, `string` (crate-private)

use super::name::{self, Place};
use super::number;
use crate::ir;

/// A real conversion VACASK cannot spell: `to_real` anywhere but over a parameter read in a
/// behavioural source's expression (a passive's value is not one: R52). A bare parameter read
/// there is `CircuitExtra::BehavUntypedParam`, which `FEATURES` lacks (A33); what remains here
/// is R52's deferred gap (c): `to_real` in a parameter expression of class Temp, which the IR's
/// dynamic form joins with a circuit-reading field.
pub(super) const NO_REAL: ir::Code = ir::Code::new(b'W', 2006);

/// An integer literal past 32 bits, which VACASK would wrap (run iw1).
pub(super) const WIDE_INT: ir::Code = ir::Code::new(b'W', 2008);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        NO_REAL,
        ir::Info {
            title: "a real conversion VACASK cannot spell",
            cite: "VD dev-builtin-behavioral; R40; DIALECT-VACASK Appendix A run b1",
            explain: "A VACASK behavioural source reads every parameter as a real and a parameter expression keeps the parameter's type; VACASK has no function that converts a value to real. So `to_real` is spelled only over a parameter read in a behavioural source's expression, and a parameter read there must be one.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        WIDE_INT,
        ir::Info {
            title: "an integer literal past 32 bits",
            cite: "DIALECT-VACASK Appendix A run iw1",
            explain: "VACASK's integers are 32 bits and a wider literal wraps; the IR's integer does not, so no VACASK spelling states it.",
            kind: ir::Kind::Refusal,
        },
    ),
];

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
    write(design, e, false, out);
}

/// Appends expression `e` of a behavioural source's own expression: as [`write_expr`], but
/// `to_int` of a parameter read is `$intparam(p)` (VACASK's only integer read of a parameter
/// there, behavioural-only: runs ip1, ip4; R56).
pub(super) fn write_behavioural(design: &ir::Design, e: ir::ExprId, out: &mut String) {
    write(design, e, true, out);
}

/// Appends a refusal, at its item, for each real conversion VACASK cannot spell ([`NO_REAL`])
/// and each integer literal it would wrap ([`WIDE_INT`]).
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    let mut handles = Vec::new();
    for (at, _) in design.reachable() {
        handles.clear();
        design.item_handles(at, &mut handles);
        // Only a behavioural source's expression is VACASK's behavioural evaluator (R52); a
        // passive's value is its parameter evaluator's, whatever the handle's kind.
        let behavioural = at.kind == ir::ItemKind::Element
            && design.row::<ir::Element>(at.index).kind == ir::ElemKind::Behav;
        let refused = handles.iter().find_map(|&h| match h {
            ir::Handle::Expr(e) => refusal(design, e, false),
            ir::Handle::ExprDyn(e) => refusal(design, e, behavioural),
            ir::Handle::Exprs(list) => design
                .exprs(list)
                .iter()
                .find_map(|&e| refusal(design, e, false)),
            ir::Handle::Params(list) => design
                .params(list)
                .iter()
                .find_map(|&(_, e)| refusal(design, e, false)),
            _ => None,
        });
        if let Some(code) = refused {
            diags.push_at(code, design, at, 0);
        }
    }
}

/// `"text"`, with `"`, `\\` and a line break escaped: VACASK reads `\"`, `\\`, `\n` back and
/// ends a string at a raw line break (settle str_escapes, unclosed_q1). The dialect's one
/// string quoter.
pub(super) fn string(text: &str, out: &mut String) {
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

// ---- private ----

/// [`write_expr`] and, with `behavioural`, [`write_behavioural`].
fn write(design: &ir::Design, e: ir::ExprId, behavioural: bool, out: &mut String) {
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
            ir::Expr::Num(id) => number::spell(design, design.num(id), out),
            ir::Expr::Str(s) => string(design.str(s), out),
            ir::Expr::Param(r) => name::spell(design.str(r.name), Place::Other, out),
            ir::Expr::Temp => out.push_str("$temp"),
            ir::Expr::Tnom => out.push_str("$tnom"),
            ir::Expr::Time => out.push_str("$abstime"),
            ir::Expr::V(net, reference) => {
                out.push_str("v(");
                name::spell(design.str(net), Place::Node, out);
                if !reference.is_none() {
                    out.push_str(", ");
                    name::spell(design.str(reference), Place::Node, out);
                }
                out.push(')');
            }
            ir::Expr::I(element) => {
                out.push_str("i(");
                name::spell(design.str(element), Place::Other, out);
                out.push(')');
            }
            ir::Expr::Call(ir::Func::ToInt, args)
                if behavioural
                    && matches!(design.expr(design.exprs(args)[0]), ir::Expr::Param(_)) =>
            {
                let ir::Expr::Param(r) = design.expr(design.exprs(args)[0]) else {
                    unreachable!("matched above");
                };
                out.push_str("$intparam(");
                name::spell(design.str(r.name), Place::Other, out);
                out.push(')');
            }
            ir::Expr::Call(ir::Func::ToReal, args) => {
                // `admits`: a parameter read in a circuit-reading field, which reads as a real.
                todo.push(Task::Expr(design.exprs(args)[0], false));
            }
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

/// What VACASK cannot spell in root `e`: an integer literal past 32 bits ([`WIDE_INT`]); a real
/// conversion it lacks ([`NO_REAL`]): in a behavioural source's expression (`circuit`) a
/// `to_real` of anything but a parameter read; in any other field a `to_real`.
fn refusal(design: &ir::Design, e: ir::ExprId, circuit: bool) -> Option<ir::Code> {
    if e.is_none() {
        return None;
    }
    let mut todo = vec![e];
    // Bounded: each row of the (acyclic) expression tree is taken once per parent.
    while let Some(x) = todo.pop() {
        match design.expr(x) {
            ir::Expr::Num(id) => {
                let l = design.num(id);
                if l.form == ir::NumForm::Int && !number::int_fits(l.mantissa) {
                    return Some(WIDE_INT);
                }
            }
            ir::Expr::Call(ir::Func::ToReal, args) => {
                let arg = design.exprs(args)[0];
                if !circuit || !matches!(design.expr(arg), ir::Expr::Param(_)) {
                    return Some(NO_REAL);
                }
            }
            ir::Expr::Unary(_, a) => todo.push(a),
            ir::Expr::Binary(_, a, b) => todo.extend([a, b]),
            ir::Expr::Ternary(a, b, c) => todo.extend([a, b, c]),
            ir::Expr::Call(_, args) | ir::Expr::Draw(_, _, _, args) | ir::Expr::List(args) => {
                todo.extend_from_slice(design.exprs(args));
            }
            _ => {}
        }
    }
    None
}

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
        F::SgnZeroPos => "sgn",
        F::Ddt => "ddt",
        F::Idt => "idt",
        other => unreachable!("FEATURES lacks {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::super::expr_read::{Context, expr};
    use super::super::read::{Cst, Lower};
    use super::*;
    use crate::syntax::ast::{Node, Tag};
    use crate::syntax::{Lang, parse, tokenize};

    /// `text`'s first instance's `v=` value read in the behavioural context: the expression
    /// written back, and whether its parameter read is wrapped in `to_real`.
    fn behavioural(text: &str) -> (String, bool) {
        let lex = Lang::VACASK.lexicon().expect("a lexicon");
        let tokens = tokenize(text, lex);
        let ast = parse(&tokens, lex);
        let mut design = ir::Design::new();
        let mut b = design.builder();
        let (file, top) = b.file("t.sim", ir::Role::Deck);
        let cst = Cst {
            tokens,
            ast,
            shift: 0,
            file,
        };
        let ast = &cst.ast;
        let inst = ast
            .fields(ast.root())
            .iter()
            .map(|&s| Node(s))
            .find(|&n| ast.tag(n) == Tag::Instance)
            .expect("an instance");
        let value = ast
            .fields(inst)
            .iter()
            .map(|&f| Node(f))
            .find(|&f| ast.tag(f) == Tag::Assign)
            .map(|f| ast.rhs(f))
            .expect("v=");
        let (mut diags, grounds, files, models) = (
            ir::Diags::new(),
            HashSet::from(["gnd".to_string()]),
            HashSet::new(),
            HashSet::new(),
        );
        let (mut reads, mut masters) = (Vec::new(), Vec::new());
        let declared = super::super::read_elements::declare(&[]);
        let folds = super::super::fold::folds(&[]);
        let mut l = Lower {
            b: &mut b,
            diags: &mut diags,
            cst: &cst,
            scope: top,
            stmt: ir::Span::NONE,
            grounds: &grounds,
            named_files: &files,
            source_models: &models,
            reads: &mut reads,
            masters: &mut masters,
            declared: &declared,
            folds: &folds,
            taken: HashSet::new(),
            gap_token: 0,
            gap_end: 0,
        };
        let e = expr(&mut l, value, Context::Behavioural).expect("read");
        let real = (0..design.expr_count()).any(|k| {
            matches!(
                design.expr(ir::ExprId(k as u32)),
                ir::Expr::Call(ir::Func::ToReal, _)
            )
        });
        let mut out = String::new();
        write_behavioural(&design, e, &mut out);
        (out, real)
    }

    /// Oracle: DIALECT-VACASK Appendix A runs bp1 (`v(n)`, `v(a, b)`, `i(inst)`, `ddt`, `idt`
    /// read the circuit in a behavioural source) and b1 (a parameter read there is a real, R40);
    /// VD cir-nodes (a `ground` name is node 0). The written text reads back as the same tree.
    #[test]
    fn a_behavioural_expression_reads_the_circuit_and_parameters_as_reals() {
        let cases = [
            ("b1 (c 0) v=v(a)*2", "v(a) * 2", false),
            ("b1 (c 0) v=v(a, gnd)+v(a, b)", "v(a) + v(a, b)", false),
            ("b1 (c 0) v=i(v1)*1k", "i(v1) * 1k", false),
            (
                "b1 (c 0) v=idt(v(a))+ddt(v(a))",
                "idt(v(a)) + ddt(v(a))",
                false,
            ),
            // A28: `idt(x, ic)` keeps its initial value; a literal-0 one is the absent one.
            ("b1 (c 0) v=idt(v(a), 3)", "idt(v(a), 3)", false),
            ("b1 (c 0) v=idt(v(a), 0.0)", "idt(v(a))", false),
            ("b1 (c 0) v=p/2", "p / 2", true),
            // R56: `$realparam(p)` is `to_real(p)`, written back as the bare `p`.
            ("b1 (c 0) v=$realparam(p)/2", "p / 2", true),
            ("b1 (c 0) v=$intparam(p)/2", "$intparam(p) / 2", false),
        ];
        for (text, want, real) in cases {
            assert_eq!(
                behavioural(&format!("t\n{text}\n")),
                (want.to_string(), real),
                "{text}"
            );
        }
    }
}
