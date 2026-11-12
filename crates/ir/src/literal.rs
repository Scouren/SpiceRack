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
//! `Literal::shape_ok(self) -> bool`, `Literal::finite(self) -> bool`

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
}

/// The powers of ten a scale letter states (A3): a `Scale` literal's `exp10` is one of these.
const SCALES: [i16; 12] = [-18, -15, -12, -9, -6, -3, -2, 3, 6, 9, 12, 15];
/// `|mantissa|` is below this (10^18).
const MANTISSA_BOUND: u64 = 1_000_000_000_000_000_000;

const _: () = assert!(size_of::<Literal>() == 16);

impl NumForm {
    /// Every form, in declaration order.
    pub const ALL: [NumForm; 5] = [
        NumForm::Plain,
        NumForm::Exponent,
        NumForm::Scale,
        NumForm::Mil,
        NumForm::Int,
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
    pub fn value(self) -> f64 {
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
    pub fn eq_value(self, other: Literal) -> bool {
        // an Int and a real literal of one value mean different things (A12 K1)
        let int = |l: Literal| l.form == NumForm::Int;
        int(self) == int(other) && self.normal() == other.normal()
    }

    /// Whether the literal meets every invariant on [`Literal`].
    pub fn is_valid(self) -> bool {
        self.shape_ok() && self.finite()
    }

    /// Whether the value is finite, from the digit count and the power where that decides it
    /// (the largest double is about 1.8e308), and by `value()` only at the boundary.
    pub(crate) fn finite(self) -> bool {
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
        self.mantissa.unsigned_abs() < MANTISSA_BOUND
            && self.frac <= 18
            && match self.form {
                NumForm::Plain | NumForm::Mil => self.exp10 == 0,
                NumForm::Scale => SCALES.contains(&self.exp10),
                NumForm::Exponent => true,
                NumForm::Int => self.exp10 == 0 && self.frac == 0,
            }
            && (self.mantissa != 0 || self == Literal::ZERO || self.form == NumForm::Int)
    }
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
