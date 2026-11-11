//! A power (`a**b`; `a^b` where the source's `^` is power) in an
//! expression kept as text, written for a target that groups it another
//! way: a chain (`a**b**c`) and a power under a unary sign (`-a**b`) get
//! the SOURCE's grouping in explicit parentheses, so the target's
//! associativity and unary rule cannot regroup them. A power the target
//! groups as the source does stays as written.
//!
//! A power's value differs too ([`Law`]): ngspice takes the base's
//! magnitude (`(-2)**3` is 8, in `{}` and in a B source; ESPice 0.3.0 the
//! same), Xyce 7.10, LTspice 26.1.1, HSPICE and Spectre keep a negative
//! base's sign under a whole exponent (-8), and Xyce gives the real part
//! of the complex power under any other (`(-2.5)**2.1` is 6.5145). Each
//! spelling writes `abs()` itself, so it reads the same on a target of
//! either kind.
//!
//! A dialect's grouping is its lexicon's operator rows ([`Grouping::of`]),
//! the rows its parser climbs by. Measured (`.param a=2 b=3 c=2`):
//! ngspice-45 `a**b**c` 64 and `-a**c` -4, in `{}` and in a B source;
//! Xyce 7.10 512 and -4; LTspice 26.1.1 512 and +4.
//!
//! The text is walked, not parsed: an operand is a number, a name, a call
//! or a delimited group, with the signs before it; any other byte ends a
//! chain and makes the next sign a prefix. A group's inside is walked the
//! same way, to [`DEPTH`] levels (deeper text stays as written).
//!
//! Cites: ngspice-45 manual §2.8.5 (`**`, `^`: "left to right" at equal
//! precedence); Xyce RG 7.10 Table 2-32; LTspice 26.1.1 help, "B.
//! Arbitrary behavioral voltage or current sources" (`**`); the runs
//! above (ZERO-LEDGER P3-077). The laws: ngspice-45 manual §5.1 (B
//! source `**`, `^`, `pow`: `pow(fabs(x), y)`); Xyce RG 7.10 §2.3 (`**`,
//! `pow`, `pwr`, `pwrs`); HSPICE UG E-2010.12 Table 27 (`x**y`);
//! settle-remote (Spectre 25.1: `(-2)**3` -8); the runs in
//! `research/impl/evidence/P3-078` (ZERO-LEDGER P3-078, P3-029, P3-016).
//!
//! Contents:
//! - `pub(super) struct Grouping`, `Grouping::of(ops: &Ops) -> Grouping`
//! - `pub(super) enum Law`
//! - `pub(super) fn group_powers(e: &str, src: Grouping, tgt: Grouping, caret: bool, law: Law) -> String`

use super::ternary::close;
use crate::syntax::lexicon::Ops;
use crate::syntax::token::Tag;

/// How a dialect groups a power.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct Grouping {
    /// `a**b**c` is `(a**b)**c` (ngspice-45: `2**3**2` is 64); else
    /// `a**(b**c)` (Xyce 7.10, LTspice 26.1.1: 512).
    pub(super) left: bool,
    /// `-a**b` is `(-a)**b` (LTspice 26.1.1: `-2**2` is 4); else
    /// `-(a**b)` (ngspice-45, Xyce 7.10: -4).
    pub(super) unary_first: bool,
}

/// What a power's value in the source asks of its spelling for the
/// target.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Law {
    /// Source and target give a power one value: as written.
    Keep,
    /// The source takes the base's magnitude (ngspice), the target keeps
    /// its sign: `abs(x)**y`.
    Abs,
    /// The target takes the base's magnitude, the source keeps a negative
    /// base's sign under a whole exponent: `(sgn(x)*abs(x)**n)` for an odd
    /// literal `n`; any other exponent as written.
    Signed,
    /// [`Law::Signed`], and under an exponent that is no whole literal the
    /// source gives the real part of the complex power (Xyce):
    /// `(abs(x)**(y)*((x)<0 ? cos(pi*(y)) : 1))`, pi a literal (ngspice's
    /// `{}` evaluator has no `pi`).
    Real,
}

