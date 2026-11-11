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

/// Ledger P3-001. HSPICE's TNOM and TEMP default to 25 °C (cmdref
/// `.OPTION TNOM`, `.TEMP`), ngspice's to 27 (manual §11.1.1): a deck
/// that leaves both to ngspice says 27 once it is written for HSPICE.
/// 1 mA into a diode (IS = 1e-14, N = 1): v(a) = kT/q · ln(I/IS + 1) is
/// 0.655118 V at 300.15 K and 0.650753 V at 298.15 K; ngspice-45 gives the
/// first on the source, ESPice's HSPICE reading the second on the deck
/// written without the two cards and the first with them (1e-16 apart:
/// one device code, so 1e-6). The cards are the ones an HSPICE deck's own
/// read and write gives back, also by way of ngspice (the store's detour).
#[test]
fn p3_001_a_deck_written_for_hspice_runs_at_the_sources_27c() {
    let deck = "t\nI1 0 a 1m\nD1 a 0 dmod\n.model dmod d (is=1e-14 n=1)\n.op\n.end\n";
    let (ng, _) = oracle::sim_pair(
        Dialect::Ngspice,
        deck,
        Dialect::Hspice,
        "a",
        At::First,
        Tol {
            rel: 1e-6,
            floor: 0.0,
        },
    );
    assert!((ng - 0.655118).abs() < 1e-6, "ngspice at 27 °C: {ng}");
    let cards = "\n.options tnom=27\n.temp 27\n";
    let hs = oracle::write(deck, Dialect::Ngspice, Dialect::Hspice);
    assert!(hs.contains(cards), "{hs}");
    let again = oracle::write(&hs, Dialect::Hspice, Dialect::Hspice);
    assert!(again.contains(cards), "{again}");
    let detour = oracle::write(&hs, Dialect::Hspice, Dialect::Ngspice);
    let back = oracle::write(&detour, Dialect::Ngspice, Dialect::Hspice);
    assert!(back.contains(cards), "{back}");
    // A deck that sets both keeps its own.
    let own = deck.replace(".op\n", ".temp 50\n.options tnom=30\n.op\n");
    let hs = oracle::write(&own, Dialect::Ngspice, Dialect::Hspice);
    assert!(hs.contains("\n.options tnom=30\n.temp 50\n"), "{hs}");
    assert!(!hs.contains("27"), "{hs}");
}

/// Ledger P3-001 (ruling: one place per writer for the nominal
/// temperature). Spectre Reference 19.1 p.187: `tnom` is an `options`
/// parameter (#7). A deck's `.options tnom=30` is the statement the writer
/// has for it, `mytnom options tnom=30`, as a Spectre deck's own `tnom`
/// is; it no longer rides in `myopts`.
#[test]
fn p3_001_a_spice_tnom_option_is_spectres_tnom_statement() {
    oracle::spec_example(
        "Spectre Reference 19.1 p.187: options tnom (#7)",
        "t\nV1 a 0 1\nR1 a 0 1k\n.options reltol=1e-4 tnom=30\n.op\n.end\n",
        Dialect::Ngspice,
        Dialect::Spectre,
        &["myopts options reltol=100u", "mytnom options tnom=30"],
    );
}

/// Ledger P3-059. Xyce RG Table 2-2 (DEVICE): MINRES and MINCAP stand in
/// for a model's default-zero internal resistance and capacitance; ngspice
/// has no such option (manual §11.1). Xyce 7.10 by hand on this deck:
/// v(a) = 2.867 V, and 0.752 V without the option, which is what ngspice
/// gives for the deck with the option left out: refused.
#[test]
fn p3_059_xyce_minres_and_mincap_are_refused_where_there_is_none() {
    let w0017 = spicerack::syntax::diag::Code::W0017;
    let deck = "t\nV1 in 0 5\nR1 in a 100\nD1 a 0 dm\n.model dm D(IS=1e-14)\n\
        .options device minres=100\n.op\n.end\n";
    let xyce = oracle::op(Dialect::Xyce, deck, "a");
    assert!((xyce - 2.867).abs() < 1e-3, "Xyce with MINRES: {xyce}");
    for d in [deck.to_string(), deck.replace("minres=100", "mincap=1n")] {
        let codes = oracle::refusal(&d, Dialect::Xyce, Dialect::Ngspice);
        assert!(codes.contains(&w0017), "{codes:?}\n{d}");
    }
}

/// Ledger P3-058. Xyce RG Table 2-6: NONLIN CONTINUATION=1 is the
/// natural-parameter homotopy (with the LOCA block), 2/`mos` the MOSFET
/// one; ngspice has gmin and source stepping only (manual §11.1.2), so the
/// deck is refused there. `continuation=gmin` is that stepping itself: left
/// out with a note, as a lone `.options loca` is (Xyce reads it only with
/// CONTINUATION on).
#[test]
fn p3_058_xyce_homotopy_is_refused_where_there_is_none() {
    let w0017 = spicerack::syntax::diag::Code::W0017;
    let deck = "t\nV1 a 0 1\nR1 a 0 1k\n.options nonlin continuation=1\n\
        .options loca stepper=0\n.op\n.end\n";
    for to in [Dialect::Ngspice, Dialect::Hspice, Dialect::Ltspice] {
        let codes = oracle::refusal(deck, Dialect::Xyce, to);
        assert!(codes.contains(&w0017), "{to:?}: {codes:?}");
    }
    let mos = deck.replace("continuation=1", "continuation=mos");
    let codes = oracle::refusal(&mos, Dialect::Xyce, Dialect::Ngspice);
    assert!(codes.contains(&w0017), "{codes:?}");
    let gmin = deck.replace("continuation=1", "continuation=gmin");
    let w = oracle::written(&gmin, Dialect::Xyce, Dialect::Ngspice);
    assert!(!oracle::notes(&w).contains(&w0017), "{}", w.text);
    assert_eq!(oracle::op(Dialect::Ngspice, &w.text, "a"), 1.0);
}

