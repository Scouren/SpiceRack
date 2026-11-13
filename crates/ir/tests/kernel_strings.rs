//! A string as an operand (the lead's rule on the strings runs): under `==` or `!=` it is the
//! feature `Extra::StringCompare`, since VACASK computes it; under any other operator, as a
//! function's, a user call's or a draw's argument, or as a conditional's condition, it is
//! `Violation::StringOperand`. A string root, a list element and a conditional's branch stay
//! values (A25 item 4).
//!
//! Oracle: the runs in `tests/fixtures/strings/runs.out` (VACASK computes `s=="tt"` and
//! `s!="ff"` in a value and in `@if`, refuses `s+1k`: "Operation not supported for string and
//! real."; ngspice-45 refuses a string `.param` itself), each deck read by hand.
//!
//! Contents: tests only.

use spicerack_ir::{
    Design, Diags, Extra, Feature, Violation, check, diag::violation_code, features_used,
};

// ---- private ----

/// The violations `check` reports on `lines` (items at depth 1), and whether the design uses
/// `StringCompare`.
fn read(lines: &str) -> (Vec<Violation>, bool) {
    let text = format!("file \"t.sp\" deck\n{lines}\n");
    let mut d = Design::new();
    let mut diags = Diags::new();
    spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{text}: {:?}", diags.rows());
    check(&d, &mut diags);
    let by_code = |c| Violation::ALL.into_iter().find(|v| violation_code(*v) == c);
    let found = diags
        .rows()
        .iter()
        .filter_map(|r| by_code(r.code))
        .collect();
    let compare = features_used(&d).contains(Feature::Extra(Extra::StringCompare));
    (found, compare)
}

#[test]
fn a_string_compared_for_equality_is_a_feature() {
    for value in [
        "(s == \"tt\") ? 1 : 2",
        "(s != \"ff\") ? 1 : 2",
        "(\"tt\" == s) ? 1 : 2",
    ] {
        let lines = format!("  param s = \"tt\"\n  param c = {value}");
        assert_eq!(read(&lines), (vec![], true), "{value}");
    }
}

#[test]
fn a_string_under_any_other_operator_or_as_an_argument_is_a_violation() {
    for value in [
        "\"tt\" + 1",
        "1 < \"tt\"",
        "-\"tt\"",
        "sin(\"tt\")",
        "\"tt\" ? 1 : 2",
    ] {
        let lines = format!("  param c = {value}");
        let (found, _) = read(&lines);
        assert_eq!(found, vec![Violation::StringOperand], "{value}");
    }
}

#[test]
fn a_string_value_stays_a_value() {
    for value in ["\"tt\"", "(1 > 0) ? \"a\" : \"b\"", "[\"a\", \"b\"]"] {
        let lines = format!("  param c = {value}");
        assert_eq!(read(&lines), (vec![], false), "{value}");
    }
}
