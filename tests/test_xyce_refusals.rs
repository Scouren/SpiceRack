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
                spec_refusal(cite, &deck, ng, xy, &[Code::W0053, Code::W0050]);
            }
        }
    }
    // The spelling of spice-bench `ngspice/tests/bsim1/test.sp`.
    let bsim1 =
        one_device("m1 d g 0 0 m1 l=1u w=1u", "nmos", 4).replace("level=4", "LEVEL=4.00000E+000");
    spec_refusal(cite, &bsim1, ng, xy, &[Code::W0053, Code::W0050]);
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
