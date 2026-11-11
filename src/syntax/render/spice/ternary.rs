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
//! `log` is the natural logarithm on ngspice, HSPICE and LTspice
//! (ngspice manual §2.8.5) and base 10 on Xyce (RG §2.3; Xyce 7.10 by
//! hand: `log(100)` is 2, `ln(100)` 4.605), so it is renamed between them
//! (`rename_call`).
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
//! - `pub(super) fn rename_call(e: &str, from: &str, to: &str) -> String`
//! - `pub(super) fn spell_nint(e: &str, to_xyce: bool) -> String`
//! - `pub(super) fn rename_word(e: &str, from: &str, to: &str) -> String`
//! - `pub(super) fn nominal(e: &str) -> String`
//! - `pub(super) fn spell_clamp(e: &str) -> String`
//! - `pub(super) fn brace_groups(e: &str) -> String`
//! - `pub(super) fn xyce_domains(e: &str) -> String`
//! - `pub(super) fn xyce_power_calls(e: &str) -> String`
//! - `pub(super) fn ngspice_power_calls(e: &str, b_source: bool) -> String`
//! - `pub(super) fn close(b: &[u8], i: usize) -> Option<usize>`

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
    let ternary = |a: &[String]| match a {
        [c, x, y] => Some(format!("(({c})?({x}):({y}))")),
        _ => None,
    };
    map_calls(e, "if", &ternary, 0)
}

/// `e` with each name `from` (any case, whole, not a call, a node or a
/// device's `@x[...]`; `1m`'s suffix is no name) renamed `to`.
pub(super) fn rename_word(e: &str, from: &str, to: &str) -> String {
    let b = e.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = String::with_capacity(e.len() + 4);
    let mut i = 0;
    while i < b.len() {
        let end = i + from.len();
        let hit = (i == 0 || !(ident(b[i - 1]) || matches!(b[i - 1], b'.' | b'@' | b'[')))
            && e.get(i..end).is_some_and(|w| w.eq_ignore_ascii_case(from))
            && b.get(end).is_none_or(|&c| !ident(c) && c != b'(');
        if hit {
            out.push_str(to);
            i = end;
            continue;
        }
        let next = e[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&e[i..i + next]);
        i += next;
    }
    out
}

/// `e` with each statistical call at its nominal, as HSPICE and Xyce
/// evaluate one outside a Monte Carlo run: `agauss(n, …)`, `gauss`,
/// `aunif`, `unif`, two-argument `limit` are `n`, Xyce's `rand()` 0.5 (Xyce 7.10 by
/// hand: all five at 1 for n = 1, `2*rand()` 1; HSPICE: ESPice's reading
/// of `param_distributions.sp` gives the nominal -2 mA), where ngspice
/// draws one (-2.178 mA).
pub(super) fn nominal(e: &str) -> String {
    let first = |a: &[String]| a.first().filter(|_| a.len() >= 2).map(|n| format!("({n})"));
    let mut e = e.to_string();
    for f in ["agauss", "gauss", "aunif", "unif"] {
        e = map_calls(&e, f, &first, 0);
    }
    // Xyce's three-argument `limit(x, lo, hi)` is a clamp (RG §2.3).
    let pair = |a: &[String]| (a.len() == 2).then(|| format!("({})", a[0]));
    e = map_calls(&e, "limit", &pair, 0);
    let half = |a: &[String]| (a.len() == 1 && a[0].is_empty()).then(|| "0.5".to_string());
    map_calls(&e, "rand", &half, 0)
}

/// `e` with each three-argument `limit(x, lo, hi)`, Xyce's and LTspice's
/// clamp (Xyce RG §2.3), spelled `min(max(x, lo), hi)`: ngspice-45's
/// `limit` is a statistical call (`limit(3,0,2)` is 5 in `{}`, 0 in a B
/// source, run by hand; `min(max(3,0),2)` is 2).
pub(super) fn spell_clamp(e: &str) -> String {
    let clamp = |a: &[String]| match a {
        [x, lo, hi] => Some(format!("min(max({x}, {lo}), {hi})")),
        _ => None,
    };
    map_calls(e, "limit", &clamp, 0)
}

