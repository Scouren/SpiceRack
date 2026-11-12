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
fn p3_083_sffm_past_ngspice_limits_is_a_behavioural_source() {
    // Ledger P3-083. SPICE3's SFFM(VO VA FC MDI FS) is VO + VA·sin(2π·FC·t
    // + MDI·sin(2π·FS·t)) (Xyce RG; HSPICE SH98 p.5-17). ngspice-45 limits
    // MDI to FC/FS and is 0 at time zero (vsrcload.c:256,272), so such a
    // wave is a behavioural source there and on ESPice. Corpus:
    // XDM/HSPICE/INDEPENDENT_SOURCES/v_sources_sffm.cir. Tolerance: 1e-3 V,
    // what Xyce's own steps leave on a 70 kHz wave (-0.64891 here).
    let tol = Tol {
        rel: 1e-3,
        floor: 1.0,
    };
    let on = [Dialect::Xyce, Dialect::Ngspice, Dialect::Espice];
    // MDI 10 past FC/FS = 4. By hand at 30 us: sin(2π·0.6 + 10·sin(2π·0.15))
    // = sin(3.76991 + 8.09017) = -0.64902 (with MDI 4: +0.66148).
    let deck = "p3-083\nV1 a 0 SFFM(0 1 20k 10 5k)\nR1 a 0 1\n.tran 50n 300u\n.end\n";
    closed_form(
        Dialect::Xyce,
        deck,
        &on,
        "a",
        At::Time(30e-6),
        -0.64902,
        tol,
    );
    // An offset: the RC starts at VO = 1 V. RK4 of it to 5 us gives 1.18534
    // (research/impl/evidence/P3-083/rk4.py); from ngspice's 0 V, 0.57881.
    let deck = "p3-083\nV1 a 0 SFFM(1 1 20k 2 5k)\nR1 a b 1k\nC1 b 0 10n\n.tran 50n 100u\n.end\n";
    closed_form(Dialect::Xyce, deck, &on, "b", At::Time(5e-6), 1.18534, tol);
    // A DC value beside the wave (the corpus decks): it at a DC analysis,
    // the wave through a transient (Xyce 7.10, run).
    let op = "p3-083\nV1 a 0 DC 3 SFFM(0 1 20k 10 5k)\nR1 a 0 1\n.op\n.end\n";
    closed_form(Dialect::Xyce, op, &on, "a", At::First, 3.0, tol);
    let tran = op.replace(".op", ".tran 50n 300u");
    closed_form(Dialect::Xyce, &tran, &on, "a", At::First, 0.0, tol);
    closed_form(
        Dialect::Xyce,
        &tran,
        &on,
        "a",
        At::Time(30e-6),
        -0.64902,
        tol,
    );
    // Inside the limits the card is ngspice's own SFFM, its frequencies
    // swapped (`Pair::sffm_swap`).
    let own = "p3-083\nV1 a 0 SFFM(0 1 20k 4 5k)\nR1 a 0 1\n.tran 50n 300u\n.end\n";
    let ng = oracle::write(own, Dialect::Xyce, Dialect::Ngspice);
    assert!(ng.contains("SFFM(0 1 5000 4 20000)"), "{ng}");
}

#[test]
fn p3_020_ltspice_sine_cycle_count_holds_after_its_cycles() {
    // Ledger P3-020. LTspice help, Current Source: SINE(Ioffset Iamp Freq
    // Td Theta Phi Ncycles) runs Ncycles cycles and then holds. By hand:
    // 0.5 + sin 30° = 1.0 before Td = 2 us; 0.5 + sin 300° = -0.36603 at
    // 9.5 us; from 17 us the value there, 0.5 + sin 570° = 0. LTspice
    // 26.1.1 on this deck: 1, -0.36601, 8e-16
    // (research/impl/evidence/P3-020/lt.cir). No other simulator's SIN
    // stops, so each gets the wave as a behavioural source of time, run
    // here on five of them. Corpus: ltspice_examples/6088.net. Tolerance:
    // 1e-3 V, the simulators' RELTOL on a 1 V sine.
    let deck = "p3-020\nV1 a 0 SINE(0.5 1 100k 2u 0 30 1.5)\nR1 a 0 1k\n.tran 10n 20u\n.end\n";
    let tol = Tol {
        rel: 1e-3,
        floor: 1.0,
    };
    ltspice_on_ngspice(deck, 18e-6, 0.0, tol);
    let on = [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Xyce,
        Dialect::Hspice,
        Dialect::Vacask,
    ];
    for (t, want) in [(1e-6, 1.0), (9.5e-6, -0.36603), (18e-6, 0.0)] {
        closed_form(Dialect::Ltspice, deck, &on, "a", At::Time(t), want, tol);
    }
    // Spectre (no local run): the same expression as a `bsource` of
    // `$time` (Spectre Reference 5.0 [CMP] p.856; `?:` pp.867-868), in
    // series with the 0 V source.
    let sp = oracle::write(deck, Dialect::Ltspice, Dialect::Spectre);
    for want in [
        "v1 (a v1_w) vsource dc=0",
        "bv1_w (v1_w 0) bsource v=$time<0.000002 ? 0.5+1*exp(",
    ] {
        assert!(sp.lines().any(|l| l.starts_with(want)), "{want}\n{sp}");
    }
    // LTspice itself keeps the count.
    let lt = oracle::write(deck, Dialect::Ltspice, Dialect::Ltspice);
    assert!(lt.contains("SIN(0.5 1 100000 0.000002 0 30 1.5)"), "{lt}");
}

