//! Each SPICE device VACASK runs through its `sp_*` modules (diode, BJT,
//! level-1 MOSFET, JFET, coupled inductors, controlled sources), emitted and
//! run.
//!
//! Cites: real VACASK; the closed forms named in each test.
//!
//! Contents: none (tests only).

use super::*;

// ---- private ----

#[test]
fn a_diode_runs_under_the_spice_device_set() {
    let deck = "diode\n.model dmod D (IS=1e-14 N=1.0)\nR1 vin a 1k\nD1 a 0 dmod\nV1 vin 0 1\n.temp 27\n.op\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(netlist.contains("load \"spice/diode.osdi\""), "{}", netlist);
    assert!(netlist.contains("model dmod sp_diode"), "{}", netlist);
    let raw = run(deck);
    // 1 V through 1k into a diode: a forward drop somewhere in the 0.4-0.8 V
    // band, and the rest across the resistor.
    let va = col(&raw, "a")[0];
    assert!(
        (0.4..0.8).contains(&va),
        "diode drop {} V is not physical",
        va
    );
}

#[test]
fn a_bjt_runs_and_carries_its_polarity() {
    let deck = "bjt\n.model qn NPN (IS=1e-15 BF=100)\nRb vb base 10k\nRc vcc coll 1k\nQ1 coll base 0 qn\nVcc vcc 0 5\nVb vb 0 1\n.op\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(netlist.contains("model qn sp_bjt (type=1"), "{}", netlist);
    let raw = run(deck);
    let vbe = col(&raw, "base")[0];
    assert!(
        (0.5..0.9).contains(&vbe),
        "Vbe {} V is not a forward-biased NPN",
        vbe
    );
    // With beta=100 the stage is on, so the collector is pulled well below Vcc.
    assert!(col(&raw, "coll")[0] < 4.9);
}

#[test]
fn a_level_1_mosfet_runs() {
    let deck = "mos\n.model nm NMOS (LEVEL=1 VTO=1.0 KP=2e-5 LAMBDA=0.05)\nRd vdd drain 1k\nM1 drain gate 0 0 nm W=10u L=1u\nVdd vdd 0 5\nVg gate 0 3\n.op\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(netlist.contains("load \"spice/mos1.osdi\""), "{}", netlist);
    assert!(netlist.contains("model nm sp_mos1 (type=1"), "{}", netlist);
    assert!(netlist.contains("w=10u l=1u"), "{}", netlist);
    let raw = run(deck);
    let vd = col(&raw, "drain")[0];
    assert!(
        vd < 5.0 && vd > 0.0,
        "drain {} V is not a conducting NMOS",
        vd
    );
}

#[test]
fn a_jfet_runs() {
    let deck = "jfet\n.model jn NJF (VTO=-2 BETA=1e-3)\nRd vdd drain 1k\nJ1 drain 0 0 jn\nVdd vdd 0 5\n.op\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(netlist.contains("load \"spice/jfet1.osdi\""), "{}", netlist);
    let raw = run(deck);
    assert!(col(&raw, "drain")[0] < 5.0);
}

#[test]
fn an_inductor_and_its_mutual_coupling_run() {
    let deck = "xfmr\nL1 p 0 1m\nL2 s 0 4m\nK1 L1 L2 0.99\nR1 drive p 1\nR2 s 0 1meg\nV1 drive 0 0 AC 1\n.ac dec 5 1k 100k\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(
        netlist.contains("k1 () mutual k=9.9e-1 ind1=\"l1\" ind2=\"l2\""),
        "{}",
        netlist
    );
    let raw = run(deck);
    // Turns ratio sqrt(4m/1m) = 2, so the secondary rises above the primary.
    let p = cplx(&raw, "p");
    let s = cplx(&raw, "s");
    let ratio = s.last().unwrap().norm() / p.last().unwrap().norm();
    close(ratio, 2.0 * 0.99, 2e-2);
}

#[test]
fn controlled_sources_run() {
    let deck = "ctl\nR0 in 0 1k\nE1 e 0 in 0 2\nR1 e 0 1k\nG1 0 g in 0 2m\nR2 g 0 1k\nF1 0 f V1 -2\nR3 f 0 1k\nH1 h 0 V1 -2k\nR4 h 0 1k\nV1 in 0 2\n.op\n.end\n";
    let raw = run(deck);
    close(col(&raw, "e")[0], 4.0, 1e-9); // VCVS: 2 V * 2
    close(col(&raw, "g")[0], 2.0 * 2e-3 * 1e3, 1e-9); // VCCS: 2 V * 2 mS * 1k
    // i(V1) = -2 mA, so the CCCS drives -2 * -2 mA = 4 mA into 1k.
    close(col(&raw, "f")[0], 4.0, 1e-9);
    close(col(&raw, "h")[0], 4.0, 1e-9);
}

