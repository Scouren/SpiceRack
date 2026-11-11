//! What the VACASK writer does not write -> a refusal code, never a broken
//! deck. A card leaves the lists below with the row that translates it.
//!
//! Cites: real VACASK, for what it refuses or runs wrong.
//!
//! Contents: none (tests only).

use super::*;

// ---- private ----

#[test]
fn refusals_are_errors_not_broken_decks() {
    let refuses = |deck: &str| emit(deck).is_err();
    let with = |card: &str| format!("c\n{card}\n.end\n");

    // No measurement statement at all: `measure` is "Command not found".
    assert!(
        refuses("m\n.meas tran vmax MAX V(out)\n.op\n.end\n"),
        "measures must refuse"
    );

    // A PWL repeat (`r=`): VACASK's `period` repeats from time 0 and wants
    // the last value equal to the first (`docs/dev-builtin-src.md`, PWL).
    assert!(
        refuses(&with("V1 a 0 PWL(0 0 1m 1) r=0")),
        "PWL r= must refuse"
    );

    // Elements the VACASK writer refuses today (a card leaves this list
    // with the row that translates it: ZERO-LEDGER P4-002 took the B source).
    for card in [
        "S1 a 0 c 0 sw\n.model sw SW",
        "A1 a d_and\n.model d_and d_and",
    ] {
        assert!(refuses(&with(card)), "{card} must refuse");
    }

    // Analyses VACASK does not implement.
    for card in [".pz a 0 b 0 vol pz", ".disto dec 10 1 1k", ".sens V(out)"] {
        assert!(refuses(&rc(card)), "{card} must refuse");
    }
}

/// ZERO-LEDGER P4-007. Oracle: a port's impedance is positive: Xyce 7.10
/// stops on another ("The negative impedance -50 for port 1 is not
/// supported for .LIN analysis", `Netlists/Output/SPARAMS/NegativeZ0.cir.sh`),
/// and VACASK's `acsp` takes Z0 from a series resistor
/// (`docs/cmd-analysis-acsp.md`), which a zero leaves out. The `.sp` itself
/// is written (`acsp`), so the port is the only refusal.
#[test]
fn p4_007_a_port_without_a_positive_z0_is_refused() {
    for z0 in ["-50", "0"] {
        let deck =
            format!("t\nV1 a 0 dc 0 ac 1 portnum 1 z0 {z0}\nR1 a 0 50\n.sp dec 2 1k 10k\n.end\n");
        assert_eq!(emit(&deck).unwrap_err(), [Code::W0007], "z0 {z0}");
    }
}