#[test]
fn p3_011_pulse_cycle_count_is_kept() {
    // Ledger P3-011. A PULSE's eighth value is its cycle count: LTspice's
    // Ncycles (help, Current Source), ngspice's NP (manual §4.1.1); V1
    // after the last pulse. LTspice 26.1.1 on this deck: 1 V at 7 us (the
    // second pulse), 0 V at 12 us (research/impl/evidence/P3-011/lt.cir);
    // ngspice-45 and ESPice 0.3.0 run `PULSE(… 2)` alike. Xyce 7.10
    // ignores an eighth value (1 V at 12 us), so Xyce, HSPICE (its text on
    // ESPice) and VACASK get the PWL of the two pulses. Corpus:
    // ltspice_demos/LT1158_F18.net. Tolerance: both points are on a flat
    // segment, so only the simulators' rounding is left.
    let deck = "p3-011\nV1 a 0 PULSE(0 1 1u 1n 1n 2u 5u 2)\nR1 a 0 1k\n.tran 10n 20u\n.end\n";
    let tol = Tol {
        rel: 1e-6,
        floor: 1.0,
    };
    ltspice_on_ngspice(deck, 7e-6, 1.0, tol);
    ltspice_on_ngspice(deck, 12e-6, 0.0, tol);
    let pwl = [Dialect::Xyce, Dialect::Hspice, Dialect::Vacask];
    for (t, want) in [(7e-6, 1.0), (12e-6, 0.0)] {
        closed_form(Dialect::Ltspice, deck, &pwl, "a", At::Time(t), want, tol);
    }
}

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

#[test]
fn p3_021_an_ngspice_sffm_is_written_where_spice3s_form_runs_it() {
    // Ledger P3-021. ngspice-45's SFFM is (VO VA FM MDI FC): VO + VA·sin(2π·
    // FC·t + MDI·sin(2π·FM·t)) after time zero and 0 up to it, MDI limited
    // to [0, FC/FM], an FC of 0 read as 500/TSTOP (manual §4.1.5;
    // `vsrcload.c:228-280`). SPICE3's (VO VA FC MDI FS) is the same sum
    // from time zero with no limit (Xyce RG 7.10 Table 2-59; HSPICE UG
    // E-2010.12 p.256). By hand at 213.7 us for SFFM(0 1 200 4 10k):
    // sin(2π·2.137 + 4·sin(2π·0.04274)) = sin(13.42717 + 1.06129) =
    // 0.93892; with the two frequencies swapped, -0.15971
    // (research/impl/evidence/P3-021/closed.py). Tolerance: 1e-3 V, above
    // the 1.4e-4 a line between two 0.5 us steps is off a 10.8 kHz wave.
    let tol = Tol {
        rel: 1e-3,
        floor: 1.0,
    };
    let card = |wave: &str, run: &str| format!("p3-021\nV2 a 0 {wave}\nR1 a 0 1k\n{run}\n.end\n");
    let tran = ".tran 0.5u 1m";
    let at = At::Time(213.7e-6);
    let deck = card("SFFM(0 1 200 4 10k)", tran);
    let on = [Dialect::Ngspice, Dialect::Xyce, Dialect::Vacask];
    closed_form(Dialect::Ngspice, &deck, &on, "a", at, 0.93892, tol);
    // Spectre is not run: `freq` is its sine's frequency, `fmmodfreq` the
    // "FM modulation frequency".
    let cite = "Spectre Reference 5.0 pp.684-685: freq, fmmodindex, fmmodfreq";
    let fm = "v2 (a 0) vsource dc=0 type=sine sinedc=0 ampl=1 freq=10k fmmodindex=4 fmmodfreq=200";
    oracle::spec_example(cite, &deck, Dialect::Ngspice, Dialect::Spectre, &[fm]);
    // An offset (SPICE3's value at time zero, where ngspice is 0), an MDI
    // ngspice limits and an FC of 0 make SPICE3's form another wave:
    // refused. Xyce 7.10 ran each as it was written: 0.5 V at time zero;
    // -0.0956 V and -0.1991 V at 213.7 us for ngspice's 0.9995 V and
    // 0.7583 V; 0.8729 V for 0.1213 V (evidence/P3-021/run.txt).
    let spice3 = [
        Dialect::Xyce,
        Dialect::Hspice,
        Dialect::Ltspice,
        Dialect::Spectre,
        Dialect::Vacask,
    ];
    for wave in [
        "SFFM(0.5 1 200 4 10k)",
        "SFFM(0 1 200 80 10k)",
        "SFFM(0 1 200 -4 10k)",
        "SFFM(0 1 200 4 0)",
    ] {
        let deck = card(wave, tran);
        for to in spice3 {
            let codes = oracle::refusal(&deck, Dialect::Ngspice, to);
            let w0108 = spicerack::syntax::diag::Code::W0108;
            assert!(codes.contains(&w0108), "{wave} for {to:?}: {codes:?}");
        }
        // ESPice runs ngspice's SFFM: the card is written as it is.
        let own = oracle::write(&deck, Dialect::Ngspice, Dialect::Espice);
        assert!(own.contains("SFFM("), "{own}");
    }
    // The legal neighbour: an MDI of FC/FM is inside the limit, written,
    // and the same wave on Xyce.
    let edge = card("SFFM(0 1 200 50 10k)", tran);
    sim_pair(Dialect::Ngspice, &edge, Dialect::Xyce, "a", at, tol);
    // A card with no DC value is 0 V at an operating point too, whatever
    // its offset (ngspice-45, run); one with a DC value is that value.
    let op = card("SFFM(0.5 1 200 4 10k)", ".op");
    sim_pair(Dialect::Ngspice, &op, Dialect::Ngspice, "a", At::First, tol);
    let given = card("DC 3 SFFM(0.5 1 200 4 10k)", ".op");
    sim_pair(
        Dialect::Ngspice,
        &given,
        Dialect::Ngspice,
        "a",
        At::First,
        tol,
    );
}
