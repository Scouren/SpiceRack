//! A power (`a**b`; `a^b` where the source's `^` is power) in an
//! expression kept as text, written for a target that groups it another
//! way: a chain (`a**b**c`) and a power under a unary sign (`-a**b`) get
//! the SOURCE's grouping in explicit parentheses, so the target's
//! associativity and unary rule cannot regroup them. A power the target
//! groups as the source does stays as written.
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
//! above (ZERO-LEDGER P3-077).
//!
//! Contents:
//! - `pub(super) struct Grouping`, `Grouping::of(ops: &Ops) -> Grouping`
//! - `pub(super) fn group_powers(e: &str, src: Grouping, tgt: Grouping, caret: bool) -> String`

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
/// groups another way than `src` parenthesised as `src` groups it; every
/// other byte as written. `caret`: the source reads `^` as power too.
pub(super) fn group_powers(e: &str, src: Grouping, tgt: Grouping, caret: bool) -> String {
    let out = walk(e, Rule { src, tgt, caret }, 0);
    let added = |c: char| out.matches(c).count() as i64 - e.matches(c).count() as i64;
    debug_assert_eq!(added('('), added(')'), "parentheses are added in pairs");
    debug_assert!(src != tgt || out == e, "one grouping: as written");
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
/// it, every group explicit; `None` where the target groups it the same
/// way: as written.
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
    regroup.then(|| grouped(&xs, r))
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
            acc = format!("{base}{op}{a}");
        }
        return acc;
    }
    let mut acc = atoms[n - 1].1.clone();
    for k in (0..n - 1).rev() {
        let exp = if k == n - 2 { acc } else { wrap(&acc) };
        acc = format!("{}{}{exp}", atoms[k].1, atoms[k + 1].0);
    }
    acc
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
    use super::{Grouping, group_powers};

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
            assert_eq!(group_powers(e, src, tgt, false), want, "{e}");
        }
        assert_eq!(group_powers("v(a)^3^2", NG, XY, true), "(v(a)^3)^2");
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
            assert_eq!(group_powers(e, src, tgt, false), e);
        }
    }
}
