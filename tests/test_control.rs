//! An ngspice `.control` script's loops and `if`s over constants, written
//! for a target: the reader unrolls the script into the lines it runs
//! (`src/syntax/render/read/unroll.rs`), each pass's `alterparam` at its
//! `reset` the next circuit state of the run plan. The oracle is ngspice
//! running the source script itself, plot by plot, beside the divider's
//! closed form at each state; the written deck runs on ngspice (the
//! unrolled script) and on Xyce (one table row a state). A script the
//! reader cannot follow refuses the deck by its code.
//!
//! Unsupported fails, never skips: without `ngspice` and `Xyce` on PATH
//! every run fails; run the tests in `nix develop .#testing`.
//!
//! Cites: ZERO-LEDGER P5-088; DESIGN-control-epochs §4, §8 D1; ngspice-45
//! manual §13.6 (the control structures), §13.5.5 (`alterparam`), §13.5.45
//! (`let`), §13.7 (`csnumprec`). Run on ngspice-45 and Xyce 7.10 by hand:
//! research/impl/evidence/P5-088/RESULTS.txt.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use spicerack::backend::Dialect;
use spicerack::syntax::diag::{Code, Kind, info};

/// An ngspice divider up to its `.control` line: v(out) = rr / (1k + rr).
const SCRIPTED: &str = "* script\n.param rr=1k\nV1 in 0 1\nR1 in out 1k\nR2 out 0 {rr}\n.control\n";

// ---- private ----

/// v(out) of each plot `text` gives on `on`'s simulator, in run order.
fn outs(on: Dialect, text: &str) -> Vec<f64> {
    let plots = oracle::run(on, text);
    (0..plots.len())
        .map(|k| oracle::value(&plots[k..k + 1], "out", oracle::At::First))
        .collect()
}

/// ZERO-LEDGER P5-088 (DESIGN-control-epochs §4, §8 D1). Oracle: ngspice-45
/// running the source script itself, a plot a run (the harness writes
/// every plot of a `.control` run), and the closed form v(out) = rr / (1k
/// + rr) at each state; each script was run by hand first
/// (research/impl/evidence/P5-088/RESULTS.txt §1). The deck written from
/// the unrolled script runs on ngspice and, one table row a state, on
/// Xyce 7.10 (§3). Tolerance: a linear solve of three elements, RELTOL
/// class; the six-digit case is 4e-6 from its unrounded value.
#[test]
fn p5_088_ngspice_control_logic_static_bounds() {
    // The script, and v(out) of each of its runs.
    let cases: [(&str, &[f64]); 6] = [
        // A word a pass.
        (
            "foreach r 1k 2k 3k\nalterparam rr = $r\nreset\nop\nend\n",
            &[0.5, 2.0 / 3.0, 0.75],
        ),
        // A counter the script sets before its first analysis.
        (
            "let k = 1\nwhile k le 3\nlet rv = k * 1k\nalterparam rr = $&rv\nreset\nop\n\
             print v(out)\nlet k = k + 1\nend\n",
            &[0.5, 2.0 / 3.0, 0.75],
        ),
        // An `if` in a `repeat`: 3k, 9k, 3k.
        (
            "let k = 0\nrepeat 3\nlet k = k + 1\nif k = 2\nalterparam rr = 9k\nelse\n\
             alterparam rr = 3k\nend\nreset\nop\nend\n",
            &[0.75, 0.9, 0.75],
        ),
        // Nested: the inner words again for each outer word.
        (
            "foreach a 1 2\nforeach b 1k 3k\nlet prod = $a * $b\nalterparam rr = $&prod\n\
             reset\nop\nend\nend\n",
            &[0.5, 0.75, 2.0 / 3.0, 6.0 / 7.0],
        ),
        // A `dowhile` runs its body once; a `while` that is false, never.
        (
            "let k = 5\ndowhile k < 3\nalterparam rr = 3k\nreset\nop\nlet k = k + 1\nend\n\
             while k < 3\nalterparam rr = 9k\nreset\nop\nend\n",
            &[0.75],
        ),
        // A `$&` value is its six digits: rr = 1.23456.
        (
            "let s = 1.2345649\nalterparam rr = $&s\nreset\nop\n",
            &[1.23456 / 1001.23456],
        ),
    ];
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    for (script, want) in cases {
        let deck = format!("{SCRIPTED}{script}.endc\n.end\n");
        let own = outs(Dialect::Ngspice, &deck);
        assert_eq!(own.len(), want.len(), "the source: {own:?}\n{deck}");
        for target in [Dialect::Ngspice, Dialect::Xyce] {
            let text = oracle::write(&deck, Dialect::Ngspice, target);
            let got = outs(target, &text);
            assert_eq!(got.len(), want.len(), "{target:?}: {got:?}\n{text}");
            for (k, ((g, o), w)) in got.iter().zip(&own).zip(want).enumerate() {
                assert!(
                    tol.within(*g, *o) && tol.within(*g, *w),
                    "{target:?} run {k}: {got:?}, the source {own:?}, want {want:?}\n{text}"
                );
            }
        }
    }
    // The script written from the plan: each pass's value, `reset`, its
    // run; the loop's own lines are gone.
    let deck = format!("{SCRIPTED}{}.endc\n.end\n", cases[0].0);
    let text = oracle::write(&deck, Dialect::Ngspice, Dialect::Ngspice);
    let script: Vec<&str> = text.lines().skip_while(|l| *l != ".control").collect();
    assert_eq!(
        script,
        [
            ".control",
            "alterparam rr=1000",
            "reset",
            "op",
            "alterparam rr=2000",
            "reset",
            "op",
            "alterparam rr=3000",
            "reset",
            "op",
            ".endc",
            ".end",
        ],
        "{text}"
    );
    // A line that changes no run is noted once however many passes ran it
    // (W0025): the first `let`, the loop's two, its `print`.
    let deck = format!("{SCRIPTED}{}.endc\n.end\n", cases[1].0);
    let w = oracle::written(&deck, Dialect::Ngspice, Dialect::Xyce);
    let notes = oracle::notes(&w);
    let left_out = notes.iter().filter(|c| **c == Code::W0025).count();
    assert_eq!(left_out, 4, "{notes:?}");
}

