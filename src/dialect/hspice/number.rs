//! HSPICE number text ↔ `ir::Literal` (ruling R1: the digits as written, no floating
//! arithmetic). One grammar, in a card value and in an expression alike: digits, `.`, digits, an
//! exponent `e`/`E`/`d`/`D` with its sign, or one scale (`t g k m u n p f a`, `meg` and `x` for
//! 1e6, `mil` for 25.4e-6), then a unit tail of letters that is ignored (`10pf` = 1e-11, `1kHz` =
//! 1000); a card value ignores the rest of its token too (`1k5` = 1000). An exponent and a scale
//! together are no number (`1e3k`: ESPice refuses the card, and the VAMS conformance suite's
//! `invalid/hspice_exponent_scale.sp` expects it). `a` is atto (1e-18), unlike Xyce.
//!
//! In: the text at a number's first byte (no sign: a sign is the expression parser's, or
//! [`card`]'s). Out: the literal and the bytes it took, scale and tail included. The writer
//! spells a literal back with its digits and form.
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §2 (branch `differences`); settle `num_scale.sp`,
//! `num_exp_scale.sp` (ESPice 0.3.0 in HSPICE mode: ESPice is not HSPICE); `rewrite/IR-KERNEL.md`
//! §7.3 (A3); `rewrite/RULINGS.md` R1.
//!
//! Contents: `pub(super) struct Scan`, `pub(super) enum Fit` (`negate`, `build`),
//! `pub(super) struct Long` (`negate`), `pub(super) fn scan(text: &str) -> Option<Scan>`,
//! `pub(super) fn card(token: &str) -> Option<Scan>`, `pub(super) fn whole(&str) -> Option<Scan>`,
//! `pub(super) fn spell(d: &ir::Design, lit: ir::Literal, out: &mut String)`

use crate::ir;

/// A number read from the start of a text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Scan {
    /// The literal, valid (`Literal::is_valid`); for a [`Fit::Long`] the digits rounded half to
    /// even to 18, for the reader's own checks only (the design gets the long digits).
    pub(super) literal: ir::Literal,
    /// Bytes taken: digits, point, exponent, scale and unit tail.
    pub(super) len: usize,
    /// How the digits fit a literal's mantissa.
    pub(super) fit: Fit,
}

/// How a number's digits fit a literal's 18-digit mantissa.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Fit {
    /// They fit: `literal` is the number as written.
    Exact,
    /// More digits than the mantissa holds (A18, R37): the design holds them as written.
    Long(Box<Long>),
}

/// A long number's parts for `Builder::long_literal` (A18).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Long {
    /// `[-]digits[.digits]` as written.
    pub(super) significand: String,
    /// The exponent plus the scale letter's power.
    pub(super) exp10: i16,
    /// `Plain`, `Exponent` or `Scale`.
    pub(super) written: ir::NumForm,
}

/// Significant digits a literal's mantissa holds (`|mantissa| < 10^18`).
const DIGITS_MAX: usize = 18;

impl Long {
    /// The negated number: the sign before the significand flips.
    pub(super) fn negate(&mut self) {
        self.significand = match self.significand.strip_prefix('-') {
            Some(rest) => rest.to_string(),
            None => format!("-{}", self.significand),
        };
    }
}

impl Fit {
    /// The fit of the negated number (`literal`'s mantissa is negated by the caller).
    pub(super) fn negate(&mut self) {
        if let Fit::Long(l) = self {
            l.negate();
        }
    }

    /// Appends the number to `b`: `lit` as it is, or the long digits (A18).
    pub(super) fn build(&self, b: &mut ir::Builder<'_>, lit: ir::Literal) -> ir::ExprId {
        match self {
            Fit::Long(l) => b.long_literal(&l.significand, l.exp10, l.written),
            Fit::Exact => b.literal(lit),
        }
    }
}

