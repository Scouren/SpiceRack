//! A VACASK number token's bytes → the decimal literal as written and its VACASK type, and a
//! literal → VACASK's spelling of the same digits. No floating arithmetic (R1).
//!
//! VACASK's grammar (VD input-numbers; `syntax/dialects/vacask.rs` cites `lib/dfllexer.l`
//! L594–L693): digits, an optional point and fraction, then either an exponent `e`/`E` (and
//! nothing after it) or one scale (`meg`, `mil`, `a f p n u m k K M x X G T`) followed by
//! letters or `_` it ignores; a bare mantissa takes no letters (`1k5`, `1e3k` are no numbers,
//! S 1.05–1.07); `0x`/`0X` hex. An integer literal (digits only, or hex) is VACASK's integer
//! type (RUN i1, f1).
//!
//! An integer literal is `NumForm::Int` (A12 K1); every other form is real, and the writer
//! spells a whole `Plain` number with a trailing point (`2.`) so that VACASK reads it real.
//! VACASK's integers are 32 bits and wrap (run iw1: `2147483648` is −2147483648,
//! `99999999999` is 1215752191): an integer literal past `i32` is refused here (`IntRange`).
//! A real with more significant digits than a mantissa holds is a `Long` literal, its digits
//! kept as written (A18, R37): VACASK rounds them correctly (run lr1, ties to even).
//!
//! Cites: VD input-numbers; `rewrite/INV-semantics.md` 1.01–1.07, 1.13; `IR-KERNEL.md` A3.
//!
//! Contents: `Number`, `NotANumber`, `lex`, `push`, `spell`, `int_fits` (crate-private)

use crate::ir;

/// A number as VACASK reads it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Number {
    /// The digits as written; for a long one, only its `exp10` and written `form` are read.
    pub(super) literal: ir::Literal,
    /// VACASK types it integer (digits only, or hex).
    pub(super) int: bool,
    /// A long literal's significand as written (`digits[.digits]`, no sign), or `None`.
    pub(super) long: Option<String>,
}

/// Why a token is no VACASK number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotANumber {
    /// Not VACASK's grammar (`1k5`, `1e3k`, `1d3`), or more than 18 digits after the point
    /// with no more than 18 significant ones.
    Grammar,
    /// An integer past `i32`, which VACASK wraps to 32 bits (run iw1).
    IntRange,
    /// A long literal whose value is not finite.
    NotFinite,
}

/// The most digits a literal's mantissa holds (`|mantissa| < 10^18`, A3).
const DIGITS_MAX: usize = 18;

/// The scale suffixes, the three-letter ones first, and their powers of ten (`None`: `mil`).
const SCALES: &[(&str, Option<i16>)] = &[
    ("meg", Some(6)),
    ("mil", None),
    ("a", Some(-18)),
    ("f", Some(-15)),
    ("p", Some(-12)),
    ("n", Some(-9)),
    ("u", Some(-6)),
    ("m", Some(-3)),
    ("k", Some(3)),
    ("K", Some(3)),
    ("M", Some(6)),
    ("x", Some(6)),
    ("X", Some(6)),
    ("G", Some(9)),
    ("T", Some(12)),
];

/// The letter the writer spells each power of ten with (one per power: `meg`, never `M`).
const SCALE_LETTERS: &[(i16, &str)] = &[
    (-18, "a"),
    (-15, "f"),
    (-12, "p"),
    (-9, "n"),
    (-6, "u"),
    (-3, "m"),
    (3, "k"),
    (6, "meg"),
    (9, "G"),
    (12, "T"),
];

