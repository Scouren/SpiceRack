//! A card value's text → the text the expression reader reads: the one reader of HSPICE's
//! expression quotes every reader area calls. `'…'` and `"…"` both delimit an expression
//! (`expr_quotes.sp`; `.param t2="a^3"` is 8, `param_string.sp`); braces are no expression
//! anywhere (`expr_braces.sp`: ESPice refuses `{2*1k}`). A quote or brace inside the value's
//! own quotes is not settled and refused.
//!
//! Cites: settle decks above (ESPice 0.3.0 in HSPICE mode: ESPice is not HSPICE);
//! `rewrite/DIALECT-HSPICE.md` §3 (branch `differences`).
//!
//! The value read, before anything is appended, is one [`Val`] in every area, appended by one
//! [`emit`] (a refused statement leaves no row: every area validates, then pushes).
//!
//! Contents: `pub(super) enum Val` (`Val::of(Scan) -> Val`), `pub(super) enum Wrap`
//! (`Wrap::plain(self) -> Option<&str>`), `pub(super) fn emit(&mut Lower, Stmt, &Val) -> ExprId`,
//! `pub(super) fn unwrap(&str) -> Wrap`

use super::expr_read::Tree;
use super::number::{self, Scan};
use super::read::{Lower, Stmt};
use crate::ir;

/// A card value read but not appended.
#[derive(Clone, Debug)]
pub(super) enum Val {
    /// A number as written and how its digits fit (`number::Fit`, A18).
    Lit(ir::Literal, number::Fit),
    /// An expression, read by the evaluator its area picked.
    Tree(Tree),
    /// A string, unquoted.
    Str(String),
}

/// A card value's text by its quotes ([`unwrap`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wrap<'t> {
    /// No wrapper: the text, trimmed.
    Bare(&'t str),
    /// Quotes around the whole text: its inside, and whether a quote or brace stands inside it
    /// (not settled: refused).
    Whole(&'t str, bool),
    /// A brace anywhere, or quotes around parts of the text (`'a'+'b'`): no HSPICE value.
    Parts,
}

impl<'t> Wrap<'t> {
    /// The text an evaluator reads when nothing stands nested: a bare text, or the inside of a
    /// whole wrapper with no wrapper in it.
    pub(super) fn plain(self) -> Option<&'t str> {
        match self {
            Wrap::Bare(t) | Wrap::Whole(t, false) => Some(t),
            Wrap::Whole(_, true) | Wrap::Parts => None,
        }
    }
}

impl Val {
    /// A scanned card number.
    pub(super) fn of(s: Scan) -> Val {
        Val::Lit(s.literal, s.fit)
    }
}

/// Appends `v` in `st`'s scope and returns its root.
pub(super) fn emit(lw: &mut Lower<'_, '_>, st: Stmt, v: &Val) -> ir::ExprId {
    match v {
        Val::Lit(lit, fit) => lw.number(*lit, fit),
        Val::Tree(t) => lw.expr(st, t, st.scope),
        Val::Str(s) => lw.b.string(s),
    }
}

/// `text` by its quotes: the one reader of `'…'` and `"…"` every area calls.
pub(super) fn unwrap(text: &str) -> Wrap<'_> {
    let t = text.trim();
    let wrapper = |c: char| matches!(c, '{' | '}' | '\'' | '"');
    for q in ['\'', '"'] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) && !t[1..t.len() - 1].contains(q) {
            let inner = &t[1..t.len() - 1];
            return Wrap::Whole(inner, inner.contains(wrapper));
        }
    }
    if t.contains(wrapper) {
        Wrap::Parts
    } else {
        Wrap::Bare(t)
    }
}
