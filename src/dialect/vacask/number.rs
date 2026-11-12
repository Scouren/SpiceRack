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

/// The literal `text` (a whole number token, no sign) writes, and its type.
///
/// # Errors
/// [`NotANumber::Grammar`] for a spelling VACASK does not read as a number.
pub(super) fn lex(text: &str) -> Result<Number, NotANumber> {
    let _ = (text, DIGITS_MAX, SCALES, a12::INT_FORM);
    todo!("S4.0: number::lex")
}

/// Appends VACASK's spelling of `literal`: its digits, then `e<exp10>` (Exponent), VACASK's
/// letter for the power (Scale; `meg` for 10⁶, never `M`; a power VACASK has no letter for as
/// `e<exp10>`), `mil` (Mil), or nothing (Plain). A negative mantissa is spelled with its `-`.
pub(super) fn spell(literal: ir::Literal, out: &mut String) {
    let _ = (literal, out);
    todo!("S4.0: number::spell")
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
