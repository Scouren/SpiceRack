//! A `?:` as another dialect reads it: a ternary in a then-branch
//! parenthesized, which ngspice-45 misreads bare (`x>0?y>0?4:8:16` is 4
//! at x=1, y=-1, where C's grammar, which Xyce follows, gives 8;
//! `x>0?(y>0?4:8):16` is 8 on both), and for Xyce a blank before each
//! `:`, which Xyce 7.10 otherwise reads into a name (`x<p?p:n` is "syntax
//! error in expression", `x<p?p :n` is `n`). Each run by hand.
//!
//! Cites: ngspice manual §2.8.5 (`?:`); Xyce RG §2.1.18 (`?:`), §2.3
//! (`:` in hierarchical names).
//!
//! Contents:
//! - `pub(super) fn parenthesize(e: &str, then: bool, blank: bool) -> String`

/// Ternaries nested deeper than this are left as written (an expression
/// in a deck is a line, not a tree this deep).
const DEPTH: u32 = 64;

/// What [`parenthesize`] changes.
#[derive(Clone, Copy)]
struct Mode {
    then: bool,
    blank: bool,
}

/// `e` with each ternary in a then-branch parenthesized when `then` (the
/// source reads C's grammar) and a blank before each ternary's `:` when
/// `blank` (Xyce). Text with no ternary, or an unmatched `?`, comes back
/// as written.
pub(super) fn parenthesize(e: &str, then: bool, blank: bool) -> String {
    if !e.contains('?') {
        return e.to_string();
    }
    fix(e, Mode { then, blank }, 0)
}

// ---- private ----

/// `s` with its nested ternaries parenthesized, at nesting `depth`.
fn fix(s: &str, m: Mode, depth: u32) -> String {
    if depth > DEPTH {
        return s.to_string();
    }
    let marks = top(s);
    let Some(q) = marks.iter().position(|&(_, c)| c == b'?') else {
        return groups(s, m, depth);
    };
    let mut open = 0u32;
    let mut colon = None;
    for &(i, c) in &marks[q + 1..] {
        match (c, open) {
            (b'?', _) => open += 1,
            (_, 0) => {
                colon = Some(i);
                break;
            }
            _ => open -= 1,
        }
    }
    let (qi, Some(ci)) = (marks[q].0, colon) else {
        return groups(s, m, depth);
    };
    let cond = groups(&s[..qi], m, depth);
    let (yes, no) = (
        fix(&s[qi + 1..ci], m, depth + 1),
        fix(&s[ci + 1..], m, depth + 1),
    );
    let yes = if m.then { wrap(&yes) } else { yes };
    let gap = if m.blank && !yes.ends_with(char::is_whitespace) {
        " "
    } else {
        ""
    };
    format!("{cond}?{yes}{gap}:{no}")
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
    use super::parenthesize;

    #[test]
    fn a_nested_ternary_is_parenthesized() {
        let p = |e: &str| parenthesize(e, true, false);
        assert_eq!(p("{x>0?y>0?4 :8 :8}"), "{x>0?(y>0?4 :8) :8}");
        assert_eq!(
            parenthesize("{x>0?y>0?4 :8 :8}", false, false),
            "{x>0?y>0?4 :8 :8}"
        );
        assert_eq!(p("X<P?P:X>N?N:X"), "X<P?P:X>N?N:X");
        assert_eq!(p("'a?b?c:d:e'"), "'a?(b?c:d):e'");
        assert_eq!(p("f(a?b?c:d:e, 2)"), "f(a?(b?c:d):e, 2)");
        assert_eq!(p("a?(b?c:d):e"), "a?(b?c:d):e");
        assert_eq!(p("v(a)*2"), "v(a)*2");
        assert_eq!(p("a?b"), "a?b");
        assert_eq!(p("\"C:?x\" a?b?c:d:e"), "\"C:?x\" a?(b?c:d):e");
    }

    #[test]
    fn xyce_gets_a_blank_before_each_colon() {
        let x = |e: &str| parenthesize(e, false, true);
        assert_eq!(x("X<P?P:X>N?N:X"), "X<P?P :X>N?N :X");
        assert_eq!(x("x<p ? p : n"), "x<p ? p : n");
    }
}