/// `e` with each `nint(x)` rounding as the source's does on the target:
/// ngspice's rounds a half to even (`nint(2.5)` is 2, `nint(-1.5)` -2:
/// C's `rint`, run by hand), Xyce's away from zero (3, -2; Xyce 7.10 by
/// hand). `to_xyce`: ngspice's rounding spelled for Xyce, else Xyce's
/// for ngspice; the spellings ran by hand on both at ±0.5, ±1.5, 2.5.
pub(super) fn spell_nint(e: &str, to_xyce: bool) -> String {
    let round = |a: &[String]| match a {
        [x] if to_xyce => {
            let r = format!("floor(({x})+0.5)");
            Some(format!(
                "((({x})+0.5)=={r} && fmod({r},2)!=0 ? {r}-1 : {r})"
            ))
        }
        [x] => Some(format!("(({x})>=0 ? floor(({x})+0.5) : ceil(({x})-0.5))")),
        _ => None,
    };
    map_calls(e, "nint", &round, 0)
}

/// `e` with each call of `from` (any case, a whole name before `(`)
/// renamed `to`.
pub(super) fn rename_call(e: &str, from: &str, to: &str) -> String {
    let b = e.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut out = String::with_capacity(e.len() + 4);
    let mut i = 0;
    while i < b.len() {
        let end = i + from.len();
        let hit = (i == 0 || !ident(b[i - 1]))
            && e.get(i..end).is_some_and(|w| w.eq_ignore_ascii_case(from))
            && e[end..].trim_start().starts_with('(');
        if hit {
            out.push_str(to);
            i = end;
            continue;
        }
        let next = e[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&e[i..i + next]);
        i += next;
    }
    out
}