/// One operand of a chain: the operator before it (`""` at the head), its
/// signs, its primary.
type Operand = (&'static str, String, String);

/// What [`walk`] respells a chain by.
#[derive(Clone, Copy)]
struct Rule {
    src: Grouping,
    tgt: Grouping,
    /// The source reads `^` as power.
    caret: bool,
    law: Law,
}

/// The nesting [`walk`] follows.
const DEPTH: u32 = 64;

impl Grouping {
    /// The grouping `ops` gives `**`: to the left unless `Ops::right`
    /// lists it; a unary sign above it when `Ops::prefix_bp` reaches its
    /// binding power (a prefix's operand takes only tighter operators).
    /// A dialect with no `**` row (CDL) groups as ngspice does.
    pub(super) fn of(ops: &Ops) -> Grouping {
        let power = ops.infix.iter().find(|(t, _)| *t == Tag::StarStar);
        Grouping {
            left: !ops.right.contains(&Tag::StarStar),
            unary_first: power.is_some_and(|&(_, bp)| ops.prefix_bp >= bp),
        }
    }
}

/// `e` with each power chain and each power under a unary sign that `tgt`
/// groups another way than `src` parenthesised as `src` groups it, and
/// each power `law` gives another value respelled; every other byte as
/// written. `caret`: the source reads `^` as power too.
pub(super) fn group_powers(e: &str, src: Grouping, tgt: Grouping, caret: bool, law: Law) -> String {
    let rule = Rule {
        src,
        tgt,
        caret,
        law,
    };
    let out = walk(e, rule, 0);
    let added = |c: char| out.matches(c).count() as i64 - e.matches(c).count() as i64;
    debug_assert_eq!(added('('), added(')'), "parentheses are added in pairs");
    debug_assert!(
        src != tgt || law != Law::Keep || out == e,
        "one grouping, one law: as written"
    );
    out
}

// ---- private ----

/// `s` with its chains respelled, at nesting `depth`.
fn walk(s: &str, r: Rule, depth: u32) -> String {
    if depth >= DEPTH {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + 8);
    // `unary`: a sign here is a prefix (the text's start, or after an
    // operator), not a binary operator.
    let (mut i, mut unary) = (0, true);
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            out.push(char::from(b[i]));
            i += 1;
            continue;
        }
        let Some(first) = operand(s, i, unary) else {
            let next = s[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&s[i..i + next]);
            (i, unary) = (i + next, true);
            continue;
        };
        // The chain from here: each operand's text since the one before
        // (the operator, its signs, the blanks) and its primary.
        let mut xs: Vec<(&str, String)> = Vec::new();
        let (mut from, (mut at, mut end)) = (i, first);
        loop {
            xs.push((&s[from..at], inner(&s[at..end], r, depth)));
            let next = power_op(s, end, r.caret).and_then(|k| operand(s, k, true));
            let Some(next) = next else { break };
            (from, (at, end)) = (end, next);
        }
        match chain(&xs, r) {
            Some(spelled) => out.push_str(&spelled),
            None => xs
                .iter()
                .for_each(|(lead, p)| out.extend([*lead, p.as_str()])),
        }
        (i, unary) = (end, false);
    }
    out
}

/// The operand at `s[i..]`: where its primary starts (after its signs,
/// taken only when `signed`) and where it ends. A primary is a number, a
/// name, a call (`f(x)`, `f (x)`, `@m1[gm]`) or a delimited group.
fn operand(s: &str, i: usize, signed: bool) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    let mut at = i;
    while signed && at < b.len() && matches!(b[at], b'+' | b'-' | b' ' | b'\t') {
        at += 1;
    }
    let c = *b.get(at)?;
    if let Some(j) = close(b, at) {
        return Some((at, j + 1));
    }
    let word = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'$' | b'@' | b'#');
    if !word(c) {
        return None;
    }
    let number = c.is_ascii_digit() || c == b'.';
    let mut end = match number {
        true => at + crate::unit::parse_spice_number(&s[at..])?.1,
        false => at,
    };
    while end < b.len() && word(b[end]) {
        end += 1;
    }
    let blanks = b[end..]
        .iter()
        .take_while(|&&c| matches!(c, b' ' | b'\t'))
        .count();
    let open = match b.get(end + blanks) {
        Some(b'(') if !number => Some(end + blanks),
        _ if !number && b.get(end) == Some(&b'[') => Some(end),
        _ => None,
    };
    let end = open.and_then(|k| close(b, k)).map_or(end, |j| j + 1);
    assert!(
        at < end && end <= b.len(),
        "an operand has a byte, inside the text"
    );
    Some((at, end))
}

