//! The expression rules between dialects (`render/spice/powers.rs`,
//! `ternary.rs`, `values.rs`, `render/read/values.rs`): a power's grouping
//! and its value under a negative base, a function's domain, a number's
//! reading. Each test runs the source deck on the source's simulator and
//! the written deck on the target's (`oracle::sim_pair`), or holds the
//! written deck to a closed form a manual clause or a recorded run gives
//! (`research/impl/evidence/` in the ledger's tree names each run).
//!
//! Unsupported fails, never skips: no ngspice or Xyce on PATH fails these
//! tests (`nix develop .#testing`).
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use spicerack::backend::Dialect;

// ---- private ----

/// Ledger P3-016: a Xyce call outside its real domain keeps Xyce's value
/// on ngspice. Xyce 7.10 keeps the real part of the complex result
/// (`sqrt(-2.5)` is 0, `acosh(-2.5)` acosh(2.5), `acosh(0.5)` 0,
/// `(-2.5)**2.1` 2.5^2.1·cos(2.1π) = 6.5145) and clamps `atanh`'s
/// argument to ±(1 - 1e-12); ngspice-45 fails the operating point on each
/// call and gives the power as 2.5^2.1 = 6.8497. Oracle: Xyce on the
/// source deck, ngspice on the written one.
#[test]
fn p3_016_a_xyce_call_outside_its_domain_keeps_its_value_on_ngspice() {
    // A zero has no relative error; the power differs by 7e-12 between
    // the two evaluators.
    let tol = oracle::Tol {
        rel: 1e-9,
        floor: 1e-12,
    };
    for (call, x, want) in [
        ("sqrt(v(x))", -2.5, 0.0),
        ("atanh(v(x))", -2.5, -(1.0 - 1e-12_f64).atanh()),
        ("acosh(v(x))", -2.5, 2.5_f64.acosh()),
        ("acosh(v(x))", 0.5, 0.0),
        (
            "v(x)**2.1",
            -2.5,
            2.5_f64.powf(2.1) * (2.1 * std::f64::consts::PI).cos(),
        ),
    ] {
        let deck = format!("t\nVx x 0 {x}\nRx x 0 1\nB1 o 0 V={{{call}}}\nR1 o 0 1\n.op\n.end\n");
        let at = oracle::At::First;
        let (xy, _) = oracle::sim_pair(Dialect::Xyce, &deck, Dialect::Ngspice, "o", at, tol);
        assert!(tol.within(xy, want), "Xyce 7.10 {call} at {x}: {xy}");
    }
}

/// Ledger P3-078: a negative base under an odd whole exponent keeps its
/// sign on ngspice. Xyce 7.10 gives `v(n)**3`, `pow(v(n),3)` and `{n**3}`
/// -8 for n = -2 (RG §2.3); ngspice-45 reads each as |n|^3 = 8 (manual
/// §5.1), so each is written `(sgn(x)*abs(x)**3)`. Oracle: Xyce on the
/// source deck, ngspice on the written one.
#[test]
fn p3_078_an_odd_power_of_a_negative_base_keeps_its_sign_on_ngspice() {
    let tol = oracle::Tol {
        rel: 1e-9,
        floor: 0.0,
    };
    let deck = "t\n.param n=-2\nVn n 0 -2\nRn n 0 1\nB1 p 0 V={v(n)**3}\nR1 p 0 1\n\
                B2 q 0 V={pow(v(n),3)}\nR2 q 0 1\nV3 r 0 {n**3}\nR3 r 0 1\n.op\n.end\n";
    for node in ["p", "q", "r"] {
        let at = oracle::At::First;
        let (xy, _) = oracle::sim_pair(Dialect::Xyce, deck, Dialect::Ngspice, node, at, tol);
        assert!(tol.within(xy, -8.0), "Xyce 7.10 v({node}): {xy}");
    }
}

