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
fn an_ltspice_startup_transient_is_ltspices_and_a_ramp_elsewhere() {
    // LTspice help, `.TRAN`: `.tran <Tstop> [modifiers]`, the modifier
    // `startup` (sources ramped on over the first 20 µs) and `uic` in any
    // order. LTspice writes its own card (`Tstep` 0: LTspice's default)
    // and the source as it stands. Every other target has no such start:
    // the DC source carries the ramp LTspice 26.1.1 ran (linear, 0 to its
    // value at 20 µs) beside its DC value, in the target manual's PWL
    // form, and the card loses the flag. Checks: 2 + 2 per target, 6
    // targets.
    let text = "t\nR1 a 0 1k\nV1 a 0 1\n.tran 750u startup uic\n.end\n";
    let lt = render_text(text, Dialect::Ltspice, "t", Dialect::Ltspice).expect("written");
    assert!(
        lt.text.lines().any(|l| l == ".tran 0 750u UIC startup"),
        "{}",
        lt.text
    );
    assert!(lt.text.lines().any(|l| l == "V1 a 0 1"), "{}", lt.text);
    let cards = "V1 a 0 1 PWL(0 0 0.00002 1)";
    for (d, source) in [
        (Dialect::Ngspice, cards),
        (Dialect::Espice, cards),
        (Dialect::Hspice, cards),
        (Dialect::Xyce, cards),
        (
            Dialect::Spectre,
            "v1 (a 0) vsource dc=1 type=pwl wave=[0 0 20u 1]",
        ),
        (
            Dialect::Vacask,
            "v1 (a 0) vsource dc=1e0 type=\"pwl\" wave=[0, 0, 2e-5, 1e0]",
        ),
    ] {
        let w = render_text(text, Dialect::Ltspice, "t", d).expect("written");
        assert!(w.text.lines().any(|l| l == source), "{d:?}: {}", w.text);
        assert!(
            !w.text.to_ascii_lowercase().contains("startup"),
            "{d:?}: {}",
            w.text
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

#[test]
fn a_xyce_sensitivity_is_an_analysis_the_spice_targets_lack() {
    // Xyce RG `.SENS` (`objfunc=` over the deck's `.tran`); ngspice manual
    // `.SENS` (one output, operating point or AC only): W0015, not W0001.
    // Xyce RG `.PREPROCESS`, `YPDE`: Xyce's own (W0039). Checks: 2 + 2.
    let text = "t\nV1 1 0 1\nR1 1 0 1k\n.sens objfunc={v(1)} param=R1:R\n.tran 1u 1m\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice).expect_err("refused");
    let codes: Vec<Code> = ng.diags.iter().map(|g| g.code).collect();
    assert_eq!(codes, [Code::W0015], "{}", ng.messages);
    assert!(render_text(text, Dialect::Xyce, "t", Dialect::Xyce).is_ok());
    for line in [
        ".preprocess replaceground true",
        "YPDE d1 1 0 DIODE na=1e15",
    ] {
        let text = format!("t\nR1 1 0 1k\n{line}\n.op\n.end\n");
        let ng = render_text(&text, Dialect::Xyce, "t", Dialect::Ngspice).expect_err("refused");
        let codes: Vec<Code> = ng.diags.iter().map(|g| g.code).collect();
        assert_eq!(codes, [Code::W0039], "{}", ng.messages);
    }
}

#[test]
fn a_xyce_lin_dc_and_a_model_before_the_value_read() {
    // Xyce RG `.DC [LIN] src start stop step`; Xyce RG resistor and
    // capacitor `[model name] [value]`; ngspice-45 reads an R's model
    // after its value or before, a C's or an L's only before (run: `C1 1 0
    // 1u CM` is "unknown parameter (cm)"). Checks: 4.
    let text = "t\nV1 1 0 1\nR2 2 0 1k\nRm 1 2 RMOD 1k\n.model RMOD R (TC1=0.01)\nC1 2 0 CMOD 1p\n.model CMOD C\n.DC LIN V1 1 5 2\n.print dc v(2)\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in [".dc V1 1 5 2", "Rm 1 2 1k RMOD", "C1 2 0 CMOD 1p"] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
    // Xyce's model `L` multiplies the value (Xyce 7.10: 10 mH), ngspice's
    // does not (ngspice-45: 5 mH), so the card stays Xyce's text.
    let text = "t\nI1 0 1 SIN(0 1m 1k)\nL1 1 0 IND 5m\n.model IND L (L=2)\n.tran 1u 2m\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice).expect_err("refused");
    assert!(
        ng.diags.iter().any(|g| g.code == Code::W0001),
        "{}",
        ng.messages
    );
}

#[test]
fn a_coupling_of_three_inductors_is_every_pair_where_k_takes_two() {
    // Xyce RG and LTspice help `K`: one card couples every pair of its
    // inductors; ngspice manual `K` (two inductors), Spectre
    // `mutual_inductor`: one card per pair. Xyce 7.10 ran the one-card deck
    // and ngspice-45 the pairwise one to the same v(3), v(4) (8e-4 of
    // scale, the step error at 1 µs). Checks: 3 + 1.
    let text = "t\nV1 1 0 SIN(0 1 1k)\nR1 1 2 10\nL1 2 0 1m\nL2 3 0 2m\nL3 4 0 3m\nR2 3 0 100\nR3 4 0 50\nK1 L1 L2 L3 0.9\n.tran 1u 3m\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in ["K1 L1 L2 0.9", "K1_2 L1 L3 0.9", "K1_3 L2 L3 0.9"] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
    let lt = render_text(text, Dialect::Xyce, "t", Dialect::Ltspice)
        .expect("written")
        .text;
    assert!(lt.lines().any(|l| l == "K1 L1 L2 L3 0.9"), "{lt}");
}

#[test]
fn an_ltspice_library_path_with_blanks_is_one_file() {
    // LTspice help `.LIB <filename>`, `.INCLUDE <filename>`: no sections,
    // the rest of the card is the file. Read as file `C:\…\My` and section
    // `Documents\std.dio` before; Spectre wrote `include "C:\users\My"
    // section=Documents\std.dio`. A sectionless library is Spectre's
    // `include "f"` (Spectre Reference 5.0 p.875). Checks: 2.
    let text = "* t\n.lib C:\\users\\My Documents\\std.dio\nR1 1 0 1k\n.op\n.end\n";
    let sp = render_text(text, Dialect::Ltspice, "t", Dialect::Spectre)
        .expect("written")
        .text;
    assert!(
        sp.lines()
            .any(|l| l == "include \"C:\\users\\My Documents\\std.dio\""),
        "{sp}"
    );
    // ngspice: the whole file is `.include`, the blank path quoted (ngspice
    // manual §2.6).
    let ng = render_text(text, Dialect::Ltspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(
        ng.lines()
            .any(|l| l == ".include \"C:\\users\\My Documents\\std.dio\""),
        "{ng}"
    );
}

#[test]
fn a_control_pre_osdi_is_the_decks_osdi_load() {
    // ngspice manual `pre_osdi file` (an OSDI load before the netlist is
    // parsed). Dropped before (W0025), so the deck loaded no PSP103. VACASK's
    // benchmark/ring/ngspice deck as written for ngspice ran on ngspice-45
    // to the original's v(1) (5.6e-17 V over 20,552 points). Checks: 2.
    let text = "t\nR1 1 0 1k\nI1 0 1 1m\n.control\npre_osdi psp103v4.osdi\nop\n.endc\n.end\n";
    let ng = render_text(text, Dialect::Ngspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(ng.lines().any(|l| l == "pre_osdi psp103v4.osdi"), "{ng}");
    let va = render_text(text, Dialect::Ngspice, "t", Dialect::Vacask)
        .expect("written")
        .text;
    assert!(va.lines().any(|l| l == "load \"psp103v4.osdi\""), "{va}");
}

#[test]
fn an_initial_condition_by_expression_is_written_in_the_targets_delimiters() {
    // Xyce RG `.IC`/`.NODESET` (`V(node)=value`, bare `node value`; a
    // value may be a braced expression); ngspice manual `.ic`. Read as a
    // number only before (W0001). Checks: 3.
    let text = "t\n.param V1=1.0\nC1 1 0 1u\nR1 1 0 1k\n.ic v(1)={V1}\n.nodeset 1 {2*V1}\n.tran 0.1m 1m\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(ng.lines().any(|l| l == ".ic V(1)={V1}"), "{ng}");
    assert!(ng.lines().any(|l| l == ".nodeset V(1)={2*V1}"), "{ng}");
    let hs = render_text(text, Dialect::Xyce, "t", Dialect::Hspice)
        .expect("written")
        .text;
    assert!(hs.lines().any(|l| l == ".ic V(1)='V1'"), "{hs}");
}

#[test]
fn a_subcircuit_func_is_the_subcircuits_or_the_tops() {
    // ngspice-45 and Xyce 7.10 ran both subcircuits' `f` (2 V, 3 V: the
    // first reads its subcircuit's `r0`); Spectre Reference p.871:
    // functions "at the top level only" (W0106). One reading only its
    // arguments, under its own name, moves to the top. Checks: 2 + 1 + 1.
    let text = "t\n.subckt s1 a b r0=2\n.func f(x) {x*r0}\nR1 a b {f(1k)}\n.ends\n.subckt s2 a b\n.func f(x) {x*3}\nR1 a b {f(1k)}\n.ends\nX1 1 0 s1\nX2 2 0 s2\nI1 0 1 1m\nI2 0 2 1m\n.op\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    let lines: Vec<&str> = ng.lines().collect();
    let at = |l: &str| lines.iter().position(|x| *x == l).expect(l);
    assert!(
        at(".subckt s1 a b PARAMS: r0=2") < at(".func f(x) {x*r0}"),
        "{ng}"
    );
    assert!(at(".subckt s2 a b") < at(".func f(x) {x*3}"), "{ng}");
    let sp = render_text(text, Dialect::Xyce, "t", Dialect::Spectre).expect_err("refused");
    assert!(
        sp.diags.iter().any(|g| g.code == Code::W0106),
        "{}",
        sp.messages
    );
    let pure = "t\n.subckt s1 a b\n.func g(x) {x*2}\nR1 a b {g(1k)}\n.ends\nX1 1 0 s1\nI1 0 1 1m\n.op\n.end\n";
    let sp = render_text(pure, Dialect::Xyce, "t", Dialect::Spectre)
        .expect("written")
        .text;
    assert!(
        sp.lines().next().is_some_and(|_| sp.contains("real g(")),
        "{sp}"
    );
}

#[test]
fn p1_022_lines_source_own_simulator_rejects() {
    // Xyce 7.10 `-norun` on a deck holding each line alone (V9 a 0 1, R9
    // a 0 1, `.tran 1n 1u`): each stops Xyce with the message beside it
    // (the corpus decks: Xyce_Regression BUG_204, BUG_636, DC_excessArgs,
    // AC_extractACData1, NOISE_extractNOISEData1, bad_fft_line1,
    // bad_dot_four_line1, DeviceBlock_extractNodes_1, query.cir,
    // DeviceBlock_extractMIDeviceData2, BUG_657, BUG_80, BUG_630). Each
    // is left out with P0010 on every target, no W0001; the legal
    // neighbour of each class (Xyce runs it) carries no P0010. Checks: 3
    // per rejected line × 2 targets, 1 per neighbour × 2 targets.
    let rejected = [
        ".DC VIN 5 5",                       // .DC line not formatted correctly
        ".DC V1 -8.0 -4.0 0.0 4.0",          // Extraneous values on .DC line
        ".tran 1u",                          // .TRAN line has an unexpected number of fields
        ".ac dec 5 100",                     // .AC line has an unexpected number of fields
        ".noise v(4) v1 dec 5 100",          // .NOISE line has an unexpected number of fields
        ".FFT",                              // .FFT line needs at least 2 arguments
        ".FOUR",                             // .FOUR line needs at least 3 arguments
        "R2 out_1",                          // Not enough fields on input line for device R2
        "R4 4a 4 1 1K",                      // Unrecognized parameter 1K for device R4
        "Llead1 node1 0 10 20",              // Unrecognized parameter 20 for device LLEAD1
        "V1 1 0 PWL",                        // PWL device missing source parameters
        "V3 3 0 PWL TD=1",                   // PWL device missing source parameters
        "V2 10 0 PWL ...",                   // Could not parse time/value pairs for PWL
        "V1 1 0 PWL (0.0 0.0 1.0 1.0) R=-1", // PWL source repeat value (R) must be >= 0 and < last value
        "V1 1 0 PWL (0.0 0.0 1.0) R=0",      // the same: (0,0) is the last whole pair
    ];
    let legal = [
        ".DC V9 0 5 1 V8 0 1 0.5",
        ".tran 1n 2u",
        ".ac dec 5 1 100",
        ".noise v(a) V9 dec 5 100 1k",
        ".FFT v(a)",
        ".FOUR 1k v(a)",
        "R1 1 0 1k",
        "C1 1 0 1p IC=1",
        "V1 1 0 PWL 0 0 1u 1",
        // Names Xyce reads with `#`, `+`, `[` in them (Xyce_Regression
        // INVALID_CHARS/valid_chars_*.cir, CircuitSim90/MOS3/rich3.cir).
        "rs#1 1 0 100k",
        "R+ 1 2 100K",
        ".DC v1[ 1 1 1",
        // A continuation after a comment line (Xyce_Regression BUG_250).
        "M1 a 0 0 0 NM\n;c\n+ L=1E-4 W=1000\n.model NM NMOS",
    ];
    let deck = |l: &str| {
        format!("t\nV9 a 0 1\nV8 b 0 1\nR9 a 0 1\n{l}\n.tran 1n 1u\n.print tran v(a)\n.end\n")
    };
    for d in [Dialect::Ngspice, Dialect::Xyce] {
        for l in rejected {
            let w = render_text(&deck(l), Dialect::Xyce, "t", d).expect(l);
            let codes: Vec<Code> = w.notes.diags.iter().map(|g| g.code).collect();
            assert!(codes.contains(&Code::P0010), "{d:?} {l}: {codes:?}");
            assert!(!codes.contains(&Code::W0001), "{d:?} {l}: {codes:?}");
            assert!(!w.text.lines().any(|x| x == l), "{d:?} {l}: {}", w.text);
        }
        for l in legal {
            let codes: Vec<Code> = match render_text(&deck(l), Dialect::Xyce, "t", d) {
                Ok(w) => w.notes.diags.iter().map(|g| g.code).collect(),
                Err(e) => e.diags.iter().map(|g| g.code).collect(),
            };
            assert!(!codes.contains(&Code::P0010), "{d:?} {l}: {codes:?}");
        }
    }
}

#[test]
fn p1_056_remaining_one_line_xyce_rejects() {
    // Oracle: Xyce 7.10 `-norun` on the deck below holding each line alone
    // (V9, R9, L2, L3, `.model DM D`, `.model RM R`, `.param rv`). It stops
    // on every `rejected` line with the message beside it and reads every
    // `legal` one, its neighbour (the corpus decks: Xyce_Regression BUG_198,
    // BUG_1578, BUG_401, bad_command_line, BUG_1116, BUG_587, BUG_67,
    // BUG_726, INVALID_CHARS/*_fails, lead_pde_2d2, bad_fft_line2,
    // MEASURE*, improperVectorParams, bad_vpat). A rejected line is left
    // out with P0010 and no W0001; a legal one carries no P0010. Checks: 2
    // per rejected line and 1 per legal line, on 2 targets.
    let rejected = [
        "A1 a 0 1",    // Invalid device type for device A1
        "N1 a 0 1",    // Invalid device type for device N1
        "# a comment", // Unrecognized line
        "Really good example of perfect nonsense that is too easy to get from a user!", // Illegal value found for device REALLY for parameter = R value = OF
        "T1 1 0 2 0 50 1n",             // Unrecognized parameter 50 for device T1
        "D1 a DM",                      // Model is required for device D1
        "D1 a 0",                       // Model is required for device D1
        "D1 (a 0)",                     // Model is required for device D1
        "R1 a 0 RM 1k 2k",              // Unrecognized parameter 2K for device R1
        "C1 a 0 1p 2 3",                // Unrecognized parameter 2 for device C1
        "K1 L2 L3 0.75 0.5",            // Specified model not found for device K1
        "R1 1 2 2.0e+{1}",              // Invalid notation encountered
        "B1 1 0 V={POLY I(V9) 300u 1}", // Syntax error in number of nodes in expression
        ".print tran {I(V9 R9)}",       // Unrecognized character: V in expression I(V9 R9)
        ".fft v(a) WINDOW=BOGO",        // Invalid WINDOW type BOGO on .FFT line
        ".fft v(a) WINDOW=GAUSS",       // Invalid WINDOW type GAUSS on .FFT line
        ".fft v(a) FORMAT=bogo",        // Invalid FORMAT type BOGO on .FFT line
        ".fft v(a) NP=0",               // NP value on .FFT line must be a power of 2, and >=4
        ".measure tran tran avg v(a)",  // Illegal name in .MEASURE line
        ".measure bleem m1 avg v(a)",   // Unknown mode in .MEASURE line
        ".measure tran m1 avg",         // M1 has incomplete MEASURE line
        ".measure tran m1 trig v(a)=0.5", // M1 has invalid MEASURE line
        ".measure tran m1 find v(a) when", // M1 has invalid MEASURE line
        ".measure fft m1 avg v(a)", // Only ENOB, EQN/PARAM, FIND, SDFR, SNDR, SNR and THD measure types
        ".measure fft m1 find v(a) when v(a)=0.5", // Only FIND-AT supported for FFT measure M1
        ".sens objfunc={V(a)}{V(1)} param=R9:R", // .SENS line is mis-formatted
        ".sens objfunc={V(a)},{V(1)}, param=R9:R", // .SENS line is mis-formatted
        ".sens objfunc=,{V(a)},{V(1)} param=R9:R", // .SENS line is mis-formatted
        ".sens objfunc={V(a)} param=R9:R V9:DCV0", // .SENS line is mis-formatted
        ".sens objfunc={V(a)} param=R9:R,V9:DCV0,", // .SENS line is mis-formatted
        ".sens objfunc={V(a)} param=R9:R,,V9:DCV0", // .SENS line is mis-formatted
        "V1 1 0 PAT(5 0 0n 1n 1n 5n b10z)", // Invalid bit symbol in DATA field
        "V1 1 0 PAT(5 0 0n 1n 1n 5n 10)", // Invalid DATA field
        "V1 1 0 PAT(5 0 0n -1n 1n 5n b10)", // TR, TF and TSAMPLE must all be non-negative
        "V1 1 0 PAT(5 0 0n 1n 0n 5n b10)", // TR, TF and TSAMPLE must all be non-negative
        "V1 1 0 PAT(5 0 0n 1n 1n 5n)", // VHI, VLO, TD, TR, TF, TSAMPLE and DATA parameters are all required
        "V1 1 0 PAT(5 0 0n 1n 1n 5n b10 RB=2)", // Only RB=1 is supported
        "v4 { 0 1",                    // Not enough fields on input line for device V4
        "V1 a 0 {1",                   // Invalid DC value "{1" for device V1
        "V1 1",                        // Not enough fields on input line for device V1
        "V1",                          // Not enough fields on input line for device V1
    ];
    let legal = [
        "T1 1 0 2 0 Z0=50 TD=1n",
        "D1 a 0 DM",
        "D1 a 0 DM 2",
        "D1 (a 0) DM",
        "D1 a,0,DM",
        "R1 a 0 RM 1k",
        "R1 a 0 RM L=1u W=1u",
        "R1 a 0 RM rv",
        "R1 a 0 1k TC=1, 2",
        "K1 L2 L3 0.75",
        "B1 1 0 V={POLY(1) v(a,0) 1E-3 1}",
        ".fft v(a) WINDOW=HANN FORMAT=UNORM NP=8",
        ".fft v(a) WINDOW=hanning FORMAT=norm NP=2",
        ".fft v(a) NP=1",
        ".measure tran m1 avg v(a)",
        ".measure tran m1 max v(a) from=0 to=1u",
        ".measure tran m1 when v(a)=0.5",
        ".measure tran m1 trig v(a)=0.5 targ v(a)=0.6",
        ".measure tran trig max v(a)",
        ".measure fft m1 find v(a) at=1e3",
        ".measure fft m1 find v(a) at = 1e3",
        ".measure fft m1 sfdr v(a)",
        ".sens objfunc={V(a)} param=R9:R,\n+ V9:DCV0",
        ".sens objfunc={V(a)},{V(1)} param=R9:R",
        ".sens objfunc={V(a)} param=R9:R ,V9:DCV0",
        ".sens objfunc = {V(a)} param = R9:R",
        "V1 1 0 PAT(5 0 -1n 1n 1n 5n b10)",
        "V1 1 0 PAT(5 0 0n 1n 1n 5n b10 RB=0)",
        "V1 1 0 PAT(VHI=5 VLO=0 TD=0 TR=1n TF=1n TSAMPLE=5n DATA=b10)",
        "V1 pat 0 DC 5",
        ".sens v(a) param=R9:R",
        "V1 1 0",
        "V1 a,0",
        "V1(a 0)",
        "r2 1 2;",
        "v3 1{ 0 1",
        "v4 } 0 1",
        "v3 3( 0 1",
        "v4 \" 0 1",
    ];
    let deck = |l: &str| {
        format!(
            "t\nV9 a 0 1\nR9 a 0 1\nL2 1 0 1u\nL3 1 0 1u\n.model DM D\n.model RM R (RSH=1)\n.param rv=1k\n{l}\n.tran 1n 1u\n.print tran v(a)\n.end\n"
        )
    };
    let codes = |l: &str, d: Dialect| -> Vec<Code> {
        match render_text(&deck(l), Dialect::Xyce, "t", d) {
            Ok(w) => w.notes.diags.iter().map(|g| g.code).collect(),
            Err(e) => e.diags.iter().map(|g| g.code).collect(),
        }
    };
    for d in [Dialect::Ngspice, Dialect::Xyce] {
        for l in rejected {
            let c = codes(l, d);
            assert!(c.contains(&Code::P0010), "{d:?} {l}: {c:?}");
            assert!(!c.contains(&Code::W0001), "{d:?} {l}: {c:?}");
        }
        for l in legal {
            let c = codes(l, d);
            assert!(!c.contains(&Code::P0010), "{d:?} {l}: {c:?}");
        }
    }
}
