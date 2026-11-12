//! A run card's words and values as ngspice's `INPgetTok`/`INPevaluate` read them: the words of a
//! card's text, a card value (a number as `INPevaluate` reads it, or a `{…}`/`'…'` numparam
//! expression), an output (`v(n)`, `v(n,m)`, `i(src)`) and a node path. Text in, plain values
//! out; nothing is pushed here, so a caller validates a whole card before its first push.
//!
//! Cites: ngspice-45 `inpgtok.c` (blanks, `,`, `=` and parentheses end a token), `inpeval.c`
//! (`INPevaluate` with gobble: the rest of a number's token is ignored); runs: a bare parameter
//! name in a dot card is an error (`.tran 1m ts`: circuit not parsed), `{ts}` is read.
//!
//! Contents: `pub(super) enum Val`, `pub(super) enum Out`, `pub(super) fn split`,
//! `pub(super) fn value`, `pub(super) fn emit`, `pub(super) fn output`, `pub(super) fn path`,
//! `pub(super) fn probe`, `pub(super) fn invalid`

use super::super::codes;
use super::super::declare::Class;
use super::super::expr_read::{self, Fail, Tree};
use super::super::number::{self, Grammar};
use super::super::read::{Lower, Stmt};
use super::super::tables::Eval;
use crate::ir;

/// A card value read but not yet appended.
#[derive(Clone, Debug)]
pub(super) enum Val {
    /// A number as written, and whether its digits were rounded (A3).
    Lit(ir::Literal, bool),
    /// A numparam expression (`{…}`).
    Expr(Tree),
}

/// An output a run card names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Out {
    /// `v(n)`, or `v(n,m)` with the reference node.
    V(Vec<String>, Option<Vec<String>>),
    /// `i(src)`.
    I(Vec<String>),
}

/// The words of `text`: blanks and `,` separate them, `=` is a word of its own, and a word
/// runs on through a `(…)`, `{…}` or `'…'` group it opens (`v(a, b)` is one word).
pub(super) fn split(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut depth = 0usize;
    let mut quote = false;
    for c in text.chars() {
        if quote {
            word.push(c);
            quote = c != '\'';
            continue;
        }
        match c {
            '(' | '{' => {
                depth += 1;
                word.push(c);
            }
            ')' | '}' => {
                depth = depth.saturating_sub(1);
                word.push(c);
            }
            '\'' => {
                quote = true;
                word.push(c);
            }
            _ if depth > 0 => word.push(c),
            ' ' | '\t' | ',' => flush(&mut word, &mut out),
            '=' => {
                flush(&mut word, &mut out);
                out.push("=".to_string());
            }
            _ => word.push(c),
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A card value: a number (`INPevaluate`: sign, digits, exponent, scale, the rest of the
/// token ignored) or a `{…}`/`'…'` numparam expression that reads no temperature or circuit.
/// Anything else (a bare name) is ngspice's error.
pub(super) fn value(lw: &Lower<'_, '_>, st: Stmt, word: &str) -> Result<Val, Fail> {
    if let Some(inner) = wrapped(word) {
        if lw.class(st.container, inner, &[]) >= Class::Temp {
            return Err(invalid());
        }
        return expr_read::parse_in(inner, Eval::Numparam).map(Val::Expr);
    }
    let (negative, digits) = match word.as_bytes().first() {
        Some(b'-') => (true, &word[1..]),
        Some(b'+') => (false, &word[1..]),
        _ => (false, word),
    };
    let s = number::scan(digits, Grammar::Inp).ok_or_else(invalid)?;
    let mut lit = s.literal;
    if negative && lit.mantissa != 0 {
        lit.mantissa = -lit.mantissa;
    }
    Ok(Val::Lit(lit, s.rounded))
}

/// Appends `v` for an item of `st` and returns its root.
pub(super) fn emit(lw: &mut Lower<'_, '_>, st: Stmt, v: &Val) -> ir::ExprId {
    match v {
        Val::Lit(lit, rounded) => {
            if *rounded {
                lw.note(st, codes::ROUNDED);
            }
            lw.b.literal(*lit)
        }
        Val::Expr(tree) => lw.expr(st, tree, st.scope),
    }
}

/// `v(n)`, `v(n,m)` or `i(src)` (either case), or `None`.
pub(super) fn output(lw: &mut Lower<'_, '_>, word: &str) -> Option<Out> {
    let lower = word.to_ascii_lowercase();
    let (head, inner) = lower.split_once('(')?;
    let inner = inner.strip_suffix(')')?;
    let parts: Vec<&str> = inner
        .split([',', ' ', '\t'])
        .filter(|p| !p.is_empty())
        .collect();
    match (head.trim(), parts.as_slice()) {
        ("v", [n]) => Some(Out::V(path(lw, n), None)),
        ("v", [n, m]) => Some(Out::V(path(lw, n), Some(path(lw, m)))),
        ("i", [e]) => Some(Out::I(element(lw, e))),
        _ => None,
    }
}

/// A node path as ngspice names a flattened node (`x1.n`: the instances, then the net; `gnd`
/// is `0`).
pub(super) fn path(lw: &mut Lower<'_, '_>, node: &str) -> Vec<String> {
    let mut parts: Vec<&str> = node.split('.').collect();
    let net = parts.pop().unwrap_or("");
    let mut out: Vec<String> = parts.iter().map(|p| lw.names.fold(p)).collect();
    out.push(lw.names.node(net));
    out
}

/// The probe of `out`.
pub(super) fn probe(lw: &mut Lower<'_, '_>, out: &Out) -> ir::Probe {
    fn refs(p: &[String]) -> Vec<&str> {
        p.iter().map(String::as_str).collect()
    }
    match out {
        Out::V(n, None) => lw.b.probe_v(&refs(n)),
        Out::V(n, Some(m)) => lw.b.probe_vdiff(&refs(n), &refs(m)),
        Out::I(e) => lw.b.probe_i(&refs(e)),
    }
}

/// `P1300`, `InvalidInSource`: a run card ngspice rejects.
pub(super) fn invalid() -> Fail {
    Fail {
        code: super::RUN_INVALID,
        class: ir::OpaqueClass::InvalidInSource,
    }
}

// ---- private ----

/// An element path (`x1.r1`: the instances, then the element), each part folded.
fn element(lw: &mut Lower<'_, '_>, name: &str) -> Vec<String> {
    name.split('.').map(|p| lw.names.fold(p)).collect()
}

/// The text inside one `{…}` or `'…'` that wraps all of `word`.
fn wrapped(word: &str) -> Option<&str> {
    let inner = word
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .or_else(|| word.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))?;
    (!inner.trim().is_empty()).then_some(inner)
}

/// Moves a finished word into `out`.
fn flush(word: &mut String, out: &mut Vec<String>) {
    if !word.is_empty() {
        out.push(std::mem::take(word));
    }
}