/// ZERO-LEDGER P5-088: a script the reader cannot follow refuses the deck
/// (W0160), each beside the neighbour it does follow, and a script that
/// runs more lines than are unrolled refuses by its own code (W0161).
/// Oracle: ngspice-45, where each refused line makes what runs depend on
/// more than the script's own constants (RESULTS.txt §2): a `set`
/// variable, a `foreach` over `temp` (ngspice reads it as its option), a
/// `let` first set after an analysis (gone with the next plot), a `repeat`
/// count that is no plain number, `break`, a variable in an analysis
/// command, a `set` of the loop's own variable before its use, a node
/// named as the counter (its plot's vector hides the counter, and the
/// loop never ends).
#[test]
fn p5_088_a_script_that_cannot_be_followed_is_refused() {
    // The refused script, and the neighbour that is followed.
    const PAIRS: [(&str, &str); 7] = [
        (
            "set vals = ( 1k 4k )\nforeach r $vals\nalterparam rr = $r\nreset\nop\nend\n",
            "foreach r 1k 4k\nalterparam rr = $r\nreset\nop\nend\n",
        ),
        (
            "foreach temp 3k 9k\nalterparam rr = $temp\nreset\nop\nend\n",
            "foreach t 3k 9k\nalterparam rr = $t\nreset\nop\nend\n",
        ),
        (
            "op\nlet j = 0\nwhile j < 2\nalterparam rr = 3k\nreset\nop\nlet j = j + 1\nend\n",
            "let j = 0\nwhile j < 2\nalterparam rr = 3k\nreset\nop\nlet j = j + 1\nend\n",
        ),
        (
            "repeat 2.7\nalterparam rr = 3k\nreset\nop\nend\n",
            "repeat 2\nalterparam rr = 3k\nreset\nop\nend\n",
        ),
        (
            "repeat 2\nalterparam rr = 3k\nreset\nop\nbreak\nend\n",
            "repeat 2\nalterparam rr = 3k\nreset\nop\nprint v(out)\nend\n",
        ),
        (
            "foreach d 1 2\ndc V1 $d $d 1\nend\n",
            "foreach d 3k 9k\nalterparam rr = $d\nreset\ndc V1 0 1 1\nend\n",
        ),
        (
            "foreach noaskquit 3k 9k\nset noaskquit\nalterparam rr = $noaskquit\nreset\nop\nend\n",
            "foreach noaskquit 3k 9k\nalterparam rr = $noaskquit\nreset\nop\nset noaskquit\nend\n",
        ),
    ];
    for (refused, followed) in PAIRS {
        let deck = format!("{SCRIPTED}{refused}.endc\n.end\n");
        for target in [Dialect::Ngspice, Dialect::Xyce] {
            let codes = oracle::refusal(&deck, Dialect::Ngspice, target);
            assert!(
                codes.contains(&Code::W0160),
                "{target:?}: {codes:?}\n{deck}"
            );
        }
        let deck = format!("{SCRIPTED}{followed}.endc\n.end\n");
        let text = oracle::write(&deck, Dialect::Ngspice, Dialect::Ngspice);
        assert!(text.contains("\nreset\n"), "{text}");
    }
    // A node of the deck named as the counter.
    let counter = "let k = 1\nwhile k le 2\nalterparam rr = 3k\nreset\nop\nlet k = k + 1\nend\n";
    let hidden = format!("{}{counter}.endc\n.end\n", SCRIPTED.replace("out", "k"));
    let codes = oracle::refusal(&hidden, Dialect::Ngspice, Dialect::Ngspice);
    assert!(codes.contains(&Code::W0160), "{codes:?}\n{hidden}");
    let seen = format!("{SCRIPTED}{counter}.endc\n.end\n");
    let text = oracle::write(&seen, Dialect::Ngspice, Dialect::Ngspice);
    assert!(text.contains("\nreset\n"), "{text}");
    // More lines than are unrolled: a long sweep, a loop that never ends.
    for script in [
        "repeat 9000\nalterparam rr = 3k\nreset\nop\nend\n",
        "let k = 0\nwhile k < 1\nalterparam rr = 3k\nreset\nop\nend\n",
    ] {
        let deck = format!("{SCRIPTED}{script}.endc\n.end\n");
        let codes = oracle::refusal(&deck, Dialect::Ngspice, Dialect::Xyce);
        assert!(codes.contains(&Code::W0161), "{script}: {codes:?}");
        assert!(!codes.contains(&Code::W0160), "{script}: {codes:?}");
    }
    assert_eq!(info(Code::W0161).kind, Kind::Refusal);
}
