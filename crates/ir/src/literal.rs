//! Decimal literals (Addendum A3, A12 K1): a number as written, `mantissa × 10^(exp10 − frac)`
//! (`Mil`: that times 25.4e-6; `Int`: a whole number of integer type), its correctly rounded
//! value, the shortest literal of a double, and exact decimal comparison. Split from `expr.rs`;
//! re-exported there (`expr::Literal`).
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.3 (A3); `rewrite/AMEND-A12.md` K1; `rewrite/RULINGS.md` R1, R16.
//!
//! Contents: `pub struct Literal`, `pub enum NumForm` (`ALL`, `COUNT`), `Literal::ZERO`,
//! `Literal::value(self) -> f64`, `Literal::from_f64(f64) -> Literal`,
//! `Literal::eq_value(self, Literal) -> bool`, `Literal::is_valid(self) -> bool`; crate:
//! `Literal::shape_ok(self) -> bool`, `Literal::finite(self) -> bool`; `pub fn significant_digits(&str) -> usize`,
//! `pub const fn exceeds_mantissa(usize) -> bool`, `pub enum Decimal`,
//! `pub fn decimal(&str, i32, NumForm) -> Option<Decimal<'_>>`; crate: `long_ok`, `long_violation`,
//! `long_shape_ok`, `long_finite`,
//! `long_value`, `long_normal`, `short_normal`

/// A decimal literal, as written: `mantissa × 10^(exp10 − frac)` (`Mil`: that times 25.4e-6).
/// The IR states the number; the double is each simulator's own arithmetic on these digits.
///
/// Invariants (asserted by `Builder::literal`, checked by `check`): `|mantissa| < 10¹⁸`;
/// `frac <= 18`; `Plain` and `Mil` have `exp10 == 0`, `Int` has `frac == 0` and `exp10 == 0`; a `Scale` has an `exp10` some scale letter
/// states (−18, −15, −12, −9, −6, −3, −2, 3, 6, 9, 12, 15); a zero is [`Literal::ZERO`] or the
/// Int zero; `value()` is finite. Equality (`==`) is field by field (law L5): `1k` and
/// `1e3` differ; [`Literal::eq_value`] compares meanings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Literal {
    /// Every digit as one integer, sign included (`-0.050000`: −50000).
    pub mantissa: i64,
    /// Digits after the decimal point, trailing zeros counted (`0.050000`: 6).
    pub frac: u8,
    /// The exponent, or the scale suffix's power of ten (`100u`: −6).
    pub exp10: i16,
    /// How the source wrote the power of ten.
    pub form: NumForm,
}

/// How a literal wrote its power of ten. A writer keeps the form where its dialect has it
/// (its own letter for a `Scale`), else spells `e<exp10>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NumForm {
    /// No exponent and no suffix (`0.05`); `exp10` is 0.
    Plain,
    /// An exponent (`1e-4`, `2.5E3`); also a suffix combined with an exponent (`1e3k`, folded by
    /// the reader).
    Exponent,
    /// A scale suffix (`100u`, `1meg`); `exp10` is the suffix's power of ten.
    Scale,
    /// The `mil` suffix: the digits times 25.4e-6 m; `exp10` is 0.
    Mil,
    /// A whole number of integer type (A12 K1: VACASK's typed arithmetic): `frac` and `exp10`
    /// are 0. Every other form is real.
    Int,
    /// More significant digits than the mantissa holds (A18, R37): the significand as written
    /// lives in the design's long-literal pool; `mantissa` is its index there, `exp10` the power
    /// of ten written or implied by a scale letter, and `frac` the form that power was written
    /// in (`Plain`, `Exponent` or `Scale`, as a `NumForm` index). Real. Only the design can read
    /// it: `Design::literal_value`, `Design::literal_eq`, `Design::long_digits`.
    Long,
}

/// A number as written, read by the one bound (A18): a short literal, or a `Long` one's parts
/// (`Builder::long_literal` takes them as they are).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decimal<'s> {
    /// Fits the mantissa.
    Short(Literal),
    /// More significant digits than the mantissa holds: the significand as written, its power
    /// and the form that power was written in.
    Long(&'s str, i16, NumForm),
}

/// The powers of ten a scale letter states (A3): a `Scale` literal's `exp10` is one of these.
const SCALES: [i16; 12] = [-18, -15, -12, -9, -6, -3, -2, 3, 6, 9, 12, 15];
/// The most significant digits a short literal's mantissa holds (A18's one bound).
const MAX_DIGITS: usize = 18;
/// `|mantissa|` is below this (10^18): at most `MAX_DIGITS` digits.
const MANTISSA_BOUND: u64 = 1_000_000_000_000_000_000;
const _: () = assert!(MANTISSA_BOUND == 10u64.pow(MAX_DIGITS as u32));

