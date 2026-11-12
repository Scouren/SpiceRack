//! The Xyce target's refusals found by the cross-sim triage (ZERO-LEDGER
//! P1-066): what real Xyce 7.10 stops on, or runs as something else, the
//! writer refuses with the code that says why.
//!
//! Unsupported fails, never skips.
//!
//! Cites: per test (RG = Xyce Reference Guide 7.10).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::spec_refusal;
use spicerack::syntax::Dialect;
use spicerack::syntax::diag::Code;

// ---- private ----

/// One device of model type `kind` at `level` on a 1 V supply, as ngspice
/// reads it: `line` is the instance.
fn one_device(line: &str, kind: &str, level: u32) -> String {
    format!("* t\nvd d 0 1\nvg g 0 1\n{line}\n.model m1 {kind} level={level}\n.op\n.end\n")
}

/// ZERO-LEDGER P1-066. Oracle: Xyce 7.10.0 and ngspice-45 on these very
/// decks (research/impl/evidence/P1-066-xyce-model-levels/probes/): ngspice
/// runs each, Xyce stops with "Model is required for device M1 and no
/// valid model card found" (MOSFET 9: runs its BSIM3, 1.010e-12 A where
/// ngspice's MOS9 gives 1.036e-5 A). The card is left out (W0053) and the
/// instance refused (W0050). Checks: 28 + 1 + 3.
#[test]
fn p1_066_an_ngspice_level_xyce_has_no_device_for_is_refused() {
    let cite = "Xyce RG 7.10 `M`, `Q`, `Z` level tables; Xyce 7.10.0 run";
    let (ng, xy) = (Dialect::Ngspice, Dialect::Xyce);
    let none: [(&str, &str, &[u32]); 4] = [
        ("m1 d g 0 0 m1 l=1u w=1u", "mos", &[4, 5, 9, 55, 56, 68, 73]),
        ("q1 d g 0 m1", "", &[4, 9]),
        ("z1 d g 0 m1", "mf", &[2, 3, 4]),
        ("z1 d g 0 m1", "hfet", &[5, 6]),
    ];
    for (line, kind, levels) in none {
        for &level in levels {
            for polarity in ["n", "p"] {
                // `nmos`/`pmos`, `npn`/`pnp`, `nmf`/`pmf`, `nhfet`/`phfet`.
                let kind = match kind {
                    "" => format!("{polarity}{}", if polarity == "n" { "pn" } else { "np" }),
                    k => format!("{polarity}{k}"),
                };
                let deck = one_device(line, &kind, level);
                spec_refusal(cite, &deck, ng, xy, &[Code::W0050, Code::W0053]);
            }
        }
    }
    // The spelling of spice-bench `ngspice/tests/bsim1/test.sp`.
    let bsim1 =
        one_device("m1 d g 0 0 m1 l=1u w=1u", "nmos", 4).replace("level=4", "LEVEL=4.00000E+000");
    spec_refusal(cite, &bsim1, ng, xy, &[Code::W0050, Code::W0053]);
    // Levels both have stay written: MOSFET 1, BJT 1, MESFET 1.
    for (line, kind) in [
        ("m1 d g 0 0 m1 l=1u w=1u", "nmos"),
        ("q1 d g 0 m1", "npn"),
        ("z1 d g 0 m1", "nmf"),
    ] {
        let w = oracle::write(&one_device(line, kind, 1), ng, xy);
        assert!(
            w.to_lowercase().contains(".model m1"),
            "{kind} level 1: {w}"
        );
    }
}

/// ZERO-LEDGER P1-066. Oracle: Xyce 7.10.0 on these very card pairs over
/// the same RC (research/impl/evidence/P1-066-xyce-one-analysis/results.txt):
/// `.dc` then `.tran` writes the transient plot alone (104 points), `.tran`
/// then `.ac` the AC plot alone, two `.dc` cards one plot of 9 points for
/// sweeps of 3 and 3, `.ac` then `.noise` the noise plot alone; an `.op`
/// before a `.tran` is not run (RG `.OP`), which stays a note. Checks: 5 + 3.
#[test]
fn p1_066_a_second_analysis_for_xyce_is_refused() {
    let cite = "Xyce 7.10.0 run: of two analysis cards the last alone is run";
    let (ng, xy) = (Dialect::Ngspice, Dialect::Xyce);
    let rc = "* t\nv1 1 0 dc 1 ac 1 sin(0 1 1k)\nr1 1 2 1k\nc1 2 0 1u\n";
    let deck = |cards: &str| format!("{rc}{cards}\n.end\n");
    for cards in [
        ".dc v1 0 1 0.5\n.tran 10u 1m",
        ".tran 10u 1m\n.ac dec 2 1 100",
        ".dc v1 0 1 0.5\n.dc v1 2 4 1",
    ] {
        spec_refusal(cite, &deck(cards), ng, xy, &[Code::W0321]);
    }
    // Beside a note (W0052: the `.noise` integrals, the `.op`).
    for cards in [
        ".ac dec 2 1 100\n.noise v(2) v1 dec 2 1 100",
        ".op\n.dc v1 0 1 0.5\n.tran 10u 1m",
    ] {
        let codes = oracle::refusal(&deck(cards), ng, xy);
        assert!(codes.contains(&Code::W0321), "{cards}: {codes:?}");
    }
    // One sweeping analysis is written: alone, with an `.op` (noted), with
    // its `.four`, and a two-source `.dc` (one card).
    for cards in [
        ".op\n.tran 10u 1m",
        ".tran 10u 1m\n.four 1k v(2)",
        ".dc v1 0 1 0.5 v1 0 1 1",
    ] {
        let w = oracle::write(&deck(cards), ng, xy);
        assert!(w.contains(".end"), "{cards}: {w}");
    }
}
