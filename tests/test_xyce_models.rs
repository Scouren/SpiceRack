//! ngspice model cards written for Xyce by Xyce's own device
//! (`render/spice/xyce_models.rs`): a parameter Xyce spells another way, or
//! whose default is another value there, is written so that Xyce computes
//! ngspice's numbers. The oracle of every test is a closed form the deck is
//! built to give, met by real ngspice-45 on the source and by real Xyce
//! 7.10 on the deck written for it.
//!
//! Unsupported fails, never skips: no Xyce or ngspice on PATH fails
//! (`nix develop .#testing`).
//!
//! Cites: per test (RG = Xyce Reference Guide 7.10).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol, closed_form};
use spicerack::syntax::Dialect;

// ---- private ----

/// ZERO-LEDGER P1-066. ngspice's MESFET threshold is `vt0` (alias `vto`),
/// -2 V when the card gives none (ngspice-45 `mes.c`, `messetup.c`);
/// Xyce's `Z` level 1 reads `VTO` alone, 0 V by default, and ignores `VT0`
/// with a warning (7.10.0: research/impl/evidence/P1-066-xyce-mes/runs.txt).
/// Oracle: the Statz drain current in saturation (vds > 3/alpha = 1.5 V;
/// lambda 0 and b 0.3, both simulators' defaults), id = beta·vgst²/(1 +
/// b·vgst), through a 1 kΩ load from 3 V, met by real ngspice on the
/// source and real Xyce on the written deck. GMIN across the device moves
/// the node by 3e-9 V: 1e-6 relative. Checks: 3 × 2.
#[test]
fn p1_066_a_mesfet_threshold_holds_on_xyce() {
    let deck = |card: &str| {
        format!(
            "* t\nvdd 9 0 dc 3\nrl 9 1 1k\nvgs 2 0 dc -1\nz1 1 2 0 zm\n\
             .model zm nmf level=1 beta=1.4e-3{card}\n.op\n.end\n"
        )
    };
    let v1 = |vgst: f64| 3.0 - 1e3 * 1.4e-3 * vgst * vgst / (1.0 + 0.3 * vgst);
    let ng = Dialect::Ngspice;
    let on = [ng, Dialect::Xyce];
    let tol = Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    for (card, vto) in [(" vt0=-1.3", -1.3), (" vto=-1.3", -1.3), ("", -2.0)] {
        let want = v1(-1.0 - vto);
        closed_form(ng, &deck(card), &on, "1", At::First, want, tol);
    }
}
