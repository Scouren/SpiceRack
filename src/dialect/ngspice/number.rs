//! ngspice number text ↔ `ir::Literal` (ruling R1: the digits as written, no floating
//! arithmetic). ngspice has two number readers, and which one reads a number depends on where it
//! stands, so this file has two grammars:
//!
//! - [`Grammar::Inp`]: `INPevaluate` (`spicelib/parser/inpeval.c`), used for card values and
//!   inside a B-source expression (`inpptree.c` `PTlex` calls it): digits, `.`, digits, an
//!   exponent `e`/`E`/`d`/`D` with its sign, then one scale letter (`t g k m u n p f a`, `meg`,
//!   `mil`), then a unit tail that is ignored.
//! - [`Grammar::Numparam`]: numparam's `fetchnumber` (`frontend/numparam/xpressn.c`), used inside
//!   `{…}` and in `.param`, `.func`, `.if`: `sscanf("%lG")` (exponent `e`/`E` only), then one
//!   scale letter read by `parseunit` (`meg` by prefix, else `m` is milli: `2mil` is 2 milli with
//!   the tail `il`), then a tail of `alfa` bytes (letters, `_`, `[`, `]`).
//!
//! In: the text at a number's first byte (no sign: a sign is the expression parser's, or
//! [`card`]'s). Out: the literal and the bytes it took, scale and tail included. The writer
//! spells a literal back with its digits and form in the grammar of the place it goes.
//!
//! Cites: ngspice-45 source `inpeval.c` `INPevaluate`, `xpressn.c` `fetchnumber`, `parseunit`;
//! `rewrite/INV-semantics.md` 1.01–1.07; `rewrite/IR-KERNEL.md` §7.3 (A3); `rewrite/RULINGS.md`
//! R1. Runs (ngspice-45): `{2*2mil}` = 4e-3 and `2*2mil` in a B source = 1.016e-4; `{1d3}` is an
//! error, `1d3` in a B source is 1000; `{1e3k}` = 1e6.
//!
//! Contents: `pub(super) enum Grammar`, `pub(super) struct Scan`,
//! `pub(super) fn scan(text: &str, grammar: Grammar) -> Option<Scan>`,
//! `pub(super) fn card(token: &str) -> Option<Scan>`,
//! `pub(super) fn spell(lit: ir::Literal, grammar: Grammar, out: &mut String)`

use crate::ir;

/// Which of ngspice's two number readers reads a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Grammar {
    /// `INPevaluate`: card values and B-source expressions.
    Inp,
    /// numparam's `fetchnumber`: braces, `.param`, `.func`, `.if`.
    Numparam,
}

/// A number read from the start of a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Scan {
    /// The literal, valid (`Literal::is_valid`).
    pub(super) literal: ir::Literal,
    /// Bytes taken: digits, point, exponent, scale and unit tail.
    pub(super) len: usize,
    /// Whether the digits were rounded to 18 significant ones (A3: the reader notes it).
    pub(super) rounded: bool,
}

/// Significant digits a literal's mantissa holds (`|mantissa| < 10^18`).
const DIGITS_MAX: usize = 18;

/// The number at the start of `text` under `grammar`, or `None` when `text` does not start with
/// one (no digit) or its value is not a finite double (`1e999`).
pub(super) fn scan(text: &str, grammar: Grammar) -> Option<Scan> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut digits = String::new();
    let mut frac = 0usize;
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        digits.push(bytes[at] as char);
        at += 1;
    }
    if at < bytes.len() && bytes[at] == b'.' {
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            digits.push(bytes[at] as char);
            frac += 1;
            at += 1;
        }
    }
    if digits.is_empty() {
        return None;
    }
    let (exponent, after) = exponent(bytes, at, grammar);
    at = after;
    let (scale, after) = scale(bytes, at, grammar);
    at = after;
    at = tail(bytes, at, grammar);
    let (literal, rounded) = literal(&digits, frac, exponent, scale)?;
    debug_assert!(literal.is_valid(), "scan builds valid literals");
    debug_assert!(at <= text.len(), "a scan takes bytes of the text");
    Some(Scan {
        literal,
        len: at,
        rounded,
    })
}

