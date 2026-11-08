//! A number token's spelling → its `f64`, by the lexicon's [`Numbers`]
//! table: the one place a number is decoded. The tokenizer calls it once per [`Tag::Number`] token.
//!
//! The grammar each [`Tail`] reads is the old reader's `Num` rule for that
//! dialect; the arithmetic is the old lowering's (`num_value`): ngspice's
//! `INPevaluate` ([`crate::unit::ngspice_value`]), or the mantissa and
//! shifted exponent read as one decimal. Until the switch (phase 2b) every
//! number token in the corpora was checked against that oracle to the bit.
//!
//! Contents:
//! - `pub(super) fn value(text: &str, lex: &Lexicon) -> f64`
//!
//! A Decimal number is decoded in a stack buffer (`decimal_in_place`),
//! the old reader's arithmetic (`decimal`) its reference and its path for
//! a spelling longer than the buffer.
//!
//! [`Tag::Number`]: crate::syntax::token::Tag::Number

use crate::syntax::lexicon::{Arithmetic, Lexicon, Numbers, Scale, Tail};

/// `text`'s value; NaN when the lexicon's number grammar does not read all
/// of it (`1k5` in Spectre, `0x1F` in VACASK, `1µ` in ngspice).
pub(super) fn value(text: &str, lex: &Lexicon) -> f64 {
    decode(text, &lex.numbers, lex.fold_case).unwrap_or(f64::NAN)
}

// ---- private ----

fn decode(text: &str, n: &Numbers, fold: bool) -> Option<f64> {
    let b = text.as_bytes();
    let digits = |q: usize| b[q..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut mag = digits(0);
    if b.get(mag) == Some(&b'.') {
        mag += 1 + digits(mag + 1);
    }
    let mantissa = mag;
    if b.get(mag).is_some_and(|c| n.exponent.contains(c)) {
        let sign = usize::from(matches!(b.get(mag + 1), Some(b'+' | b'-')));
        let exp = digits(mag + 1 + sign);
        if exp > 0 {
            mag += 1 + sign + exp;
        }
    }
    if let Tail::Letters { hex: true, .. } = n.tail
        && matches!(b, [b'0', b'x' | b'X', h, ..] if h.is_ascii_hexdigit())
    {
        return None;
    }
    // VACASK L594-L693: a scale or a name byte right after an exponent
    // makes the exponent no exponent, and the number no number.
    if let Tail::Letters {
        after_exponent: false,
        ..
    } = n.tail
        && mag > mantissa
        && mag < b.len()
    {
        mag = mantissa;
    }
    let rest = &text[mag..];
    let scale = n.scales.iter().find(|(s, _)| {
        rest.get(..s.len()).is_some_and(|p| {
            if fold {
                p.eq_ignore_ascii_case(s)
            } else {
                p == *s
            }
        })
    });
    let after = &rest[scale.map_or(0, |(s, _)| s.len())..];
    let frac = match n.tail {
        Tail::Spice { dot } => {
            let unit = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || (dot && c == b'.');
            if !rest.bytes().all(unit) {
                return None;
            }
            None
        }
        Tail::Ltspice => {
            if scale.is_some() && !after.is_empty() && after.bytes().all(|c| c.is_ascii_digit()) {
                Some(after)
            } else if rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == 'µ')
            {
                None
            } else {
                return None;
            }
        }
        Tail::Letters { .. } => {
            let letters = after.bytes().all(|c| c.is_ascii_alphabetic() || c == b'_');
            if !(rest.is_empty() || (scale.is_some() && letters)) {
                return None;
            }
            None
        }
    };
    match n.arithmetic {
        Arithmetic::Ngspice => crate::unit::ngspice_value(text),
        Arithmetic::Decimal => {
            let (head, scale) = (&text[..mag], scale.map(|(_, s)| *s));
            decimal_in_place(head, frac, scale).unwrap_or_else(|| decimal(head, frac, scale))
        }
    }
}

