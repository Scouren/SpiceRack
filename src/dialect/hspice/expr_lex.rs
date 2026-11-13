//! HSPICE expression text → tokens: numbers by `number.rs`, names (letters, digits, `_`, `.`,
//! `[`, `]`: the ngspice lead, unsettled), probes `v(a)`, `v(a,b)`, `i(vname)`, and the operators
//! HSPICE has (`tables.rs`). A brace, a `%`, a lone `!` or `=`, or a quote inside the
//! expression is no HSPICE expression (`expr_braces.sp`, `expr_percent.sp`, `expr_not.sp`:
//! ESPice refuses each). `expr_read.rs` parses the tokens.
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §3 (branch `differences`); the settle decks above (ESPice
//! 0.3.0 in HSPICE mode: ESPice is not HSPICE).
//!
//! Contents: `pub(super) enum Tok`, `pub(super) enum Op`, `pub(super) struct Leaves`,
//! `pub(super) fn leaves(&str) -> Leaves`, `pub(super) fn calls(&str) -> Vec<(String, usize)>`,
//! `pub(super) fn lex(&str) -> Result<Vec<Tok>, Fail>`

use super::expr_read::Fail;
use super::number;
use crate::ir;

/// A token of an expression.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    Num(ir::Literal, number::Fit),
    Ident(String),
    V(String, Option<String>),
    I(String),
    Open,
    Close,
    Comma,
    Op(Op),
}

/// An operator token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Op {
    Plus,
    Minus,
    Star,
    Slash,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Question,
    Colon,
}

/// What a text reads by its own spelling ([`leaves`]).
#[derive(Clone, Debug, Default)]
pub(super) struct Leaves {
    /// A probe, `time` or `hertz`.
    pub(super) circuit: bool,
    /// `temper`.
    pub(super) temp: bool,
    /// Every other name, and whether it is called.
    pub(super) names: Vec<(String, bool)>,
}

