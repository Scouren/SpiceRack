//! IR text, the expression layer: expression rows in, canonical text out (`write_expr`), and
//! text in, expression rows out (`parse_expr`), plus the spelling of literals, names, strings
//! and references every row's `dump` uses. Grammar and precedence: `text.rs`'s header.
//!
//! Canonical spelling: one space around a binary operator, `, ` between arguments, the fewest
//! parentheses that rebuild the same tree (every binary level left-associative; the ternary
//! right-associative in its else branch; a ternary as condition or then-branch parenthesized;
//! the `Neg` of a literal written `-(…)` so it is not the literal's own sign). Both walks use
//! explicit stacks (C11): the printer a stack of pieces, the parser shunting-yard's two stacks.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9 (text), §7.2 (A4: references), §7.3 (A3: literals).
//!
//! Contents: `pub struct Ctx` (`Ctx::detached`), `pub fn write_expr`, `pub fn write_value`,
//! `pub fn write_literal`, `pub fn write_name`, `pub fn write_string`, `pub fn write_ref`;
//! crate: `pub(crate) fn binary_prec(BinOp) -> u8`, `pub(crate) fn func_text(Func) -> &'static str`

use crate::design::Design;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, Expr, Func, Literal, NumForm, UnOp};
use crate::id::{DefId, ExprId, ItemRef, Ref, ScopeId};
use crate::item::{DefKind, ItemKind};

/// Where an expression or reference is written: the scope its item sits in (for the A4 rule:
/// bare when `Design::lookup` from here binds the same definition), the item (a `Measure`'s
/// reads may bind measures), and the function whose body is being written (`NONE` elsewhere: a
/// formal argument is bare only inside its function).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ctx {
    /// The scope that lists the item.
    pub scope: ScopeId,
    /// The item being written.
    pub item: ItemRef,
    /// The `Func` definition whose body is being written, or `NONE`.
    pub formals: DefId,
}

/// The scale letters of IR text, longest first so `meg`/`mil`/`peta` win over `m`/`p`.
const SCALES: [(&str, i16); 12] = [
    ("peta", 15),
    ("meg", 6),
    ("a", -18),
    ("f", -15),
    ("p", -12),
    ("n", -9),
    ("u", -6),
    ("m", -3),
    ("c", -2),
    ("k", 3),
    ("g", 9),
    ("t", 12),
];

/// Binding power of each binary operator (higher binds tighter); the ternary is 1, unary 12.
const UNARY: u8 = 12;
const ATOM: u8 = 13;
const TERNARY: u8 = 1;

impl Ctx {
    /// A context with no scope: every resolved reference is written `name#k` (always exact).
    pub const fn detached() -> Ctx {
        Ctx {
            scope: ScopeId::NONE,
            item: ItemRef::NONE,
            formals: DefId::NONE,
        }
    }
}

