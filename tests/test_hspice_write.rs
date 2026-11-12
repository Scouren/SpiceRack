//! The HSPICE target's conformance gaps (`render/spice/`, the HSPICE
//! choices), each a failed run or a vector lost in the spice-bench ->
//! HSPICE cells of `tests/test_cross_sim` on a deck written from ngspice
//! text. No HSPICE runs here: a written line is checked against the HSPICE
//! command reference's clause (`oracle::spec_example`), and the written
//! deck runs on ESPice's HSPICE reading (`--tokenizer=hspice`) to the
//! closed form the deck is built to give, met by real ngspice on the
//! source (`oracle::closed_form`).
//!
//! Unsupported fails, never skips: no ESPice or ngspice on PATH fails
//! (`nix develop .#testing`).
//!
//! Cites: per test (cmdref = HSPICE Reference Manual: Commands and Control
//! Options, E-2010.12).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol, closed_form, sim_pair, spec_example, spec_refusal};
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;

// ---- private ----

/// Ledger P3-064. An ngspice `@dev[param]` vector (manual §15.6) is no
/// HSPICE output variable (cmdref `.PROBE`, p.255: "voltage, current, or
/// element template"), and ESPice's HSPICE reading stops on a `.probe` of
/// one (spice-bench `examples/various/diode_ac_par.sp`: "@d1[gd]: d1 has
/// no variable 'gd'"). The deck written for HSPICE leaves it out and says
/// so (W0052), keeps the other saves, and runs: 1 V over 1k and 1k is
/// 0.5 V at node 2.
#[test]
fn p3_064_an_ngspice_device_vector_is_no_hspice_probe() {
    let deck = "t\nV1 1 0 1\nR1 1 2 1k\nR2 2 0 1k\n.save @r1[i] v(2)\n.op\n.end\n";
    let w = oracle::written(deck, Dialect::Ngspice, Dialect::Hspice);
    assert!(oracle::notes(&w).contains(&Code::W0052), "{}", w.text);
    assert!(!w.text.contains('@'), "{}", w.text);
    assert!(w.text.lines().any(|l| l == ".probe v(2)"), "{}", w.text);
    // A linear divider: the two solvers' rounding only.
    let tol = Tol {
        rel: 1e-9,
        floor: 0.0,
    };
    let on = [Dialect::Ngspice, Dialect::Hspice];
    closed_form(Dialect::Ngspice, deck, &on, "2", At::First, 0.5, tol);
}

/// Ledger P3-064. HSPICE prints a transient from time 0 unless `START=`
/// says otherwise (cmdref `.TRAN`, p.312: `.TRAN tstep1 tstop1
/// [START=val] [UIC]`; "START: Time when printing or plotting begins"),
/// so a zero start is the card without the word: the card ngspice's `hsa`
/// reading takes too (twelve spice-bench decks of the hsa cell stopped on
/// `.tran 1p 1n start=0`). A start that is not zero keeps the word. The
/// deck runs: 1 V stepped onto 1k and 1u is 1 - e^-5 after 5 ms; a step
/// of a hundredth of the time constant leaves the trapezoid 1e-6 off, so
/// the simulators' RELTOL is the bound.
#[test]
fn p3_064_hspices_tran_leaves_a_zero_start_to_the_default() {
    let rc = "t\nV1 1 0 pulse(0 1 0 1n 1n 1 2)\nR1 1 2 1k\nC1 2 0 1u\n";
    let cite = "HSPICE cmdref E-2010.12 `.TRAN` (p.312): tstep1 tstop1 [START=val] [UIC]";
    let deck = format!("{rc}.tran 1u 5m 0 10u\n.end\n");
    spec_example(
        cite,
        &deck,
        Dialect::Ngspice,
        Dialect::Hspice,
        &[".tran 1u 5m"],
    );
    let late = format!("{rc}.tran 1u 5m 1m\n.end\n");
    spec_example(
        cite,
        &late,
        Dialect::Ngspice,
        Dialect::Hspice,
        &[".tran 1u 5m START=1m"],
    );
    let tol = Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    let on = [Dialect::Ngspice, Dialect::Hspice];
    closed_form(
        Dialect::Ngspice,
        &deck,
        &on,
        "2",
        At::Last,
        0.9932620530009145,
        tol,
    );
}

