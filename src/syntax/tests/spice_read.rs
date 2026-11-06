//! The SPICE-family reader's phase-7 readings (stream spice-read): each
//! construct it now reads, written by every target in its own form or
//! refused with the code naming what the target lacks. Each test names its
//! clause, the text derived by hand from it, and how many checks it makes.
//!
//! Contents: none (tests only).

use crate::syntax::Dialect;
use crate::syntax::diag::Code;
use crate::syntax::render::render_text;

// ---- private ----

#[test]
fn an_ltspice_startup_transient_is_ltspices_and_refused_elsewhere() {
    // LTspice help, `.TRAN`: `.tran <Tstop> [modifiers]`, the modifier
    // `startup` (sources ramped on over the first 20 µs) and `uic` in any
    // order. LTspice writes its own card (`Tstep` 0: LTspice's default);
    // every other simulator has no such ramp (W0037). Checks: 1 + 6.
    let text = "t\nR1 a 0 1k\nV1 a 0 1\n.tran 750u startup uic\n.end\n";
    let lt = render_text(text, Dialect::Ltspice, "t", Dialect::Ltspice).expect("written");
    assert!(
        lt.text.lines().any(|l| l == ".tran 0 750u UIC startup"),
        "{}",
        lt.text
    );
    for d in [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Spectre,
        Dialect::Vacask,
    ] {
        let e = render_text(text, Dialect::Ltspice, "t", d).expect_err("refused");
        assert!(
            e.diags.iter().any(|g| g.code == Code::W0037),
            "{d:?}: {}",
            e.messages
        );
    }
}

#[test]
fn an_ltspice_backanno_is_left_out_with_its_note() {
    // LTspice help, `.BACKANNO`: the viewer's port-current names, no number
    // of the run. Left out on every target, noted W0036. Checks: 2 per
    // target, 3 targets.
    let text = "t\nR1 a 0 1k\nV1 a 0 1\n.op\n.backanno\n.end\n";
    for d in [Dialect::Ltspice, Dialect::Ngspice, Dialect::Spectre] {
        let w = render_text(text, Dialect::Ltspice, "t", d).expect("written");
        assert!(
            !w.text.to_ascii_lowercase().contains("backanno"),
            "{d:?}: {}",
            w.text
        );
        assert!(w.notes.diags.iter().any(|g| g.code == Code::W0036), "{d:?}");
    }
}

#[test]
fn a_xyce_global_param_is_a_top_level_param_unless_it_reads_a_sweep() {
    // Xyce RG `.GLOBAL_PARAM`; ngspice reads it as the `.param` it is
    // once; Xyce 7.10 runs `{TEMP+10}` (and a value reading it) otherwise
    // as a `.param` (Xyce_Regression BUG_1203_SON/dot_step.cir, both run):
    // that one stays Xyce's text; Xyce writes the rest back as
    // `.global_param` (a constant `temp={…}` differs too). Checks: 2 + 3.
    let text = "t\n.global_param g=2\nR1 1 0 {g*1k}\nI1 0 1 1m\n.op\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice).expect("written");
    assert!(ng.text.lines().any(|l| l == ".param g=2"), "{}", ng.text);
    let xy = render_text(text, Dialect::Xyce, "t", Dialect::Xyce).expect("written");
    assert!(
        xy.text.lines().any(|l| l == ".global_param g=2"),
        "{}",
        xy.text
    );
    let swept = "t\n.global_param a={temp+10}\n.global_param b={2*a}\n.global_param c=3\nR1 1 0 {b+c}\nI1 0 1 1m\n.op\n.end\n";
    let xy = render_text(swept, Dialect::Xyce, "t", Dialect::Xyce)
        .expect("written")
        .text;
    assert!(xy.lines().any(|l| l == ".global_param b={2*a}"), "{xy}");
    assert!(xy.lines().any(|l| l == ".global_param c=3"), "{xy}");
    let ng = render_text(swept, Dialect::Xyce, "t", Dialect::Ngspice).expect_err("refused");
    assert!(
        ng.diags.iter().any(|g| g.code == Code::W0001),
        "{}",
        ng.messages
    );
}

#[test]
fn a_xyce_bare_initial_condition_is_a_node_voltage() {
    // Xyce 7.10 runs `.ic 1 1.0` as `.ic v(1)=1.0` (both run: 0.3679 V at
    // 1 ms on an RC of 1 ms). Checks: 1.
    let text = "t\nC1 1 0 1u\nR1 1 0 1k\n.ic 1 1.0\n.tran 0.1m 1m\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice).expect("written");
    assert!(ng.text.lines().any(|l| l == ".ic V(1)=1"), "{}", ng.text);
}

#[test]
fn a_log_or_list_dc_is_written_where_the_target_sweeps_so() {
    // Xyce RG `.DC` (`DEC src start stop points`, `src LIST v…`; Xyce
    // 7.10 ran each: 1, 1.585, …, 10 for DEC 5 over 1..10); HSPICE `.DC var
    // type np start stop`, `POI np v…`; ngspice manual `.dc`: linear only
    // (W0038). Checks: 3.
    let text = "t\nV1 1 0 1\nV2 2 0 1\nR1 1 0 1k\nR2 2 0 1k\n.DC DEC V1 1 10 5 V2 LIST 0 1\n.end\n";
    let read = |d| render_text(text, Dialect::Xyce, "t", d);
    let xy = read(Dialect::Xyce).expect("written").text;
    assert!(
        xy.lines().any(|l| l == ".dc DEC V1 1 10 5 V2 LIST 0 1"),
        "{xy}"
    );
    let hs = read(Dialect::Hspice).expect("written").text;
    assert!(
        hs.lines().any(|l| l == ".dc V1 DEC 5 1 10 V2 POI 2 0 1"),
        "{hs}"
    );
    let ng = read(Dialect::Ngspice).expect_err("refused");
    assert!(
        ng.diags.iter().any(|g| g.code == Code::W0038),
        "{}",
        ng.messages
    );
}