/// Ledger P3-029: ngspice's power takes the base's magnitude. ngspice-45
/// gives `v(n)**3` and `pow(v(n),3)` |n|^3 = 8 in a B source and
/// `pwr(v(n),2)` sgn(n)*|n|^2 = -4 for n = -2 (manual §5.1); Xyce 7.10
/// reads them as -8, -8 and 4 (RG §2.3), so the base is written `abs()`
/// and `pwr` with its sign. Oracle: ngspice on the source deck, Xyce on
/// the written one.
#[test]
fn p3_029_an_ngspice_power_takes_the_magnitude_of_its_base_on_xyce() {
    let tol = oracle::Tol {
        rel: 1e-9,
        floor: 0.0,
    };
    let deck = "t\nVn n 0 -2\nRn n 0 1\nB1 p 0 V=v(n)**3\nR1 p 0 1\n\
                B2 q 0 V=pow(v(n),3)\nR2 q 0 1\nB3 r 0 V=pwr(v(n),2)\nR3 r 0 1\n.op\n.end\n";
    for (node, want) in [("p", 8.0), ("q", 8.0), ("r", -4.0)] {
        let at = oracle::At::First;
        let (ng, _) = oracle::sim_pair(Dialect::Ngspice, deck, Dialect::Xyce, node, at, tol);
        assert!(tol.within(ng, want), "ngspice-45 v({node}): {ng}");
    }
}

/// Ledger P3-077: a power keeps the source's grouping. ngspice-45 reads
/// `a**b**c` as (2**3)**2 = 64 (manual §2.8.5), Xyce 7.10 as 2**(3**2) =
/// 512: each deck written for the other gives the source's value. LTspice
/// 26.1.1 reads `-a**c` as (-2)**2 = 4 (its help: unary operators bind
/// above `**`; run by hand, `evidence/P3-077/lt.cir`), ngspice as -4: the
/// LTspice deck on ngspice gives 4. Oracles: the source simulator
/// (`sim_pair`); LTspice's recorded run (not in `.#testing`).
#[test]
fn p3_077_a_power_keeps_the_source_grouping() {
    // A voltage source's node: exact but for the solve's last bit.
    let tol = oracle::Tol {
        rel: 1e-12,
        floor: 0.0,
    };
    let at = oracle::At::First;
    let deck = "t\n.param a=2 b=3 c=2\nV1 p 0 {a**b**c}\nR1 p 0 1k\n.op\n.end\n";
    let (ng, _) = oracle::sim_pair(Dialect::Ngspice, deck, Dialect::Xyce, "p", at, tol);
    assert!(tol.within(ng, 64.0), "ngspice-45: {ng}");
    let (xy, _) = oracle::sim_pair(Dialect::Xyce, deck, Dialect::Ngspice, "p", at, tol);
    assert!(tol.within(xy, 512.0), "Xyce 7.10: {xy}");
    let lt = "t\n.param a=2 c=2\n.param q=-a**c\nV1 q 0 {q}\nR1 q 0 1k\n.op\n.end\n";
    oracle::closed_form(Dialect::Ltspice, lt, &[Dialect::Ngspice], "q", at, 4.0, tol);
}

/// The premise of the power rules: the two simulators give one text two
/// values. ngspice-45 groups `a**b**c` to the left (manual §2.8.5: "left
/// to right" at equal precedence; (2**3)**2 = 64), Xyce 7.10 to the right
/// (2**(3**2) = 512). Oracle: both simulators, on the same deck.
#[test]
fn ngspice_and_xyce_group_a_power_chain_differently() {
    let deck = "t\n.param a=2 b=3 c=2\nV1 p 0 {a**b**c}\nR1 p 0 1k\n.op\n.end\n";
    // A voltage source's node: exact but for the solve's last bit.
    let tol = oracle::Tol {
        rel: 1e-12,
        floor: 0.0,
    };
    let (ng, xy) = (
        oracle::op(Dialect::Ngspice, deck, "p"),
        oracle::op(Dialect::Xyce, deck, "p"),
    );
    assert!(tol.within(ng, 64.0), "ngspice-45: {ng}");
    assert!(tol.within(xy, 512.0), "Xyce 7.10: {xy}");
}
