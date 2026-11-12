//! IR text, the expression reader: bytes of one line in, expression rows out, by shunting-yard
//! (an operand stack and an operator stack, no recursion on nesting depth: C11). Rows are built
//! in the order a builder caller writes them (operands left to right, then their operator;
//! arguments, then the call), so `parse(dump(d))` reproduces `d`'s ids. Also the lexers of a
//! literal, a name and a quoted run, shared with `Words`.
//!
//! References are read as written: `name` unresolved (the linker binds it after the whole
//! text), `name?` held unresolved (`DefId::HELD` until `text::parse` clears it), `name#k` bound
//! to definition `k`. A ternary's `?` must follow a blank; a `?` glued to a name is that name's
//! mark.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, §7.2 (A4), §7.3 (A3); `compiler-construction` rule 7.
//!
//! Contents: crate: `pub(crate) fn parse_expr(&str, usize, usize, &mut Builder<'_>) ->
//! Result<(ExprId, usize), usize>`

use std::borrow::Cow;

use crate::build::Builder;
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, Func, UnOp};
use crate::id::ExprId;
use crate::text_expr::{binary_prec, func_text};
use crate::text_lex::{
    Lexed, Mark, apply_mark, ident_or_quoted, lex_mark, lex_name, lex_number, name_byte,
    number_end, unquote,
};

/// An entry of the operator stack.
enum Op<'t> {
    Un(UnOp),
    Bin(BinOp),
    /// A ternary's `?` seen, its `:` not yet.
    Quest,
    /// A ternary's `:` seen: the condition and then-branch are on the value stack.
    Colon,
    /// A grouping `(`; `base` is the value count when it opened.
    Paren(usize),
    /// An argument list: what it builds and the value count when it opened.
    Args(Call<'t>, usize),
}

/// What a closed argument list builds.
enum Call<'t> {
    Func(Func),
    Draw(Dist, DrawWhen, DrawPer),
    User(Cow<'t, str>, Mark),
    List,
}

/// The binary operators, longest spelling first.
const BINARY: [(&str, BinOp); 19] = [
    ("||", BinOp::Or),
    ("&&", BinOp::And),
    ("==", BinOp::Eq),
    ("!=", BinOp::Ne),
    ("<<", BinOp::Shl),
    (">>", BinOp::Shr),
    ("<=", BinOp::Le),
    (">=", BinOp::Ge),
    ("//", BinOp::IntDiv),
    ("/:", BinOp::DivTyped),
    ("|", BinOp::BitOr),
    ("^", BinOp::BitXor),
    ("&", BinOp::BitAnd),
    ("<", BinOp::Lt),
    (">", BinOp::Gt),
    ("+", BinOp::Add),
    ("-", BinOp::Sub),
    ("*", BinOp::Mul),
    ("/", BinOp::Div),
];