/// Appends `e` in canonical spelling. Total over a design that passes `check`.
pub fn write_expr(design: &Design, e: ExprId, ctx: &Ctx, out: &mut String) {
    enum Piece {
        Expr(ExprId),
        Text(&'static str),
        Paren(ExprId),
    }
    /// Writes `open` and queues the arguments, separated by `, `, then `close`.
    fn args(
        stack: &mut Vec<Piece>,
        out: &mut String,
        items: &[ExprId],
        open: &'static str,
        close: &'static str,
    ) {
        out.push_str(open);
        stack.push(Piece::Text(close));
        for (k, &a) in items.iter().enumerate().rev() {
            stack.push(Piece::Expr(a));
            if k > 0 {
                stack.push(Piece::Text(", "));
            }
        }
    }
    let mut stack = vec![Piece::Expr(e)];
    let mut scratch = String::new();
    while let Some(piece) = stack.pop() {
        let e = match piece {
            Piece::Text(t) => {
                out.push_str(t);
                continue;
            }
            Piece::Paren(e) => {
                out.push('(');
                stack.push(Piece::Text(")"));
                e
            }
            Piece::Expr(e) => e,
        };
        // pieces are pushed in reverse: the last pushed is written first
        let operand = |stack: &mut Vec<Piece>, child: ExprId, paren: bool| {
            stack.push(if paren {
                Piece::Paren(child)
            } else {
                Piece::Expr(child)
            })
        };
        match design.expr(e) {
            Expr::Num(n) => write_literal(design.num(n), out),
            Expr::Str(s) => write_string(design.str(s), out),
            Expr::Param(r) => {
                scratch.clear();
                expr_ref(design, r, ctx, false, &mut scratch);
                out.push_str(&scratch);
            }
            Expr::Time => out.push_str("$time"),
            Expr::Temp => out.push_str("$temp"),
            Expr::Freq => out.push_str("$freq"),
            Expr::TStep => out.push_str("$tstep"),
            Expr::TStop => out.push_str("$tstop"),
            Expr::Tnom => out.push_str("$tnom"),
            Expr::V(net, reference) => {
                out.push_str("v(");
                write_name(design.str(net), out);
                if !reference.is_none() {
                    out.push_str(", ");
                    write_name(design.str(reference), out);
                }
                out.push(')');
            }
            Expr::I(element) => {
                out.push_str("i(");
                write_name(design.str(element), out);
                out.push(')');
            }
            Expr::Unary(op, a) => {
                out.push_str(match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                    UnOp::BitNot => "~",
                });
                let literal = matches!(design.expr(a), Expr::Num(_));
                operand(
                    &mut stack,
                    a,
                    prec(design, a) < UNARY || (op == UnOp::Neg && literal),
                );
            }
            Expr::Binary(op, a, b) => {
                let p = binary_prec(op);
                operand(&mut stack, b, prec(design, b) <= p);
                stack.push(Piece::Text(binary_text(op)));
                operand(&mut stack, a, prec(design, a) < p);
            }
            Expr::Ternary(c, a, b) => {
                operand(&mut stack, b, false);
                stack.push(Piece::Text(" : "));
                operand(&mut stack, a, prec(design, a) <= TERNARY);
                stack.push(Piece::Text(" ? "));
                operand(&mut stack, c, prec(design, c) <= TERNARY);
            }
            Expr::Call(f, l) => {
                out.push_str(func_text(f));
                args(&mut stack, out, design.exprs(l), "(", ")");
            }
            Expr::Draw(d, w, p, l) => {
                out.push_str("draw:");
                out.push_str(dist_name(d));
                out.push_str(when_name(w));
                out.push_str(per_name(p));
                args(&mut stack, out, design.exprs(l), "(", ")");
            }
            Expr::UserCall(r, l) => {
                out.push('@');
                scratch.clear();
                expr_ref(design, r, ctx, true, &mut scratch);
                out.push_str(&scratch);
                args(&mut stack, out, design.exprs(l), "(", ")");
            }
            Expr::List(l) => args(&mut stack, out, design.exprs(l), "[", "]"),
        }
    }
}

/// Appends a field value: an atom as is (a literal, a string, a reference, a leaf), anything
/// else as `(expr)`.
pub fn write_value(design: &Design, e: ExprId, ctx: &Ctx, out: &mut String) {
    let atom = matches!(
        design.expr(e),
        Expr::Num(_)
            | Expr::Str(_)
            | Expr::Param(_)
            | Expr::Time
            | Expr::Temp
            | Expr::Freq
            | Expr::TStep
            | Expr::TStop
            | Expr::Tnom
    );
    if !atom {
        out.push('(');
    }
    write_expr(design, e, ctx, out);
    if !atom {
        out.push(')');
    }
}

/// Appends a literal: its digits with `frac` of them after the point, then `e<exp10>`, a scale
/// letter, `mil`, or nothing (A3).
pub fn write_literal(l: Literal, out: &mut String) {
    if l.mantissa < 0 {
        out.push('-');
    }
    let digits = l.mantissa.unsigned_abs().to_string();
    let frac = l.frac as usize;
    if frac == 0 {
        out.push_str(&digits);
    } else {
        let padded = format!("{digits:0>width$}", width = frac + 1);
        let (int, dec) = padded.split_at(padded.len() - frac);
        out.push_str(int);
        out.push('.');
        out.push_str(dec);
    }
    match l.form {
        NumForm::Plain => {}
        NumForm::Exponent => out.push_str(&format!("e{}", l.exp10)),
        NumForm::Scale => {
            let letter = SCALES
                .iter()
                .find(|s| s.1 == l.exp10)
                .expect("a Scale literal's exp10 has a letter");
            out.push_str(letter.0);
        }
        NumForm::Mil => out.push_str("mil"),
    }
}

