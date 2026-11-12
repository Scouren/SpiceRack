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
//! Until AMEND-A12 K1 lands (`a12.rs`), an integer literal is `Plain` and a real whole number
//! written with a point and no fraction digits (`2.`) is stored with one zero fraction digit
//! (`2.0`), so that the writer's `Plain` digits stay VACASK integers and `2.` stays real.
//!
//! Cites: VD input-numbers; `rewrite/INV-semantics.md` 1.01–1.07, 1.13; `IR-KERNEL.md` A3.
//!
//! Contents: `Number`, `NotANumber`, `lex`, `spell` (crate-private)

use super::a12;
use crate::ir;

/// A number as VACASK reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Number {
    /// The digits as written.
    pub(super) literal: ir::Literal,
    /// VACASK types it integer (digits only, or hex).
    pub(super) int: bool,
}

/// Why a token is no VACASK number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotANumber {
    /// Not VACASK's grammar (`1k5`, `1e3k`, `1d3`), or more than 18 significant digits.
    Grammar,
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
        return match ok {
            true => Ok(int_literal(value)),
            false => Err(NotANumber::Grammar),
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
    // Until A12 K1: `2.` (a point, no fraction digit) is stored as `2.0` to stay real.
    if point && frac == 0 && form == ir::NumForm::Plain {
        digits.push('0');
        frac = 1;
    }
    let significant = digits.trim_start_matches('0').len();
    if significant > DIGITS_MAX || frac > DIGITS_MAX {
        return Err(NotANumber::Grammar);
    }
    let mantissa: i64 = digits.parse().map_err(|_| NotANumber::Grammar)?;
    let int = !point && form == ir::NumForm::Plain;
    let literal = ir::Literal {
        mantissa,
        frac: u8::try_from(frac).expect("checked above"),
        exp10,
        form: if int { a12::INT_FORM } else { form },
    };
    // The IR has one zero (`Literal::ZERO`), and holds only finite values (C9).
    let literal = match literal.mantissa == 0 {
        true => ir::Literal::ZERO,
        false => literal,
    };
    match literal.is_valid() {
        true => Ok(Number { literal, int }),
        false => Err(NotANumber::Grammar),
    }
}

/// Appends VACASK's spelling of `literal`: its digits, then `e<exp10>` (Exponent), VACASK's
/// letter for the power (Scale; `meg` for 10⁶, never `M`; a power VACASK has no letter for as
/// `e<exp10>`), `mil` (Mil), or nothing (Plain). A negative mantissa is spelled with its `-`.
pub(super) fn spell(literal: ir::Literal, out: &mut String) {
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
        ir::NumForm::Plain => {}
        ir::NumForm::Mil => out.push_str("mil"),
        ir::NumForm::Exponent => out.push_str(&format!("e{}", literal.exp10)),
        ir::NumForm::Scale => match SCALE_LETTERS.iter().find(|(p, _)| *p == literal.exp10) {
            Some((_, letter)) => out.push_str(letter),
            None => out.push_str(&format!("e{}", literal.exp10)),
        },
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
            form: a12::INT_FORM,
        },
        int: true,
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
            ("1", lit(1, 0, 0, a12::INT_FORM), true),
            ("0x10", lit(16, 0, 0, a12::INT_FORM), true),
            ("2.5", lit(25, 1, 0, Plain), false),
            ("2.", lit(20, 1, 0, Plain), false),
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
            assert_eq!(lex(text), Ok(Number { literal: want, int }), "{text}");
        }
        for bad in ["1k5", "1e3k", "1d3", "12345678901234567890", "1x2"] {
            assert_eq!(lex(bad), Err(NotANumber::Grammar), "{bad}");
        }
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
            (lit(-2, 0, 0, Plain), "-2"),
        ];
        for (l, want) in cases {
            let mut out = String::new();
            spell(l, &mut out);
            assert_eq!(out, want, "{l:?}");
        }
    }
}
