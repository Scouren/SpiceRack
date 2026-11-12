//! LTspice as the source: what only LTspice writes, run on the real
//! simulators after the rewrite, against a closed form LTspice 26.1.1
//! itself met (its runs are recorded per test: LTspice is in
//! `.#testing-ltspice`, not `.#testing`, so no test here runs it).
//!
//! Unsupported fails, never skips: a missing simulator fails the test
//! (`nix develop .#testing`).
//!
//! Cites: per test.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol, write};
use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;

// ---- private ----

/// Ledger P5-006. LTwiki, "Undocumented LTspice": a capacitor's `x<n>`
/// is n such capacitors in parallel, each with its own `Rser`, `Lser`,
/// `Rpar`, `Cpar`; `V=`, `Irms=`, `mfg=`, `pn=` are ratings. LTspice
/// 26.1.1 ran this deck (`research/impl/evidence/P5-006`): 0.632120,
/// 0.754747, 0.316060, 0.632120, 0.369020, the closed forms of the one
/// scaled capacitor below. Each simulator runs the written deck against
/// them; ESPice ignores `m=` on R, C and L (run), so the values are
/// scaled.
#[test]
fn p5_006_an_ltspice_capacitors_units_are_one_scaled_capacitor() {
    let deck = "t\nV1 in 0 PULSE(0 1 0 1n 1n 1 2)\nR1 in o1 1k\nC1 o1 0 1u x2\n\
        R2 in o2 1k\nC2 o2 0 1u Rser=1k x2 V=6.3 Irms=1 mfg=\"KEMET\" pn=\"C0805\"\n\
        R3 in o3 1k\nC3 o3 0 1u x2 Rpar=2k\nR4 in o4 1k\nC4 o4 0 1u V=4 Cpar=0.5u x2\n\
        R5 in o5 1k\nC5 o5 0 1u x2 Lser=100m\n.tran 0 5m 0 10u\n.end\n";
    // The integrator's error at a 10 µs step: RELTOL (1e-3).
    let tol = Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    let cases = [
        // 2 µF from 1 kΩ: τ = 2 ms, 1 − e^(−1).
        ("o1", 2e-3, 0.632_121),
        // 2 µF in series with Rser/2 = 500 Ω: τ = 3 ms, 1 − (2/3) e^(−1).
        ("o2", 3e-3, 0.754_747),
        // 2 µF across Rpar/2 = 1 kΩ: 0.5 (1 − e^(−t / 1 ms)).
        ("o3", 1e-3, 0.316_060),
        // 2 µF beside Cpar·2 = 1 µF: τ = 3 ms, 1 − e^(−1).
        ("o4", 3e-3, 0.632_121),
        // 2 µF in series with Lser/2 = 50 mH from 1 kΩ: s = −513.17 /s
        // and −19486.83 /s, i = 20 (e^(s1 t) − e^(s2 t)) / 18973.67 A,
        // v = 1 − 1 kΩ · i.
        ("o5", 1e-3, 0.369_021),
    ];
    for d in [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Xyce,
        Dialect::Vacask,
    ] {
        let text = write(deck, Dialect::Ltspice, d);
        let plots = oracle::run(d, &text);
        for (node, t, want) in cases {
            let got = oracle::value(&plots, node, At::Time(t));
            assert!(
                tol.within(got, want),
                "{d:?}: v({node}) = {got} at {t}, want {want}\n{text}"
            );
        }
    }
    // Spectre runs nowhere here: the same network in its instance form
    // (Spectre Reference, `capacitor` `c`, `resistor` `r`), no flag left.
    let sp = write(deck, Dialect::Ltspice, Dialect::Spectre);
    assert!(sp.lines().any(|l| l == "c1 (o1 0) capacitor c=2u"), "{sp}");
    assert!(
        sp.lines().any(|l| l == "rc2_ser (o2 c2_s1) resistor r=500"),
        "{sp}"
    );
    // LTspice keeps its own card.
    let lt = write(deck, Dialect::Ltspice, Dialect::Ltspice);
    assert!(lt.lines().any(|l| l == "C1 o1 0 1u x2"), "{lt}");
}