/// The index after the power operator (`**`; `^` when `caret`) and the
/// blanks around it at `s[i..]`; `None` when no power follows.
fn power_op(s: &str, i: usize, caret: bool) -> Option<usize> {
    let t = s[i..].trim_start_matches([' ', '\t']);
    let n = match t {
        _ if t.starts_with("**") => 2,
        _ if caret && t.starts_with('^') => 1,
        _ => return None,
    };
    Some(s.len() - t[n..].trim_start_matches([' ', '\t']).len())
}

/// Primary `p` with the inside of its group (a call's arguments, a
/// parenthesised, braced or quoted group) walked.
fn inner(p: &str, r: Rule, depth: u32) -> String {
    let b = p.as_bytes();
    let open = b
        .iter()
        .position(|&c| matches!(c, b'(' | b'[' | b'{' | b'\'' | b'"'));
    match open.filter(|&k| close(b, k) == Some(b.len() - 1)) {
        Some(k) => {
            let last = b.len() - 1;
            format!(
                "{}{}{}",
                &p[..=k],
                walk(&p[k + 1..last], r, depth + 1),
                &p[last..]
            )
        }
        None => p.to_string(),
    }
}

/// A lead's operator (`""` before a chain's head) and its signs, blanks
/// dropped.
fn parts(lead: &str) -> (&'static str, String) {
    let t = lead.trim_start();
    let op = match t {
        _ if t.starts_with("**") => "**",
        _ if t.starts_with('^') => "^",
        _ => "",
    };
    (
        op,
        t[op.len()..]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect(),
    )
}

/// The chain `xs` (each operand's lead and primary) as the source groups
/// it, every group explicit, each power as the law spells it; `None`
/// where the target groups it the same way and the law changes none: as
/// written.
fn chain(xs: &[(&str, String)], r: Rule) -> Option<String> {
    assert!(!xs.is_empty(), "a chain has an operand");
    let n = xs.len();
    let xs: Vec<Operand> = xs
        .iter()
        .map(|(lead, p)| {
            let (op, sign) = parts(lead);
            (op, sign, p.clone())
        })
        .collect();
    // A sign on the last operand is that operand's own in every dialect.
    let signed = xs[..n - 1].iter().any(|x| !x.1.is_empty());
    let regroup =
        (n >= 3 && r.src.left != r.tgt.left) || (signed && r.src.unary_first != r.tgt.unary_first);
    // Respelled when the law changes a power of it too: the grouped text
    // under the law against the grouped text under none.
    let spelled = grouped(&xs, r);
    let kept = Rule {
        law: Law::Keep,
        ..r
    };
    (regroup || spelled != grouped(&xs, kept)).then_some(spelled)
}