const _: () = assert!(size_of::<Literal>() == 16);

impl NumForm {
    /// Every form, in declaration order.
    pub const ALL: [NumForm; NumForm::Long as usize + 1] = [
        NumForm::Plain,
        NumForm::Exponent,
        NumForm::Scale,
        NumForm::Mil,
        NumForm::Int,
        NumForm::Long,
    ];
    /// Form count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Literal {
    /// Zero: the only literal whose mantissa is 0.
    pub const ZERO: Literal = Literal {
        mantissa: 0,
        frac: 0,
        exp10: 0,
        form: NumForm::Plain,
    };

    /// The correctly rounded double of the exact decimal value (round half to even), what the
    /// reference evaluator and the passes compute with. Not any simulator's arithmetic.
    ///
    /// # Panics
    /// On a `Long` literal, whose digits only the design holds: `Design::literal_value`.
    pub fn value(self) -> f64 {
        assert!(
            self.form != NumForm::Long,
            "a Long literal's value is Design::literal_value"
        );
        let (m, k) = self.exact();
        // Rust's decimal parser rounds correctly (round half to even), the IR's reference rule
        let v: f64 = format!("{m}e{k}")
            .parse()
            .expect("a decimal literal parses");
        debug_assert!(!v.is_nan(), "a decimal is never NaN");
        v
    }

    /// The shortest decimal that reads back to `value` under [`Literal::value`], in engineering
    /// form: a `Scale` when the power of ten is a multiple of 3 that a scale letter states
    /// (`1e-4` → mantissa 100, `Scale`, exp10 −6), `Plain` for powers 0, `Exponent` with an exp10
    /// that is a multiple of 3 otherwise; zero is [`Literal::ZERO`]. Asserts `value` is finite
    /// (the NaN gate, C9). For built numbers only: a reader passes the digits it read.
    pub fn from_f64(value: f64) -> Literal {
        assert!(value.is_finite(), "a built number is finite");
        if value == 0.0 {
            return Literal::ZERO;
        }
        // shortest round-trip digits: `d.ddde±x`
        let sci = format!("{:e}", value.abs());
        let (digits, x) = sci.split_once('e').expect("`{:e}` has an exponent");
        let x: i32 = x.parse().expect("`{:e}`'s exponent is an integer");
        let digits: String = digits.chars().filter(|c| *c != '.').collect();
        let n = digits.len() as i32;
        let m: i64 = digits.parse().expect("at most 17 significant digits");
        // value = m × 10^p; engineering exponent e3 = the multiple of 3 at or below x
        let p = x - (n - 1);
        let e3 = x.div_euclid(3) * 3;
        let (mantissa, frac) = if p >= e3 {
            (m * 10i64.pow((p - e3) as u32), 0)
        } else {
            (m, (e3 - p) as u8)
        };
        assert!(
            mantissa.unsigned_abs() < MANTISSA_BOUND,
            "17 digits and two zeros stay under 10^18"
        );
        let form = if e3 == 0 {
            NumForm::Plain
        } else if SCALES.contains(&(e3 as i16)) {
            NumForm::Scale
        } else {
            NumForm::Exponent
        };
        let sign = if value < 0.0 { -1 } else { 1 };
        Literal {
            mantissa: sign * mantissa,
            frac,
            exp10: e3 as i16,
            form,
        }
    }

    /// Whether the two literals state the same exact decimal number (`1k` and `1e3`; `Mil`
    /// compared after its factor). Exact: no rounding to a double.
    ///
    /// # Panics
    /// On a `Long` literal: `Design::literal_eq` compares those.
    pub fn eq_value(self, other: Literal) -> bool {
        assert!(
            self.form != NumForm::Long && other.form != NumForm::Long,
            "a Long literal compares through Design::literal_eq"
        );
        // an Int and a real literal of one value mean different things (A12 K1)
        let int = |l: Literal| l.form == NumForm::Int;
        int(self) == int(other) && self.normal() == other.normal()
    }

    /// Whether the literal meets every invariant on [`Literal`]. False for a `Long` literal,
    /// whose digits only the design holds (it is built with `Builder::long_literal`).
    pub fn is_valid(self) -> bool {
        self.shape_ok() && self.finite()
    }

    /// Whether the value is finite, from the digit count and the power where that decides it
    /// (the largest double is about 1.8e308), and by `value()` only at the boundary.
    pub(crate) fn finite(self) -> bool {
        assert!(self.form != NumForm::Long, "a short literal");
        let (m, k) = self.exact();
        if m == 0 {
            return true;
        }
        let digits = m.unsigned_abs().ilog10() as i32 + 1;
        match digits + k {
            ..=308 => true,
            310.. => false,
            309 => self.value().is_finite(),
        }
    }

