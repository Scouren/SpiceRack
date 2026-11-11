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