/// What `text` reads by its own spelling, outside numbers: `v(`/`i(` probes, `time`, `hertz`
/// (circuit), `temper`, and every other name (`true`: called as a function).
pub(super) fn leaves(text: &str) -> Leaves {
    let b = text.as_bytes();
    let mut out = Leaves::default();
    let mut k = 0;
    // Bounded: every step advances `k`.
    while k < b.len() {
        let c = b[k];
        if c.is_ascii_digit() || c == b'.' {
            k += 1;
            while k < b.len() && (b[k].is_ascii_alphanumeric() || b[k] == b'.' || b[k] == b'_') {
                k += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = k;
            while k < b.len() && ident_byte(b[k]) {
                k += 1;
            }
            let word = &text[start..k];
            let call = text[k..].trim_start().starts_with('(');
            let lower = word.to_ascii_lowercase();
            match (lower.as_str(), call) {
                ("v" | "i", true) | ("time" | "hertz", false) => out.circuit = true,
                ("temper", false) => out.temp = true,
                _ => out.names.push((word.to_string(), call)),
            }
        } else {
            k += 1;
        }
    }
    out
}

/// `text` as tokens.
pub(super) fn lex(text: &str) -> Result<Vec<Tok>, Fail> {
    let b = text.as_bytes();
    let mut toks = Vec::new();
    let mut k = 0;
    // Bounded: every arm advances `k`.
    while k < b.len() {
        let c = b[k];
        let next = b.get(k + 1).copied();
        let (tok, len) = match c {
            b' ' | b'\t' | b'\r' | b'\n' => {
                k += 1;
                continue;
            }
            b'0'..=b'9' | b'.' => {
                let s = number::scan(&text[k..]).ok_or_else(Fail::invalid)?;
                (Tok::Num(s.literal, s.fit), s.len)
            }
            c if c.is_ascii_alphabetic() || c == b'_' => ident_or_probe(text, k)?,
            b'(' => (Tok::Open, 1),
            b')' => (Tok::Close, 1),
            b',' => (Tok::Comma, 1),
            b'*' if next == Some(b'*') => (Tok::Op(Op::Pow), 2),
            b'*' => (Tok::Op(Op::Star), 1),
            b'^' => (Tok::Op(Op::Pow), 1),
            b'/' => (Tok::Op(Op::Slash), 1),
            b'+' => (Tok::Op(Op::Plus), 1),
            b'-' => (Tok::Op(Op::Minus), 1),
            b'=' if next == Some(b'=') => (Tok::Op(Op::Eq), 2),
            b'!' if next == Some(b'=') => (Tok::Op(Op::Ne), 2),
            b'<' if next == Some(b'=') => (Tok::Op(Op::Le), 2),
            b'<' => (Tok::Op(Op::Lt), 1),
            b'>' if next == Some(b'=') => (Tok::Op(Op::Ge), 2),
            b'>' => (Tok::Op(Op::Gt), 1),
            b'&' if next == Some(b'&') => (Tok::Op(Op::And), 2),
            b'|' if next == Some(b'|') => (Tok::Op(Op::Or), 2),
            b'?' => (Tok::Op(Op::Question), 1),
            b':' => (Tok::Op(Op::Colon), 1),
            _ => return Err(Fail::invalid()),
        };
        toks.push(tok);
        k += len;
    }
    Ok(toks)
}

/// Every `name(…)` in `text` outside quotes, with its argument count (top-level commas plus
/// one; `name()` has none), in text order: for the arity check against the `.func`s in scope.
pub(super) fn calls(text: &str) -> Vec<(String, usize)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut quote: Option<u8> = None;
    let mut k = 0;
    // Bounded: each byte is passed once by `k`; an argument scan moves `j` ahead of it.
    while k < b.len() {
        let c = b[k];
        if let Some(q) = quote {
            quote = (c != q).then_some(q);
            k += 1;
            continue;
        }
        if c == b'"' || c == b'\'' {
            quote = Some(c);
            k += 1;
            continue;
        }
        let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        if !(c.is_ascii_alphabetic() || c == b'_') || (k > 0 && ident(b[k - 1])) {
            k += 1;
            continue;
        }
        let start = k;
        while k < b.len() && ident(b[k]) {
            k += 1;
        }
        let mut j = k;
        while j < b.len() && b[j] == b' ' {
            j += 1;
        }
        if j >= b.len() || b[j] != b'(' {
            continue;
        }
        let (mut depth, mut commas, mut empty) = (0usize, 0usize, true);
        for &d in &b[j + 1..] {
            match d {
                b'(' => depth += 1,
                b')' if depth == 0 => break,
                b')' => depth -= 1,
                b',' if depth == 0 => commas += 1,
                b' ' | b'\t' => continue,
                _ => {}
            }
            empty = false;
        }
        let count = if empty && commas == 0 { 0 } else { commas + 1 };
        out.push((text[start..k].to_string(), count));
    }
    out
}

// ---- private ----

/// A byte that continues a name.
fn ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'[' | b']')
}

/// The name at `k`, or a `v(…)`/`i(…)` probe with its raw arguments.
fn ident_or_probe(text: &str, k: usize) -> Result<(Tok, usize), Fail> {
    let b = text.as_bytes();
    let mut end = k;
    while end < b.len() && ident_byte(b[end]) {
        end += 1;
    }
    let word = &text[k..end];
    let rest = &text[end..];
    let after = rest.trim_start();
    let probe = word.eq_ignore_ascii_case("v") || word.eq_ignore_ascii_case("i");
    if !(probe && after.starts_with('(')) {
        return Ok((Tok::Ident(word.to_string()), end - k));
    }
    let open = end + (rest.len() - after.len());
    let close = text[open..].find(')').ok_or_else(Fail::invalid)? + open;
    let args: Vec<&str> = text[open + 1..close].split(',').map(str::trim).collect();
    let name_ok = |a: &str| !a.is_empty() && !a.contains(|c: char| c.is_whitespace() || c == '(');
    if !args.iter().all(|a| name_ok(a)) {
        return Err(Fail::invalid());
    }
    let tok = match (word.as_bytes()[0].to_ascii_lowercase(), args.as_slice()) {
        (b'v', [n]) => Tok::V(n.to_string(), None),
        (b'v', [n, m]) => Tok::V(n.to_string(), Some(m.to_string())),
        // an element's IR name is its whole name, letter included (R23)
        (b'i', [e]) => Tok::I(e.to_string()),
        _ => return Err(Fail::invalid()),
    };
    Ok((tok, close + 1 - k))
}