/// Reads one expression from `text[start..end]` and returns it with the position where it
/// stopped: `end`, or an unmatched `)`, `,` or `]` (not consumed). `Err` carries the byte where
/// the text stopped being an expression; rows built before the error stay, unreachable.
pub(crate) fn parse_expr(
    text: &str,
    start: usize,
    end: usize,
    b: &mut Builder<'_>,
) -> Result<(ExprId, usize), usize> {
    assert!(start <= end && end <= text.len(), "a range of the text");
    let s = text.as_bytes();
    let mut vals: Vec<ExprId> = Vec::new();
    let mut ops: Vec<Op<'_>> = Vec::new();
    let mut operand = true;
    let mut p = start;
    loop {
        while p < end && (s[p] == b' ' || s[p] == b'\t') {
            p += 1;
        }
        if p >= end {
            break;
        }
        let c = s[p];
        if operand {
            match c {
                b'(' => {
                    ops.push(Op::Paren(vals.len()));
                    p += 1;
                }
                b'[' => {
                    ops.push(Op::Args(Call::List, vals.len()));
                    p += 1;
                }
                b')' | b']' => {
                    // an empty argument list: `@f()`, `[]`
                    let fresh =
                        matches!(ops.last(), Some(Op::Args(_, base)) if *base == vals.len());
                    if !fresh {
                        return Err(p);
                    }
                    close(&mut vals, &mut ops, b, c).ok_or(p)?;
                    operand = false;
                    p += 1;
                }
                b'-' if p + 1 < end && s[p + 1].is_ascii_digit() => {
                    let q = number_end(s, p + 1, end);
                    vals.push(push_number(b, &text[p..q]).ok_or(p)?);
                    operand = false;
                    p = q;
                }
                b'-' | b'!' | b'~' => {
                    ops.push(Op::Un(match c {
                        b'-' => UnOp::Neg,
                        b'!' => UnOp::Not,
                        _ => UnOp::BitNot,
                    }));
                    p += 1;
                }
                b'+' => p += 1, // unary plus is no node
                b'0'..=b'9' => {
                    let q = number_end(s, p, end);
                    vals.push(push_number(b, &text[p..q]).ok_or(p)?);
                    operand = false;
                    p = q;
                }
                b'"' => {
                    let (v, q) = unquote(text, p, end).ok_or(p)?;
                    vals.push(b.string(&v));
                    operand = false;
                    p = q;
                }
                b'$' => {
                    let q = p
                        + 1
                        + s[p + 1..end]
                            .iter()
                            .take_while(|c| c.is_ascii_lowercase())
                            .count();
                    let leaf = match &text[p..q] {
                        "$time" => b.time(),
                        "$temp" => b.temp(),
                        "$freq" => b.freq(),
                        "$tstep" => b.tstep(),
                        "$tstop" => b.tstop(),
                        "$tnom" => b.tnom(),
                        _ => return Err(p),
                    };
                    vals.push(leaf);
                    operand = false;
                    p = q;
                }
                b'@' => {
                    let (name, q) = ident_or_quoted(text, p + 1, end).ok_or(p)?;
                    let (mark, q) = lex_mark(text, q, end);
                    if q >= end || s[q] != b'(' {
                        return Err(q);
                    }
                    ops.push(Op::Args(Call::User(name, mark), vals.len()));
                    p = q + 1;
                }
                b'`' => {
                    let (name, q) = unquote(text, p, end).ok_or(p)?;
                    let (mark, q) = lex_mark(text, q, end);
                    let e = b.param_ref(&name);
                    apply_mark(b, e, mark);
                    vals.push(e);
                    operand = false;
                    p = q;
                }
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    let q = p + s[p..end].iter().take_while(|c| name_byte(**c)).count();
                    let word = &text[p..q];
                    if q < end && s[q] == b'(' {
                        p = call_open(text, word, q, end, b, &mut vals, &mut ops)?;
                        operand = !matches!(word, "v" | "i");
                        continue;
                    }
                    if word == "draw" && q < end && s[q] == b':' {
                        let (call, q) = draw_spec(text, q, end).ok_or(p)?;
                        ops.push(Op::Args(call, vals.len()));
                        p = q;
                        continue;
                    }
                    let (mark, q) = lex_mark(text, q, end);
                    let e = b.param_ref(word);
                    apply_mark(b, e, mark);
                    vals.push(e);
                    operand = false;
                    p = q;
                }
                _ => return Err(p),
            }
            continue;
        }
        match c {
            b')' | b']' | b',' => {
                let open = ops.iter().any(|o| matches!(o, Op::Paren(_) | Op::Args(..)));
                if !open {
                    break; // the expression ends before an unmatched closer
                }
                reduce_to_group(&mut vals, &mut ops, b).ok_or(p)?;
                if c == b',' {
                    if !matches!(ops.last(), Some(Op::Args(..))) {
                        return Err(p);
                    }
                    operand = true;
                } else {
                    close(&mut vals, &mut ops, b, c).ok_or(p)?;
                }
                p += 1;
            }
            b'?' => {
                while matches!(ops.last(), Some(Op::Un(_) | Op::Bin(_))) {
                    reduce(&mut vals, &mut ops, b).ok_or(p)?;
                }
                ops.push(Op::Quest);
                operand = true;
                p += 1;
            }
            b':' => {
                while matches!(ops.last(), Some(Op::Un(_) | Op::Bin(_) | Op::Colon)) {
                    reduce(&mut vals, &mut ops, b).ok_or(p)?;
                }
                if !matches!(ops.last(), Some(Op::Quest)) {
                    return Err(p);
                }
                *ops.last_mut().expect("a `?` on the stack") = Op::Colon;
                operand = true;
                p += 1;
            }
            _ => {
                let Some(&(spelled, op)) = BINARY.iter().find(|(t, _)| text[p..end].starts_with(t))
                else {
                    return Err(p);
                };
                let power = binary_prec(op);
                while let Some(top) = ops.last() {
                    let higher = match top {
                        Op::Un(_) => true,
                        Op::Bin(q) => binary_prec(*q) >= power,
                        Op::Quest | Op::Colon | Op::Paren(_) | Op::Args(..) => false,
                    };
                    if !higher {
                        break;
                    }
                    reduce(&mut vals, &mut ops, b).ok_or(p)?;
                }
                ops.push(Op::Bin(op));
                operand = true;
                p += spelled.len();
            }
        }
    }
    if operand {
        return Err(p);
    }
    while let Some(top) = ops.last() {
        if matches!(top, Op::Quest | Op::Paren(_) | Op::Args(..)) {
            return Err(p); // an unclosed `(`, `[`, call or `?`
        }
        reduce(&mut vals, &mut ops, b).ok_or(p)?;
    }
    match vals.as_slice() {
        [e] => Ok((*e, p)),
        _ => Err(p),
    }
}

// ---- private ----

/// Pushes a number word as its literal, short or `Long` (A18); `None` when it is no number.
fn push_number(b: &mut Builder<'_>, word: &str) -> Option<ExprId> {
    Some(match lex_number(word)? {
        Lexed::Short(l) => b.literal(l),
        Lexed::Long(significand, exp10, form) => b.long_literal(significand, exp10, form),
    })
}