/// A card value's whole token read as `INPevaluate` reads it with `gobble` set: an optional
/// sign, the number, and the rest of the token ignored (`1k5` is 1000, `10v` is 10). `None` when
/// the token does not start with a number after its sign.
pub(super) fn card(token: &str) -> Option<Scan> {
    let (negative, rest) = match token.as_bytes().first() {
        Some(b'-') => (true, &token[1..]),
        Some(b'+') => (false, &token[1..]),
        _ => (false, token),
    };
    let mut s = scan(rest, Grammar::Inp)?;
    if negative && s.literal.mantissa != 0 {
        s.literal.mantissa = -s.literal.mantissa;
    }
    s.len = token.len();
    Some(s)
}

/// Appends `lit` as `grammar` spells it: the digits with `frac` of them after the point, then
/// `e<exp10>` (Exponent), the scale letter ngspice has for the power (Scale; `e<exp10>` for
/// a power it has no letter for: 10⁻², 10¹⁵), `mil` (Mil, `Inp` only; under `Numparam` `mil`
/// reads as milli, so the exact value is spelled in exponent form).
pub(super) fn spell(lit: ir::Literal, grammar: Grammar, out: &mut String) {
    assert!(lit.is_valid(), "spell: a valid literal");
    let start = out.len();
    if lit.form == ir::NumForm::Mil && grammar == Grammar::Numparam {
        // 25.4e-6 = 254e-7: the same exact decimal, in a form numparam reads.
        let mantissa = i128::from(lit.mantissa) * 254;
        digits_into(mantissa, u32::from(lit.frac), out);
        out.push_str("e-7");
        return;
    }
    digits_into(i128::from(lit.mantissa), u32::from(lit.frac), out);
    match lit.form {
        // an `Int` literal (A12, outside FEATURES: `Extra::IntLiteral`) is its digits, as Plain
        ir::NumForm::Plain | ir::NumForm::Int => {}
        ir::NumForm::Exponent => {
            out.push('e');
            out.push_str(&lit.exp10.to_string());
        }
        ir::NumForm::Scale => match letter(lit.exp10) {
            Some(l) => out.push_str(l),
            None => {
                out.push('e');
                out.push_str(&lit.exp10.to_string());
            }
        },
        ir::NumForm::Mil => out.push_str("mil"),
    }
    debug_assert!(out.len() > start, "a literal spells at least one digit");
}

// ---- private ----

/// The exponent at `at` (`e`, `E`; `d`, `D` under `Inp`), with its sign and digits: its value
/// (`None` for none) and where it ends. `INPevaluate` takes an exponent letter with no digits
/// as exponent 0; `sscanf` leaves such a letter to the tail.
fn exponent(bytes: &[u8], at: usize, grammar: Grammar) -> (Option<i32>, usize) {
    let letters: &[u8] = match grammar {
        Grammar::Inp => b"eEdD",
        Grammar::Numparam => b"eE",
    };
    if at >= bytes.len() || !letters.contains(&bytes[at]) {
        return (None, at);
    }
    let mut k = at + 1;
    let negative = match bytes.get(k) {
        Some(b'-') => {
            k += 1;
            true
        }
        Some(b'+') => {
            k += 1;
            false
        }
        _ => false,
    };
    let first = k;
    let mut value: i32 = 0;
    while k < bytes.len() && bytes[k].is_ascii_digit() {
        // Saturates: a huge exponent is no finite literal, refused by `literal`.
        value = value
            .saturating_mul(10)
            .saturating_add(i32::from(bytes[k] - b'0'));
        k += 1;
    }
    if k == first {
        return match grammar {
            Grammar::Inp => (Some(0), k),
            Grammar::Numparam => (None, at),
        };
    }
    (Some(if negative { -value } else { value }), k)
}

