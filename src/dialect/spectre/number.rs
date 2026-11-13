//! A Spectre number token's bytes → the decimal literal as written, and a literal → Spectre's
//! spelling of the same digits. No floating arithmetic (R1).
//!
//! Spectre's grammar (MAN: UG §2.4; CODE: `syntax/dialects/spectre.rs` SCALES and the lexicon
//! tests; **no run**, `rewrite/DIALECT-SPECTRE.md` §10): digits, an optional point and
//! fraction, then an exponent `e`/`E` and its digits, then at most one case-sensitive scale
//! letter (`P T G M k K _ % c m u n p f a`; `M` is mega, `m` milli; there is no `meg` and no
//! `mil`: `1meg` is 1 milli, `eg` a unit) and any unit letters after it (`1pf`, `50_Ohms`).
//! Nothing may follow a bare mantissa but a scale (`1k5` is no number); `1d3` is no number. An
//! exponent and a scale together (`1e3k`) fold into one exponent. `_` is a scale of one: the
//! literal is plain.
//!
//! Every Spectre number is real (MAN: SR5 p.871, real scalars only), so no literal is
//! `NumForm::Int`. A literal with more significant digits than a mantissa holds is refused
//! (`TooLong`): no run settled how Spectre rounds one.
//!
//! Cites: `rewrite/DIALECT-SPECTRE.md` §2 (scale letters, exponent); `IR-KERNEL.md` A3.
//!
//! Contents: `NotANumber`, `lex`, `push`, `spell` (crate-private)

use crate::ir;

/// Why a token is no number this reader reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotANumber {
    /// Not Spectre's grammar (`1k5`, `1d3`, a bare point).
    Grammar,
    /// More significant digits than a literal's mantissa holds.
    TooLong,
}

/// The scale letters and their powers of ten (`_` is one).
const SCALES: &[(u8, i16)] = &[
    (b'P', 15),
    (b'T', 12),
    (b'G', 9),
    (b'M', 6),
    (b'k', 3),
    (b'K', 3),
    (b'_', 0),
    (b'%', -2),
    (b'c', -2),
    (b'm', -3),
    (b'u', -6),
    (b'n', -9),
    (b'p', -12),
    (b'f', -15),
    (b'a', -18),
];

/// The letter the writer spells each power with: one per power (`k`, never `K`; `c`, never `%`,
/// which reads as the modulo operator before an operand).
const LETTERS: &[(i16, &str)] = &[
    (-18, "a"),
    (-15, "f"),
    (-12, "p"),
    (-9, "n"),
    (-6, "u"),
    (-3, "m"),
    (-2, "c"),
    (3, "k"),
    (6, "M"),
    (9, "G"),
    (12, "T"),
    (15, "P"),
];

/// The most digits after the point a literal holds (A3).
const FRAC_MAX: usize = 18;

/// The literal `text` (a whole number token, no sign) writes.
///
/// # Errors
/// [`NotANumber::Grammar`] for a spelling Spectre does not read as a number;
/// [`NotANumber::TooLong`] for more digits than a literal holds.
pub(super) fn lex(text: &str) -> Result<ir::Literal, NotANumber> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut digits = String::new();
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        digits.push(char::from(bytes[at]));
        at += 1;
    }
    let mut frac = 0usize;
    if at < bytes.len() && bytes[at] == b'.' {
        at += 1;
        while at < bytes.len() && bytes[at].is_ascii_digit() {
            digits.push(char::from(bytes[at]));
            frac += 1;
            at += 1;
        }
    }
    if digits.is_empty() {
        return Err(NotANumber::Grammar);
    }
    let mut exp10: i32 = 0;
    let mut form = ir::NumForm::Plain;
    if at < bytes.len() && matches!(bytes[at], b'e' | b'E') {
        let (e, used) = exponent(&text[at + 1..])?;
        exp10 = e;
        form = ir::NumForm::Exponent;
        at += 1 + used;
    }
    if let Some(&b) = bytes.get(at) {
        let (_, power) = SCALES
            .iter()
            .find(|(s, _)| *s == b)
            .ok_or(NotANumber::Grammar)?;
        let tail = &bytes[at + 1..];
        if !tail.iter().all(|b| b.is_ascii_alphabetic() || *b == b'_') {
            return Err(NotANumber::Grammar);
        }
        exp10 += i32::from(*power);
        if form == ir::NumForm::Plain && *power != 0 {
            form = ir::NumForm::Scale;
        }
    }
    if ir::literal::exceeds_mantissa(digits.trim_start_matches('0').len()) || frac > FRAC_MAX {
        return Err(NotANumber::TooLong);
    }
    let mantissa: i64 = digits.parse().map_err(|_| NotANumber::Grammar)?;
    let literal = ir::Literal {
        mantissa,
        frac: u8::try_from(frac).expect("checked above"),
        exp10: i16::try_from(exp10).map_err(|_| NotANumber::Grammar)?,
        form,
    };
    // The IR has one real zero (`Literal::ZERO`), and holds only finite values (C9).
    let literal = match literal.mantissa == 0 {
        true => ir::Literal::ZERO,
        false => literal,
    };
    match literal.is_valid() {
        true => Ok(literal),
        false => Err(NotANumber::Grammar),
    }
}

