//! IR text, the lexers: a literal, a name, a quoted run and a reference's mark, from the bytes of
//! a line, shared by the statement cursor (`text_words`) and the expression reader
//! (`text_parse`). Literals are read digit for digit into `Literal` (A3: no floating
//! arithmetic); a reference's mark is `?` (held unresolved) or `#k` (bound to definition `k`).
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (A4), §7.3 (A3), §9.
//!
//! Contents: crate: `pub(crate) enum Mark`, `pub(crate) fn lex_literal(&str) -> Option<Literal>`,
//! `pub(crate) fn lex_name(&str, usize, usize) -> Option<(Cow<'_, str>, usize)>`,
//! `pub(crate) fn unquote(&str, usize, usize) -> Option<(Cow<'_, str>, usize)>`,
//! `pub(crate) fn lex_mark(&str, usize, usize) -> (Mark, usize)`,
//! `pub(crate) fn apply_mark(&mut Builder<'_>, ExprId, Mark)`,
//! `pub(crate) fn number_end(&[u8], usize, usize) -> usize`,
//! `pub(crate) fn ident_or_quoted(&str, usize, usize) -> Option<(Cow<'_, str>, usize)>`,
//! `pub(crate) fn name_byte(u8) -> bool`

use std::borrow::Cow;

use crate::build::Builder;
use crate::expr::{ExprTag, Literal, NumForm};
use crate::id::{DefId, ExprId};

/// How a reference was marked in the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    /// `name`: unresolved, for the linker.
    Bare,
    /// `name?`: held unresolved through the link.
    Held,
    /// `name#k`: bound to definition `k`.
    Def(u32),
}

/// The scale letters, longest first (as `text_expr`'s writer spells them).
const SCALES: [(&str, i16); 12] = [
    ("peta", 15),
    ("meg", 6),
    ("a", -18),
    ("f", -15),
    ("p", -12),
    ("n", -9),
    ("u", -6),
    ("m", -3),
    ("c", -2),
    ("k", 3),
    ("g", 9),
    ("t", 12),
];

/// A literal as one word: `[-]digits[.digits]` then `e<int>`, a scale letter, `mil`, or nothing.
/// `None` for anything else, or a literal outside `Literal`'s invariants.
pub(crate) fn lex_literal(word: &str) -> Option<Literal> {
    let (negative, rest) = match word.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, word),
    };
    let int_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    let (int, rest) = rest.split_at(int_len);
    let (dec, rest) = match rest.strip_prefix('.') {
        Some(r) => r.split_at(r.bytes().take_while(u8::is_ascii_digit).count()),
        None => ("", rest),
    };
    if int.is_empty() && dec.is_empty() || dec.len() > 18 || int.len() + dec.len() > 19 {
        return None;
    }
    let digits = int.bytes().chain(dec.bytes()).try_fold(0u64, |n, c| {
        n.checked_mul(10)?.checked_add((c - b'0') as u64)
    })?;
    let (exp10, form) = if rest.is_empty() {
        (0, NumForm::Plain)
    } else if rest == "mil" {
        (0, NumForm::Mil)
    } else if rest == "i" {
        (0, NumForm::Int)
    } else if let Some(e) = rest.strip_prefix('e') {
        (e.parse::<i16>().ok()?, NumForm::Exponent)
    } else {
        (SCALES.iter().find(|s| s.0 == rest)?.1, NumForm::Scale)
    };
    let mantissa = i64::try_from(digits).ok()?;
    let l = Literal {
        mantissa: if negative { -mantissa } else { mantissa },
        frac: dec.len() as u8,
        exp10,
        form,
    };
    l.is_valid().then_some(l)
}

/// A name in a name position at `text[at..end]`: bare `[A-Za-z0-9_.$]+` or backtick-quoted.
/// Returns it and the position after it.
pub(crate) fn lex_name(text: &str, at: usize, end: usize) -> Option<(Cow<'_, str>, usize)> {
    let s = text.as_bytes();
    if at < end && s[at] == b'`' {
        return unquote(text, at, end);
    }
    let q = at + s[at..end].iter().take_while(|c| name_byte(**c)).count();
    (q > at).then(|| (Cow::Borrowed(&text[at..q]), q))
}