/// The literal `text` (a whole number token, no sign) writes, and its type.
///
/// # Errors
/// [`NotANumber::Grammar`] for a spelling VACASK does not read as a number.
pub(super) fn lex(text: &str) -> Result<Number, NotANumber> {
    let bytes = text.as_bytes();
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        let ok = !hex.is_empty() && hex.len() <= 15 && hex.bytes().all(|b| b.is_ascii_hexdigit());
        let value = i64::from_str_radix(hex, 16).map_err(|_| NotANumber::Grammar)?;
        return match (ok, int_fits(value)) {
            (true, true) => Ok(int_literal(value)),
            (true, false) => Err(NotANumber::IntRange),
            (false, _) => Err(NotANumber::Grammar),
        };
    }
    let mut at = 0;
    let mut digits = String::new();
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        digits.push(bytes[at] as char);
        at += 1;
    }
    let mut point = false;
    let mut frac = 0usize;
    if at < bytes.len() && bytes[at] == b'.' {
        point = true;
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            digits.push(bytes[at] as char);
            frac += 1;
            at += 1;
        }
    }
    if digits.is_empty() {
        return Err(NotANumber::Grammar);
    }
    let rest = &text[at..];
    let (exp10, form) = match rest.as_bytes().first() {
        None => (0, ir::NumForm::Plain),
        Some(b'e' | b'E') => (exponent(&rest[1..])?, ir::NumForm::Exponent),
        Some(_) => scale(rest)?,
    };
    let int = !point && form == ir::NumForm::Plain;
    let significant = digits.trim_start_matches('0').len();
    if int && significant > DIGITS_MAX {
        return Err(NotANumber::IntRange);
    }
    if ir::literal::exceeds_mantissa(significant) {
        let whole = &digits[..digits.len() - frac];
        let significand = match frac {
            0 => whole.to_string(),
            _ => format!("{whole}.{}", &digits[digits.len() - frac..]),
        };
        let finite = format!("{significand}e{exp10}")
            .parse::<f64>()
            .is_ok_and(f64::is_finite);
        if !finite {
            return Err(NotANumber::NotFinite);
        }
        let literal = ir::Literal {
            exp10,
            form,
            ..ir::Literal::ZERO
        };
        return Ok(Number {
            literal,
            int,
            long: Some(significand),
        });
    }
    if frac > DIGITS_MAX {
        return Err(NotANumber::Grammar);
    }
    let mantissa: i64 = digits.parse().map_err(|_| NotANumber::Grammar)?;
    if int && !int_fits(mantissa) {
        return Err(NotANumber::IntRange);
    }
    let literal = ir::Literal {
        mantissa,
        frac: u8::try_from(frac).expect("checked above"),
        exp10,
        form: if int { ir::NumForm::Int } else { form },
    };
    // The IR has one real zero (`Literal::ZERO`), and holds only finite values (C9).
    let literal = match literal.mantissa == 0 && !int {
        true => ir::Literal::ZERO,
        false => literal,
    };
    match literal.is_valid() {
        true => Ok(Number {
            literal,
            int,
            long: None,
        }),
        false => Err(NotANumber::Grammar),
    }
}

/// Pushes number `n` (negated when `negative`) as a literal row.
pub(super) fn push(b: &mut ir::Builder<'_>, n: &Number, negative: bool) -> ir::ExprId {
    match &n.long {
        Some(digits) => {
            let sign = if negative { "-" } else { "" };
            b.long_literal(&format!("{sign}{digits}"), n.literal.exp10, n.literal.form)
        }
        None => {
            let mut literal = n.literal;
            if negative {
                literal.mantissa = -literal.mantissa;
            }
            b.literal(literal)
        }
    }
}

/// Whether integer `value` is one of VACASK's (32 bits: run iw1).
pub(super) fn int_fits(value: i64) -> bool {
    i32::try_from(value).is_ok()
}

/// Appends VACASK's spelling of `literal`: its digits, then `e<exp10>` (Exponent), VACASK's
/// letter for the power (Scale; `meg` for 10⁶, never `M`; a power VACASK has no letter for as
/// `e<exp10>`), `mil` (Mil), or nothing (Plain). A negative mantissa is spelled with its `-`.
/// A long literal (of `design`) is its significand as written, with a point when it has none
/// and its power is plain (else VACASK reads an integer), then its power the same way.
pub(super) fn spell(design: &ir::Design, literal: ir::Literal, out: &mut String) {
    if let Some((digits, written)) = design.long_digits(literal) {
        out.push_str(digits);
        match written {
            ir::NumForm::Plain if !digits.contains('.') => out.push('.'),
            ir::NumForm::Plain => {}
            _ => power(literal.exp10, written, out),
        }
        return;
    }
    if literal.mantissa < 0 {
        out.push('-');
    }
    let digits = literal.mantissa.unsigned_abs().to_string();
    let frac = usize::from(literal.frac);
    if frac == 0 {
        out.push_str(&digits);
    } else {
        let padded = format!("{digits:0>width$}", width = frac + 1);
        let (whole, part) = padded.split_at(padded.len() - frac);
        out.push_str(whole);
        out.push('.');
        out.push_str(part);
    }
    match literal.form {
        ir::NumForm::Int => {}
        // VACASK reads digits alone as an integer: a real whole number gets its point.
        ir::NumForm::Plain if literal.frac == 0 => out.push('.'),
        ir::NumForm::Plain => {}
        ir::NumForm::Mil => out.push_str("mil"),
        form @ (ir::NumForm::Exponent | ir::NumForm::Scale) => power(literal.exp10, form, out),
        ir::NumForm::Long => unreachable!("long_digits spells a Long literal"),
    }
}

// ---- private ----

/// The integer literal `value`.
fn int_literal(value: i64) -> Number {
    Number {
        literal: ir::Literal {
            mantissa: value,
            frac: 0,
            exp10: 0,
            form: ir::NumForm::Int,
        },
        int: true,
        long: None,
    }
}

