//! A Xyce deck written for ngspice: what Xyce means by a line that ngspice
//! reads another way (`render/spice/`, the Xyce → ngspice pair rules). Each
//! test runs the Xyce source on real Xyce 7.10 and the deck written from
//! it on real ngspice-45 (`oracle::sim_pair`), or checks a refusal against
//! the ngspice clause that makes it one.
//!
//! Unsupported fails, never skips: no Xyce or ngspice on PATH fails
//! (`nix develop .#testing`).
//!
//! Cites: per test (RG = Xyce Reference Guide 7.10).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol, sim_pair};
use spicerack::syntax::Dialect;

/// Two direct solves of one linear deck: both simulators give the closed
/// form to rounding (their RELTOL, 1e-3, bounds no iteration here).
const LINEAR: Tol = Tol {
    rel: 1e-9,
    floor: 1e-12,
};

// ---- private ----

/// Ledger P3-099. Xyce RG §2.3.4: `$G_A` and `$G_AB` are two global nodes,
/// and `G_AB` a third, plain one. Xyce 7.10 by hand on this deck:
/// V($G_A) = 1, V($G_AB) = 2 (2 mA into 1 kΩ), V(G_AB) = 3 (3 mA into
/// 1 kΩ). ngspice reads `$` as a comment, so each global is renamed and
/// declared `.global` (manual §2.4.4): whole names only, or `$G_AB` lands
/// on `G_AB` and ngspice gives 2.5 V there (5 mA into 500 Ω).
#[test]
fn p3_099_a_global_that_heads_another_keeps_its_own_node() {
    let deck = "* t\nV1 $G_A 0 1\nR1 $G_AB 0 1k\nI1 0 $G_AB 2m\n\
        R2 G_AB 0 1k\nI2 0 G_AB 3m\n.op\n.end\n";
    let (xyce, _) = sim_pair(
        Dialect::Xyce,
        deck,
        Dialect::Ngspice,
        "G_AB",
        At::First,
        LINEAR,
    );
    assert!(LINEAR.within(xyce, 3.0), "Xyce v(G_AB) = {xyce}");
    let w = oracle::write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.lines().any(|l| l == ".global G_A G_AB_1"), "{w}");
    assert!(w.lines().any(|l| l.starts_with("R1 G_AB_1 0 ")), "{w}");
}