/// The mantissa `head` (digits, a point, an exponent), an LTspice `frac`
/// after its scale and the scale, as one decimal: values.rs `num_value`,
/// step for step, so `0.1n` is exactly 1e-10 and a decoded number equals
/// the old reader's to the bit. The reference [`decimal_in_place`] is held
/// to, and its fallback for a spelling too long for that buffer.
fn decimal(head: &str, frac: Option<&str>, scale: Option<Scale>) -> Option<f64> {
    let mut m = head.to_ascii_lowercase().replace('d', "e");
    if let Some(f) = frac {
        m = format!("{m}.{f}");
    }
    let (base, exp) = match m.split_once('e') {
        Some((base, e)) => (base, e.parse::<i32>().ok()?),
        None => (m.as_str(), 0),
    };
    let shift = match scale {
        Some(Scale::Mil) => return Some(m.parse::<f64>().ok()? * 25.4e-6),
        Some(Scale::Exp(k)) => k,
        None => 0,
    };
    format!("{base}e{}", exp + shift).parse().ok()
}

/// [`decimal`] in a stack buffer, with no allocation per number token;
/// `None` when the spelling does not fit, and [`decimal`] decides.
fn decimal_in_place(head: &str, frac: Option<&str>, scale: Option<Scale>) -> Option<Option<f64>> {
    // The folded mantissa, then `e` and an i32's 11 characters.
    const CAP: usize = 64;
    let len = head.len() + frac.map_or(0, |f| f.len() + 1);
    if len + 12 > CAP {
        return None;
    }
    let mut buf = [0u8; CAP];
    for (d, &c) in buf.iter_mut().zip(head.as_bytes()) {
        *d = match c.to_ascii_lowercase() {
            b'd' => b'e',
            c => c,
        };
    }
    if let Some(f) = frac {
        buf[head.len()] = b'.';
        buf[head.len() + 1..len].copy_from_slice(f.as_bytes());
    }
    let m = std::str::from_utf8(&buf[..len]).ok()?;
    let (base, exp) = match m.split_once('e') {
        Some((base, e)) => match e.parse::<i32>() {
            Ok(e) => (base.len(), e),
            Err(_) => return Some(None),
        },
        None => (len, 0),
    };
    let shift = match scale {
        Some(Scale::Mil) => return Some(m.parse::<f64>().ok().map(|v| v * 25.4e-6)),
        Some(Scale::Exp(k)) => k,
        None => 0,
    };
    buf[base] = b'e';
    let mut tail = &mut buf[base + 1..];
    let written = std::io::Write::write_fmt(&mut tail, format_args!("{}", exp + shift));
    assert!(written.is_ok(), "an i32 fits the 11 bytes reserved for it");
    let end = CAP - tail.len();
    Some(std::str::from_utf8(&buf[..end]).ok()?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `decimal_in_place` against `decimal`, the old reader's arithmetic,
    /// bit for bit over generated spellings: every mantissa shape, every
    /// exponent letter and sign, every scale, LTspice fractions, and
    /// exponents past i32 and past the buffer.
    #[test]
    fn in_place_equals_decimal_bit_for_bit() {
        let mantissas = [
            "0",
            "1",
            "12",
            "1.",
            ".5",
            "0.1",
            "3.14159",
            "123456789012345678901234567890",
        ];
        let exps = [
            "",
            "e3",
            "E-3",
            "d+7",
            "D0",
            "e308",
            "e-330",
            "e99999999999",
            "e2147483635",
            "e-2147483633",
        ];
        let fracs = [
            None,
            Some("7"),
            Some("05"),
            Some("123456789012345678901234567890"),
        ];
        let scales = [
            None,
            Some(Scale::Mil),
            Some(Scale::Exp(-15)),
            Some(Scale::Exp(3)),
            Some(Scale::Exp(12)),
        ];
        let mut n = 0;
        for m in mantissas {
            for e in exps {
                let head = format!("{m}{e}");
                for f in fracs {
                    for s in scales {
                        let want = decimal(&head, f, s).map(f64::to_bits);
                        if let Some(got) = decimal_in_place(&head, f, s) {
                            assert_eq!(got.map(f64::to_bits), want, "{head} {f:?} {s:?}");
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n > 1000, "most spellings take the buffer: {n}");
    }
}
