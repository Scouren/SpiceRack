//! Decimal literals (Addendum A3): `Literal::value` against the decimal's correctly rounded
//! double, `from_f64` as its inverse, `eq_value` across forms, and one literal per broken
//! invariant beside a legal neighbour.
//!
//! Oracles: Rust's own float literals, which the compiler rounds correctly (an independent
//! implementation of decimal → double); the inverse `from_f64(v).value() == v` over a fixed list
//! and 10⁶ generated doubles (seed printed on failure); hand-derived values in the comments.
//!
//! Contents: none (tests only).

use spicerack_ir::{Design, Literal, NumForm, Role, Span};

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

#[test]
fn meaning_compares_reals_by_value_and_ints_by_form() {
    // A15 K4 (R28): `eq_meaning` equates two real literals of one value whatever their forms,
    // an Int only with an Int; `eq_ignoring_spans` compares the forms too
    let deck = |l: Literal| {
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let (_, top) = b.file("t.sp", Role::Deck);
            let v = b.literal(l);
            b.param(top, "p", v, Span::NONE);
        }
        d
    };
    let reals = [
        lit(1, 0, 3, NumForm::Scale),
        lit(1, 0, 3, NumForm::Exponent),
        lit(1000, 0, 0, NumForm::Plain),
        lit(10000, 1, 0, NumForm::Plain),
    ];
    for a in reals {
        for b in reals {
            assert!(deck(a).eq_meaning(&deck(b)), "{a:?} means {b:?}");
            assert_eq!(
                deck(a).eq_ignoring_spans(&deck(b)),
                a == b,
                "{a:?} vs {b:?}"
            );
        }
    }
    let int = lit(1000, 0, 0, NumForm::Int);
    assert!(deck(int).eq_meaning(&deck(int)));
    for r in reals {
        assert!(
            !deck(int).eq_meaning(&deck(r)),
            "an Int is not the real {r:?}"
        );
    }
}

