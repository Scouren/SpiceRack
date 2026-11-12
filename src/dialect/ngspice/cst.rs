//! ngspice's CST (the `syntax` front end's `Ast` and `Tokens`, read as a private concrete syntax
//! tree) → what the reader needs of it: a statement's body, chained branch and closer (guarded:
//! only a block statement's `rhs` is a body), its head line and full extent in tokens, a field's
//! tokens and text, a path without its quotes. Pure reads; no IR.
//!
//! Cites: `src/syntax/ast.rs` (the row layouts each `Tag` states), `src/syntax/token.rs`.
//!
//! Contents: `pub(super) fn body`, `next`, `closer`, `line`, `extent`, `joined`, `unquote`

use crate::syntax::ast::{Ast, NONE, Node, Tag, WordKind};
use crate::syntax::token::{Tag as T, Tokens};

/// A statement's body; empty for a statement with none (its `rhs` is no body).
pub(super) fn body<'x>(ast: &'x Ast<'_>, n: Node) -> &'x [u32] {
    if ast.tag(n).word_kinds()[1] == WordKind::Body {
        ast.body(n)
    } else {
        &[]
    }
}

/// A chain's next branch (`.elseif`, `.else`), if `n` has a body and one.
pub(super) fn next(ast: &Ast<'_>, n: Node) -> Option<Node> {
    (ast.tag(n).word_kinds()[1] == WordKind::Body)
        .then(|| ast.next(n))
        .flatten()
}

/// The statement that closed block `n`, if it has a body and the file did not end first.
pub(super) fn closer(ast: &Ast<'_>, n: Node) -> Option<Node> {
    (ast.tag(n).word_kinds()[1] == WordKind::Body)
        .then(|| ast.closer(n))
        .flatten()
}

/// The tokens of a statement's head line: its first token to the token before its
/// `EndOfStatement`.
pub(super) fn line(toks: &Tokens<'_>, first: u32) -> (u32, u32) {
    let tags = toks.tags();
    let eos = (first as usize..tags.len())
        .find(|&k| tags[k] == T::EndOfStatement)
        .expect("every statement ends with an EndOfStatement");
    (first, (eos as u32).saturating_sub(1).max(first))
}

/// A statement's first and last token: its head line, or through its closer (a block), or
/// through its last body statement (a block the file ends first).
pub(super) fn extent(ast: &Ast<'_>, toks: &Tokens<'_>, n: Node) -> (u32, u32) {
    let first = ast.main_token(n);
    let mut last = line(toks, first).1;
    let mut at = n;
    // Bounded by the nesting depth: each step moves to a later, inner statement.
    for _ in 0..ast.len() {
        if let Some(c) = closer(ast, at) {
            last = last.max(line(toks, ast.main_token(c)).1);
            break;
        }
        let tail = chain_last(ast, at);
        match body(ast, tail).last() {
            Some(&s) => {
                at = Node(s);
                last = last.max(line(toks, ast.main_token(at)).1);
            }
            None => {
                last = last.max(line(toks, ast.main_token(tail)).1);
                break;
            }
        }
    }
    (first, last)
}

/// A field node's tokens joined, one blank per gap.
pub(super) fn joined(toks: &Tokens<'_>, ast: &Ast<'_>, n: Node) -> String {
    let Some((a, b)) = node_tokens(ast, n) else {
        return String::new();
    };
    let mut out = String::new();
    for k in a..=b {
        if k > a && toks.start(k) > toks.end(k - 1) {
            out.push(' ');
        }
        out.push_str(toks.text(k));
    }
    out
}

/// A path chunk without its quotes.
pub(super) fn unquote(text: &str) -> String {
    let t = text.trim();
    for q in ['"', '\''] {
        if let Some(inner) = t.strip_prefix(q).and_then(|r| r.strip_suffix(q)) {
            return inner.to_string();
        }
    }
    t.to_string()
}

// ---- private ----

/// The last branch of an `.if` chain (the node itself for any other statement).
fn chain_last(ast: &Ast<'_>, n: Node) -> Node {
    let mut at = n;
    // Bounded: a chain's branches are distinct nodes.
    for _ in 0..ast.len() {
        match next(ast, at) {
            Some(n) => at = n,
            None => break,
        }
    }
    at
}

/// The first and last token of a field node's subtree (every token its rows name).
fn node_tokens(ast: &Ast<'_>, n: Node) -> Option<(u32, u32)> {
    let (mut lo, mut hi) = (u32::MAX, 0u32);
    let mut note = |t: u32| {
        if t != NONE {
            lo = lo.min(t);
            hi = hi.max(t);
        }
    };
    let mut stack = vec![n];
    // Bounded: a field's subtree is a tree of earlier rows.
    while let Some(x) = stack.pop() {
        let tag = ast.tag(x);
        if tag != Tag::Root {
            note(ast.main_token(x));
        }
        let [kl, kr] = tag.word_kinds();
        for (kind, w) in [(kl, ast.lhs(x)), (kr, ast.rhs(x))] {
            match kind {
                WordKind::Unused | WordKind::Value | WordKind::Name | WordKind::Body => {}
                WordKind::Node => stack.push(Node(w)),
                WordKind::NodeOpt if w != NONE => stack.push(Node(w)),
                WordKind::NodeOpt => {}
                WordKind::Token | WordKind::TokenOpt => note(w),
                WordKind::List => {
                    stack.extend(ast.list(w).iter().filter(|&&c| c != NONE).map(|&c| Node(c)))
                }
                WordKind::Header => {
                    for at in [w, w + 1] {
                        if ast.extra(at) != NONE {
                            stack.push(Node(ast.extra(at)));
                        }
                    }
                    stack.extend(ast.list(w + 3).iter().map(|&c| Node(c)));
                    stack.extend(ast.fields(x).iter().map(|&c| Node(c)));
                }
                WordKind::Group => {
                    note(ast.extra(w));
                    stack.extend(ast.list(w + 1).iter().map(|&c| Node(c)));
                }
                WordKind::Pair => stack.extend([Node(ast.extra(w)), Node(ast.extra(w + 1))]),
                WordKind::IndexTail => {
                    stack.push(Node(ast.extra(w)));
                    note(ast.extra(w + 1));
                }
            }
        }
    }
    (lo <= hi).then_some((lo, hi))
}