/// The number at the start of `text`, or `None` when `text` does not start with
/// one (no digit), its value is not a finite double (`1e999`), or it has both an exponent and a
/// scale (`1e3k`).
pub(super) fn scan(text: &str) -> Option<Scan> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut digits = String::new();
    let mut frac = 0usize;
    // `at` after the significand's last byte: the long form's text as written
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
    let significand = &text[..at];
    let (exponent, after) = exponent(bytes, at);
    at = after;
    let (scale, after) = scale(bytes, at);
    if exponent.is_some() && scale.is_some() {
        return None;
    }
    at = tail(bytes, after);
    let (literal, rounded) = literal(&digits, frac, exponent, scale)?;
    debug_assert!(literal.is_valid(), "scan builds valid literals");
    debug_assert!(at <= text.len(), "a scan takes bytes of the text");
    let long = ir::literal::exceeds_mantissa(ir::literal::significant_digits(significand));
    debug_assert_eq!(long, rounded, "one digit bound (the kernel's)");
    let written = match (exponent, scale) {
        (None, None) => Some(ir::NumForm::Plain),
        (None, Some(Some(_))) => Some(ir::NumForm::Scale),
        (Some(_), Some(Some(_)) | None) => Some(ir::NumForm::Exponent),
        (_, Some(None)) => None, // mil
    };
    let fit = match written {
        _ if !long => Fit::Exact,
        // a long `mil` folds 254 into 18 rounded digits: no literal holds it (`literal` refused it)
        None => unreachable!("HSPICE read: a long mil number has no literal"),
        Some(written) => {
            let power = i64::from(exponent.unwrap_or(0)) + scale.flatten().map_or(0, i64::from);
            Fit::Long(Box::new(Long {
                significand: significand.to_string(),
                exp10: i16::try_from(power).ok()?,
                written,
            }))
        }
    };
    Some(Scan {
        literal,
        len: at,
        fit,
    })
}

/// A card value's whole token: an optional sign, the number, and the rest of the token ignored
/// (`1k5` is 1000, `10v` is 10: `num_scale.sp`). `None` when the token does not start with a
/// number after its sign.
pub(super) fn card(token: &str) -> Option<Scan> {
    let (negative, rest) = match token.as_bytes().first() {
        Some(b'-') => (true, &token[1..]),
        Some(b'+') => (false, &token[1..]),
        _ => (false, token),
    };
    let mut s = scan(rest)?;
    if negative && s.literal.mantissa != 0 {
        s.literal.mantissa = -s.literal.mantissa;
        s.fit.negate();
    }
    s.len = token.len();
    Some(s)
}

/// A token that is one card number and nothing after it (R37: a long one's digits kept), or
/// `None`.
pub(super) fn whole(token: &str) -> Option<Scan> {
    let digits = token.strip_prefix(['-', '+']).unwrap_or(token);
    let s = scan(digits)?;
    (s.len == digits.len()).then(|| card(token)).flatten()
}

/// Appends `lit`: the digits with `frac` of them after the point, then `e<exp10>` (Exponent),
/// the scale letter HSPICE has for the power (Scale; `e<exp10>` for a power it has no letter
/// for: 10⁻², 10¹⁵), `mil` (Mil). A `Long` literal (A18) is its significand as written, then
/// its power in its written form.
pub(super) fn spell(d: &ir::Design, lit: ir::Literal, out: &mut String) {
    let start = out.len();
    if let Some((significand, written)) = d.long_digits(lit) {
        out.push_str(significand);
        power(lit.exp10, written, out);
        return;
    }
    assert!(lit.is_valid(), "spell: a valid literal");
    digits_into(i128::from(lit.mantissa), u32::from(lit.frac), out);
    match lit.form {
        ir::NumForm::Mil => out.push_str("mil"),
        // an `Int` literal (A12, outside FEATURES: `Extra::IntLiteral`) is its digits, as Plain
        ir::NumForm::Int => {}
        form => power(lit.exp10, form, out),
    }
    debug_assert!(out.len() > start, "a literal spells at least one digit");
}

// ---- private ----

/// Appends the power `exp10` written in `form`: nothing (Plain), `e<exp10>` (Exponent), the
/// scale letter (Scale; `e<exp10>` for a power HSPICE has no letter for).
fn power(exp10: i16, form: ir::NumForm, out: &mut String) {
    match (form, letter(exp10)) {
        (ir::NumForm::Plain, _) => debug_assert_eq!(exp10, 0, "Plain has no power"),
        (ir::NumForm::Scale, Some(l)) => out.push_str(l),
        _ => {
            out.push('e');
            out.push_str(&exp10.to_string());
        }
    }
}

