//! A deck that steps (`.step`) written for another simulator steps
//! through the same points in the same order. The oracle is a divider's
//! closed form at each point the source's own simulator stepped through
//! (run by hand: `research/impl/evidence/P5-101/`), met by the real
//! simulator of each target that runs here; Spectre has no binary here,
//! so its text is held to its manual's sentence.
//!
//! Unsupported fails, never skips: without `ngspice`, `vacask` and
//! `ltspice` on PATH every run fails; run the tests in `nix develop
//! .#testing-ltspice`. LTspice runs unless CI's `LTSPICE_CI_PENDING_MIRROR`
//! marker says it is absent
//! (`tests/test_backends.rs::p1_018_ltspice_wine_pinned_into_testing`).
//!
//! Cites: ZERO-LEDGER P5-100, P5-101; Spectre Reference 19.1 `[REF]`
//! pp.406-408 (`sweep`: nesting; `log`, "Number of steps, log sweep");
//! `python/keywords.rs` `Sweep::Log` (N points in total); Xyce RG 7.10
//! §2.1.36 (`.STEP`: "the first .STEP line … will be the innermost
//! loop"); LTspice help `.step`.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;

/// A divider up to its `.step` cards: v(out) = vv · rr / (1k + rr).
const DIVIDER: &str = "* steps\n.param rr=1k vv=1\nV1 in 0 {vv}\nR1 in out 1k\nR2 out 0 {rr}\n";

// ---- private ----

/// v(out) at every point of every plot of `text` run on `on`, in run
/// order: a stepped run is a plot a point on ngspice and LTspice, one
/// plot of every point on VACASK.
fn outs(on: Dialect, text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    for p in &oracle::run(on, text) {
        let named = |n: &str| n.eq_ignore_ascii_case("v(out)") || n.eq_ignore_ascii_case("out");
        let Some(i) = p.variables.iter().position(|v| named(&v.name)) else {
            continue;
        };
        assert!(!p.is_complex, "v(out) is complex in '{}'", p.plot_name);
        out.extend_from_slice(&p.real_data[i]);
    }
    assert!(!out.is_empty(), "{on:?}: no v(out) in any plot\n{text}");
    out
}

/// ZERO-LEDGER P5-101. Oracle: Spectre Reference 19.1 p.407 (`sweep`'s
/// `log`: "Number of steps, log sweep", so N steps are N + 1 points, as
/// its `lin`), beside what the card means: `Sweep::Log`'s N points in
/// all, which ngspice-45 ran by hand as the loop written from the same
/// card (five points, 1k to 16k: research/impl/evidence/P5-101/n1.log).
/// Tolerance: a linear solve of three elements, RELTOL class.
#[test]
fn p5_101_a_log_step_counts_spectres_steps() {
    let deck = format!("{DIVIDER}.step log rr 1k 16k 5\n.op\n.end\n");
    // The card's points, as the script written from it runs them.
    let want = [0.5, 2.0 / 3.0, 0.8, 8.0 / 9.0, 16.0 / 17.0];
    let text = oracle::write(&deck, Dialect::Ngspice, Dialect::Ngspice);
    let got = outs(Dialect::Ngspice, &text);
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    assert_eq!(got.len(), want.len(), "five points: {got:?}\n{text}");
    for (g, w) in got.iter().zip(want) {
        assert!(tol.within(*g, w), "v(out) = {g}, want {w}\n{text}");
    }
    // Five points are four of Spectre's steps.
    oracle::spec_example(
        "Spectre Reference 19.1 p.407: log, Number of steps, log sweep",
        &deck,
        Dialect::Ngspice,
        Dialect::Spectre,
        &["sweep1_rr sweep param=rr start=1k stop=16k log=4 {"],
    );
    // One point is no step of Spectre's `log=`: refused, like a count
    // that is no whole number.
    for count in ["1", "2.5"] {
        let deck = format!("{DIVIDER}.step log rr 1k 16k {count}\n.op\n.end\n");
        let codes = oracle::refusal(&deck, Dialect::Ngspice, Dialect::Spectre);
        assert_eq!(codes, [Code::W0016], "log {count}");
    }
}

/// ZERO-LEDGER P5-100. Oracle: LTspice 26.1.1 running the source deck
/// itself (again here unless CI's `LTSPICE_CI_PENDING_MIRROR` marker says
/// it is absent): a range and a list `.step` loop the first card
/// innermost in either card order, as Xyce RG 7.10 §2.1.36 says of Xyce
/// (Xyce 7.10, VACASK 36be560 and ngspice-45 gave the same twelve values
/// by hand: research/impl/evidence/P5-100/RESULTS.txt), each point the
/// divider's closed form v(out) = vv · rr / (1k + rr). The written deck
/// runs on LTspice, ngspice (the loops) and VACASK. Xyce 7.10 crashes on
/// two `.step` lines beside `.op` (TESTING.md "Known gaps") and Spectre
/// has no binary here: their text is held to the manuals' order (Xyce RG
/// §2.1.36; Spectre Reference 19.1 p.406: a sweep runs what its braces
/// hold "for each value of the parameter"). Tolerance: a linear solve
/// of three elements; LTspice's raw file holds single floats (6e-8).
#[test]
fn p5_100_the_first_step_is_the_innermost_loop_on_every_target() {
    const RANGE: &str = ".step param rr 1k 3k 1k\n";
    const LIST: &str = ".step param vv list 1 2\n";
    // The cards in each order; v(out) in run order (rr innermost, then
    // vv innermost); Spectre's outer and inner sweep heads.
    let cases = [
        (
            RANGE,
            LIST,
            [0.5, 2.0 / 3.0, 0.75, 1.0, 4.0 / 3.0, 1.5],
            "sweep2_vv ",
            "  sweep1_rr ",
        ),
        (
            LIST,
            RANGE,
            [0.5, 1.0, 2.0 / 3.0, 4.0 / 3.0, 0.75, 1.5],
            "sweep1_rr ",
            "  sweep2_vv ",
        ),
    ];
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    let meets = |on: Dialect, text: &str, want: &[f64]| {
        let got = outs(on, text);
        let near = got.len() == want.len() && got.iter().zip(want).all(|(g, w)| tol.within(*g, *w));
        assert!(near, "{on:?}: v(out) = {got:?}, want {want:?}\n{text}");
    };
    let ltspice = std::env::var_os("LTSPICE_CI_PENDING_MIRROR").is_none();
    for (first, second, want, outer, inner) in cases {
        let deck = format!("{DIVIDER}{first}{second}.op\n.end\n");
        if ltspice {
            meets(Dialect::Ltspice, &deck, &want);
        }
        for to in [Dialect::Ltspice, Dialect::Ngspice, Dialect::Vacask] {
            let text = oracle::write(&deck, Dialect::Ltspice, to);
            if to != Dialect::Ltspice || ltspice {
                meets(to, &text, &want);
            }
        }
        // Xyce: the cards in the source's order.
        let xyce = oracle::write(&deck, Dialect::Ltspice, Dialect::Xyce);
        let at = |card: &str| xyce.lines().position(|l| l.starts_with(card));
        let (rr, vv) = (at(".step rr "), at(".step vv "));
        assert!(rr.is_some() && vv.is_some(), "{xyce}");
        assert_eq!(rr < vv, first == RANGE, "{xyce}");
        // Spectre: the first card's sweep inside the second's.
        let spectre = oracle::write(&deck, Dialect::Ltspice, Dialect::Spectre);
        for head in [outer, inner] {
            let held = spectre.lines().any(|l| l.starts_with(head));
            assert!(held, "no `{head}`\n{spectre}");
        }
    }
}
