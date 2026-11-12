//! Decimal literals (Addendum A3): `Literal::value` against the decimal's correctly rounded
//! double, `from_f64` as its inverse, `eq_value` across forms, and one literal per broken
//! invariant beside a legal neighbour.
//!
//! Oracles: Rust's own float literals, which the compiler rounds correctly (an independent
//! implementation of decimal → double); the inverse `from_f64(v).value() == v` over a fixed list
//! and 10⁶ generated doubles (seed printed on failure); hand-derived values in the comments.
//!
//! Contents: none (tests only).

use spicerack_ir::{Literal, NumForm};

// ---- private ----

fn lit(mantissa: i64, frac: u8, exp10: i16, form: NumForm) -> Literal {
    Literal {
        mantissa,
        frac,
        exp10,
        form,
    }
}

/// splitmix64: the test's own PRNG (no dependency).
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn value_is_the_correctly_rounded_decimal() {
    let cases = [
        // 1k = 1 × 10^3
        (lit(1, 0, 3, NumForm::Scale), 1000.0),
        // 100u = 100 × 10^-6, the double nearest 1e-4 (ngspice's own arithmetic differs: A3)
        (lit(100, 0, -6, NumForm::Scale), 1e-4),
        // 0.050000 = 50000 × 10^-6
        (lit(50000, 6, 0, NumForm::Plain), 0.05),
        // -2.5e-3 = -25 × 10^(-3-1)
        (lit(-25, 1, -3, NumForm::Exponent), -2.5e-3),
        // 1mil = 25.4e-6 m
        (lit(1, 0, 0, NumForm::Mil), 25.4e-6),
        // 2.5mil = 63.5e-6 m
        (lit(25, 1, 0, NumForm::Mil), 63.5e-6),
        // 2^53 + 1 is a tie between 2^53 and 2^53 + 2: round half to even gives 2^53
        (
            lit(9_007_199_254_740_993, 0, 0, NumForm::Plain),
            9_007_199_254_740_992.0,
        ),
        // 18 digits, just under the mantissa bound
        (
            lit(999_999_999_999_999_999, 0, 0, NumForm::Plain),
            999_999_999_999_999_999.0,
        ),
        // a percent (Spectre `%`): 5 × 10^-2
        (lit(5, 0, -2, NumForm::Scale), 0.05),
        (Literal::ZERO, 0.0),
    ];
    let cases: [(Literal, f64); 10] = cases;
    for (l, want) in cases {
        assert_eq!(l.value().to_bits(), want.to_bits(), "{l:?}");
    }
}

#[test]
fn from_f64_is_engineering_form() {
    assert_eq!(Literal::from_f64(1e-4), lit(100, 0, -6, NumForm::Scale));
    assert_eq!(Literal::from_f64(1.8), lit(18, 1, 0, NumForm::Plain));
    assert_eq!(Literal::from_f64(1234.5), lit(12345, 4, 3, NumForm::Scale));
    assert_eq!(Literal::from_f64(1e20), lit(100, 0, 18, NumForm::Exponent));
    assert_eq!(Literal::from_f64(-0.05), lit(-50, 0, -3, NumForm::Scale));
    assert_eq!(Literal::from_f64(0.0), Literal::ZERO);
    assert_eq!(Literal::from_f64(-0.0), Literal::ZERO);
}

#[test]
fn from_f64_reads_back_fixed_list() {
    let values = [
        0.0,
        1.0,
        -1.0,
        1.8,
        1e-4,
        0.05,
        0.1,
        1.0 / 3.0,
        1234.5,
        1e20,
        1e-20,
        -2.5e-3,
        6.022_140_76e23,
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
        -f64::MAX,
        123_456_789_012_345_680.0,
    ];
    for v in values {
        let l = Literal::from_f64(v);
        assert!(l.is_valid(), "{v:e} → {l:?}");
        assert_eq!(l.value(), v, "{v:e} → {l:?}");
    }
}