/// The exponent at `at` (`e`, `E`, `d`, `D`: `1d3` = 1000, `num_scale.sp`), with its sign and
/// digits: its value (`None` for none) and where it ends. A letter with no digit after it is
/// no exponent: it is the unit tail (`1e` = 1; ponytail: unsettled on ESPice, the ngspice lead).
fn exponent(bytes: &[u8], at: usize) -> (Option<i32>, usize) {
    if at >= bytes.len() || !b"eEdD".contains(&bytes[at]) {
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
        return (None, at);
    }
    (Some(if negative { -value } else { value }), k)
}

/// The scale at `at`: its power of ten (`Mil` as `Some(None)`), and where it ends: one letter,
/// `meg` and `mil` by their first three (`num_scale.sp`: `1M` = 1e-3, `1MEG` = `1X` = 1e6,
/// `1MIL` = 2.54e-5, `1A` = 1e-18).
fn scale(bytes: &[u8], at: usize) -> (Option<Option<i16>>, usize) {
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
        b'x' => 6,
        b'm' if next(1) == Some(b'e') && next(2) == Some(b'g') => return (Some(Some(6)), at + 3),
        b'm' if next(1) == Some(b'i') && next(2) == Some(b'l') => {
            return (Some(None), at + 3);
        }
        b'm' => -3,
        _ => return (None, at),
    };
    (Some(Some(power)), at + 1)
}