/// Ledger P3-015. Xyce 7.10 reads `.options temp=100` and `.option
/// Temp=100.0C`, with no package word, as `.options device temp=100` (one
/// answer by hand): ngspice's `.temp 100` (manual §11.1.1 TEMP). 1 V
/// through 1 kΩ into a diode (IS = 1e-14): its drop is 0.5011 V at 100 °C
/// and 0.6294 V at 27 °C in both simulators, which differ by 4e-5 (their
/// kT/q, TESTING.md), so 1e-3.
#[test]
fn p3_015_xyce_options_temp_without_a_package_is_the_temperature() {
    let deck = "t\nV1 in 0 1\nR1 in a 1k\nD1 a 0 dm\n.model dm D(IS=1e-14)\n\
        .options temp=100\n.op\n.end\n";
    let (xyce, _) = oracle::sim_pair(
        Dialect::Xyce,
        deck,
        Dialect::Ngspice,
        "a",
        At::First,
        Tol {
            rel: 1e-3,
            floor: 0.0,
        },
    );
    assert!((xyce - 0.5011).abs() < 1e-3, "Xyce at 100 °C: {xyce}");
    oracle::spec_example(
        "ngspice manual §11.1.1 (TEMP); Xyce 7.10: `.option Temp=100.0C` is 100 °C",
        &deck.replace(".options temp=100", ".option Temp=100.0C"),
        Dialect::Xyce,
        Dialect::Ngspice,
        &[".temp 100"],
    );
}

/// Ledger P3-008. Xyce RG `.OPTIONS PARSER` SCALE scales every L and W,
/// as ngspice's SCALE (manual p.318), HSPICE's and VACASK's `scale`. A
/// level-1 NMOS (VTO 1, KP 1e-4, LD 0.2u) at `L=1 W=10` and SCALE 1e-6,
/// gate at 2 V, drain through 1 kΩ from 5 V, saturated:
/// Id = KP/2 · W/(L − 2·LD) · (Vgs − VTO)² = 0.5e-4 · 10/0.6, so
/// v(d) = 5 − 1000·Id = 4.1667 (the junctions' GMIN moves it 4e-9; without
/// the scale v(d) = 4.5). LTspice 26.1.1 has no SCALE (`.options
/// scale=1u`: "syntax error"): refused.
#[test]
fn p3_008_xyce_parser_scale_is_scale() {
    let deck = "t\nVdd dd 0 5\nVg g 0 2\nRd dd d 1k\nM1 d g 0 0 nm L=1 W=10\n\
        .model nm NMOS(LEVEL=1 VTO=1 KP=1e-4 LD=0.2u)\n.options parser scale=1u\n.op\n.end\n";
    closed_form(
        Dialect::Xyce,
        deck,
        &[
            Dialect::Xyce,
            Dialect::Ngspice,
            Dialect::Hspice,
            Dialect::Vacask,
        ],
        "d",
        At::First,
        5.0 - 1e3 * 0.5e-4 * 10.0 / 0.6,
        Tol {
            rel: 1e-8,
            floor: 0.0,
        },
    );
    let codes = oracle::refusal(deck, Dialect::Xyce, Dialect::Ltspice);
    assert!(
        codes.contains(&spicerack::syntax::diag::Code::W0017),
        "{codes:?}"
    );
}

/// Ledger P3-012. Xyce RG Table 2-2: DEVICE GMIN is the minimum conductance,
/// ngspice's GMIN (manual p.317). 1 V through 1 MΩ into a reverse diode
/// (IS = 1e-14) whose GMIN is 1e-6 S: (1 − v)/R = GMIN·v + IS, so
/// v(a) = (1e-6 − 1e-14)/2e-6. Xyce 7.10, ngspice-45 and ESPice are each
/// within 2e-12 of it by hand; ngspice at its default GMIN gives 1.0.
/// HSPICE's GMIN is the transient's and GMINDC the DC one (cmdref
/// `.OPTION GMIN`, `.OPTION GMINDC`): both written.
#[test]
fn p3_012_xyce_device_gmin_is_gmin() {
    let deck = "t\nV1 in 0 1\nR1 in a 1meg\nD1 0 a dm\n.model dm D(IS=1e-14)\n\
        .options device gmin=1e-6\n.op\n.end\n";
    closed_form(
        Dialect::Xyce,
        deck,
        &[Dialect::Xyce, Dialect::Ngspice, Dialect::Hspice],
        "a",
        At::First,
        (1e-6 - 1e-14) / 2e-6,
        Tol {
            rel: 1e-9,
            floor: 0.0,
        },
    );
    oracle::spec_example(
        "HSPICE cmdref .OPTION GMIN (transient), .OPTION GMINDC (DC)",
        deck,
        Dialect::Xyce,
        Dialect::Hspice,
        &[".options gmin=1e-6 gmindc=1e-6"],
    );
}

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
    let deck = "t\nV1 in 0 SIN(0 1 1k)\nR1 in out 1k\nC1 out 0 1u\n.options timeint reltol=1e-6\n.tran 1m 20.25m\n.end\n";
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