/// Appends a name in a name position: bare when it is `[A-Za-z0-9_.$]+`, else backtick-quoted
/// with `` \` `` and `\\` escaped.
pub fn write_name(name: &str, out: &mut String) {
    let bare = !name.is_empty() && name.bytes().all(name_byte);
    if bare {
        out.push_str(name);
    } else {
        quote(name, '`', out);
    }
}

/// Appends a double-quoted string with `\"`, `\\` and `\n` escaped.
pub fn write_string(s: &str, out: &mut String) {
    quote(s, '"', out);
}

/// Appends a row's reference to a definition of `kind`: bare when the linker's lookup from
/// `ctx.scope` gives what the reference holds (the definition, or nothing for an unresolved
/// one), `name?` for an unresolved reference the lookup would bind, `name#k` otherwise (A4).
pub fn write_ref(design: &Design, r: Ref, kind: DefKind, ctx: &Ctx, out: &mut String) {
    write_name(design.str(r.name), out);
    let would_bind = (!ctx.scope.is_none()).then(|| design.lookup(ctx.scope, kind, r.name));
    mark(r, would_bind, out);
}

/// The binding power of a binary operator (higher binds tighter; the ternary is 1, unary 12).
pub(crate) fn binary_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 2,
        BinOp::And => 3,
        BinOp::BitOr => 4,
        BinOp::BitXor => 5,
        BinOp::BitAnd => 6,
        BinOp::Eq | BinOp::Ne => 7,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 8,
        BinOp::Shl | BinOp::Shr => 9,
        BinOp::Add | BinOp::Sub => 10,
        BinOp::Mul | BinOp::Div | BinOp::IntDiv => 11,
    }
}

/// Each function's IR-text name (snake case of the member).
pub(crate) fn func_text(f: Func) -> &'static str {
    match f {
        Func::Abs => "abs",
        Func::Sqrt => "sqrt",
        Func::SqrtAbs => "sqrt_abs",
        Func::Exp => "exp",
        Func::ExpLin14 => "exp_lin14",
        Func::Ln => "ln",
        Func::LnAbs => "ln_abs",
        Func::Log10 => "log10",
        Func::Pow => "pow",
        Func::PowAbs => "pow_abs",
        Func::PowRe => "pow_re",
        Func::Min => "min",
        Func::Max => "max",
        Func::Floor => "floor",
        Func::Ceil => "ceil",
        Func::Trunc => "trunc",
        Func::RoundHalfAway => "round_half_away",
        Func::RoundHalfEven => "round_half_even",
        Func::Sgn => "sgn",
        Func::Sin => "sin",
        Func::Cos => "cos",
        Func::Tan => "tan",
        Func::Asin => "asin",
        Func::Acos => "acos",
        Func::Atan => "atan",
        Func::Atan2 => "atan2",
        Func::Sinh => "sinh",
        Func::Cosh => "cosh",
        Func::Tanh => "tanh",
        Func::Asinh => "asinh",
        Func::Acosh => "acosh",
        Func::Atanh => "atanh",
        Func::Hypot => "hypot",
        Func::Fmod => "fmod",
        Func::Clamp => "clamp",
        Func::Step => "step",
        Func::Ramp => "ramp",
        Func::Ddt => "ddt",
        Func::Idt => "idt",
        Func::Pwl => "pwl",
    }
}

// ---- private ----

/// The binding power of an expression as an operand (atoms bind tightest).
fn prec(design: &Design, e: ExprId) -> u8 {
    match design.expr(e) {
        Expr::Ternary(..) => TERNARY,
        Expr::Binary(op, ..) => binary_prec(op),
        Expr::Unary(..) => UNARY,
        Expr::Num(_)
        | Expr::Str(_)
        | Expr::Param(_)
        | Expr::Call(..)
        | Expr::UserCall(..)
        | Expr::V(..)
        | Expr::I(_)
        | Expr::Time
        | Expr::Temp
        | Expr::Freq
        | Expr::TStep
        | Expr::TStop
        | Expr::Tnom
        | Expr::Draw(..)
        | Expr::List(_) => ATOM,
    }
}