/// The scale at `at`: its power of ten (`Mil` as `Some(None)`), and where it ends. `INPevaluate`
/// and `parseunit` both read one letter, `meg` and (`Inp` only) `mil` by their first three.
fn scale(bytes: &[u8], at: usize, grammar: Grammar) -> (Option<Option<i16>>, usize) {
    let Some(&c) = bytes.get(at) else {
        return (None, at);
    };
    let next = |k: usize| bytes.get(at + k).map(u8::to_ascii_lowercase);
    let power = match c.to_ascii_lowercase() {
        b't' => 12,
        b'g' => 9,
        b'k' => 3,
        b'u' => -6,
        b'n' => -9,
        b'p' => -12,
        b'f' => -15,
        b'a' => -18,
        b'm' if next(1) == Some(b'e') && next(2) == Some(b'g') => return (Some(Some(6)), at + 3),
        b'm' if grammar == Grammar::Inp && next(1) == Some(b'i') && next(2) == Some(b'l') => {
            return (Some(None), at + 3);
        }
        b'm' => -3,
        _ => return (None, at),
    };
    (Some(Some(power)), at + 1)
}

/// Where the unit tail after `at` ends: `alfa` bytes under `Numparam` (letters, `_`, `[`, `]`);
/// letters and `_` under `Inp` inside an expression (a card value's whole token is [`card`]'s).
fn tail(bytes: &[u8], mut at: usize, grammar: Grammar) -> usize {
    let alfa = |c: u8| {
        c.is_ascii_alphabetic()
            || c == b'_'
            || (grammar == Grammar::Numparam && (c == b'[' || c == b']'))
    };
    while at < bytes.len() && alfa(bytes[at]) {
        at += 1;
    }
    at
}

/// The literal of `digits` (`frac` of them after the point), an exponent and a scale: the
/// digits as one integer with leading zeros dropped, rounded half to even to 18 significant
/// digits when longer; an exponent and a scale together fold into `Exponent` (A3).
fn literal(
    digits: &str,
    frac: usize,
    exponent: Option<i32>,
    scale: Option<Option<i16>>,
) -> Option<(ir::Literal, bool)> {
    let significant = digits.trim_start_matches('0');
    let mut frac = frac as i32;
    let mut power: i32 = 0;
    let (mut mantissa, rounded) = if significant.len() > DIGITS_MAX {
        power += (significant.len() - DIGITS_MAX) as i32;
        (round_digits(significant, DIGITS_MAX), true)
    } else {
        (significant.parse::<u64>().unwrap_or(0), false)
    };
    // a rounding carry may give 10^18: one digit more moves into the power
    if mantissa >= 1_000_000_000_000_000_000 {
        mantissa /= 10;
        power += 1;
    }
    // Rounded-off digits come out of the fraction first, so `frac` stays as written where it can.
    let shift = power.min(frac);
    frac -= shift;
    power -= shift;
    if mantissa == 0 {
        return Some((ir::Literal::ZERO, rounded));
    }
    let mut lit = ir::Literal {
        mantissa: i64::try_from(mantissa).ok()?,
        frac: 0,
        exp10: 0,
        form: ir::NumForm::Plain,
    };
    let exp = exponent.map(i64::from);
    let scale_power = scale.map(|s| s.map_or(0, i64::from));
    let total = exp.unwrap_or(0) + scale_power.unwrap_or(0) + i64::from(power);
    let mil = matches!(scale, Some(None));
    lit.form = match (exponent.is_some(), scale.is_some(), mil, power != 0) {
        (false, false, _, false) => ir::NumForm::Plain,
        (false, true, true, false) => ir::NumForm::Mil,
        (false, true, false, false) => ir::NumForm::Scale,
        _ => ir::NumForm::Exponent,
    };
    if mil && lit.form == ir::NumForm::Exponent {
        // `1e3mil`: 25.4e-6 folds into the digits (254 × 10^-7).
        lit.mantissa = lit.mantissa.checked_mul(254)?;
        if lit.mantissa.unsigned_abs() >= 1_000_000_000_000_000_000 {
            return None;
        }
        lit.exp10 = i16::try_from(total - 7).ok()?;
    } else {
        lit.exp10 = i16::try_from(total).ok()?;
    }
    lit.frac = u8::try_from(frac).ok().filter(|f| *f <= 18)?;
    lit.is_valid().then_some((lit, rounded))
}

