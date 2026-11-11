//! What VACASK cannot do (measures, a PWL repeat, behavioural sources, switches,
//! transmission lines, XSPICE, ...) -> a refusal code, never a broken deck.
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

    // No behavioural sources, switches, transmission lines or XSPICE; a SIN
    // phase VACASK accepts and then ignores.
    for card in [
        "B1 a 0 V=v(b)*2",
        "S1 a 0 c 0 sw\n.model sw SW",
        "T1 a 0 b 0 Z0=50 TD=1n",
        "A1 a d_and\n.model d_and d_and",
        "V1 a 0 0 SIN(0 1 1k 0 0 90)",
    ] {
        assert!(refuses(&with(card)), "{card} must refuse");
    }

    // Analyses VACASK does not implement.
    for card in [".pz a 0 b 0 vol pz", ".disto dec 10 1 1k", ".sens V(out)"] {
        assert!(refuses(&rc(card)), "{card} must refuse");
    }
}

/// ZERO-LEDGER P4-107. Oracle: VACASK 36be560's own analysis list
/// (`docs/cmd-analysis-acsp.md`: `analysis name acsp`; `demo/sp/amp.sim`
/// runs `analysis sp1 acsp ports=[…]`, research/impl/evidence/P4-107/run.log):
/// the target has an S-parameter analysis, so `.sp` is W0001 (has it, not
/// written yet), never W0015 (lacks it).
#[test]
fn p4_107_an_sp_card_is_kept_text_on_vacask() {
    let deck = "t\nV1 a 0 dc 0 ac 1\nR1 a 0 50\n.sp dec 2 1k 10k\n.end\n";
    assert_eq!(emit(deck).unwrap_err(), [Code::W0001]);
}
