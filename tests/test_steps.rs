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
//! Cites: ZERO-LEDGER P5-099, P5-100, P5-101; Spectre Reference 19.1 `[REF]`
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
use spicerack::syntax::Lang;
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
/// The deck is read as the Python API gives it (`Lang::NGSPICE_GIVEN`):
/// a `.step` in an ngspice file is kept as text (W0027; ngspice has none).
/// Tolerance: a linear solve of three elements, RELTOL class.
#[test]
fn p5_101_a_log_step_counts_spectres_steps() {
    let deck = format!("{DIVIDER}.step log rr 1k 16k 5\n.op\n.end\n");
    // The card's points, as the script written from it runs them.
    let want = [0.5, 2.0 / 3.0, 0.8, 8.0 / 9.0, 16.0 / 17.0];
    let text = oracle::write(&deck, Lang::NGSPICE_GIVEN, Dialect::Ngspice);
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
        Lang::NGSPICE_GIVEN,
        Dialect::Spectre,
        &["sweep1_rr sweep param=rr start=1k stop=16k log=4 {"],
    );
    // One point is no step of Spectre's `log=`: refused, like a count
    // that is no whole number.
    for count in ["1", "2.5"] {
        let deck = format!("{DIVIDER}.step log rr 1k 16k {count}\n.op\n.end\n");
        let codes = oracle::refusal(&deck, Lang::NGSPICE_GIVEN, Dialect::Spectre);
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

/// ZERO-LEDGER P5-099. Oracle: the source's own simulator running the
/// source deck (LTspice 26.1.1, again here unless CI's
/// `LTSPICE_CI_PENDING_MIRROR` marker says it is absent; Xyce 7.10), each
/// point the divider's closed form v(out) = rr / (1k + rr). LTspice ends
/// a range on its stop value and reads the bounds in either order; Xyce
/// ends a grid at its last point (RG 7.10 §2.1.36). By hand:
/// research/impl/evidence/P5-099/RESULTS.txt. The written deck runs on
/// ngspice, VACASK, Xyce and, from Xyce, on LTspice; Spectre has no
/// binary here: where its manual does not say how a grid that misses
/// its stop ends (Reference 19.1 pp.406-408), its text is the points
/// themselves. Tolerance: a linear solve of three elements; LTspice's
/// raw file holds single floats (6e-8).
#[test]
fn p5_099_a_ranged_step_runs_the_sources_points_on_every_target() {
    // 1k · 10^(1/2) and 10k · 10^(1/2), the points two a decade.
    const HALF: f64 = 3_162.277_660_168_38;
    let out = |points: &[f64]| -> Vec<f64> { points.iter().map(|r| r / (1e3 + r)).collect() };
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
    // The source, its card, and the values of rr its simulator steps
    // through (la, lb, lc, xa, xb of RESULTS.txt).
    let cases: [(Dialect, &str, &[f64]); 5] = [
        (
            Dialect::Ltspice,
            ".step param rr 1k 10k 4k",
            &[1e3, 5e3, 9e3, 1e4],
        ),
        (
            Dialect::Ltspice,
            ".step dec param rr 1k 50k 2",
            &[1e3, HALF, 1e4, 10.0 * HALF, 5e4],
        ),
        (
            Dialect::Ltspice,
            ".step param rr 3k 1k 0.8k",
            &[1e3, 1.8e3, 2.6e3, 3e3],
        ),
        (Dialect::Xyce, ".step rr 1k 10k 4k", &[1e3, 5e3, 9e3]),
        (
            Dialect::Xyce,
            ".step dec rr 1k 50k 2",
            &[1e3, HALF, 1e4, 10.0 * HALF],
        ),
    ];
    for (from, card, points) in cases {
        let deck = format!("{DIVIDER}{card}\n.op\n.end\n");
        let want = out(points);
        if from != Dialect::Ltspice || ltspice {
            meets(from, &deck, &want);
        }
        for to in [Dialect::Ngspice, Dialect::Vacask, Dialect::Xyce] {
            let text = oracle::write(&deck, from, to);
            meets(to, &text, &want);
        }
        // LTspice runs a list as written: the points Xyce stopped at.
        let text = oracle::write(&deck, from, Dialect::Ltspice);
        if from == Dialect::Xyce && ltspice {
            meets(Dialect::Ltspice, &text, &want);
        }
        let listed = text.lines().any(|l| l.starts_with(".step param rr list "));
        assert!(listed, "{from:?} `{card}`\n{text}");
    }
    // Spectre: the points, where `step=` would leave the grid's end to it.
    for (from, card, head) in [
        (
            Dialect::Ltspice,
            ".step param rr 1k 10k 4k",
            "sweep1_rr sweep param=rr values=[1k 5k 9k 10k] {",
        ),
        (
            Dialect::Xyce,
            ".step rr 1k 10k 4k",
            "sweep1_rr sweep param=rr values=[1k 5k 9k] {",
        ),
        // A grid that reaches its stop value stays a range on every target.
        (
            Dialect::Xyce,
            ".step rr 1k 9k 4k",
            "sweep1_rr sweep param=rr start=1k stop=9k step=4k {",
        ),
    ] {
        let deck = format!("{DIVIDER}{card}\n.op\n.end\n");
        oracle::spec_example(
            "Spectre Reference 19.1 pp.406-408: sweep, values=[...]",
            &deck,
            from,
            Dialect::Spectre,
            &[head],
        );
    }
    let deck = format!("{DIVIDER}.step rr 1k 9k 4k\n.op\n.end\n");
    let text = oracle::write(&deck, Dialect::Xyce, Dialect::Ltspice);
    let range = text.lines().any(|l| l == ".step param rr 1000 9000 4000");
    assert!(range, "{text}");
}

/// ZERO-LEDGER P3-134 (the text path of P3-037). Oracle: Xyce 7.10
/// running the source deck (a `.step` over a bare device name sets its
/// default parameter, RG 7.10 `.STEP`: a source's DC value), each point
/// the divider's closed form v(out) = V1 · 3/4, met by LTspice 26.1.1 on
/// the written deck (unless CI's `LTSPICE_CI_PENDING_MIRROR` marker says
/// it is absent). Spectre has no binary here: its text is held to
/// Reference 19.1 pp.406-408 (`sweep`: `dev`, "Device instance whose
/// parameter value is to be swept"; `param`, "Name of parameter to
/// sweep"). What a target cannot say is refused: LTspice 26.1.1 runs
/// `.step param R2 …` and `.step param R2:R …` with R2 as its card has it
/// and stops on a stepped source with a waveform ("Can't sweep that.").
/// By hand: research/impl/evidence/P3-134/run.txt. Tolerance: a linear
/// solve of three elements; LTspice's raw file holds single floats (6e-8).
#[test]
fn p3_134_a_stepped_source_steps_its_own_value_on_ltspice_and_spectre() {
    const DECK: &str = "* stepped source\nV1 in 0 1\nR1 in out 1k\nR2 out 0 3k\n";
    let stepped = |card: &str| format!("{DECK}{card}\n.op\n.end\n");
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
    let sweep = "Spectre Reference 19.1 pp.406-408: sweep, dev=, param=";
    // The card (LTspice's too), and Spectre's sweep head.
    let cases = [
        (
            ".step V1 1 3 1",
            "sweep1_v1 sweep dev=v1 param=dc start=1 stop=3 step=1 {",
        ),
        (
            ".step V1 list 1 2 3",
            "sweep1_v1 sweep dev=v1 param=dc values=[1 2 3] {",
        ),
    ];
    for (card, head) in cases {
        let deck = stepped(card);
        meets(Dialect::Xyce, &deck, &[0.75, 1.5, 2.25]);
        let text = oracle::write(&deck, Dialect::Xyce, Dialect::Ltspice);
        assert!(text.lines().any(|l| l == card), "no `{card}`\n{text}");
        if ltspice {
            meets(Dialect::Ltspice, &text, &[0.75, 1.5, 2.25]);
        }
        oracle::spec_example(sweep, &deck, Dialect::Xyce, Dialect::Spectre, &[head]);
    }
    // A resistor by its value: Spectre's `r`; LTspice has no such item.
    let resistor = stepped(".step R2 1k 3k 1k");
    let head = "sweep1_r2 sweep dev=r2 param=r start=1k stop=3k step=1k {";
    oracle::spec_example(sweep, &resistor, Dialect::Xyce, Dialect::Spectre, &[head]);
    let codes = oracle::refusal(&resistor, Dialect::Xyce, Dialect::Ltspice);
    assert_eq!(codes, [Code::W0016], "{resistor}");
    // A device's parameter and a source with a waveform: neither target.
    let wave = "* wave\nV1 in 0 SIN(0 1 1k)\nR1 in 0 1k\n.step V1 1 3 1\n.op\n.end\n";
    let parameter = stepped(".step R2:R 1k 3k 1k");
    for to in [Dialect::Ltspice, Dialect::Spectre] {
        for deck in [parameter.as_str(), wave] {
            let codes = oracle::refusal(deck, Dialect::Xyce, to);
            assert_eq!(codes, [Code::W0016], "{to:?}\n{deck}");
        }
    }
    // The temperature is LTspice's `temp` item (help `.STEP`).
    let temp = stepped(".step temp 0 100 50");
    let text = oracle::write(&temp, Dialect::Xyce, Dialect::Ltspice);
    assert!(text.lines().any(|l| l == ".step temp 0 100 50"), "{text}");
}
