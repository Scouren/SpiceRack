//! Long literals (A18, R37): a literal with more significant digits than the mantissa holds
//! keeps its digits as written (`NumForm::Long`); one bound decides long against short; its
//! value is the correctly rounded value of the digits; equality is exact decimal equality, as
//! the short forms'; IR text spells the digits; a feature names it.
//!
//! Oracles: Rust's correctly rounded decimal parser (`str::parse::<f64>`) on the digits; exact
//! decimal values worked by hand; the inverse for IR text; a 23-digit sky130-style value.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    Design, Diags, Expr, ExprId, Extra, Feature, Literal, NumForm, Role, Span, check, dump,
    features_used, literal, parse,
};

// ---- private ----

/// A deck holding `p = <expression>` read from IR text; the design and `p`'s value.
fn read(value: &str) -> (Design, ExprId) {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(
        &format!("file \"t.sp\" deck\n  param p = {value}\n"),
        &mut d.builder(),
        &mut diags,
    );
    assert!(diags.is_empty(), "{value}: {:?}", diags.rows());
    let row: spicerack_ir::item::Param = d.row(0);
    (d, row.value)
}

fn num(d: &Design, e: ExprId) -> Literal {
    match d.expr(e) {
        Expr::Num(n) => d.num(n),
        other => panic!("not a literal: {other:?}"),
    }
}

fn text(d: &Design) -> String {
    let mut out = String::new();
    dump(d, d.entry(), &mut out);
    out
}

#[test]
fn one_bound_decides_long_against_short() {
    // leading zeros do not count, trailing zeros as written do (the short mantissa's count)
    assert!(!literal::exceeds_mantissa(18));
    assert!(literal::exceeds_mantissa(19));
    assert_eq!(
        literal::significant_digits("0.000123"),
        3,
        "leading zeros not counted"
    );
    assert_eq!(
        literal::significant_digits("1.2300"),
        5,
        "trailing zeros as written counted"
    );
    assert_eq!(literal::significant_digits("-000123456789012345678"), 18);
    // 18 digits: short; 19: long
    let (d, e) = read("0.123456789012345678");
    assert_eq!(num(&d, e).form, NumForm::Plain);
    let (d, e) = read("0.1234567890123456789");
    assert_eq!(num(&d, e).form, NumForm::Long);
}

#[test]
fn a_long_literal_keeps_its_digits_and_rounds_correctly() {
    // a sky130-style card value: 23 significant digits
    let digits = "0.12345678901234567890123";
    for (written, want) in [
        (digits.to_string(), format!("{digits}e0")),
        (format!("{digits}e-3"), format!("{digits}e-3")),
        (format!("-{digits}u"), format!("-{digits}e-6")),
    ] {
        let (d, e) = read(&written);
        let l = num(&d, e);
        assert_eq!(l.form, NumForm::Long, "{written}");
        let oracle: f64 = want.parse().unwrap();
        assert_eq!(d.literal_value(l).to_bits(), oracle.to_bits(), "{written}");
        // IR text spells the digits as written, round trip exact
        let t = text(&d);
        assert!(t.contains(&written), "{written} in {t}");
        let mut back = Design::new();
        parse(&t, &mut back.builder(), &mut Diags::new());
        assert!(back.eq_ignoring_spans(&d), "{written}");
        let mut diags = Diags::new();
        check(&d, &mut diags);
        assert!(diags.is_empty(), "{written}: {:?}", diags.rows());
    }
}