#[test]
fn a_list_step_is_xyces_and_ltspices_and_refused_without_a_step() {
    // Xyce RG `.STEP <name> LIST <val>*`; LTspice help `.step param
    // <name> list`, `.step temp list`; ngspice has no `.step` (W0016).
    // Checks: 4.
    let text = "t\nR1 1 0 {r}\nI1 0 1 1m\n.param r=1k\n.step temp list 25 100\n.step r list 1k 2k\n.op\n.end\n";
    let xy = render_text(text, Dialect::Xyce, "t", Dialect::Xyce)
        .expect("written")
        .text;
    assert!(xy.lines().any(|l| l == ".step temp list 25 100"), "{xy}");
    let lt = render_text(text, Dialect::Xyce, "t", Dialect::Ltspice)
        .expect("written")
        .text;
    assert!(lt.lines().any(|l| l == ".step temp list 25 100"), "{lt}");
    assert!(lt.lines().any(|l| l == ".step param r list 1k 2k"), "{lt}");
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice).expect_err("refused");
    assert!(
        ng.diags.iter().any(|g| g.code == Code::W0016),
        "{}",
        ng.messages
    );
}

#[test]
fn a_xyce_only_device_or_analysis_is_refused_as_xyces_own() {
    // Xyce RG (`YNEURON`, `.MPDE`): no other simulator has
    // either, so the refusal is W0039, not W0001 (text kept from another
    // dialect); Xyce copies its own text. Checks: 2 per deck, 2 decks.
    for line in ["YNEURON n1 1 0", ".mpde 0.1ms 0.1ms 1s"] {
        let text = format!("t\nR1 1 0 1k\n{line}\n.op\n.end\n");
        let ng = render_text(&text, Dialect::Xyce, "t", Dialect::Ngspice).expect_err("refused");
        let codes: Vec<Code> = ng.diags.iter().map(|g| g.code).collect();
        assert!(
            codes.contains(&Code::W0039) && !codes.contains(&Code::W0001),
            "{codes:?}"
        );
        assert!(
            render_text(&text, Dialect::Xyce, "t", Dialect::Xyce).is_ok(),
            "{line}"
        );
    }
}

#[test]
fn an_hspice_type_np_dc_and_bare_dcvolt_read() {
    // HSPICE `.DC var DEC np start stop`, `var POI np v…`, `var LIN np
    // start stop` (np points: 2 over 0..1m is a 1m step); `.DCVOLT node
    // val`. Xyce RG `.DC` writes DEC and LIST. Checks: 3.
    let text = "t\nv1 in 0 1\ni1 0 in 0\nr1 in 0 1k\n.dcvolt in 2\n.dc v1 poi 3 0.5 -1 2 i1 lin 2 0 1m\n.end\n";
    let xy = render_text(text, Dialect::Hspice, "t", Dialect::Xyce)
        .expect("written")
        .text;
    assert!(
        xy.lines().any(|l| l == ".dc v1 LIST 500m -1 2 i1 0 1m 1m"),
        "{xy}"
    );
    assert!(xy.lines().any(|l| l == ".ic V(in)=2"), "{xy}");
    let dec = "t\nv1 in 0 1\nr1 in 0 1k\n.dc v1 dec 2 1 100\n.end\n";
    let hs = render_text(dec, Dialect::Hspice, "t", Dialect::Hspice)
        .expect("written")
        .text;
    assert!(hs.lines().any(|l| l == ".dc v1 DEC 2 1 100"), "{hs}");
}

#[test]
fn a_line_given_by_frequency_is_its_delay() {
    // ngspice manual and Xyce RG, lossless `T`: `F=f NL=nl` is a line `nl`
    // wavelengths long at `f`, td = nl/f (nl defaults to 0.25). Real
    // ngspice ran both decks below to the same step response. Checks: 2.
    for (line, td) in [
        ("Z0=50 F=1GHZ NL=10", "TD=0.00000001"),
        ("Z0=50 F=250meg", "TD=0.000000001"),
    ] {
        let text = format!(
            "t\nV1 1 0 PULSE(0 1 0 1p 1p 1 2)\nR1 1 2 50\nT1 2 0 3 0 {line}\nR2 3 0 50\n.tran 1n 40n\n.end\n"
        );
        let ng = render_text(&text, Dialect::Xyce, "t", Dialect::Ngspice)
            .expect("written")
            .text;
        assert!(
            ng.lines()
                .any(|l| l.starts_with("T1 2 0 3 0") && l.contains(td)),
            "{ng}"
        );
    }
}

#[test]
fn a_parallel_current_source_multiplies_its_gain_and_xyce_noop_is_uic() {
    // Xyce RG and ngspice manual, the controlled sources' `m=`: M copies of
    // a G in parallel is M times its transconductance; Xyce 7.10 runs
    // `.tran … NOOP` and `UIC` alike (both run). Checks: 2.
    let text = "t\nV1 2 0 1\nG1 3 0 2 0 0.02 M=10\nR1 3 0 1\n.tran 1u 10u NOOP\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(ng.lines().any(|l| l == "G1 3 0 2 0 0.2"), "{ng}");
    assert!(ng.lines().any(|l| l == ".tran 1u 10u UIC"), "{ng}");
}
