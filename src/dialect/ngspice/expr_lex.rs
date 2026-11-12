//! ngspice expression text → tokens, as the evaluator that reads it lexes it: which evaluator
//! that is (the declare step's class, `declare.rs`: numparam, unless the text holds `v(`, `i(`, `time`, `temper` or
//! `hertz`), numbers by that evaluator's grammar (`number.rs`), names by numparam's `fetchid`
//! (`alfanum` and `.`), probes by `PTlex` (`v(a)`, `v(a,b)`, `i(vname)`, raw arguments), and the
//! operators each one has (numparam: `%`, `\\`, a single `=`, inner `{…}` as parentheses; B
//! source: none of those). `expr_read.rs` parses the tokens.
//!
//! Cites: ngspice-45 `frontend/numparam/xpressn.c` (`fetchid`, `fetchoperator`,
//! `fetchnumber`), `spicelib/parser/inpptree.c` (`PTlex`); `rewrite/INV-semantics.md` 2.06,
//! 2.24.
//!
//! Contents: `pub(super) enum Tok`, `pub(super) enum Op`, `pub(super) const PI: ir::Literal`,
//! `pub(super) const E: ir::Literal`, `pub(super) struct Leaves`, `pub(super) fn leaves(&str) -> Leaves`,
//! `pub(super) fn lex(&str, Eval) -> Result<Vec<Tok>, Fail>`

use super::expr_read::Fail;
use super::number;
use super::tables::Eval;
use crate::ir;

/// A token of an expression.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Tok {
    Num(ir::Literal, bool),
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
    Percent,
    Backslash,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Bang,
    Question,
    Colon,
}

/// What a text reads by its own spelling ([`leaves`]).
#[derive(Clone, Debug, Default)]
pub(super) struct Leaves {
    /// A probe, `time`, `hertz` or `ddt(`.
    pub(super) circuit: bool,
    /// `temper`.
    pub(super) temp: bool,
    /// Every other name, and whether it is called.
    pub(super) names: Vec<(String, bool)>,
}

/// The literal `pi` and `e` fold to in a B source (`inpptree.c` `constants[]`: M_PI, M_E), to
/// 18 digits, which read back to those doubles.
pub(super) const PI: ir::Literal = literal18(314_159_265_358_979_324);
pub(super) const E: ir::Literal = literal18(271_828_182_845_904_524);

/// What `text` reads by its own spelling, outside numbers: `v(`/`i(` probes, `time`, `hertz`,
/// `ddt(` (circuit), `temper`, and every other name (`true`: called as a function).
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
                ("v" | "i" | "ddt", true) | ("time" | "hertz", false) => out.circuit = true,
                ("temper", false) => out.temp = true,
                _ => out.names.push((word.to_string(), call)),
            }
        } else {
            k += 1;
        }
    }
    out
}

/// `text` as tokens under `eval`'s lexer.
pub(super) fn lex(text: &str, eval: Eval) -> Result<Vec<Tok>, Fail> {
    let b = text.as_bytes();
    let numparam = eval == Eval::Numparam;
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
                let s = number::scan(&text[k..], eval.grammar()).ok_or_else(Fail::invalid)?;
                (Tok::Num(s.literal, s.rounded), s.len)
            }
            c if c.is_ascii_alphabetic() || c == b'_' => ident_or_probe(text, k)?,
            b'(' => (Tok::Open, 1),
            b')' => (Tok::Close, 1),
            b'{' if numparam => (Tok::Open, 1),
            b'}' if numparam => (Tok::Close, 1),
            b',' => (Tok::Comma, 1),
            b'*' if next == Some(b'*') => (Tok::Op(Op::Pow), 2),
            b'*' => (Tok::Op(Op::Star), 1),
            b'^' => (Tok::Op(Op::Pow), 1),
            b'/' => (Tok::Op(Op::Slash), 1),
            b'%' if numparam => (Tok::Op(Op::Percent), 1),
            b'\\' if numparam => (Tok::Op(Op::Backslash), 1),
            b'+' => (Tok::Op(Op::Plus), 1),
            b'-' => (Tok::Op(Op::Minus), 1),
            b'=' if next == Some(b'=') => (Tok::Op(Op::Eq), 2),
            b'=' if numparam => (Tok::Op(Op::Eq), 1),
            b'!' if next == Some(b'=') => (Tok::Op(Op::Ne), 2),
            b'!' => (Tok::Op(Op::Bang), 1),
            b'<' if next == Some(b'>') => (Tok::Op(Op::Ne), 2),
            b'<' if next == Some(b'=') => (Tok::Op(Op::Le), 2),
            b'<' => (Tok::Op(Op::Lt), 1),
            b'>' if next == Some(b'=') => (Tok::Op(Op::Ge), 2),
            b'>' => (Tok::Op(Op::Gt), 1),
            b'&' if next == Some(b'&') => (Tok::Op(Op::And), 2),
            b'|' if next == Some(b'|') => (Tok::Op(Op::Or), 2),
            b'?' => (Tok::Op(Op::Question), 1),
            b':' => (Tok::Op(Op::Colon), 1),
            // a string value, or B-source braces (dropped as text by ngspice, INV 2.24)
            b'"' | b'\'' | b'{' | b'}' => return Err(Fail::unread()),
            _ => return Err(Fail::invalid()),
        };
        toks.push(tok);
        k += len;
    }
    Ok(toks)
}

// ---- private ----

/// A byte that continues a name (numparam `fetchid`: `alfanum` and `.`).
fn ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'[' | b']')
}

/// The name at `k`, or a `v(…)`/`i(…)` probe with its raw arguments (`PTlex`: `[^ \t,()]+`).
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

/// `mantissa × 10^-17`, plain.
const fn literal18(mantissa: i64) -> ir::Literal {
    ir::Literal {
        mantissa,
        frac: 17,
        exp10: 0,
        form: ir::NumForm::Plain,
    }
}
