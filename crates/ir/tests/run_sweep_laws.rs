//! R99's sweep laws: `on_grid` (whether a `dec`/`oct` stop is provably on its grid), and the
//! validity of the end rules `Stretch` and `NgDecade` and the count reading `NgLin`.
//!
//! Oracles: for `on_grid`, arithmetic done by hand on the decimal digits (a ratio that is an
//! exact power of the base, or not), with R99's runs as the cases: `dec 10 1 1e6` (61 values,
//! identical on both simulators), `dec 10 10 20k` (34 against 35), `dec 2 1k 5k`,
//! `oct 3 1k 5k`. For validity, the rulings' statement (R99, the lead's choice of the end
//! field): `NgDecade` only on `dec`, `Stretch` on `dec`/`oct`, neither on a sweep on its grid,
//! `NgLin` only on a linear frequency sweep by count whose count is not a literal.
//!
//! Contents: tests only.

use spicerack_ir::dc_ac::Ac;
use spicerack_ir::{
    Analysis, AnalysisKind, Design, Diags, ItemKind, SweepSpec, Violation, check,
    diag::violation_code, on_grid,
};

// ---- private ----

/// Reads `lines` (items at depth 1) as IR text, unchecked; asserts it reads clean.
fn read(lines: &str) -> Design {
    let text = format!("file \"t.sp\" deck\n{lines}\n");
    let mut d = Design::new();
    let mut diags = Diags::new();
    spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{text}: {:?}", diags.rows());
    d
}

/// The one AC analysis's sweep of `d`.
fn ac_sweep(d: &Design) -> SweepSpec {
    let (item, _) = d
        .reachable()
        .into_iter()
        .find(|(i, _)| i.kind == ItemKind::Analysis)
        .expect("an analysis");
    let row: Analysis = d.row(item.index);
    assert_eq!(row.kind, AnalysisKind::Ac);
    d.detail::<Ac>(row.detail).sweep
}

/// `on_grid` of the AC sweep `sweep`.
fn grid(sweep: &str) -> bool {
    let d = read(&format!("  analysis ac {sweep}"));
    on_grid(&d, &ac_sweep(&d))
}

/// Whether `check` reports `FieldDomain` on the deck, and nothing else.
fn refused(lines: &str) -> bool {
    let d = read(lines);
    let mut diags = Diags::new();
    check(&d, &mut diags);
    let codes: Vec<_> = diags.rows().iter().map(|r| r.code).collect();
    assert!(
        codes
            .iter()
            .all(|&c| c == violation_code(Violation::FieldDomain)),
        "{lines}: {codes:?}"
    );
    !codes.is_empty()
}

#[test]
fn a_stop_is_on_its_grid_exactly_when_the_ratio_is_a_power_of_the_base() {
    for (sweep, want) in [
        ("(dec start=1 stop=1e6 points=10)", true),
        ("(dec start=1 stop=1meg points=10)", true),
        ("(dec start=10 stop=1k points=3)", true),
        ("(dec start=1k stop=100 points=3)", true),
        ("(dec start=2.5 stop=25 points=7)", true),
        ("(dec start=10 stop=20k points=10)", false),
        ("(dec start=1k stop=5k points=2)", false),
        ("(dec start=1 stop=2 points=10)", false),
        ("(oct start=1k stop=4k points=3)", true),
        ("(oct start=1k stop=500 points=3)", true),
        ("(oct start=1k stop=5k points=3)", false),
        ("(oct start=1 stop=10 points=3)", false),
        ("(oct start=1k stop=1.5k points=1)", false),
        ("(log start=1 stop=10 points=3)", false),
        ("(lin start=1 stop=10 points=3)", false),
        ("(dec center_span start=10 stop=10 points=3)", false),
        ("(dec start=0 stop=10 points=3)", false),
    ] {
        assert_eq!(grid(sweep), want, "{sweep}");
    }
    // a read is not provably anything
    let d = read("  param f = 1\n  analysis ac (dec start=f stop=1e6 points=10)");
    assert!(!on_grid(&d, &ac_sweep(&d)));
}

#[test]
fn an_end_rule_stands_only_where_it_means_something() {
    // off the grid: each rule on its kinds
    assert!(!refused(
        "  analysis ac (dec stretch start=10 stop=20k points=10)"
    ));
    assert!(!refused(
        "  analysis ac (oct stretch start=1k stop=5k points=3)"
    ));
    assert!(!refused(
        "  analysis ac (dec ng_decade start=10 stop=20k points=10)"
    ));
    assert!(refused(
        "  analysis ac (oct ng_decade start=1k stop=5k points=3)"
    ));
    assert!(refused(
        "  analysis ac (log stretch start=1 stop=7 points=3)"
    ));
    assert!(refused(
        "  analysis ac (lin stretch start=1 stop=7 points=3)"
    ));
    // on the grid every rule gives the same values: the sweep states `Grid` (R47)
    assert!(refused(
        "  analysis ac (dec stretch start=1 stop=1e6 points=10)"
    ));
    assert!(refused(
        "  analysis ac (dec ng_decade start=1 stop=1e6 points=10)"
    ));
}

#[test]
fn ngspices_count_reading_stands_only_on_a_count_that_is_not_a_literal() {
    assert!(!refused(
        "  param n = 2\n  analysis ac (lin ng_lin start=1 stop=2 points=n)"
    ));
    assert!(refused(
        "  analysis ac (lin ng_lin start=1 stop=2 points=2)"
    ));
    assert!(refused(
        "  param n = 2\n  analysis ac (lin ng_lin start=1 stop=2 step=n)"
    ));
    assert!(refused(
        "  param n = 2\n  analysis ac (dec ng_lin start=1 stop=7 points=n)"
    ));
    // a loop's sweep is no frequency sweep
    assert!(refused(
        "  param n = 2\n  loop sweep (temp) (lin ng_lin start=1 stop=2 points=n)\n    analysis op"
    ));
}
