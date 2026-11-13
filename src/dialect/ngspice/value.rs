//! A card value's text → the text its evaluator reads: the one reader of `{…}` and `'…'` every
//! reader area calls (audit 2026-11-12 row 4: six copies disagreed, three turned `{a}+{b}`
//! into `a}+{b`). Each area keeps its own class check of what a wrapper holds: a wrapper inside
//! the value's is a parenthesis in an element value or a source field and an error in a
//! `.param`, a model parameter or an analysis field; wrappers around parts of a value are no
//! value anywhere (a model card runs on the first one alone).
//!
//! Cites: ngspice-45 runs, settle `wrap_{element,source,model,analysis,param}_{1,2,3,4}.sp`.
//!
//! The value read, before anything is appended, is one [`Val`] in every area, appended by one
//! [`emit`] (a refused statement leaves no row: every area validates, then pushes).
//!
//! Contents: `pub(super) enum Val` (`Val::of(Scan) -> Val`), `pub(super) enum Wrap`
//! (`Wrap::plain(self) -> Option<&str>`), `pub(super) fn emit(&mut Lower, Stmt, &Val) -> ExprId`,
//! `pub(super) fn unwrap(&str) -> Wrap`, `pub(super) fn parens(&str) -> String`,
//! `pub(super) fn substituted(Tree) -> Val`

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

/// A card value's text by its `{…}` / `'…'` wrapper ([`unwrap`]; settle `wrap_*.sp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wrap<'t> {
    /// No wrapper: the text, trimmed.
    Bare(&'t str),
    /// One wrapper around the whole text: its inside, and whether a wrapper stands inside it
    /// (`'a+{b}'`, `{a+'b'}`: in an element value or a source field ngspice reads it as a
    /// parenthesis, [`parens`]; in a `.param`, a model parameter or an analysis field it
    /// stops on it).
    Whole(&'t str, bool),
    /// Wrappers around parts of the text (`{a}+{b}`, `'a'+'b'`): numparam replaces each on its
    /// own, so no field reads it as one value.
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

/// `text` by its wrapper: the one reader of `{…}` and `'…'` every area calls (each keeps its
/// own class check of what the wrapper holds).
pub(super) fn unwrap(text: &str) -> Wrap<'_> {
    let t = text.trim();
    let wrapper = |c: char| matches!(c, '{' | '}' | '\'');
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') && !t[1..t.len() - 1].contains('\'')
    {
        let inner = &t[1..t.len() - 1];
        return Wrap::Whole(inner, inner.contains(wrapper));
    }
    if t.len() >= 2 && t.starts_with('{') && t.ends_with('}') {
        let inner = &t[1..t.len() - 1];
        let mut depth = 0i32;
        let wraps = inner.bytes().all(|c| {
            depth += i32::from(c == b'{') - i32::from(c == b'}');
            depth >= 0
        });
        if wraps && depth == 0 {
            return Wrap::Whole(inner, inner.contains(wrapper));
        }
    }
    if t.contains(wrapper) {
        Wrap::Parts
    } else {
        Wrap::Bare(t)
    }
}

/// A wrapper's inside with each inner `'…'` as a parenthesis, as ngspice reads one in an element
/// value or a source field (settle `wrap_element_4.sp`: `{a+'b'}` is 3); an inner `{…}` the
/// expression reader takes as a parenthesis already.
pub(super) fn parens(inner: &str) -> String {
    let mut open = false;
    inner
        .chars()
        .map(|c| match c {
            '\'' => {
                open = !open;
                if open { '(' } else { ')' }
            }
            c => c,
        })
        .collect()
}

/// A card's `{…}` / `'…'` value numparam evaluated: ngspice substitutes its result back into the
/// card as `% 23.15e` text (`numparam/xpressn.c:1176`), which the card's `INPevaluate` reads as
/// its 16 digits times a power of ten (`inpeval.c`: `'3'` runs as 3.0000000000000004, `'0.2'`
/// as 0.19999999999999998, `'1.0453385141288605'` as 1.04533851412886; bare they read 3, 0.2,
/// …8605; settle `numparam_round.sp`, spice-bench `xpressn-1.sp`). So a lone number is that
/// text, every digit kept (the writer spells the reader's digits, A18). An expression stays a
/// tree: the writer keeps it in braces, where numparam does the same again.
pub(super) fn substituted(tree: Tree) -> Val {
    let Some(lit) = tree.lone_literal() else {
        return Val::Tree(tree);
    };
    let x = lit.value();
    number::card(&format!("{x:.15e}")).map_or(Val::Tree(tree), Val::of)
}
