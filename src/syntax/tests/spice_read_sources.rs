//! The SPICE-family reader's phase-7 readings of sources, devices and
//! analyses by other dialects' forms (stream spice-read, from round 3):
//! each construct it now reads, written by every target in its own form or
//! refused with the code naming what the target lacks. Each test names its
//! clause, the text derived by hand from it, and how many checks it makes.
//!
//! Contents: none (tests only).

use crate::syntax::Dialect;
use crate::syntax::diag::Code;
use crate::syntax::render::render_text;

// ---- private ----

#[test]
fn a_xyce_table_source_is_ngspices_pwl_value_with_flat_ends() {
    // Xyce RG E/G `TABLE`; ngspice manual `pwl()` in a value. Xyce 7.10
    // ran the TABLE deck, ngspice-45 the written one to the same v(2),
    // v(3) at every point of -2..3 V (1 below 0, 3 above 1); ngspice's own
    // TABLE gives 1.05 at 0 (smoothed), so it is not the spelling; ESPice
    // has no `pwl()` (W0102). Checks: 2 + 1 + 1.
    let text = "t\nV1 1 0 0\nE1 2 0 TABLE V(1,0) = (0,1) (1,3)\nR2 2 0 1k\nG1 0 3 TABLE {V(1)} = (0,1m) (1,3m)\nR3 3 0 1k\n.dc V1 -1 2 0.25\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in [
        "E1 2 0 value={pwl(V(1,0), -1, 1, 0, 1, 1, 3, 2, 3)}",
        "G1 0 3 value={pwl(V(1), -1, 1m, 0, 1m, 1, 3m, 2, 3m)}",
    ] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
    let xy = render_text(text, Dialect::Xyce, "t", Dialect::Xyce)
        .expect("written")
        .text;
    assert!(
        xy.lines()
            .any(|l| l == "E1 2 0 TABLE {V(1,0)} = (0,1) (1,3)"),
        "{xy}"
    );
    let es = render_text(text, Dialect::Xyce, "t", Dialect::Espice).expect_err("refused");
    assert!(
        es.diags.iter().any(|g| g.code == Code::W0102),
        "{}",
        es.messages
    );
}