/// Where the unit tail after `at` ends: letters and `_` (a card value's whole token is
/// [`card`]'s).
fn tail(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && (bytes[at].is_ascii_alphabetic() || bytes[at] == b'_') {
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
    // more significant digits than the mantissa holds: rounded half to even on the digits (no
    // float), the reader's own approximation; the design gets the digits (`Fit::Long`, A18)
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
    // more fraction digits than a literal holds (18): the whole fraction moves into the power,
    // exactly (`0.0027500000000000003` is 27500000000000003 × 10^-19, form Exponent)
    if frac > 18 {
        power -= frac;
        frac = 0;
    }
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

/// The scale letter HSPICE reads for `power`, if it has one.
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

    fn read(text: &str) -> (ir::Literal, usize) {
        let s = scan(text).expect("a number");
        (s.literal, s.len)
    }

    #[test]
    fn scale_letters_are_case_blind_m_is_milli_x_is_mega_a_is_atto() {
        // Settle num_scale.sp (ESPice only): `1M` = 1e-3, `1MEG` = `1X` = 1e6, `1A` = 1e-18.
        use ir::NumForm::Scale;
        assert_eq!(read("100u"), (lit(100, 0, -6, Scale), 4));
        assert_eq!(read("1M"), (lit(1, 0, -3, Scale), 2));
        assert_eq!(read("1MEG"), (lit(1, 0, 6, Scale), 4));
        assert_eq!(read("1X"), (lit(1, 0, 6, Scale), 2));
        assert_eq!(read("2.5k"), (lit(25, 1, 3, Scale), 4));
        assert_eq!(read("1a"), (lit(1, 0, -18, Scale), 2));
    }

    #[test]
    fn digits_are_kept_as_written() {
        // R1, A3: `0.050000` keeps its six fraction digits; a point with no digits after it.
        use ir::NumForm::{Exponent, Plain};
        assert_eq!(read("0.050000").0, lit(50000, 6, 0, Plain));
        assert_eq!(read("5.").0, lit(5, 0, 0, Plain));
        assert_eq!(read(".5").0, lit(5, 1, 0, Plain));
        assert_eq!(read("1.5e-3").0, lit(15, 1, -3, Exponent));
        assert_eq!(read("0.0").0, ir::Literal::ZERO);
    }

    #[test]
    fn mil_and_d_exponents() {
        // Settle num_scale.sp (ESPice only): `1MIL` = 2.54e-5, `1d3` = 1000, `2.5D-3`.
        use ir::NumForm::{Exponent, Mil};
        assert_eq!(read("2mil"), (lit(2, 0, 0, Mil), 4));
        assert_eq!(read("1d3"), (lit(1, 0, 3, Exponent), 3));
        assert_eq!(read("2.5D-3"), (lit(25, 1, -3, Exponent), 6));
    }

    #[test]
    fn an_exponent_and_a_scale_together_are_no_number() {
        // Settle num_exp_scale.sp (ESPice refuses `1e3k`; the suite's invalid/ deck agrees).
        assert!(scan("1e3k").is_none());
        assert!(card("1e-6u").is_none());
    }

    #[test]
    fn the_unit_tail_is_taken_and_ignored() {
        // Settle num_scale.sp: `10pf` = 1e-11, `1kHz` = 1000, `1k5` (a card) = 1000.
        assert_eq!(read("1kHz").1, 4);
        assert_eq!(read("10v*2").1, 3);
        assert_eq!(read("1k5").1, 2);
        assert_eq!(card("10pf").expect("a card number").literal.value(), 1e-11);
        assert_eq!(card("1k5").expect("a card number").literal.value(), 1e3);
        assert_eq!(card("-2.5").expect("signed").literal.mantissa, -25);
    }

    #[test]
    fn more_than_eighteen_digits_are_kept_as_written() {
        // A18, R37: the significand as written and its power, the kernel's digit bound; the
        // literal beside it is rounded half to even (the reader's own checks only).
        let long = |significand: &str, exp10, written| {
            Fit::Long(Box::new(Long {
                significand: significand.to_string(),
                exp10,
                written,
            }))
        };
        let s = scan("1234567890123456789012").expect("a number");
        assert_eq!(s.fit, long("1234567890123456789012", 0, ir::NumForm::Plain));
        assert_eq!(s.literal.mantissa, 123456789012345679);
        let s = card("-0.0012345678901234567890e6").expect("a card number");
        assert_eq!(
            s.fit,
            long("-0.0012345678901234567890", 6, ir::NumForm::Exponent)
        );
        let s = scan("1.234567890123456789u").expect("a number");
        assert_eq!(s.fit, long("1.234567890123456789", -6, ir::NumForm::Scale));
        // 18 digits fit; a long `mil` is no number (no literal holds 254 times 18 digits)
        assert_eq!(scan("123456789012345678").expect("18").fit, Fit::Exact);
        assert!(scan("1234567890123456789mil").is_none());
    }

    #[test]
    fn a_long_literal_spells_its_digits_and_reads_back_to_them() {
        // Round trip through the design (A18): spell, then scan gives the same parts.
        let mut design = ir::Design::default();
        let cases = [
            "1234567890123456789012",
            "-0.0012345678901234567890e6",
            "1.234567890123456789u",
            "1.2345678901234567890e-2",
        ];
        let mut built = Vec::new();
        for text in cases {
            let s = card(text).expect(text);
            built.push((text, s.fit.build(&mut design.builder(), s.literal)));
        }
        for (text, e) in built {
            let ir::Expr::Num(n) = design.expr(e) else {
                panic!("a literal")
            };
            let mut out = String::new();
            spell(&design, design.num(n), &mut out);
            assert_eq!(out, text);
        }
    }

    #[test]
    fn no_digit_or_no_finite_value_is_no_number() {
        assert!(scan("e3").is_none());
        assert!(scan(".").is_none());
        assert!(scan("1e999").is_none());
    }

    #[test]
    fn a_fraction_longer_than_a_literal_holds_moves_into_the_exponent_exactly() {
        // sky130 BSIM4 cards (ng-models' census: 95 of 6616): 19 digits after the point.
        let s = card("0.0027500000000000003").expect("a number");
        assert_eq!(
            s.literal,
            lit(27500000000000003, 0, -19, ir::NumForm::Exponent)
        );
        assert_eq!(s.literal.value(), 0.0027500000000000003);
        let mut out = String::new();
        spell(&ir::Design::default(), s.literal, &mut out);
        assert_eq!(scan(&out).expect(&out).literal, s.literal);
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
            spell(&ir::Design::default(), l, &mut s);
            assert_eq!(scan(&s).expect(&s).literal, l, "{s}");
        }
        let mut s = String::new();
        spell(&ir::Design::default(), lit(5, 3, 0, Plain), &mut s);
        assert_eq!(s, "0.005");
    }

    #[test]
    fn spelling_a_power_hspice_has_no_letter_for_uses_an_exponent() {
        // HSPICE's letters are t g meg k m u n p f a (num_scale.sp); centi and peta are not.
        let mut s = String::new();
        spell(
            &ir::Design::default(),
            lit(3, 0, -2, ir::NumForm::Scale),
            &mut s,
        );
        assert_eq!(s, "3e-2");
    }
}