/// A power of ten written as `form`: `e<exp10>`, or VACASK's letter (`e<exp10>` for a power it
/// has no letter for).
fn power(exp10: i16, form: ir::NumForm, out: &mut String) {
    let letter = SCALE_LETTERS.iter().find(|(p, _)| *p == exp10);
    match (form, letter) {
        (ir::NumForm::Scale, Some((_, letter))) => out.push_str(letter),
        _ => out.push_str(&format!("e{exp10}")),
    }
}

/// The exponent after `e`: an optional sign and digits, and nothing after them.
fn exponent(text: &str) -> Result<i16, NotANumber> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || digits.len() > 4 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(NotANumber::Grammar);
    }
    text.trim_start_matches('+')
        .parse()
        .map_err(|_| NotANumber::Grammar)
}

/// A scale suffix and the letters or `_` after it.
fn scale(text: &str) -> Result<(i16, ir::NumForm), NotANumber> {
    let (suffix, power) = SCALES
        .iter()
        .find(|(s, _)| text.starts_with(s))
        .ok_or(NotANumber::Grammar)?;
    let tail = &text[suffix.len()..];
    if !tail.bytes().all(|b| b.is_ascii_alphabetic() || b == b'_') {
        return Err(NotANumber::Grammar);
    }
    Ok(match power {
        Some(p) => (*p, ir::NumForm::Scale),
        None => (0, ir::NumForm::Mil),
    })
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

    /// Oracle: VD input-numbers and Appendix A run f1 (`1M` 1e6, `2x` 2e6, `1mil`, hex 16).
    #[test]
    fn lex_reads_vacasks_spellings() {
        use ir::NumForm::{Exponent, Mil, Plain, Scale};
        let cases: &[(&str, ir::Literal, bool)] = &[
            ("1", lit(1, 0, 0, ir::NumForm::Int), true),
            ("0x10", lit(16, 0, 0, ir::NumForm::Int), true),
            ("0", lit(0, 0, 0, ir::NumForm::Int), true),
            ("0.0", ir::Literal::ZERO, false),
            ("2.5", lit(25, 1, 0, Plain), false),
            ("2.", lit(2, 0, 0, Plain), false),
            (".5", lit(5, 1, 0, Plain), false),
            ("0.050000", lit(50000, 6, 0, Plain), false),
            ("1e-3", lit(1, 0, -3, Exponent), false),
            ("1.5E3", lit(15, 1, 3, Exponent), false),
            ("100u", lit(100, 0, -6, Scale), false),
            ("1meg", lit(1, 0, 6, Scale), false),
            ("1M", lit(1, 0, 6, Scale), false),
            ("2x", lit(2, 0, 6, Scale), false),
            ("5K", lit(5, 0, 3, Scale), false),
            ("10kOhm", lit(10, 0, 3, Scale), false),
            ("1mil", lit(1, 0, 0, Mil), false),
            ("3m", lit(3, 0, -3, Scale), false),
        ];
        for &(text, want, int) in cases {
            let n = Number {
                literal: want,
                int,
                long: None,
            };
            assert_eq!(lex(text), Ok(n), "{text}");
        }
        for bad in ["1k5", "1e3k", "1d3", "1x2"] {
            assert_eq!(lex(bad), Err(NotANumber::Grammar), "{bad}");
        }
        // Run iw1: VACASK's integers are 32 bits.
        for wide in ["2147483648", "12345678901234567890", "0x80000000"] {
            assert_eq!(lex(wide), Err(NotANumber::IntRange), "{wide}");
        }
        assert!(lex("2147483647").is_ok());
        // A18: more than 18 significant digits of a real are kept as written.
        let long = lex("1.0000000000000001110223k").expect("a long literal");
        assert_eq!(long.long.as_deref(), Some("1.0000000000000001110223"));
        assert_eq!((long.literal.exp10, long.literal.form), (3, Scale));
    }

    /// Oracle: the same table read back (`spell` then `lex` gives the literal).
    #[test]
    fn spell_reads_back() {
        use ir::NumForm::{Exponent, Mil, Plain, Scale};
        let cases = [
            (lit(25, 1, 0, Plain), "2.5"),
            (lit(5, 1, 0, Plain), "0.5"),
            (lit(1, 0, -3, Exponent), "1e-3"),
            (lit(100, 0, -6, Scale), "100u"),
            (lit(1, 0, 6, Scale), "1meg"),
            (lit(1, 0, 0, Mil), "1mil"),
            (lit(7, 0, -2, Scale), "7e-2"),
            (lit(-2, 0, 0, ir::NumForm::Int), "-2"),
            (lit(2, 0, 0, Plain), "2."),
            (ir::Literal::ZERO, "0."),
        ];
        let design = ir::Design::new();
        for (l, want) in cases {
            let mut out = String::new();
            spell(&design, l, &mut out);
            assert_eq!(out, want, "{l:?}");
        }
    }
}