/// The index closing the group `b[i]` opens; `None` when `b[i]` opens
/// none or the group is unclosed. Quotes nest nothing.
pub(super) fn close(b: &[u8], i: usize) -> Option<usize> {
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

/// `e` with Xyce's `pow(x, y)` and `pwr(x, y)` spelled `((x)**(y))` (both
/// are its `**`: RG 7.10 §2.3; 7.10 by hand: `pwr(-2,3)` -8,
/// `pwr(-2.5,2.1)` 6.5145) and `pwrs(x, y)` `(sgn(x)*abs(x)**(y))` (its
/// definition; ngspice-45 has no `pwrs`), for the power rewrite to give
/// each the source's value on ngspice (`powers`).
pub(super) fn xyce_power_calls(e: &str) -> String {
    let power = |a: &[String]| match a {
        [x, y] => Some(format!("(({x})**({y}))")),
        _ => None,
    };
    let signed = |a: &[String]| match a {
        [x, y] => Some(format!("(sgn({x})*abs({x})**({y}))")),
        _ => None,
    };
    let e = map_calls(e, "pow", &power, 0);
    let e = map_calls(&e, "pwr", &power, 0);
    map_calls(&e, "pwrs", &signed, 0)
}

/// `e` with ngspice's power calls spelled as its `**`, which has one
/// meaning in both of its evaluators (ngspice-45 by hand, n = -2). In a B
/// source (`b_source`) `pow(n,3)` is `|n|^3` = 8: `((x)**(y))`, and
/// `pwr(n,3)` `sgn(n)*|n|^3` = -8: `(sgn(x)*((x)**(y)))`. In a `{}` value
/// `pwr(n,3)` is `|n|^3` = 8: `((x)**(y))`; `pow(n,3)` is C's (-8) and
/// stands.
pub(super) fn ngspice_power_calls(e: &str, b_source: bool) -> String {
    let power = |a: &[String]| match a {
        [x, y] => Some(format!("(({x})**({y}))")),
        _ => None,
    };
    let signed = |a: &[String]| match a {
        [x, y] => Some(format!("(sgn({x})*(({x})**({y})))")),
        _ => None,
    };
    match b_source {
        true => map_calls(&map_calls(e, "pow", &power, 0), "pwr", &signed, 0),
        false => map_calls(e, "pwr", &power, 0),
    }
}

/// `e` with Xyce's value of a call outside its real domain spelled for
/// ngspice, which fails the analysis there. Xyce keeps the real part of
/// the complex result: `sqrt(x)` is 0 under x < 0 (`sqrt(max(x,0))`),
/// `acosh(x)` is `acosh(|x|)` and 0 inside (-1, 1)
/// (`acosh(max(abs(x),1))`). Its `atanh` clamps an argument that reads a
/// parameter or a probe to ±(1 - 1e-12) (`atanh(-2.5)` is -14.1621); an
/// argument of numbers alone stands: Xyce folds it with no clamp
/// (`atanh(2)` is 0.5493) and ngspice refuses the NaN. Xyce 7.10 and
/// ngspice-45 by hand (`research/impl/evidence/P3-016`).
pub(super) fn xyce_domains(e: &str) -> String {
    let sqrt = |a: &[String]| match a {
        [x] => Some(format!("sqrt(max({x},0))")),
        _ => None,
    };
    let acosh = |a: &[String]| match a {
        [x] => Some(format!("acosh(max(abs({x}),1))")),
        _ => None,
    };
    let atanh = |a: &[String]| match a {
        [x] if reads(x) => Some(format!("atanh(min(max({x},-1+1e-12),1-1e-12))")),
        _ => None,
    };
    let e = map_calls(e, "sqrt", &sqrt, 0);
    let e = map_calls(&e, "acosh", &acosh, 0);
    map_calls(&e, "atanh", &atanh, 0)
}

/// `e` with each `{…}` group inside it in parentheses, which every
/// dialect reads as the group; the braces that hold all of `e` (a value's
/// delimiters) stay. Xyce 7.10 and LTspice 26.1.1 read `{a-b}*i(v1)` as
/// `(a-b)*i(v1)`; ngspice-45's B-source reader drops the braces
/// (`a-b*i(v1)`) and its `.param` refuses `{{a-b}*3}` (run by hand,
/// `research/impl/evidence/P3-084`). A value with a string in it is
/// copied: a brace there is text.
pub(super) fn brace_groups(e: &str) -> String {
    let t = e.trim();
    let whole = t.starts_with('{') && close(t.as_bytes(), 0) == Some(t.len() - 1);
    let parens = |s: &str| s.replace('{', "(").replace('}', ")");
    match whole {
        _ if e.contains('"') => e.to_string(),
        true if t[1..].contains('{') => format!("{{{}}}", parens(&t[1..t.len() - 1])),
        true => e.to_string(),
        false => parens(e),
    }
}

// ---- private ----

/// Whether `x` reads a parameter or a probe: a name that is no call's, or
/// a `v(…)` or `i(…)`. A number's suffix or exponent (`2k`, `1e-3`) is no
/// name, and a call of numbers (`exp(1)`) reads nothing.
fn reads(x: &str) -> bool {
    let b = x.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    (0..b.len()).any(|i| {
        let start = (b[i].is_ascii_alphabetic() || b[i] == b'_')
            && (i == 0 || !(word(b[i - 1]) || b[i - 1] == b'.'));
        let end = i + b[i..].iter().take_while(|&&c| word(c)).count();
        let probe = end == i + 1 && matches!(b[i], b'v' | b'V' | b'i' | b'I');
        start && (probe || b.get(end) != Some(&b'('))
    })
}

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

/// `s` with each call of `name` (any case, a whole name) spelled by
/// `f` from its arguments, inner calls first; a call `f` declines
/// (`None`) as written. At nesting `depth`.
fn map_calls(s: &str, name: &str, f: &dyn Fn(&[String]) -> Option<String>, depth: u32) -> String {
    let b = s.as_bytes();
    let n = name.len();
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        let word = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        let call = word
            && depth < DEPTH
            && s.get(i..i + n)
                .is_some_and(|w| w.eq_ignore_ascii_case(name))
            && b.get(i + n) == Some(&b'(');
        if let Some(j) = call.then(|| close(b, i + n)).flatten() {
            let args: Vec<String> = commas(&s[i + n + 1..j])
                .iter()
                .map(|a| map_calls(a.trim(), name, f, depth + 1))
                .collect();
            if let Some(spelled) = f(&args) {
                out.push_str(&spelled);
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

#[cfg(test)]
mod tests {
    use super::{
        Nest, Read, ngspice_nest, nominal, parenthesize, rename_call, rename_word, spell_clamp,
        spell_if, spell_nint,
    };

    /// Each spelling's value on ngspice-45 is the call's on its own
    /// simulator (Xyce 7.10, ngspice-45: `research/impl/evidence/P3-078`).
    #[test]
    fn power_calls_are_spelled_as_powers() {
        let xy = super::xyce_power_calls;
        assert_eq!(xy("pwr(V(1),v(2))"), "((V(1))**(v(2)))");
        assert_eq!(
            xy("POW(a, 3)+pwrs(-b,c)"),
            "((a)**(3))+(sgn(-b)*abs(-b)**(c))"
        );
        assert_eq!(xy("pow(a)"), "pow(a)");
        let ng = super::ngspice_power_calls;
        assert_eq!(ng("pow(v(n),3)", true), "((v(n))**(3))");
        assert_eq!(ng("pwr(v(n),2)", true), "(sgn(v(n))*((v(n))**(2)))");
        assert_eq!(ng("pow(n,3)+pwr(n,2)", false), "pow(n,3)+((n)**(2))");
    }

    /// Each spelling's value on ngspice-45 is Xyce 7.10's of the call
    /// (`research/impl/evidence/P3-016/cases.txt`).
    #[test]
    fn a_xyce_domain_is_spelled_for_ngspice() {
        let d = super::xyce_domains;
        assert_eq!(d("5V*SQRT(V(1,0))"), "5V*sqrt(max(V(1,0),0))");
        assert_eq!(
            d("acosh(V(3))+asqrt(x)"),
            "acosh(max(abs(V(3)),1))+asqrt(x)"
        );
        assert_eq!(d("ATANH(V(4))"), "atanh(min(max(V(4),-1+1e-12),1-1e-12))");
        assert_eq!(d("atanh(k)"), "atanh(min(max(k,-1+1e-12),1-1e-12))");
        // Numbers alone: Xyce folds the call with no clamp.
        for folded in ["atanh(2)", "atanh(exp(1))", "atanh(2k)", "atanh(1e-3)"] {
            assert_eq!(d(folded), folded);
        }
        assert_eq!(d("sqrt(sqrt(x))"), "sqrt(max(sqrt(max(x,0)),0))");
    }

    /// ngspice-45 gives each spelling the value Xyce 7.10 gives the
    /// braced text (`research/impl/evidence/P3-084/cases.txt`).
    #[test]
    fn a_brace_group_inside_an_expression_is_parenthesized() {
        let g = super::brace_groups;
        assert_eq!(g("{bletch-foobie}*I(V1)"), "(bletch-foobie)*I(V1)");
        assert_eq!(g("{{bletch-foobie}*3}"), "{(bletch-foobie)*3}");
        assert_eq!(g("2**{1+m}"), "2**(1+m)");
        // A value's own braces, and text with none, stay.
        for kept in ["{a*2}", " {a} ", "a+b", "\"x{y}\""] {
            assert_eq!(g(kept), kept);
        }
    }

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
    fn a_draw_is_its_nominal() {
        assert_eq!(nominal("agauss(1k,100,3)+2*rand()"), "(1k)+2*0.5");
        assert_eq!(nominal("limit(2k, 500)*unif(a,0.1)"), "(2k)*(a)");
        assert_eq!(nominal("gauss(x)"), "gauss(x)");
        assert_eq!(nominal("limit(t-d,0,1)"), "limit(t-d,0,1)");
    }

    #[test]
    fn a_clamp_is_min_of_max() {
        assert_eq!(
            spell_clamp("limit(t-d,0,1)+limit(a,b)"),
            "min(max(t-d, 0), 1)+limit(a,b)"
        );
    }

    #[test]
    fn nint_rounds_as_its_source() {
        let x = spell_nint("nint(value)", true);
        let r = "floor((value)+0.5)";
        assert_eq!(
            x,
            format!("(((value)+0.5)=={r} && fmod({r},2)!=0 ? {r}-1 : {r})")
        );
        assert_eq!(
            spell_nint("NINT(a)", false),
            "((a)>=0 ? floor((a)+0.5) : ceil((a)-0.5))"
        );
    }

    #[test]
    fn a_name_is_renamed_whole() {
        assert_eq!(
            rename_word("M*1m+mx+m(2)+2*M", "m", "m_1"),
            "m_1*1m+mx+m(2)+2*m_1"
        );
    }

    #[test]
    fn a_call_is_renamed_whole() {
        assert_eq!(
            rename_call("log(2.7)+LOG (v(1))", "log", "ln"),
            "ln(2.7)+ln (v(1))"
        );
        assert_eq!(
            rename_call("log10(x)+blog(x)+log", "log", "ln"),
            "log10(x)+blog(x)+log"
        );
    }

    #[test]
    fn xyce_gets_a_blank_before_each_colon() {
        let x = |e: &str| parenthesize(e, Read::Ngspice, true);
        assert_eq!(x("X<P?P:X>N?N:X"), "X<P?P :X>N?N :X");
        assert_eq!(x("x<p ? p : n"), "x<p ? p : n");
    }
}
