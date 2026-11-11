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
