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

// Ledger P3-080. Xyce holds a BSIM3 instance's `IC=Vds,Vgs` in its DCOP
// (RG 7.10 `M`, `IC`; Xyce 7.10: V(3) = 0.5, V(2) = 1.2 at t = 0, and
// 1.96126 / 2.0 with the IC removed); ngspice reads a MOS `ic=` only under
// UIC (manual §7.6.1), so the deck carries `.ic V(3)=0.5` and `.ic
// V(2)=1.2`. The corpus decks: XYCE_CORPUS `INIT_CONDS/comparatorIC.cir`,
// `IC_AND_NODESET/ring_osc_bsim3.cir`. t = 0: both simulators hold the
// node at the given value to 15 digits (LINEAR). t = 20 ns: the two BSIM3
// codes and integrators agree to 2.2e-4 (1.79610881 against 1.795716):
// 1e-3, each run's RELTOL.
#[test]
fn p3_080_xyce_bsim3_instance_ic_holds_in_the_op() {
    let deck = "* t\nvdd 1 0 2\nr1 1 3 10k\nc1 3 0 1p\nr2 1 2 10k\nc2 2 0 1p\n\
                mn1 3 2 0 0 n1 l=0.35u w=5u IC=0.5,1.2\n.model n1 nmos level=9\n\
                .tran 10p 20n\n.end\n";
    for (node, held) in [("3", 0.5), ("2", 1.2)] {
        let (xyce, ngspice) = sim_pair(
            Dialect::Xyce,
            deck,
            Dialect::Ngspice,
            node,
            At::First,
            LINEAR,
        );
        assert!(LINEAR.within(xyce, held), "Xyce v({node}) = {xyce}");
        assert!(
            LINEAR.within(ngspice, held),
            "ngspice v({node}) = {ngspice}"
        );
    }
    let late = Tol {
        rel: 1e-3,
        floor: 1e-12,
    };
    sim_pair(Dialect::Xyce, deck, Dialect::Ngspice, "3", At::Last, late);
    // An `.op` deck: ngspice applies no `.ic` there, so the line stays refused.
    let op = deck.replace(".tran 10p 20n", ".op");
    assert_eq!(
        oracle::refusal(&op, Dialect::Xyce, Dialect::Ngspice),
        [spicerack::syntax::diag::Code::W0049]
    );
}

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
