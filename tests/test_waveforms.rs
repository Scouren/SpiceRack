//! Source waveforms and source cards read from one dialect and written for
//! another (`render/read/sources.rs`, `render/spice/waves.rs`,
//! `render/named/sources.rs`): the red-first tests of the ledger's Phase 3
//! source rows (`research/ZERO-LEDGER.md`), one per row, named `p3_<row>_…`.
//!
//! Oracle: per test, the source simulator on the source deck against the
//! target simulator on the written deck (`oracle::sim_pair`), or a number
//! from the manual clause the test cites, met by the target simulator
//! (`oracle::closed_form`). An LTspice number is what LTspice 26.1.1
//! printed for the source deck (the row's evidence, quoted in the test);
//! LTspice runs the deck again here unless CI's `LTSPICE_CI_PENDING_MIRROR`
//! marker says it is absent
//! (`tests/test_backends.rs::p1_018_ltspice_wine_pinned_into_testing`).
//!
//! Cites: per test.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{At, Tol, closed_form, sim_pair};
use spicerack::syntax::Dialect;

// ---- private ----

/// The LTspice deck `deck` written for ngspice gives `want` volts at node
/// `a` at time `t` on real ngspice. `want` is the number LTspice 26.1.1
/// printed for `deck`; LTspice gives it again here (`sim_pair`) unless
/// the CI marker says LTspice is absent.
fn ltspice_on_ngspice(deck: &str, t: f64, want: f64, tol: Tol) {
    let on = [Dialect::Ngspice];
    closed_form(Dialect::Ltspice, deck, &on, "a", At::Time(t), want, tol);
    if std::env::var_os("LTSPICE_CI_PENDING_MIRROR").is_none() {
        sim_pair(
            Dialect::Ltspice,
            deck,
            Dialect::Ngspice,
            "a",
            At::Time(t),
            tol,
        );
    }
}

// A row's test goes below this line (other helpers: `oracle::…` by path).

#[test]
fn p3_005_ltspice_pwl_relative_time_is_absolute() {
    // Ledger P3-005. LTspice help, Voltage Source: a PWL time written `+t`
    // counts from the point before it, so this PWL is (0,0) (1u,1) (2u,2)
    // (3u,0) (5u,3) in all three spellings (`+1u`, `+ 1u`, `+{dt}`).
    // LTspice 26.1.1 on this deck: 1.0 V at 2.5 us, 1.5 V at 4 us
    // (research/impl/evidence/P3-005/lt2.cir). Corpus:
    // ltspice_demos/LT4256-2_TA01.net (`PWL(2.7m 0 +1u 48)`),
    // LTC4225-2_TA01A.net (`+ 10m`). Tolerance: both points are inside a
    // linear segment, so only the simulators' rounding is left.
    let deck = "p3-005\n.param dt=2u\nV1 a 0 PWL(0 0 1u 1 +1u 2 + 1u 0 +{dt} 3)\n\
        R1 a 0 1k\n.tran 10n 6u\n.end\n";
    let tol = Tol {
        rel: 1e-6,
        floor: 1e-3,
    };
    ltspice_on_ngspice(deck, 2.5e-6, 1.0, tol);
    ltspice_on_ngspice(deck, 4e-6, 1.5, tol);
}
