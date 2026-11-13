//! R98: a run item whose effect depends on its place in the run order, inside a subcircuit body,
//! is `RunOrderInSubckt`, and so is a print or a measure there (no reader produces one); the same
//! item at the top, and a whole-run item in the body, are not.
//!
//! Oracle: the ruling's list (an option, environment value or save from here, an alter, a loop,
//! a reset; whole-run items are placements, R27), each deck read by hand.
//!
//! Contents: tests only.

use spicerack_ir::{Design, Diags, Violation, check, diag::violation_code};

// ---- private ----

/// The violations `check` reports on `lines` (items at depth 1), read unchecked.
fn violations(lines: &str) -> Vec<Violation> {
    let text = format!("file \"t.sp\" deck\n{lines}\n");
    let mut d = Design::new();
    let mut diags = Diags::new();
    spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{text}: {:?}", diags.rows());
    check(&d, &mut diags);
    let by_code = |c| Violation::ALL.into_iter().find(|v| violation_code(*v) == c);
    diags
        .rows()
        .iter()
        .filter_map(|r| by_code(r.code))
        .collect()
}

#[test]
fn a_run_order_item_in_a_subcircuit_is_refused() {
    let order = [
        "alter (param p) value=2",
        "reset saves",
        "option rel_tol value=1m",
        "env temp=85",
        "save from_here (v (a))",
        "loop sweep (temp) (lin start=1 stop=2 step=1)\n{in}  analysis op",
        "print tran (v (a))",
        "measure m tran find (v (a)) (at value=1n)",
    ];
    for item in order {
        let top = format!("  param p = 1\n  {}", item.replace("{in}", "  "));
        assert_eq!(violations(&top), vec![], "at the top: {item}");
        let body = format!(
            "  param p = 1\n  subckt s (a)\n    {}",
            item.replace("{in}", "    ")
        );
        let got = violations(&body);
        assert!(
            !got.is_empty() && got.iter().all(|&v| v == Violation::RunOrderInSubckt),
            "in a body: {item}: {got:?}"
        );
    }
    // whole-run items in a body are placements
    for item in [
        "option rel_tol value=1m whole_run",
        "env temp=27 whole_run",
        "save (v (a))",
    ] {
        let body = format!("  subckt s (a)\n    {item}");
        assert_eq!(violations(&body), vec![], "{item}");
    }
}