/// The quoted run starting at `text[at]` (its quote character is the delimiter), unescaped
/// (`\\`, `\n`, the quote): borrowed when it has no escape. Returns it and the position after
/// the closing quote.
pub(crate) fn unquote(text: &str, at: usize, end: usize) -> Option<(Cow<'_, str>, usize)> {
    let s = text.as_bytes();
    let q = *s.get(at)?;
    let mut owned: Option<String> = None;
    let mut p = at + 1;
    while p < end {
        match s[p] {
            c if c == q => {
                let v = owned.map_or(Cow::Borrowed(&text[at + 1..p]), Cow::Owned);
                return Some((v, p + 1));
            }
            b'\\' if p + 1 < end => {
                let buf = owned.get_or_insert_with(|| text[at + 1..p].to_string());
                buf.push(match s[p + 1] {
                    b'n' => '\n',
                    c => c as char,
                });
                p += 2;
            }
            _ => {
                let len = text[p..].chars().next()?.len_utf8();
                if let Some(buf) = owned.as_mut() {
                    buf.push_str(&text[p..p + len]);
                }
                p += len;
            }
        }
    }
    None
}

/// The mark glued after a reference's name: `?`, `#digits`, or none.
pub(crate) fn lex_mark(text: &str, at: usize, end: usize) -> (Mark, usize) {
    let s = text.as_bytes();
    if at < end && s[at] == b'?' {
        return (Mark::Held, at + 1);
    }
    if at < end && s[at] == b'#' {
        let n = s[at + 1..end]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if let Ok(k) = text[at + 1..at + 1 + n].parse::<u32>() {
            return (Mark::Def(k), at + 1 + n);
        }
    }
    (Mark::Bare, at)
}

/// Applies a mark to a `Param` or `UserCall` expression row (raw: `name#k` may name a
/// definition that comes later in the text; `check` validates it).
pub(crate) fn apply_mark(b: &mut Builder<'_>, e: ExprId, mark: Mark) {
    let def = match mark {
        Mark::Bare => return,
        Mark::Held => DefId::HELD,
        Mark::Def(k) => DefId(k),
    };
    let x = &mut b.design.exprs;
    let i = e.index();
    match x.tags[i] {
        ExprTag::Param => x.b[i] = def.0,
        ExprTag::UserCall => x.calls[x.a[i] as usize].func.def = def,
        ExprTag::Num
        | ExprTag::Str
        | ExprTag::Call
        | ExprTag::Unary
        | ExprTag::Binary
        | ExprTag::Ternary
        | ExprTag::V
        | ExprTag::I
        | ExprTag::Time
        | ExprTag::Temp
        | ExprTag::Freq
        | ExprTag::TStep
        | ExprTag::TStop
        | ExprTag::Tnom
        | ExprTag::Draw
        | ExprTag::List => unreachable!("only a reference carries a mark"),
    }
}

/// The end of a number token starting at `at` (digits, a point, then an exponent or letters).
pub(crate) fn number_end(s: &[u8], at: usize, end: usize) -> usize {
    let mut q = at;
    while q < end && (s[q].is_ascii_digit() || s[q] == b'.') {
        q += 1;
    }
    let exponent = q < end
        && s[q] == b'e'
        && (s.get(q + 1).is_some_and(u8::is_ascii_digit)
            || (s.get(q + 1).is_some_and(|c| *c == b'-' || *c == b'+')
                && s.get(q + 2).is_some_and(u8::is_ascii_digit)));
    if exponent {
        q += 2;
        while q < end && s[q].is_ascii_digit() {
            q += 1;
        }
    } else {
        while q < end && s[q].is_ascii_alphabetic() {
            q += 1;
        }
    }
    q
}

/// A user function's name after `@`: an identifier or a backtick-quoted name.
pub(crate) fn ident_or_quoted(text: &str, at: usize, end: usize) -> Option<(Cow<'_, str>, usize)> {
    let s = text.as_bytes();
    if at < end && s[at] == b'`' {
        return unquote(text, at, end);
    }
    let first = *s.get(at)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    let q = at + s[at..end].iter().take_while(|c| name_byte(**c)).count();
    Some((Cow::Borrowed(&text[at..q]), q))
}

/// A byte of a bare name: `[A-Za-z0-9_.$]`.
pub(crate) fn name_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'$'
}