#[test]
fn from_f64_reads_back_generated() {
    const SEED: u64 = 0x5EED_0001;
    let mut state = SEED;
    let mut checked = 0;
    while checked < 1_000_000 {
        let v = f64::from_bits(next(&mut state));
        if !v.is_finite() {
            continue;
        }
        let l = Literal::from_f64(v);
        assert!(l.is_valid(), "seed {SEED:#x}: {v:e} → {l:?}");
        // `==`, not bits: -0.0 reads back as ZERO's 0.0, which A3 makes the one zero
        assert!(l.value() == v, "seed {SEED:#x}: {v:e} → {l:?}");
        checked += 1;
    }
}

#[test]
#[should_panic(expected = "a built number is finite")]
fn from_f64_refuses_nan() {
    let _ = Literal::from_f64(f64::NAN);
}

#[test]
fn eq_value_compares_meaning() {
    let k = lit(1, 0, 3, NumForm::Scale);
    let e3 = lit(1, 0, 3, NumForm::Exponent);
    let plain = lit(1000, 0, 0, NumForm::Plain);
    let point = lit(10000, 1, 0, NumForm::Plain);
    for (a, b) in [(k, e3), (k, plain), (e3, point), (plain, point)] {
        assert!(a.eq_value(b), "{a:?} = {b:?}");
        assert!(b.eq_value(a), "{b:?} = {a:?}");
    }
    assert_ne!(k, e3, "field by field, 1k and 1e3 differ");
    assert!(!k.eq_value(lit(1001, 0, 0, NumForm::Plain)));
    assert!(!k.eq_value(lit(-1, 0, 3, NumForm::Scale)));
    // 1mil = 25.4e-6 exactly: 25.4u
    assert!(lit(1, 0, 0, NumForm::Mil).eq_value(lit(254, 1, -6, NumForm::Scale)));
    // exact, not by double: these two round to the same double but are different decimals
    let a = lit(9_007_199_254_740_993, 0, 0, NumForm::Plain);
    let b = lit(9_007_199_254_740_992, 0, 0, NumForm::Plain);
    assert_eq!(a.value(), b.value());
    assert!(!a.eq_value(b));
    assert!(Literal::ZERO.eq_value(Literal::ZERO));
}

#[test]
fn every_invariant_has_a_breaking_literal() {
    let broken = [
        lit(1_000_000_000_000_000_000, 0, 0, NumForm::Plain), // |mantissa| = 10^18
        lit(-1_000_000_000_000_000_000, 0, 0, NumForm::Plain),
        lit(1, 19, 0, NumForm::Plain),     // frac > 18
        lit(1, 0, 1, NumForm::Plain),      // Plain with an exponent
        lit(1, 0, -3, NumForm::Mil),       // Mil with an exponent
        lit(1, 0, 4, NumForm::Scale),      // no scale letter states 10^4
        lit(1, 0, 0, NumForm::Scale),      // nor 10^0
        lit(0, 1, 0, NumForm::Plain),      // a non-canonical zero
        lit(0, 0, 3, NumForm::Scale),      // another
        lit(1, 0, 400, NumForm::Exponent), // value overflows
    ];
    for l in broken {
        assert!(!l.is_valid(), "{l:?} breaks an invariant");
    }
    let legal = [
        lit(999_999_999_999_999_999, 0, 0, NumForm::Plain),
        lit(1, 18, 0, NumForm::Plain),
        lit(1, 0, 0, NumForm::Plain),
        lit(1, 0, 0, NumForm::Mil),
        lit(1, 0, -2, NumForm::Scale),
        lit(1, 0, 15, NumForm::Scale),
        lit(1, 0, -18, NumForm::Scale),
        Literal::ZERO,
        lit(1, 0, 300, NumForm::Exponent),
        lit(1, 0, -400, NumForm::Exponent), // underflows to 0.0: finite
    ];
    for l in legal {
        assert!(l.is_valid(), "{l:?} is legal");
    }
}
