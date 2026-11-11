//! Options and temperatures from one dialect to another (the reader's
//! `render/read/options.rs`, the writers' option cards): an option written
//! as the target's option of the same meaning, dropped with a note, or
//! refused. Each test runs the written deck on the target's real simulator
//! against a closed form or the source's simulator (`oracle::closed_form`,
//! `oracle::sim_pair`), or checks the written line against the target
//! manual's clause (`oracle::spec_example`, `oracle::spec_refusal`).
//!
//! Unsupported fails, never skips: a simulator missing from PATH fails
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

/// Ledger P3-004. Xyce RG Table 2-5 (TIMEINT): `METHOD=gear` or 8 is the
/// Gear method, MAXORD the highest order tried. ngspice manual p.318:
/// `METHOD=name` (Gear), `MAXORD=x` (2 to 6 for Gear). VACASK
/// `lib/coretran.cpp`: `tran_method="gear"` reads `tran_maxord`, `"gear2"`
/// is order 2 whatever it says. Both written decks run on their simulator.
#[test]
fn p3_004_xyce_timeint_gear_is_method_gear() {
    let deck = "t\nV1 in 0 SIN(0 1 1k)\nR1 in out 1k\nC1 out 0 1u\n\
        .options timeint method=8 maxord=3\n.tran 10u 2m\n.end\n";
    let ng = oracle::spec_example(
        "ngspice manual p.318: METHOD=name, MAXORD=x",
        deck,
        Dialect::Xyce,
        Dialect::Ngspice,
        &[".options method=gear maxord=3"],
    );
    let va = oracle::write(deck, Dialect::Xyce, Dialect::Vacask);
    assert!(va.contains(" tran_method=\"gear\" tran_maxord=3"), "{va}");
    assert!(!oracle::run(Dialect::Ngspice, &ng).is_empty(), "{ng}");
    assert!(!oracle::run(Dialect::Vacask, &va).is_empty(), "{va}");
}

/// Ledger P3-003. Xyce RG `.OPTIONS TIMEINT` RELTOL is the integrator's
/// relative tolerance, ngspice's RELTOL (manual p.317). A 1 kHz sine
/// through R = 1 kΩ into C = 1 µF (τ = 1 ms, ωτ = 2π) at t = 20.25 ms, the
/// start-up term e^-20.25 gone: v(out) = cos φ / sqrt(1 + (ωτ)²) =
/// 1 / (1 + 4π²). Xyce 7.10 at RELTOL 1e-6 meets it to 2e-6; ngspice-45
/// from the 1 ms print step to 6e-4 with `reltol=1e-6` and to 0.17 at its
/// default 1e-3 (the tolerance: three times the first, far from the last).
#[test]
fn p3_003_xyce_timeint_reltol_is_reltol() {
    let deck = "t\nV1 in 0 SIN(0 1 1k)\nR1 in out 1k\nC1 out 0 1u\n        .options timeint reltol=1e-6\n.tran 1m 20.25m\n.end\n";
    let pi = std::f64::consts::PI;
    closed_form(
        Dialect::Xyce,
        deck,
        &[Dialect::Xyce, Dialect::Ngspice],
        "out",
        At::Last,
        1.0 / (1.0 + 4.0 * pi * pi),
        Tol {
            rel: 2e-3,
            floor: 0.0,
        },
    );
    // NONLIN's RELTOL is Newton's own, not SPICE's (ledger P3-009): it
    // stays Xyce's, left out with a note.
    let nonlin = deck.replace("timeint", "nonlin");
    let w = oracle::write(&nonlin, Dialect::Xyce, Dialect::Ngspice);
    assert!(!w.to_ascii_lowercase().contains("reltol"), "{w}");
}