/// Ledger P3-085. A model's dotted VERSION is one word to HSPICE (MOSFET
/// Models manual D-2010.12, p.420: "The recommended BSIM3v3 model
/// specification is LEVEL=49, VERSION=3.3.0"; Table 117 lists `3.3.0` and
/// `3.2.4`). In quotes it is an expression, which `3.3.0` is not:
/// ngspice's `hsa` reading stops on `version='3.3.0'` ("Misplaced
/// operator", 24 spice-bench decks of the hsa cell). The writer leaves the
/// word bare, and the deck runs to the source's operating point: one
/// BSIM3v3.3.0 card on two implementations, 1.4e-11 apart by hand (the
/// solvers' rounding), bounded here at 1e-6.
#[test]
fn p3_085_hspice_writes_a_dotted_version_bare() {
    let model = ".model n1 nmos level=49 version=3.3.0 tox=3.5n nch=2.4e17 nsub=5e16 vth0=0.15";
    let deck = format!(
        "t\nVdd dd 0 2\nVg g 0 1.5\nR1 dd d 1k\nM1 d g 0 0 n1 W=1u L=0.35u\n{model}\n.op\n.end\n"
    );
    let want = ".model n1 nmos (level=49 version=3.3.0 tox=3.5n nch=2.4e17 nsub=5e16 vth0=0.15)";
    let cite = "HSPICE MOSFET Models manual D-2010.12, p.420 and Table 117: VERSION=3.3.0";
    spec_example(cite, &deck, Dialect::Ngspice, Dialect::Hspice, &[want]);
    let tol = Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    sim_pair(
        Dialect::Ngspice,
        &deck,
        Dialect::Hspice,
        "d",
        At::First,
        tol,
    );
}

/// Ledger P3-086. PSpice's and LTspice's `.model new ako:base type (…)`
/// ("a kind of") is the base model with the listed parameters changed
/// (LTspice help, `.model`: "identical to the diode model NORMAL except
/// for the tt parameter"), and HSPICE has no such card (Elements and
/// Device Models manual E-2010.12). The deck is written with the model
/// in full: the base's IS, the card's BF over the base's, its VA added.
/// LTspice reads the `ako:` card itself, so its operating point is the
/// oracle for the written decks: on ESPice's HSPICE reading, and on plain
/// ngspice, which reads no `ako:` outside its PSPICE mode. By hand the
/// two are 2.4e-7 and 1.5e-6 from LTspice (the simulators' constants),
/// bounded here at 1e-5; the base model alone gives 0.70 V for the 1.54.
#[test]
fn p3_086_an_ako_model_is_written_as_its_base_with_its_own_parameters() {
    let models = ".model QP350 PNP(IS=1.4E-15 BF=70)\n.model QP AKO:QP350 PNP(BF=150 VA=100)";
    let deck = format!("t\nVcc e 0 5\nQ1 c b e QP\nRb b 0 430k\nRc c 0 1k\n{models}\n.op\n.end\n");
    let cite =
        "LTspice help, `.model`: `.model <modname> ako: <modname> [<type>] [(<parameter list>)]`";
    let want = ".model QP PNP (IS=1.4E-15 BF=150 VA=100)";
    let lt = oracle::op(Dialect::Ltspice, &deck, "c");
    let tol = Tol {
        rel: 1e-5,
        floor: 0.0,
    };
    for to in [Dialect::Hspice, Dialect::Ngspice] {
        let text = spec_example(cite, &deck, Dialect::Ngspice, to, &[want]);
        let got = oracle::op(to, &text, "c");
        assert!(
            tol.within(got, lt),
            "{to:?} v(c) = {got}, LTspice {lt}\n{text}"
        );
    }
}

/// Ledger P3-086. An `ako:` card the reader cannot resolve is refused
/// (W0142), by a SPICE target and by a named one: no card of the file is
/// its base (ngspice-45 in its PSPICE mode: "no model found for .model qp
/// ako:qp350"; LTspice 26.1.1: "Undefined model"), or its type word is
/// not the base's (ngspice: "Original (pnp) and new (npn) type for AKO
/// model disagree"; LTspice help: "it must match the type of the source
/// model if present").
#[test]
fn p3_086_an_ako_model_with_no_base_or_another_type_is_refused() {
    let cite = "HSPICE Elements and Device Models manual E-2010.12: no `ako:` model type";
    let head = "t\nV1 a 0 1\nR1 a 0 1k\n";
    let no_base = format!("{head}.model QP AKO:QP350 PNP(BF=150)\n.op\n.end\n");
    let models = ".model QP350 PNP(BF=70)\n.model QP AKO:QP350 NPN(BF=150)";
    let other_type = format!("{head}{models}\n.op\n.end\n");
    let ng = Dialect::Ngspice;
    for deck in [&no_base, &other_type] {
        spec_refusal(cite, deck, ng, Dialect::Hspice, &[Code::W0142]);
        let named = oracle::refusal(deck, ng, Dialect::Spectre);
        assert!(named.contains(&Code::W0142), "{named:?}\n{deck}");
    }
}

