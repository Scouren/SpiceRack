//! A `?:` as another dialect reads it. C's grammar (Xyce RG §2.1.18,
//! HSPICE, ngspice's B-source parser) groups `a?b?c:d:e` as
//! `a?(b?c:d):e`; ngspice-45's `{}` evaluator (numparam,
//! `src/frontend/numparam/xpressn.c`, the `first-of-triple` stack) reads
//! it as `a?c:e` (60 random cases, run by hand: `1?0?4:8:16` is 4, not 8)
//! and a deeper one in a then-branch no one way. So the then-branch is
//! parenthesized for a C source and spelled `a?c:e` for an ngspice one
//! (W0034), a deeper ngspice one refused (W0035, `W::ternary_checks`).
//! Xyce 7.10 reads `p:n` as one hierarchical name ("syntax error in
//! expression"), so on Xyce each ternary `:` has a blank before it.
//!
//! Xyce's and LTspice's `if(c,a,b)` is `((c)?(a):(b))` where the target
//! has no `if` (ngspice-45: "no such function 'if'", run by hand).
//!
//! Cites: ngspice manual §2.8.5 (`?:`); Xyce RG §2.3 (`if`); Xyce RG §2.1.18 (`?:`), §2.3
//! (`:` in hierarchical names).
//!
//! Contents:
//! - `pub(super) enum Read`, `pub(super) enum Nest`
//! - `pub(super) fn parenthesize(e: &str, read: Read, blank: bool) -> String`
//! - `pub(super) fn ngspice_nest(e: &str) -> Nest`
//! - `pub(super) fn spell_if(e: &str) -> String`

use std::cell::Cell;

/// How the source groups a ternary in a then-branch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Read {
    /// `a?b?c:d:e` is `a?(b?c:d):e`.
    C,
    /// ngspice's `{}` evaluator: `a?b?c:d:e` is `a?c:e`.
    Ngspice,
}

/// The deepest ternary an expression holds in a then-branch.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Nest {
    Flat,
    /// `a?b?c:d:e`, `c` and `d` holding none.
    One,
    /// A ternary in the inner ternary's branches.
    Deep,
}

/// Ternaries nested deeper than this are left as written (an expression
/// in a deck is a line, not a tree this deep).
const DEPTH: u32 = 64;

/// What `fix` changes, and the deepest nest it saw.
#[derive(Clone, Copy)]
struct Mode<'a> {
    read: Read,
    blank: bool,
    seen: &'a Cell<Nest>,
}

/// `e` with each ternary in a then-branch as `read` groups it (C:
/// parenthesized; ngspice: `a?c:e`, a [`Nest::Deep`] one as written) and
/// a blank before each ternary's `:` when `blank` (Xyce). Text with no
/// ternary, or an unmatched `?`, comes back as written.
pub(super) fn parenthesize(e: &str, read: Read, blank: bool) -> String {
    if !e.contains('?') {
        return e.to_string();
    }
    let seen = Cell::new(Nest::Flat);
    fix(
        e,
        Mode {
            read,
            blank,
            seen: &seen,
        },
        0,
    )
}

/// The deepest then-branch ternary of `e` (see [`Nest`]).
pub(super) fn ngspice_nest(e: &str) -> Nest {
    let seen = Cell::new(Nest::Flat);
    if e.contains('?') {
        fix(
            e,
            Mode {
                read: Read::Ngspice,
                blank: false,
                seen: &seen,
            },
            0,
        );
    }
    seen.get()
}

/// `e` with each `if(c, a, b)` call spelled `((c)?(a):(b))`, inner calls
/// first; a call of another arity as written.
pub(super) fn spell_if(e: &str) -> String {
    if !e.to_ascii_lowercase().contains("if") {
        return e.to_string();
    }
    ifs(e, 0)
}

// ---- private ----

/// `s` with its ternaries spelled, at nesting `depth`.
fn fix(s: &str, m: Mode, depth: u32) -> String {
    if depth > DEPTH {
        return s.to_string();
    }
    let Some((qi, ci)) = split(s) else {
        return groups(s, m, depth);
    };
    let cond = groups(&s[..qi], m, depth);
    let (yes, no) = (&s[qi + 1..ci], fix(&s[ci + 1..], m, depth + 1));
    let yes = match (m.read, split(yes)) {
        (Read::C, _) => wrap(&fix(yes, m, depth + 1)),
        (Read::Ngspice, Some((q, c))) => {
            let (inner, other) = (&yes[q + 1..c], &yes[c + 1..]);
            if split(inner).is_none() && split(other).is_none() {
                m.seen.set(m.seen.get().max(Nest::One));
                fix(inner, m, depth + 1)
            } else {
                m.seen.set(Nest::Deep);
                fix(yes, m, depth + 1)
            }
        }
        (Read::Ngspice, None) => fix(yes, m, depth + 1),
    };
    let gap = if m.blank && !yes.ends_with(char::is_whitespace) {
        " "
    } else {
        ""
    };
    format!("{cond}?{yes}{gap}:{no}")
}

/// The top-level `?` of `s` and the `:` matching it.
fn split(s: &str) -> Option<(usize, usize)> {
    let marks = top(s);
    let q = marks.iter().position(|&(_, c)| c == b'?')?;
    let mut open = 0u32;
    for &(i, c) in &marks[q + 1..] {
        match (c, open) {
            (b'?', _) => open += 1,
            (_, 0) => return Some((marks[q].0, i)),
            _ => open -= 1,
        }
    }
    None
}