/// The first `keep` digits of `digits` (no leading zero), rounded half to even on the rest.
fn round_digits(digits: &str, keep: usize) -> u64 {
    assert!(digits.len() > keep, "round_digits: something to round off");
    let head: u64 = digits[..keep].parse().expect("ASCII digits");
    let rest = &digits.as_bytes()[keep..];
    let up = match rest[0].cmp(&b'5') {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => rest[1..].iter().any(|&d| d != b'0') || head % 2 == 1,
    };
    head + u64::from(up)
}

/// The scale letter ngspice reads for `power`, if it has one.
fn letter(power: i16) -> Option<&'static str> {
    Some(match power {
        12 => "t",
        9 => "g",
        6 => "meg",
        3 => "k",
        -3 => "m",
        -6 => "u",
        -9 => "n",
        -12 => "p",
        -15 => "f",
        -18 => "a",
        _ => return None,
    })
}

/// Appends `mantissa` with `frac` digits after the point (`5`, 3 → `0.005`), its sign first.
fn digits_into(mantissa: i128, frac: u32, out: &mut String) {
    if mantissa < 0 {
        out.push('-');
    }
    let text = mantissa.unsigned_abs().to_string();
    let frac = frac as usize;
    if frac == 0 {
        out.push_str(&text);
        return;
    }
    let padded = format!("{text:0>width$}", width = frac + 1);
    let (int, dec) = padded.split_at(padded.len() - frac);
    out.push_str(int);
    out.push('.');
    out.push_str(dec);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(mantissa: i64, frac: u8, exp10: i16, form: ir::NumForm) -> ir::Literal {
        ir::Literal {
            mantissa,
            frac,
            exp10,
            form,
        }
    }

    fn read(text: &str, g: Grammar) -> (ir::Literal, usize) {
        let s = scan(text, g).expect("a number");
        (s.literal, s.len)
    }

    #[test]
    fn scale_letters_are_case_blind_and_m_is_milli() {
        // INPevaluate (inpeval.c), INV 1.01: `M` = `m` = 1e-3, `meg` = 1e6, `a` atto (1.02).
        use ir::NumForm::Scale;
        for g in [Grammar::Inp, Grammar::Numparam] {
            assert_eq!(read("100u", g), (lit(100, 0, -6, Scale), 4));
            assert_eq!(read("1M", g), (lit(1, 0, -3, Scale), 2));
            assert_eq!(read("1MEG", g), (lit(1, 0, 6, Scale), 4));
            assert_eq!(read("2.5k", g), (lit(25, 1, 3, Scale), 4));
            assert_eq!(read("1a", g), (lit(1, 0, -18, Scale), 2));
        }
    }

    #[test]
    fn digits_are_kept_as_written() {
        // R1, A3: `0.050000` keeps its six fraction digits; a point with no digits after it.
        use ir::NumForm::{Exponent, Plain};
        assert_eq!(read("0.050000", Grammar::Inp).0, lit(50000, 6, 0, Plain));
        assert_eq!(read("5.", Grammar::Numparam).0, lit(5, 0, 0, Plain));
        assert_eq!(read(".5", Grammar::Numparam).0, lit(5, 1, 0, Plain));
        assert_eq!(read("1.5e-3", Grammar::Inp).0, lit(15, 1, -3, Exponent));
        assert_eq!(read("0.0", Grammar::Inp).0, ir::Literal::ZERO);
    }

    #[test]
    fn mil_and_d_exponents_are_inpevaluate_only() {
        // Runs (ngspice-45): B source `2mil` = 5.08e-5, `{2mil}` = 2e-3; `1d3` = 1000 in a B
        // source and an error in braces (numparam's sscanf has no `d`).
        use ir::NumForm::{Exponent, Mil, Plain, Scale};
        assert_eq!(read("2mil", Grammar::Inp), (lit(2, 0, 0, Mil), 4));
        assert_eq!(read("2mil", Grammar::Numparam), (lit(2, 0, -3, Scale), 4));
        assert_eq!(read("1d3", Grammar::Inp), (lit(1, 0, 3, Exponent), 3));
        // numparam: `1` then the tail `d`; the `3` after it is no part of the number
        assert_eq!(read("1d3", Grammar::Numparam), (lit(1, 0, 0, Plain), 2));
    }

    #[test]
    fn a_scale_after_an_exponent_folds_into_the_exponent() {
        // INV 1.06, A3: `1e3k` = 1e6, form Exponent.
        let (l, n) = read("1e3k", Grammar::Numparam);
        assert_eq!((l, n), (lit(1, 0, 6, ir::NumForm::Exponent), 4));
        assert_eq!(l.value(), 1e6);
    }

    #[test]
    fn the_unit_tail_is_taken_and_ignored() {
        // INV 1.04: `1kHz`, `10v`; a digit ends the tail (1.05: `1k5` in an expression is an
        // error on both evaluators, run).
        assert_eq!(read("1kHz", Grammar::Numparam).1, 4);
        assert_eq!(read("10v*2", Grammar::Inp).1, 3);
        assert_eq!(read("1k5", Grammar::Inp).1, 2);
        assert_eq!(card("1k5").expect("a card number").literal.value(), 1e3);
        assert_eq!(card("-2.5").expect("signed").literal.mantissa, -25);
    }

    #[test]
    fn more_than_eighteen_digits_round_half_to_even_with_a_flag() {
        // A3: a literal with more than 18 significant digits is rounded to 18, noted.
        let s = scan("1234567890123456789012", Grammar::Inp).expect("a number");
        assert!(s.rounded);
        assert_eq!(s.literal.mantissa, 123456789012345679);
        assert_eq!(s.literal.value(), 1234567890123456789012.0_f64);
        assert!(!scan("12", Grammar::Inp).expect("a number").rounded);
    }

    #[test]
    fn no_digit_or_no_finite_value_is_no_number() {
        assert!(scan("e3", Grammar::Inp).is_none());
        assert!(scan(".", Grammar::Inp).is_none());
        assert!(scan("1e999", Grammar::Inp).is_none());
    }

    #[test]
    fn spelling_reads_back_to_the_same_literal() {
        // Round trip: spell then scan gives the literal field by field (L5 on numbers).
        use ir::NumForm::{Exponent, Mil, Plain, Scale};
        let cases = [
            lit(100, 0, -6, Scale),
            lit(50000, 6, 0, Plain),
            lit(15, 1, -3, Exponent),
            lit(1, 0, 6, Scale),
            lit(2, 0, 0, Mil),
            lit(5, 3, 0, Plain),
            lit(7, 0, 12, Scale),
            ir::Literal::ZERO,
        ];
        for l in cases {
            let mut s = String::new();
            spell(l, Grammar::Inp, &mut s);
            assert_eq!(scan(&s, Grammar::Inp).expect(&s).literal, l, "{s}");
        }
        let mut s = String::new();
        spell(lit(5, 3, 0, Plain), Grammar::Numparam, &mut s);
        assert_eq!(s, "0.005");
    }

    #[test]
    fn spelling_a_power_ngspice_has_no_letter_for_uses_an_exponent() {
        // ngspice's letters are t g meg k m u n p f a (inpeval.c); centi and peta are not.
        let mut s = String::new();
        spell(lit(3, 0, -2, ir::NumForm::Scale), Grammar::Inp, &mut s);
        assert_eq!(s, "3e-2");
        s.clear();
        spell(lit(2, 0, 0, ir::NumForm::Mil), Grammar::Numparam, &mut s);
        assert_eq!(s, "508e-7");
        assert_eq!(
            scan(&s, Grammar::Numparam).expect("e-form").literal.value(),
            5.08e-5
        );
    }
}