    /// Whether the literal meets every invariant but finiteness (`check` reports the two
    /// separately: `LiteralShape`, `NonFinite`).
    pub(crate) fn shape_ok(self) -> bool {
        let m = self.mantissa.unsigned_abs();
        let digits = if m == 0 { 0 } else { m.ilog10() as usize + 1 };
        !exceeds_mantissa(digits)
            && self.frac <= 18
            && match self.form {
                NumForm::Plain | NumForm::Mil => self.exp10 == 0,
                NumForm::Scale => SCALES.contains(&self.exp10),
                NumForm::Exponent => true,
                NumForm::Int => self.exp10 == 0 && self.frac == 0,
                NumForm::Long => false, // the design's: `check` reads its digits
            }
            && (self.mantissa != 0 || self == Literal::ZERO || self.form == NumForm::Int)
    }
}

/// How many significant digits a written significand has: its digits after leading zeros,
/// trailing zeros as written counted (the count a short `mantissa` holds): `0.000123` has 3,
/// `1.2300` has 5. Sign and point are not digits.
pub fn significant_digits(significand: &str) -> usize {
    significand
        .bytes()
        .filter(u8::is_ascii_digit)
        .skip_while(|&c| c == b'0')
        .count()
}

/// The literal a reader's number token states: `significand` as written (`[-]digits[.digits]`,
/// a point with no digit on one side allowed), `exp10` the power written in an exponent plus
/// the one a scale letter implies, `form` how it was written. A significand with more
/// significant digits than the mantissa holds is `Long` (its digits kept as written); a short
/// one with more than 18 digits after the point moves them into the power, exactly, as an
/// `Exponent`; a real zero is `Literal::ZERO`. `None` when no literal states it: a significand not in
/// that shape, a power outside `i16`, a form the power does not fit (`Plain` with a power), a
/// value that is not finite. The arithmetic every reader shares (the dialects' audit, row 7):
/// a dialect splits its token and folds its own letters into `exp10`; `mil` with an exponent,
/// which no `NumForm` states, is the dialect's.
pub fn decimal(significand: &str, exp10: i32, form: NumForm) -> Option<Decimal<'_>> {
    let exp10 = i16::try_from(exp10).ok()?;
    let body = significand.strip_prefix('-').unwrap_or(significand);
    let (int, dec) = body.split_once('.').unwrap_or((body, ""));
    let digit = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    if int.len() + dec.len() == 0 || !digit(int) || !digit(dec) {
        return None;
    }
    if exceeds_mantissa(significant_digits(significand)) {
        let long = long_ok(significand, exp10, form);
        return long.then_some(Decimal::Long(significand, exp10, form));
    }
    // at most 18 significant digits: the leading zeros add nothing, so this cannot overflow
    let digits = int.bytes().chain(dec.bytes()).try_fold(0u64, |n, c| {
        n.checked_mul(10)?.checked_add(u64::from(c - b'0'))
    })?;
    assert!(digits < MANTISSA_BOUND, "at most 18 significant digits");
    // the one zero of each type: an integer `0i`, else `Literal::ZERO` (`-0.00` too)
    if digits == 0 && form != NumForm::Int {
        return Some(Decimal::Short(Literal::ZERO));
    }
    let mantissa = i64::try_from(digits).ok()?;
    let mut l = Literal {
        mantissa: if body.len() < significand.len() {
            -mantissa
        } else {
            mantissa
        },
        frac: 0,
        exp10,
        form,
    };
    match u8::try_from(dec.len())
        .ok()
        .filter(|&f| f <= MAX_DIGITS as u8)
    {
        Some(frac) => l.frac = frac,
        // `Mil`'s factor has no `Exponent` form to move into
        None if form == NumForm::Mil => return None,
        None => {
            l.exp10 = i16::try_from(i32::from(exp10) - dec.len() as i32).ok()?;
            l.form = NumForm::Exponent;
        }
    }
    l.is_valid().then_some(Decimal::Short(l))
}

/// Whether `digits` significant digits exceed what a short literal's mantissa holds (18): the one
/// bound between a short literal and a `Long` one (A18).
pub const fn exceeds_mantissa(digits: usize) -> bool {
    digits > MAX_DIGITS
}

/// Whether a `Long` literal's parts meet its invariants: the significand is `[-]digits[.digits]`
/// with more significant digits than the mantissa holds; `written` is `Plain` (`exp10 == 0`),
/// `Exponent`, or `Scale` (a letter's power); the value is finite. `check` reports a failure as
/// `LiteralShape`, a non-finite value as `NonFinite`; this is both.
pub(crate) fn long_ok(significand: &str, exp10: i16, written: NumForm) -> bool {
    long_shape_ok(significand, exp10, written) && long_finite(significand, exp10)
}