#[test]
fn a_model_parameter_ngspice_ignores_is_left_out() {
    // ngspice-45's mos1 has no `utra` (`mos1/mos1.c`: "unrecognized
    // parameter - ignored"), nor has VACASK's `sp_mos1`, which stopped on
    // it ("Parameter 'utra' not found."; circuitsim90 MOS2 g1310.sp).
    // Saturation, lambda 0: Id = KP/2 * W/L * (Vgs-Vt)^2 = 0.4 mA, so the
    // drain sits at 5 - 1k * 0.4m = 4.6 V.
    let deck = "mos\n.model nm NMOS (LEVEL=1 VTO=1.0 KP=2e-5 UTRA=2)\nRd vdd drain 1k\n\
                M1 drain gate 0 0 nm W=10u L=1u\nVdd vdd 0 5\nVg gate 0 3\n.op\n.end\n";
    let w = spicerack::syntax::render::render_text(deck, Dialect::Ngspice, "t", Dialect::Vacask)
        .unwrap();
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0112], "{}", w.notes.messages);
    close(col(&run(deck), "drain")[0], 4.6, 1e-6);
}

#[test]
fn a_bsim_version_is_the_modules_or_refused() {
    // ngspice-45 `inpdomod.c` L271-L314: level 49 `version=3.3…` is BSIM3
    // (VACASK's `sp_bsim3v3`, which has no `version`), `3.2…` BSIM3v32;
    // level 54 `4.8…` is BSIM4, the module's own `version` string
    // (`bsim4v8.va` L164). Written bare, `3.3.0` was VACASK's syntax error
    // ("unexpected floating point number", nmos_out_BSIM330_*.sp).
    let card = |m: &str| {
        format!("m\n.model n1 nmos {m}\nM1 d g 0 0 n1 W=1u L=1u\nVd d 0 1\nVg g 0 1\n.op\n.end\n")
    };
    let b3 = card("level=49 version=3.3.0 tox=3.5n");
    assert!(!emit(&b3).unwrap().contains("version"));
    assert!(col(&run(&b3), "d")[0] == 1.0);
    let b4 = card("level=54 version=4.8.2");
    assert!(emit(&b4).unwrap().contains("version=\"4.8.2\""));
    assert!(col(&run(&b4), "d")[0] == 1.0);
    assert_eq!(
        emit(&card("level=49 version=3.2.4")).unwrap_err(),
        [Code::W0104]
    );
    assert_eq!(
        emit(&card("level=54 version=4.7")).unwrap_err(),
        [Code::W0104]
    );
    // A parameter ngspice reads that the module lacks: refused.
    assert_eq!(emit(&card("level=1 icvds=1")).unwrap_err(), [Code::W0129]);
}

#[test]
fn ltspice_parasitics_are_elements() {
    // LTspice help, Capacitor/Inductor/Voltage Source: Rser and Lser in
    // series with the device, Rpar and Cpar across it; written as explicit
    // elements, as for the SPICE targets (`spice/parasitics.rs`). Each deck
    // drives node a from 1 V through 1k; the closed forms below.
    let lt = |body: &str, ana: &str| {
        let deck = format!("t\nV1 in 0 1 AC 1\nR1 in a 1k\n{body}\n{ana}\n.end\n");
        run_text(
            &emit_from(
                &deck,
                Dialect::Ltspice,
                Lang::LTSPICE.lexicon().expect("a reader"),
            )
            .expect("written"),
        )
    };
    // Divider halves: a C's Rpar (1k), an L's Rser (1k to ground through
    // the inductor), a source's own Rser feeding 1k to ground.
    close(col(&lt("C1 a 0 1u Rpar=1k", ".op"), "a")[0], 0.5, 1e-9);
    close(col(&lt("L1 a 0 1m Rser=1k", ".op"), "a")[0], 0.5, 1e-9);
    let src = "t\nV1 a 0 1 Rser=1k\nR1 a 0 1k\n.op\n.end\n";
    let raw = run_text(
        &emit_from(
            src,
            Dialect::Ltspice,
            Lang::LTSPICE.lexicon().expect("a reader"),
        )
        .unwrap(),
    );
    close(col(&raw, "a")[0], 0.5, 1e-9);
    // AC at 1 kHz: C1 with Cpar=1u is 2u (a = 1/(1 + jwRC)), L1 with
    // Lser=1m is 2m (a = jwL/(R + jwL)).
    let w = 2.0 * std::f64::consts::PI * 1e3;
    let one = num_complex::Complex64::new(1.0, 0.0);
    let jw = num_complex::Complex64::new(0.0, w);
    let ac = ".ac lin 1 1k 1k";
    let c = cplx(&lt("C1 a 0 1u Cpar=1u", ac), "a")[0];
    let want = one / (one + jw * 1e3 * 2e-6);
    assert!((c - want).norm() <= 1e-9 * want.norm(), "{c} vs {want}");
    let l = cplx(&lt("L1 a 0 1m Lser=1m", ac), "a")[0];
    let want = jw * 2e-3 / (1e3 + jw * 2e-3);
    assert!((l - want).norm() <= 1e-9 * want.norm(), "{l} vs {want}");
}