#[test]
fn digits_as_written_are_one_literal() {
    // the dialects' audit, row 7: the digits-to-literal arithmetic each reader carried, once
    // (oracle: the literals worked by hand below)
    use spicerack_ir::literal::{Decimal, decimal};
    let short = |m, f, e, form| Some(Decimal::Short(lit(m, f, e, form)));
    let cases: [(&str, i32, NumForm, Option<Decimal<'_>>); 13] = [
        ("0.005", 0, NumForm::Plain, short(5, 3, 0, NumForm::Plain)),
        (
            "-1.50",
            3,
            NumForm::Scale,
            short(-150, 2, 3, NumForm::Scale),
        ),
        ("007", 0, NumForm::Plain, short(7, 0, 0, NumForm::Plain)),
        (
            "2.",
            -2,
            NumForm::Exponent,
            short(2, 0, -2, NumForm::Exponent),
        ),
        ("25", 0, NumForm::Mil, short(25, 0, 0, NumForm::Mil)),
        (
            "-0.00",
            0,
            NumForm::Plain,
            Some(Decimal::Short(Literal::ZERO)),
        ),
        ("0", 0, NumForm::Int, short(0, 0, 0, NumForm::Int)),
        // 19 significant digits: the digits stay as written (A18)
        (
            "1234567890123456789",
            0,
            NumForm::Plain,
            Some(Decimal::Long("1234567890123456789", 0, NumForm::Plain)),
        ),
        // more fraction digits than a literal holds: the fraction moves into the power, exactly
        (
            "0.0000000000000000001",
            0,
            NumForm::Plain,
            short(1, 0, -19, NumForm::Exponent),
        ),
        ("1", 400, NumForm::Exponent, None), // not finite
        ("1", 1, NumForm::Plain, None),      // a plain literal has no power
        ("1", 40_000, NumForm::Exponent, None),
        ("1x", 0, NumForm::Plain, None),
    ];
    let mut wrong = Vec::new();
    for (digits, exp10, form, want) in cases {
        let got = decimal(digits, exp10, form);
        if got != want {
            wrong.push(format!(
                "{digits} e{exp10} {form:?}: {got:?}, want {want:?}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_decimal_is_the_correctly_rounded_value_of_its_digits() {
    // oracle: Rust's decimal parser on the same digits and power, an independent decimal ->
    // double; 10^4 random significands of 1 to 25 digits, a point anywhere, a sign, a power
    use spicerack_ir::literal::{Decimal, decimal};
    let mut state = 7u64;
    let mut d = Design::new();
    for k in 0..10_000 {
        let len = 1 + (next(&mut state) % 25) as usize;
        let mut s: String = (0..len)
            .map(|_| char::from(b'0' + (next(&mut state) % 10) as u8))
            .collect();
        let point = (next(&mut state) % (len as u64 + 1)) as usize;
        if point < len {
            s.insert(point, '.');
        }
        if next(&mut state).is_multiple_of(2) {
            s.insert(0, '-');
        }
        let exp10 = (next(&mut state) % 61) as i32 - 30;
        let want: f64 = format!("{s}e{exp10}").parse().expect("Rust reads it");
        let got = match decimal(&s, exp10, NumForm::Exponent) {
            Some(Decimal::Short(l)) => l.value(),
            Some(Decimal::Long(digits, e, form)) => {
                let mut b = d.builder();
                let id = b.long_literal(digits, e, form);
                let spicerack_ir::Expr::Num(n) = d.expr(id) else {
                    unreachable!("a literal row")
                };
                d.literal_value(d.num(n))
            }
            None => panic!("case {k}: {s}e{exp10} is a literal"),
        };
        // `==`, not the bits: the IR's one zero literal is +0 (`Literal::ZERO`), `-0` too
        assert!(got == want, "case {k}: {s}e{exp10}: {got} against {want}");
    }
}

#[test]
fn the_dialects_spellings_are_the_shared_digits_and_power() {
    // the dialects' audit, row 7, writer half. Oracle: the spellings the two dialects' own tests
    // pin (ir-spine `src/dialect/vacask/number.rs` `spell_reads_back`, `src/dialect/ngspice/
    // number.rs` `spelling_*` and `a_long_literal_spells_*`), each dialect's letter table as
    // plain data (vacask `SCALE_LETTERS`; ngspice `letter`, inpeval.c: no centi, no peta)
    use NumForm::{Exponent, Int, Mil, Plain, Scale};
    use spicerack_ir::literal::{write_digits, write_power};
    const VACASK: &[(i16, &str)] = &[
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
    const NGSPICE: &[(i16, &str)] = &[
        (12, "t"),
        (9, "g"),
        (6, "meg"),
        (3, "k"),
        (-3, "m"),
        (-6, "u"),
        (-9, "n"),
        (-12, "p"),
        (-15, "f"),
        (-18, "a"),
    ];
    // what each dialect keeps of its own: VACASK's point on a real whole number, `mil`, and an
    // `Int` spelled as its digits
    let vacask = |l: Literal| {
        let mut out = String::new();
        write_digits(i128::from(l.mantissa), l.frac, &mut out);
        match l.form {
            Int => {}
            Plain if l.frac == 0 => out.push('.'),
            Mil => out.push_str("mil"),
            form => write_power(l.exp10, form, VACASK, &mut out),
        }
        out
    };
    let ngspice = |l: Literal| {
        let mut out = String::new();
        write_digits(i128::from(l.mantissa), l.frac, &mut out);
        match l.form {
            Mil => out.push_str("mil"),
            form => write_power(l.exp10, form, NGSPICE, &mut out),
        }
        out
    };
    let vacask_cases = [
        (lit(25, 1, 0, Plain), "2.5"),
        (lit(5, 1, 0, Plain), "0.5"),
        (lit(1, 0, -3, Exponent), "1e-3"),
        (lit(100, 0, -6, Scale), "100u"),
        (lit(1, 0, 6, Scale), "1meg"),
        (lit(1, 0, 0, Mil), "1mil"),
        (lit(7, 0, -2, Scale), "7e-2"),
        (lit(-2, 0, 0, Int), "-2"),
        (lit(2, 0, 0, Plain), "2."),
        (Literal::ZERO, "0."),
    ];
    let ngspice_cases = [
        (lit(3, 0, -2, Scale), "3e-2"),
        (lit(5, 3, 0, Plain), "0.005"),
        (lit(7, 0, 12, Scale), "7t"),
        (lit(15, 1, -3, Exponent), "1.5e-3"),
        (
            lit(27500000000000003, 0, -19, Exponent),
            "27500000000000003e-19",
        ),
        (lit(2, 0, 0, Mil), "2mil"),
    ];
    let mut wrong = Vec::new();
    for (l, want) in vacask_cases {
        if vacask(l) != want {
            wrong.push(format!("vacask {l:?}: {} not {want}", vacask(l)));
        }
    }
    for (l, want) in ngspice_cases {
        if ngspice(l) != want {
            wrong.push(format!("ngspice {l:?}: {} not {want}", ngspice(l)));
        }
    }
    // ngspice's numparam `mil`: 25.4e-6 folded into the digits (2 mil = 508e-7), its own rule
    let mut folded = String::new();
    write_digits(2 * 254, 0, &mut folded);
    write_power(-7, Exponent, NGSPICE, &mut folded);
    assert_eq!(folded, "508e-7");
    // a long literal: its significand as written, then its power in its written form
    let mut long = String::from("1.234567890123456789");
    write_power(-6, Scale, NGSPICE, &mut long);
    assert_eq!(long, "1.234567890123456789u");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