#[test]
fn long_equality_is_exact_decimal_equality() {
    let lit = |s: &str| {
        let (d, e) = read(s);
        (d, e)
    };
    // the same decimal written with trailing zeros and another power: equal by meaning
    let (a, ea) = lit("0.12345678901234567890123");
    let (b, _) = lit("123.45678901234567890123000e-3");
    assert!(a.literal_eq(num(&a, ea), num(&a, ea)));
    assert!(a.eq_meaning(&b), "one decimal, two spellings");
    assert!(!a.eq_ignoring_spans(&b), "but not the same digits");
    // two distinct 23-digit decimals that round to one double: never equal
    let (c, ec) = lit("0.12345678901234567890124");
    assert_eq!(
        a.literal_value(num(&a, ea)).to_bits(),
        c.literal_value(num(&c, ec)).to_bits(),
        "they round to one double"
    );
    assert!(!a.eq_meaning(&c), "distinct decimals stay distinct");
    // a long literal against a short one of a different value
    let (s, _) = lit("0.123456789012345678");
    assert!(!a.eq_meaning(&s));
}

#[test]
fn the_bare_row_refuses_a_long_literal() {
    let (d, e) = read("0.1234567890123456789");
    let l = num(&d, e);
    assert!(!l.is_valid(), "a bare Long is not a short literal");
    assert!(
        std::panic::catch_unwind(|| l.value()).is_err(),
        "value() asserts not Long"
    );
    assert_eq!(
        d.long_digits(l),
        Some(("0.1234567890123456789", NumForm::Plain))
    );
    let short = Literal::from_f64(1.5);
    assert_eq!(d.long_digits(short), None);
    assert_eq!(d.literal_value(short), 1.5, "the short path");
}

#[test]
fn check_refuses_a_long_literal_that_fits() {
    // the builder asserts it; a hand-broken table is what check sees
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let v = b.long_literal("0.1234567890123456789", 0, NumForm::Plain);
        b.param(top, "p", v, Span::NONE);
    }
    let mut diags = Diags::new();
    check(&d, &mut diags);
    assert!(diags.is_empty());
    assert!(features_used(&d).contains(Feature::Extra(Extra::LongLiteral)));
    assert!(
        std::panic::catch_unwind(move || {
            let mut d = Design::new();
            d.builder().long_literal("0.123", 0, NumForm::Plain);
        })
        .is_err(),
        "a literal that fits is built short"
    );
}

#[test]
fn every_zero_spelling_reads_to_one_literal_and_writes_back_to_it() {
    // the audit's -0 finding: every spelling of a zero reads to the one zero of its type and
    // writes back to that zero's own spelling, which reads to the same literal (oracle: the
    // round trip, spelling by spelling). The design (one zero) settled by two runs, 2026-11-12:
    // ngspice-45 has no `atan2` (numparam "Formula() error", the B source "no such function
    // 'atan2'") and prints `v = 0.000000e+00` for a source of `{-0}`, `{-(0)}`, `{0}`,
    // `{0*(-1)}` and B sources `V=-0`, `V=-(0)`, `V=0*(-1)` alike. VACASK (36be560): a control
    // variable `-0.0` prints `-0` and `0.0` prints `0`, but no value differs: atan2 gives `0`
    // for `(-0.0, -1.0)` and `-0` for `(0.0, -1.0)` (its atan2 is atan(y/x) with no +pi when
    // x < 0 <= y: `(1.0, -1.0)` gives -0.785398, C gives 2.35619), `1/(-0)` is "Division by
    // zero.", and a resistor of `1k*(4+atan2(-0.0, -1.0))` reads 4000 like the +0 forms. No
    // number either simulator computes sees a zero's sign; only VACASK's `print` shows it.
    let mut wrong = Vec::new();
    for word in [
        "-0",
        "-0.0",
        "-0e3",
        "-0k",
        "-0mil",
        "-0i",
        "0",
        "0.000",
        "-0.0000000000000000000",
    ] {
        let (d, e) = read(word);
        let once = text(&d);
        let (d2, e2) = read(once.split(" = ").nth(1).unwrap_or("").trim());
        let same = d.literal_eq(num(&d, e), num(&d2, e2)) && text(&d2) == once;
        if !same || once.contains("-0") {
            wrong.push(format!("{word}: wrote {}", once.trim()));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