/// The operator with its surrounding blanks.
fn binary_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => " + ",
        BinOp::Sub => " - ",
        BinOp::Mul => " * ",
        BinOp::Div => " / ",
        BinOp::IntDiv => " // ",
        BinOp::Eq => " == ",
        BinOp::Ne => " != ",
        BinOp::Lt => " < ",
        BinOp::Le => " <= ",
        BinOp::Gt => " > ",
        BinOp::Ge => " >= ",
        BinOp::And => " && ",
        BinOp::Or => " || ",
        BinOp::BitAnd => " & ",
        BinOp::BitOr => " | ",
        BinOp::BitXor => " ^ ",
        BinOp::Shl => " << ",
        BinOp::Shr => " >> ",
    }
}

fn dist_name(d: Dist) -> &'static str {
    match d {
        Dist::AbsGauss => "abs_gauss",
        Dist::RelGauss => "rel_gauss",
        Dist::AbsUnif => "abs_unif",
        Dist::RelUnif => "rel_unif",
        Dist::Limit => "limit",
    }
}

fn when_name(w: DrawWhen) -> &'static str {
    match w {
        DrawWhen::Always => ":always",
        DrawWhen::Sampling => ":sampling",
    }
}

fn per_name(p: DrawPer) -> &'static str {
    match p {
        DrawPer::Reference => ":reference",
        DrawPer::CallSite => ":call_site",
        DrawPer::Instance => ":instance",
        DrawPer::Run => ":run",
    }
}

/// A reference inside an expression: a parameter read (`call == false`) or a user call's
/// function. Bare only when the IR-text parser would bind it the same way: `link`'s lookup from
/// the scope (a measure's reads fall back to measures), or for a formal argument, inside its own
/// function; a read named like a formal of the function being written is never bare unless it
/// is that formal (the parser binds those to the function).
fn expr_ref(design: &Design, r: Ref, ctx: &Ctx, call: bool, out: &mut String) {
    let name = design.str(r.name);
    if is_ident(name) {
        out.push_str(name);
    } else {
        quote(name, '`', out);
    }
    let formal = !ctx.formals.is_none() && {
        let f: crate::item::Func = design.row(design.def(ctx.formals).item.index);
        design.names(f.args).contains(&r.name)
    };
    // what the parser would bind a bare spelling to: the formal (inside its function), else
    // the linker's lookup (a measure's reads fall back to measures)
    let would_bind = if ctx.scope.is_none() {
        None
    } else if call {
        Some(design.lookup(ctx.scope, DefKind::Func, r.name))
    } else if formal {
        Some(ctx.formals)
    } else {
        let found = design.lookup(ctx.scope, DefKind::Param, r.name);
        let measure = ctx.item.kind == ItemKind::Measure && ctx.item != ItemRef::NONE;
        Some(if found.is_none() && measure {
            design.lookup(ctx.scope, DefKind::Measure, r.name)
        } else {
            found
        })
    };
    mark(r, would_bind, out);
}

/// The mark after a reference's name, given what a bare spelling would bind to (`None`: no
/// context, so only `?` and `#k` are exact).
fn mark(r: Ref, would_bind: Option<DefId>, out: &mut String) {
    let resolved = r.is_resolved();
    match would_bind {
        Some(found) if found == r.def || (!resolved && found.is_none()) => {}
        Some(_) | None if !resolved => out.push('?'),
        Some(_) | None => out.push_str(&format!("#{}", r.def.0)),
    }
}

/// A byte of a bare name in a name position.
fn name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'$'
}

/// A bare name in an expression: `[A-Za-z_][A-Za-z0-9_.$]*`.
fn is_ident(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && bytes.all(name_byte)
}

/// Writes `s` between `q`s, escaping `q`, `\` and a newline.
fn quote(s: &str, q: char, out: &mut String) {
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(q);
}