/// Pushes `literal` (negated when `negative`) as a literal row.
pub(super) fn push(b: &mut ir::Builder<'_>, literal: ir::Literal, negative: bool) -> ir::ExprId {
    let mut literal = literal;
    if negative {
        literal.mantissa = -literal.mantissa;
    }
    b.literal(literal)
}

/// Appends Spectre's spelling of `literal`: its digits, then `e<exp10>` (Exponent), the scale
/// letter (Scale), or nothing (Plain, Int). A negative mantissa is spelled with its `-`. A
/// `Mil` literal (ngspice's, which Spectre has no letter for) is the same decimal exactly, its
/// digits times 254 and its power seven less (`1mil` is `254e-7`).
///
/// # Panics
/// On a `Long` literal: `FEATURES` lacks `LongLiteral`, so a legal design holds none.
pub(super) fn spell(literal: ir::Literal, out: &mut String) {
    assert!(
        literal.form != ir::NumForm::Long,
        "FEATURES lacks LongLiteral"
    );
    if literal.form == ir::NumForm::Mil {
        // mantissa × 10^-frac × 25.4e-6 = (mantissa × 254) × 10^-(frac + 7), exactly
        ir::literal::write_digits(i128::from(literal.mantissa) * 254, 0, out);
        out.push_str(&format!("e-{}", u32::from(literal.frac) + 7));
        return;
    }
    ir::literal::write_digits(i128::from(literal.mantissa), literal.frac, out);
    match literal.form {
        ir::NumForm::Plain | ir::NumForm::Int => {}
        form => ir::literal::write_power(literal.exp10, form, LETTERS, out),
    }
}

// ---- private ----

/// The exponent after `e`: an optional sign and one to four digits; how many bytes it took.
fn exponent(text: &str) -> Result<(i32, usize), NotANumber> {
    let sign = usize::from(text.starts_with(['+', '-']));
    let count = text[sign..].bytes().take_while(u8::is_ascii_digit).count();
    if count == 0 || count > 4 {
        return Err(NotANumber::Grammar);
    }
    let value: i32 = text[sign..sign + count]
        .parse()
        .map_err(|_| NotANumber::Grammar)?;
    Ok((
        if text.starts_with('-') { -value } else { value },
        sign + count,
    ))
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

    /// Oracle: MAN (UG §2.4) and the old lexicon's pinned table (CODE; no Spectre run):
    /// `M` mega, `m` milli, units after the scale, `1k5` and `1d3` no numbers.
    #[test]
    fn lex_reads_spectres_spellings() {
        use ir::NumForm::{Exponent, Plain, Scale};
        let cases: &[(&str, ir::Literal)] = &[
            ("2", lit(2, 0, 0, Plain)),
            ("0", ir::Literal::ZERO),
            ("2.5", lit(25, 1, 0, Plain)),
            (".5", lit(5, 1, 0, Plain)),
            ("0.050000", lit(50000, 6, 0, Plain)),
            ("1e-3", lit(1, 0, -3, Exponent)),
            ("1.5E3", lit(15, 1, 3, Exponent)),
            ("1e3k", lit(1, 0, 6, Exponent)),
            ("100u", lit(100, 0, -6, Scale)),
            ("1M", lit(1, 0, 6, Scale)),
            ("1m", lit(1, 0, -3, Scale)),
            ("1meg", lit(1, 0, -3, Scale)),
            ("5K", lit(5, 0, 3, Scale)),
            ("1pf", lit(1, 0, -12, Scale)),
            ("5%", lit(5, 0, -2, Scale)),
            ("2P", lit(2, 0, 15, Scale)),
            ("50_Ohms", lit(50, 0, 0, Plain)),
        ];
        for &(text, want) in cases {
            assert_eq!(lex(text), Ok(want), "{text}");
        }
        for bad in ["1k5", "1d3", ".", "1e", "1x"] {
            assert_eq!(lex(bad), Err(NotANumber::Grammar), "{bad}");
        }
        assert_eq!(lex("1.0000000000000001110223"), Err(NotANumber::TooLong));
    }

    /// Oracle: a round trip (`spell` then `lex` gives the literal back, form included).
    #[test]
    fn spell_reads_back() {
        use ir::NumForm::{Exponent, Plain, Scale};
        let cases = [
            (lit(25, 1, 0, Plain), "2.5"),
            (lit(5, 1, 0, Plain), "0.5"),
            (lit(1, 0, -3, Exponent), "1e-3"),
            (lit(100, 0, -6, Scale), "100u"),
            (lit(1, 0, 6, Scale), "1M"),
            (lit(7, 0, -2, Scale), "7c"),
            (lit(3, 2, 15, Scale), "0.03P"),
            (lit(2, 0, 0, Plain), "2"),
            (ir::Literal::ZERO, "0"),
        ];
        for (l, want) in cases {
            let mut out = String::new();
            spell(l, &mut out);
            assert_eq!(out, want, "{l:?}");
            assert_eq!(lex(want), Ok(l), "{want}");
        }
        let mut out = String::new();
        spell(lit(-4, 0, 3, Scale), &mut out);
        assert_eq!(out, "-4k");
        // a mil: 1.5 × 25.4e-6 = 3810e-8, digit for digit
        let mut out = String::new();
        spell(lit(15, 1, 0, ir::NumForm::Mil), &mut out);
        assert_eq!(out, "3810e-8");
    }
}