/// `xs` as the source of `r` groups it, in explicit parentheses.
fn grouped(xs: &[Operand], r: Rule) -> String {
    assert!(!xs.is_empty(), "a chain has an operand");
    let plain = |xs: &[Operand]| -> Vec<(&'static str, String)> {
        xs.iter().map(|(op, _, p)| (*op, p.clone())).collect()
    };
    if r.src.unary_first {
        // A sign is its operand's own: `(-a)**b`; a lone signed exponent
        // needs no parentheses.
        let atoms: Vec<(&'static str, String)> = xs
            .iter()
            .enumerate()
            .map(
                |(k, (op, sign, p))| match sign.is_empty() || (k == 1 && xs.len() == 2) {
                    true => (*op, format!("{sign}{p}")),
                    false => (*op, format!("({sign}{p})")),
                },
            )
            .collect();
        return fold(&atoms, r);
    }
    // A sign takes the rest of the chain as its operand: `-a**b` is
    // `-(a**b)`, `a**-b**c` is `a**-(b**c)`.
    let Some(k) = xs.iter().position(|x| !x.1.is_empty()) else {
        return fold(&plain(xs), r);
    };
    let mut tail = xs[k..].to_vec();
    let sign = std::mem::take(&mut tail[0].1);
    let rest = match tail.len() {
        1 => tail[0].2.clone(),
        _ => wrap(&grouped(&tail, r)),
    };
    let mut atoms = plain(&xs[..k]);
    atoms.push((xs[k].0, format!("{sign}{rest}")));
    fold(&atoms, r)
}

/// `atoms` (each with the operator before it) as one power, grouped as
/// the source of `r` groups a chain, each inner power parenthesised.
fn fold(atoms: &[(&'static str, String)], r: Rule) -> String {
    let n = atoms.len();
    assert!(n >= 1, "a chain has an operand");
    if r.src.left {
        let mut acc = atoms[0].1.clone();
        for (k, (op, a)) in atoms.iter().enumerate().skip(1) {
            let base = if k == 1 { acc } else { wrap(&acc) };
            acc = power(&base, op, a, r.law);
        }
        return acc;
    }
    let mut acc = atoms[n - 1].1.clone();
    for k in (0..n - 1).rev() {
        let exp = if k == n - 2 { acc } else { wrap(&acc) };
        acc = power(&atoms[k].1, atoms[k + 1].0, &exp, r.law);
    }
    acc
}

/// The power `b op x` as `law` spells it. As written under [`Law::Keep`],
/// for a base that is a non-negative number or an `abs()` call, and for
/// an even literal exponent: every dialect gives `|b|^x` there.
fn power(b: &str, op: &str, x: &str, law: Law) -> String {
    let (base, exp) = (bare(b), number(x));
    let abs = base
        .get(..4)
        .is_some_and(|h| h.eq_ignore_ascii_case("abs("))
        && close(base.as_bytes(), 3) == Some(base.len() - 1);
    let positive = abs || number(b).is_some_and(|v| v >= 0.0);
    let whole = exp.filter(|v| v.fract() == 0.0);
    let even = whole.is_some_and(|v| v % 2.0 == 0.0);
    match (law, whole) {
        _ if law == Law::Keep || positive || even => format!("{b}{op}{x}"),
        (Law::Abs, _) => format!("abs({base}){op}{x}"),
        (_, Some(_)) => format!("(sgn({base})*abs({base}){op}{x})"),
        (Law::Real, None) => {
            let y = bare(x);
            format!("(abs({base}){op}({y})*(({base})<0 ? cos(3.141592653589793*({y})) : 1))")
        }
        _ => format!("{b}{op}{x}"),
    }
}

/// `t` without the blanks and the parentheses around all of it.
fn bare(t: &str) -> &str {
    let mut t = t.trim();
    while t.starts_with('(') && close(t.as_bytes(), 0) == Some(t.len() - 1) {
        t = t[1..t.len() - 1].trim();
    }
    t
}

/// The value of `t` when it is one signed number, bare or in
/// parentheses.
fn number(t: &str) -> Option<f64> {
    let t = bare(t);
    let m = t.strip_prefix(['-', '+']).unwrap_or(t).trim_start();
    let (v, end, _) = crate::unit::parse_spice_number(m)?;
    let whole = m[end..].bytes().all(|c| c.is_ascii_alphabetic());
    whole.then_some(if t.starts_with('-') { -v } else { v })
}

/// `t` in parentheses, unless it is one parenthesised group already.
fn wrap(t: &str) -> String {
    match t.starts_with('(') && close(t.as_bytes(), 0) == Some(t.len() - 1) {
        true => t.to_string(),
        false => format!("({t})"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Grouping, Law, group_powers};

    const NG: Grouping = Grouping {
        left: true,
        unary_first: false,
    };
    const XY: Grouping = Grouping {
        left: false,
        unary_first: false,
    };
    const LT: Grouping = Grouping {
        left: false,
        unary_first: true,
    };

    /// Each spelling's value is the source simulator's on the source text
    /// (a=2, b=3, c=2; ngspice-45, Xyce 7.10, LTspice 26.1.1, run by hand:
    /// `research/impl/evidence/P3-077`): `(a**b)**c` 64, `a**(b**c)` 512,
    /// `-(a**c)` -4, `(-a)**c` 4 on all three.
    #[test]
    fn a_chain_and_a_signed_power_take_the_source_grouping() {
        for (e, src, tgt, want) in [
            ("a**b**c", NG, XY, "(a**b)**c"),
            ("a**b**c", XY, NG, "a**(b**c)"),
            ("a**b**c**d", NG, XY, "((a**b)**c)**d"),
            ("a**b**c**d", XY, NG, "a**(b**(c**d))"),
            ("-a**c", NG, LT, "-(a**c)"),
            ("-a**c", LT, NG, "(-a)**c"),
            ("a**-b**c", XY, NG, "a**-(b**c)"),
            (
                "f(x**y**z, 1e-3**2**n)",
                NG,
                XY,
                "f((x**y)**z, (1e-3**2)**n)",
            ),
            ("x>0?a:b**2**c", NG, XY, "x>0?a:(b**2)**c"),
        ] {
            assert_eq!(group_powers(e, src, tgt, false, Law::Keep), want, "{e}");
        }
        let caret = group_powers("v(a)^3^2", NG, XY, true, Law::Keep);
        assert_eq!(caret, "(v(a)^3)^2");
    }

    /// What the target groups as the source does is copied byte for byte.
    #[test]
    fn a_power_the_target_groups_alike_stays_as_written() {
        for (e, src, tgt) in [
            ("a ** b", NG, XY),
            ("-a**c", NG, XY),
            ("2-a**c", LT, NG),
            ("a**-b", LT, NG),
            ("exp(-(TIME - 7e-3)**2 / 5e-6)", XY, NG),
            ("{Cjo}/(1+max(V(bias),-.5*{Vj})/{Vj})**{m}", LT, NG),
            ("v(a)^3^2", NG, XY),
            ("a**b**c", NG, NG),
        ] {
            assert_eq!(group_powers(e, src, tgt, false, Law::Keep), e);
        }
    }

    /// Each spelling's value on real ngspice-45 and Xyce 7.10 is the
    /// source simulator's on the source text (v(n) = -2, v(m) = -2.5,
    /// v(y) = 2.1; `research/impl/evidence/P3-078`): `abs(v(n))**3` 8 on
    /// both, `(sgn(v(n))*abs(v(n))**3)` -8 on ngspice, the cosine form
    /// 6.514489 on ngspice (Xyce's `v(m)**v(y)`: 6.514489).
    #[test]
    fn a_power_is_spelled_for_the_other_law() {
        const SIGNED: &str = "(sgn(V(1))*abs(V(1))**-3)";
        const REAL: &str = "(abs(V(1))**(2.1)*((V(1))<0 ? cos(3.141592653589793*(2.1)) : 1))";
        for (e, src, tgt, law, want) in [
            ("v(n)**3", NG, XY, Law::Abs, "abs(v(n))**3"),
            ("(v(a)-v(b))**1.5", NG, XY, Law::Abs, "abs(v(a)-v(b))**1.5"),
            ("a**b**c", NG, XY, Law::Abs, "abs(abs(a)**b)**c"),
            ("(V(1))**-3", XY, NG, Law::Signed, SIGNED),
            ("-V(1)**3", XY, NG, Law::Real, "-(sgn(V(1))*abs(V(1))**3)"),
            ("-a**3", LT, NG, Law::Signed, "(sgn(-a)*abs(-a)**3)"),
            ("(V(1))**2.1", XY, NG, Law::Real, REAL),
        ] {
            assert_eq!(group_powers(e, src, tgt, false, law), want, "{e}");
        }
    }

    /// Where every dialect gives one value (an even literal exponent, a
    /// base that cannot be negative) and where no row gives a law (LTspice's
    /// fractional exponent), the text stands.
    #[test]
    fn a_power_of_one_value_stands_under_every_law() {
        for (e, src, tgt, law) in [
            ("v(n)**2", NG, XY, Law::Abs),
            ("2**v(n)", NG, XY, Law::Abs),
            ("(V(1))**-2", XY, NG, Law::Real),
            ("-V(1)**2.0", XY, NG, Law::Real),
            ("abs(-V(1))**(v(2))", XY, NG, Law::Real),
            ("(V(1))**2.1", LT, NG, Law::Signed),
            ("X1:L1:L**2", XY, NG, Law::Real),
        ] {
            assert_eq!(group_powers(e, src, tgt, false, law), e);
        }
    }
}