/// ZERO-LEDGER P4-002. Oracle: ngspice-45 on this same deck, every node at
/// t = 2 ms to 15 digits (research/impl/evidence/P4-002/ngspice.run.log);
/// each is a closed form too: b = 3*2-1, c = -(1m*2*2)*1k, d = 2 ln 3 +
/// log10 3 (ngspice's `log` is natural), e = 2*|-2|^3, g = -(2/1k)*1k,
/// m = 2m*1e3, p = 2*(2-5), r = 2+5+2+sqrt 3+e^2+2+3. `ln` and `sqrt` take
/// `v(a)+1`: VACASK's first Newton iterate is 0 V, where `ln(0)` stops it
/// (measured: the simulator's limit, not the written text's).
#[test]
fn p4_002_a_behavioural_source_runs_on_vacask() {
    let deck = "t\n.param G=2\nV1 a 0 dc 2\nR1 a 0 1k\n\
        B1 b 0 V=3*v(a)-1\nRb b 0 1k\n\
        B2 c 0 I=1m*v(a)*v(a)\nRc c 0 1k\n\
        B3 d 0 V=ln(v(a)+1)+log(v(a)+1)+log10(v(a)+1)\n\
        B4 e 0 V=pow(-2,3)+(-2)**3\n\
        B5 f 0 V=v(a)>1 ? 7 : 9\n\
        B6 g 0 V=i(V1)*1k\n\
        B7 h 0 V=temper\n\
        B8 k 0 V=pi\n\
        B9 m 0 V=time*1e3\n\
        B10 n 0 V=!(v(a)-2)\n\
        B11 p 0 V=G*v(a,b)\n\
        B12 q 0 V=(v(a)>1)&&(v(b)>9)\n\
        B13 r 0 V=min(v(a),v(b))+max(v(a),v(b))+abs(-v(a))+sqrt(v(a)+1)+exp(v(a))+floor(2.5)+ceil(2.5)\n\
        B14 s 0 V=(v(a)>3)||(v(b)>1)\n\
        B15 u 0 V=-v(a)+1Meg*1u\n\
        .tran 1m 2m\n.end\n";
    let raw = run(deck);
    let last = |node: &str| *col(&raw, node).last().expect("a point");
    for (node, want) in [
        ("b", 5.0),
        ("c", -4.0),
        ("d", 2.674345832055882),
        ("e", 16.0),
        ("f", 7.0),
        ("g", -2.0),
        ("h", 27.0),
        ("k", std::f64::consts::PI),
        ("m", 2.0),
        ("n", 1.0),
        ("p", -6.0),
        ("r", 23.12110690649953),
        ("s", 1.0),
        ("u", -1.0),
    ] {
        close(last(node), want, 1e-9);
    }
    // `close` is relative: the one zero (`&&` false) is compared exactly.
    assert_eq!(last("q"), 0.0);
}

/// ZERO-LEDGER P4-002, a Spectre deck's `bsource`: `$time` is VACASK's
/// `$abstime`, `temp` its `$temp`, a parameter name lowercase as the VACASK
/// deck declares it. Oracle: the closed form at t = 2 ms and 27 C,
/// 2*2 + 2m*1k + 27 = 33.
#[test]
fn p4_002_a_spectre_bsource_runs_on_vacask() {
    let deck = "// t\nsimulator lang=spectre\nparameters Gain=2\nv1 (a 0) vsource dc=2\n\
        r1 (a 0) resistor r=1k\nb1 (b 0) bsource v=Gain*v(a)+$time*1k+temp\n\
        rb (b 0) resistor r=1k\ntran1 tran step=1m stop=2m\n";
    let netlist = emit_from(
        deck,
        Dialect::Spectre,
        Lang::SPECTRE.lexicon().expect("a reader"),
    )
    .expect("written");
    let raw = run_text(&netlist);
    close(*col(&raw, "b").last().expect("a point"), 33.0, 1e-9);
}

/// ZERO-LEDGER P4-005. Oracle: the closed form of a lossless line matched
/// at both ends (source and load Z0): `b` is `a` delayed by TD = 2 ns, no
/// reflection. The pulse is on `a` from 1.1 ns to 6.1 ns, so at 2.5 ns
/// a = 0.5 and b = 0, at 5 ns both are 0.5, at 7 ns a = 0 and b = 0.5.
/// ngspice-45 on this deck gives the same six numbers
/// (research/impl/evidence/P4-005/ngspice.run.log).
#[test]
fn p4_005_a_lossless_line_delays_on_vacask() {
    let deck = "t\nV1 in 0 PULSE(0 1 1n 0.1n 0.1n 5n 20n)\nR1 in a 50\n\
        T1 a 0 b 0 Z0=50 TD=2n\nR2 b 0 50\n.tran 0.05n 10n\n.end\n";
    let raw = run(deck);
    let time = col(&raw, "time");
    let at = |node: &str, t: f64| {
        let i = time.iter().position(|&x| x >= t).expect("a point at t");
        col(&raw, node)[i]
    };
    for (t, a, b) in [(2.5e-9, 0.5, 0.0), (5e-9, 0.5, 0.5), (7e-9, 0.0, 0.5)] {
        assert!((at("a", t) - a).abs() < 1e-6, "a at {t:e}: {}", at("a", t));
        assert!((at("b", t) - b).abs() < 1e-6, "b at {t:e}: {}", at("b", t));
    }
}