/// `b` in parentheses when it holds a ternary at its top level, its
/// surrounding blanks kept outside.
fn wrap(b: &str) -> String {
    if !top(b).iter().any(|&(_, c)| c == b'?') {
        return b.to_string();
    }
    let t = b.trim();
    let lead = &b[..b.len() - b.trim_start().len()];
    let trail = &b[b.trim_end().len()..];
    format!("{lead}({t}){trail}")
}

/// `s` with each delimited group's inside fixed (a `"…"` string as
/// written).
fn groups(s: &str, m: Mode, depth: u32) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + 4);
    let mut i = 0;
    while i < b.len() {
        let Some(j) = close(b, i) else {
            let next = s[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&s[i..i + next]);
            i += next;
            continue;
        };
        out.push_str(&s[i..=i]);
        if b[i] == b'"' {
            out.push_str(&s[i + 1..j]);
        } else {
            out.push_str(&fix(&s[i + 1..j], m, depth + 1));
        }
        out.push_str(&s[j..=j]);
        i = j + 1;
    }
    out
}

/// The `?` and `:` of `s` outside every delimited group.
fn top(s: &str) -> Vec<(usize, u8)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(j) = close(b, i) {
            i = j + 1;
            continue;
        }
        if matches!(b[i], b'?' | b':') {
            out.push((i, b[i]));
        }
        i += 1;
    }
    out
}

/// `s` with its `if` calls spelled, at nesting `depth`.
fn ifs(s: &str, depth: u32) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        let word = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        let call = word
            && depth < DEPTH
            && s[i..].len() > 2
            && s[i..i + 2].eq_ignore_ascii_case("if")
            && b[i + 2] == b'(';
        if let Some(j) = call.then(|| close(b, i + 2)).flatten() {
            let args = commas(&s[i + 3..j]);
            if let [c, x, y] = args.as_slice() {
                let [c, x, y] = [c, x, y].map(|a| ifs(a.trim(), depth + 1));
                out.push_str(&format!("(({c})?({x}):({y}))"));
                i = j + 1;
                continue;
            }
        }
        let next = s[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&s[i..i + next]);
        i += next;
    }
    out
}

/// `s` split at its commas outside every delimited group.
fn commas(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let (mut out, mut from, mut i) = (Vec::new(), 0, 0);
    while i < b.len() {
        if let Some(j) = close(b, i) {
            i = j + 1;
            continue;
        }
        if b[i] == b',' {
            out.push(&s[from..i]);
            from = i + 1;
        }
        i += 1;
    }
    out.push(&s[from..]);
    out
}

/// The index closing the group `b[i]` opens; `None` when `b[i]` opens
/// none or the group is unclosed. Quotes nest nothing.
fn close(b: &[u8], i: usize) -> Option<usize> {
    let shut = |c: u8| match c {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        b'\'' | b'"' => Some(c),
        _ => None,
    };
    let mut want = vec![shut(b[i])?];
    let mut j = i + 1;
    while j < b.len() {
        let top = want[want.len() - 1];
        if b[j] == top {
            want.pop();
            if want.is_empty() {
                return Some(j);
            }
        } else if !matches!(top, b'\'' | b'"')
            && let Some(s) = shut(b[j])
        {
            want.push(s);
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Nest, Read, ngspice_nest, parenthesize, spell_if};

    #[test]
    fn a_c_then_branch_ternary_is_parenthesized() {
        let p = |e: &str| parenthesize(e, Read::C, false);
        assert_eq!(p("{x>0?y>0?4 :8 :8}"), "{x>0?(y>0?4 :8) :8}");
        assert_eq!(p("X<P?P:X>N?N:X"), "X<P?P:X>N?N:X");
        assert_eq!(p("'a?b?c:d:e'"), "'a?(b?c:d):e'");
        assert_eq!(p("f(a?b?c:d:e, 2)"), "f(a?(b?c:d):e, 2)");
        assert_eq!(p("a?(b?c:d):e"), "a?(b?c:d):e");
        assert_eq!(p("v(a)*2"), "v(a)*2");
        assert_eq!(p("a?b"), "a?b");
        assert_eq!(p("\"C:?x\" a?b?c:d:e"), "\"C:?x\" a?(b?c:d):e");
    }

    #[test]
    fn an_ngspice_then_branch_ternary_is_spelled_as_ngspice_reads_it() {
        let p = |e: &str| parenthesize(e, Read::Ngspice, false);
        assert_eq!(p("{x>0?y>0?4 :8 :16}"), "{x>0?4 :16}");
        assert_eq!(p("0?42:1?41:40"), "0?42:1?41:40");
        assert_eq!(ngspice_nest("{x>0?y>0?4 :8 :16}"), Nest::One);
        assert_eq!(ngspice_nest("0?42:1?41:40"), Nest::Flat);
        assert_eq!(ngspice_nest("0?0?4:1?8:16:32"), Nest::Deep);
        assert_eq!(ngspice_nest("a?(b?c:d):e"), Nest::Flat);
    }

    #[test]
    fn an_if_call_is_a_ternary() {
        assert_eq!(spell_if("IF(V(1)>0,2*V(1),0)"), "((V(1)>0)?(2*V(1)):(0))");
        assert_eq!(
            spell_if("if(a, if(b,1,2), f(c,d))"),
            "((a)?(((b)?(1):(2))):(f(c,d)))"
        );
        assert_eq!(spell_if("diff(a,b)+if(a,b)"), "diff(a,b)+if(a,b)");
    }

    #[test]
    fn xyce_gets_a_blank_before_each_colon() {
        let x = |e: &str| parenthesize(e, Read::Ngspice, true);
        assert_eq!(x("X<P?P:X>N?N:X"), "X<P?P :X>N?N :X");
        assert_eq!(x("x<p ? p : n"), "x<p ? p : n");
    }
}