/// A `Long` row's fault against the long pool `long` (A18): `LiteralShape` when its index,
/// written form or digits are out of their invariants, `NonFinite` when its value is not
/// finite, `None` when it is valid. For `check`, on any table contents.
pub(crate) fn long_violation(long: &[Box<str>], l: Literal) -> Option<crate::check::Violation> {
    use crate::check::Violation;
    let written = NumForm::ALL.get(l.frac as usize).copied();
    let text = usize::try_from(l.mantissa).ok().and_then(|k| long.get(k));
    match (text, written) {
        (Some(t), Some(w)) if long_shape_ok(t, l.exp10, w) => {
            (!long_finite(t, l.exp10)).then_some(Violation::NonFinite)
        }
        _ => Some(Violation::LiteralShape),
    }
}

/// `long_ok` without finiteness.
pub(crate) fn long_shape_ok(significand: &str, exp10: i16, written: NumForm) -> bool {
    let body = significand.strip_prefix('-').unwrap_or(significand);
    let (int, dec) = body.split_once('.').unwrap_or((body, ""));
    let digits = |s: &str| s.bytes().all(|c| c.is_ascii_digit());
    let form = match written {
        NumForm::Plain => exp10 == 0,
        NumForm::Exponent => true,
        NumForm::Scale => SCALES.contains(&exp10),
        NumForm::Mil | NumForm::Int | NumForm::Long => false,
    };
    form && digits(int) && digits(dec) && exceeds_mantissa(significant_digits(significand))
}

/// Whether a long literal's value is finite, from its digit count and power as the short rule.
pub(crate) fn long_finite(significand: &str, exp10: i16) -> bool {
    let (_, d, p) = long_normal(significand, exp10);
    match d.len() as i32 + p {
        ..=308 => true,
        310.. => false,
        309 => long_value(significand, exp10).is_finite(),
    }
}

/// The correctly rounded double of a long literal's digits (Rust's decimal parser, the IR's
/// reference rule, as `Literal::value`).
pub(crate) fn long_value(significand: &str, exp10: i16) -> f64 {
    let (negative, d, p) = long_normal(significand, exp10);
    let sign = if negative { "-" } else { "" };
    format!("{sign}{d}e{p}")
        .parse()
        .expect("a decimal literal parses")
}

/// A long literal's exact value as (negative, digits with no leading or trailing zero, power):
/// equal decimals give equal triples, as `normal` does for the short forms.
pub(crate) fn long_normal(significand: &str, exp10: i16) -> (bool, String, i32) {
    let (negative, body) = match significand.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, significand),
    };
    let (int, dec) = body.split_once('.').unwrap_or((body, ""));
    let all: String = int.chars().chain(dec.chars()).collect();
    let trimmed = all.trim_start_matches('0');
    let kept = trimmed.trim_end_matches('0');
    let power = exp10 as i32 - dec.len() as i32 + (trimmed.len() - kept.len()) as i32;
    (negative, kept.to_string(), power)
}

/// A short literal's exact value in `long_normal`'s terms (zero: no digits, power 0), so a long
/// and a short literal compare exactly.
pub(crate) fn short_normal(l: Literal) -> (bool, String, i32) {
    let (m, k) = l.normal();
    if m == 0 {
        return (false, String::new(), 0);
    }
    (m < 0, m.unsigned_abs().to_string(), k)
}

// ---- private ----

impl Literal {
    /// The exact value as `m × 10^k` (`Mil` folded in: 25.4e-6 = 254 × 10^-7).
    fn exact(self) -> (i128, i32) {
        let k = self.exp10 as i32 - self.frac as i32;
        match self.form {
            NumForm::Mil => (self.mantissa as i128 * 254, k - 7),
            NumForm::Plain | NumForm::Exponent | NumForm::Scale => (self.mantissa as i128, k),
            NumForm::Int => (self.mantissa as i128, 0),
            NumForm::Long => unreachable!("a Long literal's digits are the design's"),
        }
    }

    /// The exact value with trailing zeros moved into the power: equal decimals give equal pairs.
    fn normal(self) -> (i128, i32) {
        let (mut m, mut k) = self.exact();
        if m == 0 {
            return (0, 0);
        }
        // at most 21 digits (an 18-digit mantissa times 254)
        for _ in 0..24 {
            if m % 10 != 0 {
                break;
            }
            m /= 10;
            k += 1;
        }
        debug_assert!(m % 10 != 0, "every trailing zero moved");
        (m, k)
    }
}
