//! A run card's words and values as ngspice's `INPgetTok`/`INPevaluate` read them: the words of a
//! card's text, a card value (a number as `INPevaluate` reads it, or a `{…}`/`'…'` numparam
//! expression), a state card's node (`v(x1.n)`) and its path. Outputs are the outputs area's
//! (`read_outputs::probe`). Text in, plain values
//! out; nothing is pushed here, so a caller validates a whole card before its first push.
//!
//! Cites: ngspice-45 `inpgtok.c` (blanks, `,`, `=` and parentheses end a token), `inpeval.c`
//! (`INPevaluate` with gobble: the rest of a number's token is ignored); runs: a bare parameter
//! name in a dot card is an error (`.tran 1m ts`: circuit not parsed), `{ts}` is read.
//!
//! Contents: `pub(in super::super) fn split`, `pub(super) fn value` (a [`Val`]),
//! `pub(super) fn node`, `pub(super) fn invalid`

use super::super::declare::Class;
use super::super::expr_read::{self, Fail};
use super::super::number;
use super::super::read::{Lower, Stmt};
use super::super::tables::Eval;
use super::super::value::{Val, Wrap, unwrap};
use crate::ir;

/// The words of `text`: blanks and `,` separate them, `=` is a word of its own, and a word
/// runs on through a `(…)`, `{…}` or `'…'` group it opens (`v(a, b)` is one word).
/// The control area calls it on a script command's arguments (`1n 1u uic` of `tran 1n 1u uic`).
pub(in super::super) fn split(text: &str) -> Vec<String> {
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
    match unwrap(word) {
        // "Bad syntax!" (settle wrap_analysis_1/2.sp), "Syntax error: letter [{]" (3/4)
        Wrap::Parts | Wrap::Whole(_, true) => return Err(invalid()),
        Wrap::Whole(inner, false) if !inner.trim().is_empty() => {
            if lw.class(st.container, inner, &[]) >= Class::Temp {
                return Err(invalid());
            }
            return expr_read::parse_in(inner, Eval::Numparam).map(Val::Tree);
        }
        Wrap::Whole(..) | Wrap::Bare(_) => {}
    }
    let s = number::card(word).ok_or_else(invalid)?;
    Ok(Val::Lit(s.literal, s.fit))
}

/// The node of `v(n)` (either case), as a path, or `None` (`.ic`, `.nodeset`: `inppas3.c`).
pub(super) fn node(lw: &mut Lower<'_, '_>, word: &str) -> Option<Vec<String>> {
    let (head, inner) = word.split_once('(')?;
    let inner = inner.strip_suffix(')')?.trim();
    let ok =
        head.trim().eq_ignore_ascii_case("v") && !inner.is_empty() && !inner.contains([',', ' ']);
    ok.then(|| path(lw, inner))
}

/// `P1300`, `InvalidInSource`: a run card ngspice rejects.
pub(super) fn invalid() -> Fail {
    Fail {
        code: super::RUN_INVALID,
        class: ir::OpaqueClass::InvalidInSource,
    }
}

// ---- private ----

/// A node path as ngspice names a flattened node (`x1.n`: the instances, then the net; `gnd`
/// is `0`).
fn path(lw: &mut Lower<'_, '_>, node: &str) -> Vec<String> {
    let mut parts: Vec<&str> = node.split('.').collect();
    let net = parts.pop().unwrap_or("");
    let mut out: Vec<String> = parts.iter().map(|p| lw.names.fold(p)).collect();
    out.push(lw.names.node(net));
    out
}

/// Moves a finished word into `out`.
fn flush(word: &mut String, out: &mut Vec<String>) {
    if !word.is_empty() {
        out.push(std::mem::take(word));
    }
}