/// What Xyce prints for `text` run as a user runs it, with no `-r` raw
/// file: each column of its print file `t.cir.prn` (RG 7.10 §2.1.31, the
/// default format), its header and its first row's value.
fn xyce_printed(text: &str) -> Vec<(String, f64)> {
    let dir = tempfile::tempdir().expect("a scratch directory");
    std::fs::write(dir.path().join("t.cir"), text).expect("the deck written");
    let out = std::process::Command::new("Xyce")
        .arg("t.cir")
        .current_dir(dir.path())
        .output()
        .expect("Xyce on PATH (nix develop .#testing)");
    let log = String::from_utf8_lossy(&out.stdout);
    let prn = std::fs::read_to_string(dir.path().join("t.cir.prn"))
        .unwrap_or_else(|_| panic!("no print file:\n{log}\n{text}"));
    let mut rows = prn.lines().map(str::split_whitespace);
    let (head, first) = (rows.next().expect("a header"), rows.next().expect("a row"));
    head.map(str::to_string)
        .zip(first.map(|v| v.parse().expect("a number")))
        .collect()
}

/// Ledger P3-043. `.save all` (the Python `tb.save("all")`) asks for
/// every node, and the decks written for HSPICE, Xyce and VACASK lost it.
/// HSPICE writes no waveform unless `.option post` says so (cmdref
/// `.OPTION POST`, p.596: "POST=0: Does not output simulation results",
/// the default) and every node with it (`.OPTION PROBE`, p.601: "All
/// node voltage and source currents are output"). Xyce 7.10 prints
/// nothing without a `.print` ("Netlist warning: No print specified");
/// `V(*)` is every node voltage (RG 7.10 §2.1.31.11) on the print of the
/// kind the deck runs, the one print Xyce takes ("Analysis type TRAN and
/// print type DC are inconsistent." stops it); a deck of two kinds is
/// refused (W0321), as Xyce runs one card alone. VACASK saves every node when no `save`
/// is given (`lib/coredcinc.cpp`, `addAllUnknowns`): no line, no note.
/// The decks run: 1 V over 1k, 1k and 2k is 0.75 V and 0.5 V, in Xyce's
/// print file too. A list is left out and noted, as before.
#[test]
fn p3_043_save_all_is_written_for_hspice_xyce_and_vacask() {
    let net = "t\nV1 1 0 1\nR1 1 2 1k\nR2 2 3 1k\nR3 3 0 2k\n";
    let op = format!("{net}.save all\n.op\n.end\n");
    let ng = Dialect::Ngspice;
    let post = "HSPICE cmdref E-2010.12 `.OPTION POST` (p.596): POST=0: Does not output";
    spec_example(post, &op, ng, Dialect::Hspice, &[".option post"]);
    let star = "Xyce RG 7.10 §2.1.31.11: V(*) will print all of the node voltages";
    let xy = spec_example(star, &op, ng, Dialect::Xyce, &[".print dc V(*)"]);
    // A linear divider: the solvers' rounding, the print file's nine digits.
    let tol = Tol {
        rel: 1e-8,
        floor: 0.0,
    };
    let printed = xyce_printed(&xy);
    for (name, want) in [("V(1)", 1.0), ("V(2)", 0.75), ("V(3)", 0.5)] {
        let got = printed.iter().find(|(n, _)| n == name);
        let near = got.is_some_and(|(_, v)| tol.within(*v, want));
        assert!(near, "{name}, want {want}: {printed:?}");
    }
    let tran = format!("{net}.save all\n.op\n.tran 1u 2u\n.end\n");
    let xy = spec_example(star, &tran, ng, Dialect::Xyce, &[".print tran V(*)"]);
    let printed = xyce_printed(&xy);
    let out = printed.iter().find(|(n, _)| n == "V(3)");
    assert!(out.is_some_and(|(_, v)| tol.within(*v, 0.5)), "{printed:?}");
    // Two kinds: Xyce runs one card alone, so the deck is refused (W0321,
    // ZERO-LEDGER P1-066-xyce-one-analysis).
    let both = format!("{net}.save all\n.tran 1u 2u\n.ac dec 1 1k 10k\n.end\n");
    let codes = oracle::refusal(&both, ng, Dialect::Xyce);
    assert!(codes.contains(&Code::W0321), "{codes:?}");
    let va = oracle::written(&op, ng, Dialect::Vacask);
    assert!(!oracle::notes(&va).contains(&Code::W0023), "{}", va.text);
    let on = [ng, Dialect::Hspice, Dialect::Xyce, Dialect::Vacask];
    for (node, want) in [("2", 0.75), ("3", 0.5)] {
        closed_form(ng, &op, &on, node, At::First, want, tol);
    }
    // The legal neighbour, as before: a list.
    let list = format!("{net}.save v(2)\n.op\n.end\n");
    for to in [Dialect::Xyce, Dialect::Vacask] {
        let w = oracle::written(&list, ng, to);
        assert!(oracle::notes(&w).contains(&Code::W0023), "{to:?}");
        assert!(!w.text.contains(".print"), "{}", w.text);
    }
}