/// Ledger P4-073 (spec S4-rf-inert-text). LTspice help, `.WAVE`: the card
/// writes the named nodes to a .wav file. LTspice 26.1.1 ran this deck
/// with the card and without it to the same V(out) = 0.632122881442 at
/// 1 ms and wrote `out.wav`
/// (research/impl/evidence/S4-rf-inert-text/RESULTS.txt §3). No other
/// simulator has the card, so it is left out with a note (W0336), and each
/// simulator runs the written deck to the closed form: 1 µF from 1 kΩ,
/// τ = 1 ms, 1 − e^(−1) at 1 ms.
#[test]
fn p4_073_an_ltspice_wave_card_is_left_out() {
    let plain = "t\nV1 in 0 PULSE(0 1 0 1n 1n 1 2)\nR1 in out 1k\nC1 out 0 1u\n\
        .tran 0 5m 0 10u\n.end\n";
    let wave = plain.replace(".end\n", ".wave out.wav 16 44100 V(out)\n.end\n");
    // The integrator's error at a 10 µs step: RELTOL (1e-3).
    let tol = Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    for d in [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Xyce,
        Dialect::Vacask,
    ] {
        let w = oracle::written(&wave, Dialect::Ltspice, d);
        assert_eq!(w.text, write(plain, Dialect::Ltspice, d), "{d:?}");
        assert!(oracle::notes(&w).contains(&Code::W0336), "{d:?}");
        let plots = oracle::run(d, &w.text);
        let got = oracle::value(&plots, "out", At::Time(1e-3));
        assert!(tol.within(got, 0.632_121), "{d:?}: {got}\n{}", w.text);
    }
    // Spectre and HSPICE run nowhere here: the same text as the deck
    // without the card.
    for d in [Dialect::Spectre, Dialect::Hspice] {
        let w = oracle::written(&wave, Dialect::Ltspice, d);
        assert_eq!(w.text, write(plain, Dialect::Ltspice, d), "{d:?}");
        assert!(oracle::notes(&w).contains(&Code::W0336), "{d:?}");
    }
    // LTspice keeps its own card.
    let lt = write(&wave, Dialect::Ltspice, Dialect::Ltspice);
    let kept = lt.lines().any(|l| l == ".wave out.wav 16 44100 V(out)");
    assert!(kept, "{lt}");
}

/// Ledger P4-067 (spec S4-rf-private-cards). LTspice help, ".MACHINE —
/// Arbitrary State Machine": five cards (`.mach[ine]`, `.state`, `.rule`,
/// `.output`, `.endmach[ine]`); LTspice 26.1.1 ran the three decks below
/// (the machine inverts V(1): V(2) = 1 at 0.75 ms, 0 at 0.25 ms, both
/// spellings). `.softstart` scales the soft-start time of LTspice's own
/// regulator models (the example decks' comments); LTspice 26.1.1 reads
/// it and stops on an unknown card. ngspice-45 stops on each
/// ("unimplemented dot command") and Xyce 7.10 reads past each
/// (research/impl/evidence/S4-rf-private-cards/RESULTS.txt). So every
/// other target refuses each as LTspice's own (W0039), never as text no
/// rule read (W0001), and LTspice gets its lines back as written.
#[test]
fn p4_067_ltspice_own_cards_are_refused_as_its_own() {
    use spicerack::syntax::diag::Code;
    let lt = Dialect::Ltspice;
    let rc = "t\nV1 1 0 PULSE(0 1 0 1u 1u .5m 1m)\nR1 2 0 1k\n";
    let end = ".tran 3m\n.end\n";
    let machine = ".machine\n.state S0 0\n.rule S0 S0 V(1) > .5\n.output (2) V(1) < .5\n\
        .endmachine\n";
    let cards = [
        ".softstart .5\n",
        machine,
        ".mach\n.output (2) V(1) < .5\n.endmach\n",
    ];
    for card in cards {
        let deck = format!("{rc}{card}{end}");
        for to in [
            Dialect::Ngspice,
            Dialect::Espice,
            Dialect::Hspice,
            Dialect::Xyce,
            Dialect::Spectre,
            Dialect::Vacask,
        ] {
            let codes = oracle::refusal(&deck, lt, to);
            let own = codes.contains(&Code::W0039) && !codes.contains(&Code::W0001);
            assert!(own, "{to:?}: {codes:?}\n{deck}");
        }
        // LTspice gets each card back as written.
        let back = write(&deck, lt, lt);
        for line in card.lines() {
            assert!(back.lines().any(|l| l == line), "{line}\n{back}");
        }
    }
    // The legal neighbour: the circuit without the cards is written.
    let plain = format!("{rc}{end}");
    assert!(write(&plain, lt, Dialect::Ngspice).contains(".tran"));
}
