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
