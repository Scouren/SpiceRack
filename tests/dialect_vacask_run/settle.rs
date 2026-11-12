//! The settle decks' facts (`tests/fixtures/dialect/vacask/settle/run/`): what VACASK 36be560
//! printed and wrote for each deck, asserted, so that a binary update that changes a rule this
//! area cites goes red before a rule quietly goes wrong (R5).
//!
//! Contents: tests only.

use super::{close, raw, settle, value, vectors};

// ---- private ----

#[test]
fn ic_next_an_analysis_list_holds_for_that_analysis_only() {
    // Run e1: `ic={"b", 1}` with icmode="uic" starts t1 at 1 V; t2 starts at 0.
    let r = settle("ic_next.sim");
    assert!(
        close(value(&raw(&r, "t1.raw"), "b", 0), 1.0, 1e-12),
        "{}",
        r.out
    );
    assert!(
        close(value(&raw(&r, "t2.raw"), "b", 0), 0.0, 1e-12),
        "{}",
        r.out
    );
}

#[test]
fn save_scope_v_governs_op_and_tran_not_ac() {
    // Run e5: `default` saves every node and every branch current (an inductor's too);
    // `save v(b)` limits op and tran to b, the ac keeps its default; `full` adds nothing here.
    let r = settle("save_scope.sim");
    let all = ["a", "b", "c", "l1:flow(br)", "v1:flow(br)"];
    assert_eq!(vectors(&raw(&r, "opd.raw")), all);
    assert_eq!(vectors(&raw(&r, "opv.raw")), ["b"]);
    assert_eq!(vectors(&raw(&r, "trv.raw")), ["time", "b"]);
    let ac = vectors(&raw(&r, "acv.raw"));
    assert_eq!(ac[0], "frequency");
    assert_eq!(&ac[1..], all);
    assert_eq!(vectors(&raw(&r, "opf.raw")), all);
}

#[test]
fn maxstep0_a_zero_ceiling_is_none() {
    // Run e7: `maxstep=0` writes the same raw file as no maxstep (the span/50 cap of tr1), and
    // `start=0` the same as no start.
    let r = settle("maxstep0.sim");
    let get = |n: &str| r.raws.iter().find(|(k, _)| k == n).expect(n).1.clone();
    assert_eq!(get("tq.raw"), get("tw.raw"));
    // VD cmd-analysis-tran: `start` defaults to 0, so a literal 0 is none given (R47).
    assert_eq!(get("tq.raw"), get("tz.raw"));
}

#[test]
fn two_blocks_one_control_block_per_file() {
    let r = settle("two_blocks.sim");
    assert!(
        r.out.contains("Only one control block is allowed."),
        "{}",
        r.out
    );
}

#[test]
fn dup_name_a_second_analysis_of_one_name_is_refused() {
    let r = settle("dup_name.sim");
    assert!(r.out.contains("Analysis 'op1' redefinition."), "{}", r.out);
}

#[test]
fn alter_shape_names_come_before_the_assignments() {
    let r = settle("alter_shape.sim");
    assert!(
        r.out
            .contains("Parser syntax error, unexpected =, expecting ) or \",\""),
        "{}",
        r.out
    );
}

#[test]
fn unknown_option_is_refused() {
    let r = settle("unknown_option.sim");
    assert!(r.out.contains("Parameter 'foo' not found."), "{}", r.out);
}

#[test]
fn sweep_bound_a_sweep_binds_the_next_analysis() {
    let r = settle("sweep_bound.sim");
    assert!(r.out.contains("expecting sweep or analysis"), "{}", r.out);
}

#[test]
fn options_mid_an_option_holds_from_where_it_stands() {
    // `options temp=127` between op1 and op2: r1 = 1k (1 + 1m (127 - 27)) = 1.1k from op2 on.
    let r = settle("options_mid.sim");
    let i = |n: &str| value(&raw(&r, n), "v1:flow(br)", 0).abs();
    assert!(close(i("op1.raw"), 1e-3, 1e-9), "{}", r.out);
    assert!(close(i("op2.raw"), 1.0 / 1100.0, 1e-9), "{}", r.out);
    assert!(close(i("op3.raw"), 1.0 / 1100.0, 1e-9), "{}", r.out);
}

#[test]
fn plan_an_alter_persists() {
    // Run e2: after `alter instance("r2") r=3k` every later op sees the 1k/3k divider.
    let r = settle("plan.sim");
    let b = |n: &str| value(&raw(&r, n), "b", 0);
    assert!(close(b("op1.raw"), 0.5, 1e-9), "{}", r.out);
    assert!(close(b("op2.raw"), 0.75, 1e-9), "{}", r.out);
    assert!(close(b("op3.raw"), 0.75, 1e-9), "{}", r.out);
}

#[test]
fn ac_points_a_lin_count_is_intervals() {
    // VD cmd-analysis-ac: `mode="lin" points=2` is 3 frequencies; dec 10..1k at 2 per decade
    // is 5.
    let r = settle("ac_points.sim");
    assert_eq!(raw(&r, "a1.raw").re(0).len(), 3, "{}", r.out);
    assert_eq!(raw(&r, "a2.raw").re(0).len(), 5, "{}", r.out);
}

#[test]
fn save_from_here_saves_accumulate_from_where_they_stand() {
    // Run sv1: `save v(b)`, op0, op1, `save v(a)`, op2: op1 holds b alone, op2 b and a.
    let r = settle("save_from_here.sim");
    assert_eq!(vectors(&raw(&r, "op0.raw")), ["b"]);
    assert_eq!(vectors(&raw(&r, "op1.raw")), ["b"]);
    assert_eq!(vectors(&raw(&r, "op2.raw")), ["b", "a"]);
}

#[test]
fn save_clear_drops_every_save_before_it() {
    // Run sv2: after `clear saves` op2 holds the default set; a later `save v(b)` starts anew.
    let r = settle("save_clear.sim");
    assert_eq!(vectors(&raw(&r, "op1.raw")), ["a"]);
    assert_eq!(vectors(&raw(&r, "op2.raw")), ["a", "b", "v1:flow(br)"]);
    assert_eq!(vectors(&raw(&r, "op3.raw")), ["b"]);
}

#[test]
fn names_analysis_and_sweep_names_vacask_accepts() {
    // Kind words, reserved words, case (op1 and OP1 are two), `_` and `$`, an analysis and a
    // sweep of one name, one sweep name in two chains: every one runs (A35's naming pass).
    let r = settle("names.sim");
    let names: Vec<&str> = r.raws.iter().map(|(n, _)| n.as_str()).collect();
    for n in [
        "op.raw",
        "tran.raw",
        "save.raw",
        "op1.raw",
        "OP1.raw",
        "_x1.raw",
        "$x.raw",
        "x.raw",
        "axis1.raw",
        "loop1.raw",
    ] {
        assert!(names.contains(&n), "{n} not written: {}", r.out);
    }
}

#[test]
fn names_quoted_an_analysis_name_is_an_identifier() {
    let r = settle("names_quoted.sim");
    assert!(
        r.out.contains("Syntax error, unexpected string."),
        "{}",
        r.out
    );
}

#[test]
fn names_chain_the_sweeps_of_one_analysis_have_distinct_names() {
    let r = settle("names_chain.sim");
    assert!(
        r.out.contains("Sweep does not have a unique name."),
        "{}",
        r.out
    );
}

#[test]
fn names_mc_quoted_an_mc_name_is_an_identifier() {
    let r = settle("names_mc_quoted.sim");
    assert!(
        r.out.contains("Syntax error, unexpected string."),
        "{}",
        r.out
    );
}
