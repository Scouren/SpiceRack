//! The census's placements (the lead's ruling, 2026-11-13): an item kind inside a scope kind no
//! dialect expresses is `Misplaced`; one some dialect expresses stays legal. Each placement the
//! census found unproduced is listed here once, on one side.
//!
//! Oracle: the ruling's table in `rewrite/IR-PRUNE.md` group 6, each row from a spec row or a
//! run (`crates/ir/tests/fixtures/placed/`, ngspice-45), written here by hand; never the table
//! in `check_scopes.rs`.
//!
//! Contents: tests only.

use spicerack_ir::{Design, Diags, Violation, check, diag::violation_code};

/// The placements made violations: (scope opener, item). A title below a deck's top, the
/// census's other four, is no design at all: `Builder::title` asserts it, IR text refuses it
/// (P9001).
const REFUSED: [(&str, &str); 31] = [
    (BRANCH, "section l2"),
    (BRANCH, "func f(x) = x"),
    (BRANCH, "global g"),
    (BRANCH, "device_library resistor"),
    (BRANCH, "statistics process (p gauss spread=1)"),
    (BRANCH, "option rel_tol value=1m"),
    (BRANCH, "save (v (a))"),
    (BRANCH, "print tran (v (a))"),
    (BRANCH, "alter (param p) value=2"),
    (
        BRANCH,
        "loop sweep (temp) (list values=[1])\n{in}  analysis op",
    ),
    (BRANCH, "reset saves"),
    (BRANCH, "elaborate (s)"),
    (LOOP, "section l2"),
    (LOOP, "subckt t (a)"),
    (LOOP, "if (1)\n{in}  element resistor r9 (a 0) value=1"),
    (LOOP, "include \"x.sp\""),
    (LOOP, "load veriloga \"x.va\""),
    (LOOP, "device_library resistor"),
    (LOOP, "global g"),
    (LOOP, "param q = 1"),
    (LOOP, "func f(x) = x"),
    (LOOP, "model dm diode params=(is=1e-14)"),
    (LOOP, "statistics process (p gauss spread=1)"),
    (LOOP, "element resistor r9 (a 0) value=1"),
    (SECTION, "section l2"),
    (SECTION, "elaborate (s)"),
    (SUBCKT, "section l2"),
    (SUBCKT, "load veriloga \"x.va\""),
    (SUBCKT, "device_library resistor"),
    (SUBCKT, "statistics process (p gauss spread=1)"),
    (SUBCKT, "elaborate (s)"),
];

/// The placements kept: some dialect expresses each (group 6 names which).
const KEPT: [(&str, &str); 18] = [
    (BRANCH, "analysis op"),
    (BRANCH, "env temp=85"),
    (BRANCH, "nodeset ic transient node=(a) value=1"),
    (BRANCH, "measure m tran find (v (a)) (at value=1n)"),
    (BRANCH, "param q = 1"),
    (BRANCH, "load veriloga \"x.va\""),
    (BRANCH, "include \"x.sp\""),
    (BRANCH, "subckt t (a)"),
    (LOOP, "measure m tran find (v (a)) (at value=1n)"),
    (LOOP, "print tran (v (a))"),
    (SECTION, "alter (param p) value=2"),
    (
        SECTION,
        "loop sweep (temp) (list values=[1])\n{in}  analysis op",
    ),
    (SECTION, "reset saves"),
    (SECTION, "measure m tran find (v (a)) (at value=1n)"),
    (SECTION, "print tran (v (a))"),
    (SECTION, "save (v (a))"),
    (SECTION, "statistics process (p gauss spread=1)"),
    (SUBCKT, "save (v (a))"),
];

const BRANCH: &str = "if (1)";
const LOOP: &str = "loop sweep (temp) (list values=[1])";
const SECTION: &str = "section lib1";
const SUBCKT: &str = "subckt s2 (a)";

// ---- private ----

#[test]
fn a_placement_no_dialect_expresses_is_refused() {
    for (opener, item) in REFUSED {
        let got = violations(opener, item);
        assert!(
            !got.is_empty() && got.iter().all(|&v| v == Violation::Misplaced),
            "{opener} / {item}: {got:?}"
        );
    }
}

#[test]
fn a_placement_some_dialect_expresses_stays_legal() {
    for (opener, item) in KEPT {
        assert_eq!(violations(opener, item), vec![], "{opener} / {item}");
    }
    // the same items at the top are legal
    for (_, item) in REFUSED.iter().chain(&KEPT) {
        let top = format!(
            "  param p = 1\n  subckt s (a)\n  {}",
            item.replace("{in}", "  ")
        );
        assert_eq!(top_violations(&top), vec![], "at the top: {item}");
    }
}

/// The violations `check` reports on `item` inside one scope opened by `opener` at the top.
fn violations(opener: &str, item: &str) -> Vec<Violation> {
    let lines = format!(
        "  param p = 1\n  subckt s (a)\n  {opener}\n    {}",
        item.replace("{in}", "    ")
    );
    top_violations(&lines)
}

/// The violations `check` reports on a deck of `lines` (items at depth 1), read unchecked.
fn top_violations(lines: &str) -> Vec<Violation> {
    let (d, mut diags) = read(lines);
    codes(&d, &mut diags)
}

/// A deck of `lines` (items at depth 1), read unchecked; asserts the text reads.
fn read(lines: &str) -> (Design, Diags) {
    let text = format!("file \"t.sp\" deck\n{lines}\n");
    let mut d = Design::new();
    let mut diags = Diags::new();
    spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{text}: {:?}", diags.rows());
    (d, diags)
}

/// The violations `check` reports on `d`.
fn codes(d: &Design, diags: &mut Diags) -> Vec<Violation> {
    check(d, diags);
    let by_code = |c| Violation::ALL.into_iter().find(|v| violation_code(*v) == c);
    diags
        .rows()
        .iter()
        .filter_map(|r| by_code(r.code))
        .collect()
}