/// Opens `word(`: a probe (read whole), a built-in function's argument list, or an error.
/// Returns the position after what it consumed.
fn call_open<'t>(
    text: &'t str,
    word: &str,
    paren: usize,
    end: usize,
    b: &mut Builder<'_>,
    vals: &mut Vec<ExprId>,
    ops: &mut Vec<Op<'t>>,
) -> Result<usize, usize> {
    if word == "v" || word == "i" {
        let blank = |p: usize| {
            p + text.as_bytes()[p..end]
                .iter()
                .take_while(|c| **c == b' ')
                .count()
        };
        let (first, mut p) = lex_name(text, blank(paren + 1), end).ok_or(paren)?;
        let mut second = None;
        p = blank(p);
        if word == "v" && p < end && text.as_bytes()[p] == b',' {
            let (n, q) = lex_name(text, blank(p + 1), end).ok_or(p)?;
            second = Some(n);
            p = blank(q);
        }
        if p >= end || text.as_bytes()[p] != b')' {
            return Err(p);
        }
        vals.push(if word == "v" {
            b.v(&first, second.as_deref())
        } else {
            b.i(&first)
        });
        return Ok(p + 1);
    }
    let func = Func::ALL
        .into_iter()
        .find(|f| func_text(*f) == word)
        .ok_or(paren)?;
    ops.push(Op::Args(Call::Func(func), vals.len()));
    Ok(paren + 1)
}

/// `draw:<dist>:<when>:<per>(` after the word `draw`; returns the call and the position after
/// the `(`.
fn draw_spec(text: &str, colon: usize, end: usize) -> Option<(Call<'static>, usize)> {
    let open = colon + text[colon..end].find('(')?;
    let mut parts = text[colon + 1..open].split(':');
    let dist = match parts.next()? {
        "abs_gauss" => Dist::AbsGauss,
        "rel_gauss" => Dist::RelGauss,
        "abs_unif" => Dist::AbsUnif,
        "rel_unif" => Dist::RelUnif,
        "limit" => Dist::Limit,
        _ => return None,
    };
    let when = match parts.next()? {
        "always" => DrawWhen::Always,
        "sampling" => DrawWhen::Sampling,
        _ => return None,
    };
    let per = match parts.next()? {
        "reference" => DrawPer::Reference,
        "call_site" => DrawPer::CallSite,
        "instance" => DrawPer::Instance,
        "run" => DrawPer::Run,
        _ => return None,
    };
    parts
        .next()
        .is_none()
        .then_some((Call::Draw(dist, when, per), open + 1))
}

/// Reduces operators down to the innermost open group (`(` or an argument list).
fn reduce_to_group(
    vals: &mut Vec<ExprId>,
    ops: &mut Vec<Op<'_>>,
    b: &mut Builder<'_>,
) -> Option<()> {
    while let Some(top) = ops.last() {
        match top {
            Op::Paren(_) | Op::Args(..) => return Some(()),
            Op::Quest => return None,
            Op::Un(_) | Op::Bin(_) | Op::Colon => reduce(vals, ops, b)?,
        }
    }
    Some(())
}

/// Closes the innermost group with `closer`: a `(` with `)`, an argument list with its closer.
fn close(
    vals: &mut Vec<ExprId>,
    ops: &mut Vec<Op<'_>>,
    b: &mut Builder<'_>,
    closer: u8,
) -> Option<()> {
    match ops.pop()? {
        Op::Paren(base) if closer == b')' && vals.len() == base + 1 => Some(()),
        Op::Args(call, base) => {
            let list = matches!(call, Call::List);
            if list != (closer == b']') || vals.len() < base {
                return None;
            }
            let args = vals.split_off(base);
            let n = args.len();
            let e = match call {
                Call::Func(f) => {
                    let (min, max) = f.arity();
                    (n >= min as usize && (max == u8::MAX || n <= max as usize)).then_some(())?;
                    b.call(f, &args)
                }
                Call::Draw(d, w, p) => {
                    let (min, max) = d.arity();
                    (n >= min as usize && n <= max as usize).then_some(())?;
                    b.draw(d, w, p, &args)
                }
                Call::User(name, mark) => {
                    let e = b.user_call(&name, &args);
                    apply_mark(b, e, mark);
                    e
                }
                Call::List => b.list(&args),
            };
            vals.push(e);
            Some(())
        }
        Op::Paren(_) | Op::Un(_) | Op::Bin(_) | Op::Quest | Op::Colon => None,
    }
}

/// Applies the operator on top of the stack to its operands.
fn reduce(vals: &mut Vec<ExprId>, ops: &mut Vec<Op<'_>>, b: &mut Builder<'_>) -> Option<()> {
    let e = match ops.pop()? {
        Op::Un(op) => {
            let a = vals.pop()?;
            b.unary(op, a)
        }
        Op::Bin(op) => {
            let r = vals.pop()?;
            let l = vals.pop()?;
            b.binary(op, l, r)
        }
        Op::Colon => {
            let other = vals.pop()?;
            let then = vals.pop()?;
            let cond = vals.pop()?;
            b.ternary(cond, then, other)
        }
        Op::Quest | Op::Paren(_) | Op::Args(..) => return None,
    };
    vals.push(e);
    Some(())
}