#[test]
fn an_ltspice_parasitic_is_an_element_where_c_and_l_take_none() {
    // LTspice help, Capacitor and Inductor (Rser, Lser in series; Rpar,
    // Cpar across); ngspice-45 stops at `rser` on a C. The written deck ran
    // on ngspice-45 to the closed forms: i = 1 mA e^(-t/1 ms) (C 1 µF,
    // Rser 1k) to 5e-7, 0.1 A (1 - e^(-t/0.1 ms)) (L 1 mH, Rser 10) to
    // 4e-6, 1 mA + 1 A e^(-t/1 µs) (Rser 1, Rpar 1k) to 1.2e-6. Checks: 4.
    let text =
        "* t\nV1 a 0 1\nC1 a 0 1u Rser=1k V=50 Irms=1\nL1 a 0 1m Rser=10 Rpar=1k\n.tran 5m\n.end\n";
    let ng = render_text(text, Dialect::Ltspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in [
        "C1 C1_s1 0 1u",
        "RC1_ser a C1_s1 1k",
        "RL1_ser a L1_s1 10",
        "RL1_par a 0 1k",
    ] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
}

#[test]
fn an_ltspice_sources_rser_and_cpar_are_elements_elsewhere() {
    // LTspice help, Voltage Source (`Rser=`, `Cpar=`); read as text
    // before (W0001). The written deck ran on ngspice-45 to the closed form
    // v(a) = 0.5 V (1 - e^(-t/0.5 ms)) (1k into 1 µF || 1k) to 9.7e-7.
    // Spectre and VACASK write the same network (named/parasitics.rs; a
    // VACASK run in test_vacask_runs). Checks: 3 + 2.
    let text = "* t\nV1 a 0 PULSE(0 1 0 1n 1n 1 2) Rser=1k Cpar=1u\nR1 a 0 1k\n.tran 5m\n.end\n";
    let ng = render_text(text, Dialect::Ltspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in ["RV1_ser a V1_s1 1k", "CV1_par a 0 1u"] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
    assert!(ng.lines().any(|l| l.starts_with("V1 V1_s1 0 ")), "{ng}");
    let sp = render_text(text, Dialect::Ltspice, "t", Dialect::Spectre)
        .expect("written")
        .text;
    for want in [
        "rv1_ser (a v1_s1) resistor r=1k",
        "cv1_par (a 0) capacitor c=1u",
    ] {
        assert!(sp.lines().any(|l| l == want), "{want}\n{sp}");
    }
}

#[test]
fn a_xyce_lossy_line_is_ngspices_o_and_refused_where_none_is() {
    // Xyce RG and ngspice manual §6.3: `O n1 n2 n3 n4 model` on an LTRA
    // model (Xyce 7.10 and ngspice-45 ran LTRA/zero_length_* alike to
    // 5e-8); HSPICE has no `O`, Spectre and VACASK no LTRA (W0105).
    // Checks: 1 + 3.
    let text = "t\nV1 1 0 1\nO1 1 0 2 0 cab\n.model cab ltra r=0.05 c=20pF len=100\nR1 2 0 10\n.op\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(ng.lines().any(|l| l == "O1 1 0 2 0 cab"), "{ng}");
    for t in [Dialect::Hspice, Dialect::Spectre, Dialect::Vacask] {
        let e = render_text(text, Dialect::Xyce, "t", t).expect_err("refused");
        assert!(
            e.diags.iter().any(|g| g.code == Code::W0105),
            "{t:?} {}",
            e.messages
        );
    }
}

#[test]
fn a_xyce_lin_over_ports_is_ngspices_sp() {
    // Xyce RG port device and `.LIN` (S over the `.AC` sweep); ngspice
    // manual `.sp` and `portnum`/`z0`. Xyce 7.10's Touchstone and ngspice-45's
    // S_i_j agreed to 1e-15 on 14 Output/SPARAMS, XDM and YLIN decks (2 to 5
    // ports). Checks: 3 + 2.
    let text = "t\nP1 1 0 port=1 z0=50\nR1 1 2 30\nC1 2 0 1n\nP2 2 0 port=2\n.AC dec 2 1e6 1e8\n.LIN FORMAT=TOUCHSTONE2\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in [
        "VP1 1 0 0 portnum 1 z0 50",
        "VP2 2 0 0 portnum 2 z0 50",
        ".sp dec 2 1000000 100000000",
    ] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
    let xy = render_text(text, Dialect::Xyce, "t", Dialect::Xyce)
        .expect("written")
        .text;
    for want in ["P1 1 0 port=1 z0=50", ".LIN FORMAT=TOUCHSTONE2"] {
        assert!(xy.lines().any(|l| l == want), "{want}\n{xy}");
    }
}

#[test]
fn an_hspice_noise_takes_the_ac_sweep_and_a_keyword_dc_reads() {
    // HSPICE `.NOISE out src [interval]` over the `.AC` sweep; the ESPice
    // corpus's noise_ac_sweep.expected.json (oracle: ngspice on `.noise
    // v(out) vin dec 2 1k 100k 1`) matched ngspice-45 on the written deck
    // exactly, every column. HSPICE `.DC v1 START= STOP= STEP=`: ngspice-45
    // gave the divider's 1/4 at 0..1 V. Checks: 2.
    let text = "t\nvin in 0 dc 1 ac 1\nr1 in out 1k\nc1 out 0 1n\n.ac dec 2 1k 100k\n.noise v(out) vin 1\n.dc vin start=0 stop=1 step=0.25\n.end\n";
    let ng = render_text(text, Dialect::Hspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in [".noise V(out) vin dec 2 1k 100k 1", ".dc vin 0 1 250m"] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
}

#[test]
fn an_hspice_only_analysis_is_refused_as_the_targets_lack() {
    // HSPICE UG `.SN`, `.TRANNOISE`, `.LSTB` (no ngspice, Xyce, LTspice
    // or VACASK analysis of the kind: W0015), `.STATEYE` (HSPICE's own:
    // W0039), not W0001. Checks: 2 per deck, 2 decks.
    for (line, code) in [
        (".lstb mode=single vsource=vlstb", Code::W0015),
        (".stateye T=1n trf=10p", Code::W0039),
    ] {
        let text = format!("t\nr1 a 0 1k\nv1 a 0 1\n{line}\n.op\n.end\n");
        let ng = render_text(&text, Dialect::Hspice, "t", Dialect::Ngspice).expect_err("refused");
        let codes: Vec<Code> = ng.diags.iter().map(|g| g.code).collect();
        assert_eq!(codes, [code], "{}", ng.messages);
        assert!(render_text(&text, Dialect::Hspice, "t", Dialect::Hspice).is_ok());
    }
}

#[test]
fn an_hspice_value_modifier_scales_or_clamps_the_value() {
    // HSPICE UG, E/G `VALUE=` with `SCALE=`, `MAX=`, `MIN=`; the ESPice
    // corpus's value_modifier.sp analytic oracle (v(a) = 0.5: 3 and 0.4),
    // which ngspice-45 gave on the written deck. Checks: 2.
    let text = "t\nva a 0 0.5\ne1 o1 0 value='2*v(a)' scale=3\nr1 o1 0 1k\ne2 o2 0 value='2*v(a)' max=0.4\nr2 o2 0 1k\n.op\n.end\n";
    let ng = render_text(text, Dialect::Hspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in ["B1 o1 0 V=(2*v(a))*(3)", "B2 o2 0 V=min(2*v(a), 0.4)"] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
}

#[test]
fn an_am_source_is_read_in_its_dialects_order() {
    // ngspice-45 `vsrcload.c`: AM(VO VMO VMA FM FC TD PHASEM PHASEC),
    // VO + (VMO + VMA sin(2π FM t')) sin(2π FC t'); ngspice-45 ran it to that
    // closed form to 5e-9. HSPICE UG: AM(SA OC FM FC TD) = SA (OC + sin)
    // sin, i.e. VO 0, VMO SA·OC, VMA SA (ngspice-45 ran the written deck to
    // the HSPICE form, 5e-9). VACASK `lib/devvisrc.cpp` type="am": its
    // written deck ran to the closed form (4e-16); a carrier phase, which
    // the pinned VACASK drops, is refused. Xyce and LTspice have no AM.
    // Checks: 2 + 1 + 2 + 2.
    let ng = "t\nV1 1 0 AM(0.5 1 0.25 1k 20k 0 30 60)\nR1 1 0 1k\n.tran 1u 2m\n.end\n";
    let back = render_text(ng, Dialect::Ngspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(
        back.lines()
            .any(|l| l == "V1 1 0 0 AM(0.5 1 0.25 1000 20000 0 30 60)"),
        "{back}"
    );
    let va = render_text(ng, Dialect::Ngspice, "t", Dialect::Vacask).expect_err("refused");
    assert!(
        va.diags.iter().any(|g| g.code == Code::W0108),
        "{}",
        va.messages
    );
    let hs = "t\nv1 1 0 AM(2 0.5 1k 20k 0.1m)\nr1 1 0 1k\n.tran 1u 2m\n.end\n";
    let n = render_text(hs, Dialect::Hspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(
        n.lines()
            .any(|l| l == "V1 1 0 0 AM(0 1 2 1000 20000 0.0001)"),
        "{n}"
    );
    let ng0 = "t\nV1 1 0 AM(0.5 1 0.25 1k 20k 0 30)\nR1 1 0 1k\n.tran 1u 2m\n.end\n";
    let va = render_text(ng0, Dialect::Ngspice, "t", Dialect::Vacask)
        .expect("written")
        .text;
    assert!(va.contains("type=\"am\" sinedc=5e-1 ampl=1e0 modindex=2.5e-1 modfreq=1e3 freq=2e4 delay=0 modphase=3e1"), "{va}");
    for t in [Dialect::Xyce, Dialect::Ltspice] {
        let e = render_text(ng0, Dialect::Ngspice, "t", t).expect_err("refused");
        assert!(
            e.diags.iter().any(|g| g.code == Code::W0108),
            "{t:?} {}",
            e.messages
        );
    }
}

#[test]
fn a_pwl_repeat_inside_the_parentheses_is_the_repeat() {
    // ngspice-45 reads `pwl(0 0 1m 1 r=0 td=0.5m)` as `pwl(0 0 1m 1) r=0
    // td=0.5m` (both decks ran to the same v(a), 0.75 V at 2.25 ms); read
    // before as four more points `r 0 td 0.5m`. Checks: 1 + 1.
    let text = "t\nv1 a 0 pwl(0 0 1m 1 r=0 td=0.5m)\nr1 a 0 1k\n.tran 10u 3m\n.end\n";
    let ng = render_text(text, Dialect::Ngspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(
        ng.lines()
            .any(|l| l == "V1 a 0 0 PWL(0 0 0.001 1) r=0 td=0.0005"),
        "{ng}"
    );
    let lt = render_text(text, Dialect::Ngspice, "t", Dialect::Ltspice).expect_err("refused");
    assert!(
        lt.diags.iter().any(|g| g.code == Code::W0008),
        "{}",
        lt.messages
    );
}

#[test]
fn a_xyce_print_kind_is_the_kind_not_an_output() {
    // Xyce RG `.PRINT` kinds (`homotopy`, `hb_fd`, …): read as the kind,
    // not as a first output `homotopy`; ngspice has no such analysis, so
    // the card is not written (W0023). Checks: 2 + 1.
    let text =
        "t\nV1 1 0 1\nR1 1 0 1k\n.dc V1 0 1 0.5\n.print homotopy v(1)\n.print dc v(1)\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice).expect("written");
    assert!(!ng.text.contains("homotopy"), "{}", ng.text);
    assert!(
        ng.notes.diags.iter().any(|g| g.code == Code::W0023),
        "{:?}",
        ng.notes.diags
    );
    let xy = render_text(text, Dialect::Xyce, "t", Dialect::Xyce)
        .expect("written")
        .text;
    assert!(xy.lines().any(|l| l == ".print homotopy v(1)"), "{xy}");
}

#[test]
fn a_xyce_b_poly_is_the_e_f_g_h_poly_it_spells() {
    // Xyce RG, B with POLY in its expression; ngspice manual §5.2-§5.5 E/F
    // POLY (its B has no poly(): "no such function 'poly'"). The two cards
    // are WAMPDE/sandler24.cir's own, with its commented E/F equivalents
    // `EGND 99 0 POLY(2) 3 0 4 0 0 0.5 0.5`, `FB 7 99 POLY(5) VB …`.
    // Checks: 2.
    let text = "t\nV3 3 0 1\nV4 4 0 2\nBGND 99 0 V={POLY(2) V(3) V(4) 0 0.5 0.5}\nR1 99 0 1k\nVB 7 0 0\nBB 7 0 I={POLY(1) I(VB) 0 2}\n.op\n.end\n";
    let ng = render_text(text, Dialect::Xyce, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    for want in [
        "EBGND 99 0 POLY(2) 3 0 4 0 0 500m 500m",
        "FBB 7 0 POLY(1) VB 0 2",
    ] {
        assert!(ng.lines().any(|l| l == want), "{want}\n{ng}");
    }
}

#[test]
fn an_hspice_pz_over_a_source_is_ngspices_over_its_nodes() {
    // HSPICE UG `.PZ v(out) src`; ngspice manual `.pz in+ in- out+ out-
    // vol pz`. The ESPice corpus's pz_source.expected.json (its twin
    // `.pz in 0 out 0 vol pz`, ngspice-44.2) poles -1.893e6, -4.402e5 and
    // zeros -7.5e5 ± 2.107e6j, which ngspice-45 gave on the written deck.
    // HSPICE `.SNAC`/`.SAMPLE` (W0015) and `.VEC` (W0039). Checks: 1 + 2.
    let text = "t\nvin in 0 dc 0 ac 1\nr1 in out 1k\nc1 out 0 1n\n.pz v(out) vin\n.end\n";
    let ng = render_text(text, Dialect::Hspice, "t", Dialect::Ngspice)
        .expect("written")
        .text;
    assert!(ng.lines().any(|l| l == ".pz in 0 out 0 vol pz"), "{ng}");
    for (line, code) in [
        (".sample fs=1meg maxfld=10", Code::W0015),
        (".vec 'stim.vec'", Code::W0039),
    ] {
        let t = format!("t\nr1 a 0 1k\nv1 a 0 1\n{line}\n.op\n.end\n");
        let e = render_text(&t, Dialect::Hspice, "t", Dialect::Ngspice).expect_err("refused");
        assert!(e.diags.iter().any(|g| g.code == code), "{}", e.messages);
    }
}
