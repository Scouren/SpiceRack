//! A deck that changes its circuit between analyses (VACASK `alter` in a
//! `control` block) written for another simulator: the oracle is each
//! circuit state's closed form, met by real VACASK on the source deck and
//! by real ngspice on the deck written from it; a target whose deck is one
//! circuit refuses by its code. A deck that steps (`.step`) is the same
//! script's `foreach` loops on ngspice: the oracle is the closed form at
//! the points Xyce and LTspice stepped through. An ngspice `.control`
//! script's `alterparam` and `reset` is the same plan: the oracle is
//! ngspice running the source script itself. On Xyce, LTspice and HSPICE
//! the states are one table the deck runs over (`.DATA`, an index step,
//! `.ALTER` blocks): the oracle is each state's closed form, met by Xyce
//! and by ESPice reading the HSPICE text.
//!
//! Unsupported fails, never skips: without `vacask`, `ngspice`, `Xyce`
//! and `espice` on PATH every run fails; run the tests in
//! `nix develop .#testing`.
//!
//! Cites: DESIGN-control-epochs §5 (O1, O2), ZERO-LEDGER P5-001; VACASK
//! 36be560 `docs/cmd-alter.md`; ngspice manual §13.5.5 (`alterparam`),
//! §13.5.65 (`reset`), §13.6.4 (`foreach`); Xyce RG 7.10 §2.1.36 (`.STEP`).
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use spicerack::backend::Dialect;
use spicerack::syntax::diag::{Code, Kind, info};

/// A VACASK deck up to its `control` line: i1 = 1 mA into the resistance
/// the `@if` on the top parameters selects (1k; 2k; 2k‖2k; 4k).
const EPOCHS_HEAD: &str = "t\nground 0\nload \"spice/resistor.osdi\"\nmodel resistor sp_resistor\n\
    model isource isource\nparameters a=1 b=0\ni1 (0 2) isource dc=1m\n@if a==1\n  r1 (2 0) resistor r=1k\n\
    @elseif a==2\n  r1 (2 0) resistor r=2k\n  @if b==1\n  r2 (2 0) resistor r=2k\n  @end\n@else\n  \
    r1 (2 0) resistor r=4k\n@end\ncontrol\n";

/// A divider every `.step` test steps, up to the end of R2's card: v(out)
/// = V1 · R2 / (1k + R2), R2 = rr (times 1 + 0.01 (T − 27) with `tc1=0.01`).
const STEPPED: &str = "* steps\n.param rr=1k\nV1 in 0 1\nR1 in out 1k\nR2 out 0 {rr}";

/// An ngspice divider up to its `.control` line: v(out) = rr / (1k + rr).
const SCRIPTED: &str = "* script\n.param rr=1k\nV1 in 0 1\nR1 in out 1k\nR2 out 0 {rr}\n.control\n";

// ---- private ----

/// ZERO-LEDGER P5-001 step 2 (DESIGN-control-epochs O1, O2). Oracle: the
/// closed form of each circuit state, v(2) = i1 · R, which VACASK 36be560
/// and ngspice-45 both gave by hand (research/impl/evidence/P5-001-s2/).
/// Each state is read from the deck cut after its analysis, as the last
/// plot of that run (the epoch split: a run of the multi-state deck
/// equals that state's own run). Tolerance: the two
/// simulators' linear solves of a two-element circuit, RELTOL class.
#[test]
fn p5_001_s2_a_vacask_top_parameter_alter_runs_as_an_ngspice_control_script() {
    const STEPS: [(&str, f64); 4] = [
        ("  analysis op1 op\n", 1.0),
        (
            "  alter instance(\"__topinst__\") a=2\n  analysis op2 op\n",
            2.0,
        ),
        (
            "  alter instance(\"__topinst__\") a=2 b=1\n  analysis op3 op\n",
            1.0,
        ),
        (
            "  alter instance(\"__topinst__\") a=4\n  analysis op4 op\n",
            4.0,
        ),
    ];
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    let mut body = String::new();
    for (step, want) in STEPS {
        body.push_str(step);
        let deck = format!("{EPOCHS_HEAD}{body}endc\n");
        let text = oracle::write(&deck, Dialect::Vacask, Dialect::Ngspice);
        let plots = oracle::run(Dialect::Ngspice, &text);
        let got = oracle::value(&plots[plots.len() - 1..], "2", oracle::At::First);
        assert!(
            tol.within(got, want),
            "ngspice v(2) = {got}, want {want}\n{text}"
        );
        let own = oracle::run(Dialect::Vacask, &deck);
        let got = oracle::value(&own[own.len() - 1..], "2", oracle::At::First);
        assert!(
            tol.within(got, want),
            "VACASK v(2) = {got}, want {want}\n{deck}"
        );
    }
    // The script itself ([NG] §13.5.5: `alterparam`, then `reset`): each
    // state's delta before its first run.
    let deck = format!("{EPOCHS_HEAD}{body}endc\n");
    let text = oracle::write(&deck, Dialect::Vacask, Dialect::Ngspice);
    let script: Vec<&str> = text.lines().skip_while(|l| *l != ".control").collect();
    assert_eq!(
        script,
        [
            ".control",
            "op",
            "alterparam a=2",
            "reset",
            "op",
            "alterparam a=2",
            "alterparam b=1",
            "reset",
            "op",
            "alterparam a=4",
            "reset",
            "op",
            ".endc",
            ".end",
        ],
        "{text}"
    );
    // VACASK keeps its own statements, each once.
    let own = oracle::write(&deck, Dialect::Vacask, Dialect::Vacask);
    assert_eq!(own.matches("alter instance").count(), 3, "{own}");
}

/// ZERO-LEDGER P5-001 steps 2 and 5: a target whose deck is one circuit
/// refuses the second state (W0130), not as foreign text (W0001).
/// Oracle: each target's own rule. ESPice 0.3.0 ignores `.control`
/// commands (run, evidence/P5-001-s2/espice.log); Xyce RG `.STEP`,
/// LTspice help `.step` and HSPICE UG `.ALTER` (pp.107-108) run every
/// analysis of the deck at every state, and here the second state runs
/// two analyses where the first runs one.
#[test]
fn p5_001_s2_a_second_circuit_state_is_refused_where_a_deck_is_one_circuit() {
    let deck = "t\nground 0\nload \"spice/resistor.osdi\"\nmodel resistor sp_resistor\n\
        model isource isource\nparameters a=1\ni1 (0 2) isource dc=1m\nr1 (2 0) resistor r=a*1k\n\
        control\n  analysis op1 op\n  alter instance(\"__topinst__\") a=2\n  analysis op2 op\n  \
        analysis op3 op\nendc\n";
    for target in [
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Ltspice,
    ] {
        let codes = oracle::refusal(deck, Dialect::Vacask, target);
        assert!(codes.contains(&Code::W0130), "{target:?}: {codes:?}");
        assert!(!codes.contains(&Code::W0001), "{target:?}: {codes:?}");
    }
    assert_eq!(info(Code::W0130).kind, Kind::Refusal);
    // Spectre keeps the statement as text until its `alter` is written
    // (ZERO-LEDGER P5-050): unchanged by this step.
    let codes = oracle::refusal(deck, Dialect::Vacask, Dialect::Spectre);
    assert_eq!(codes, [Code::W0001]);
}

/// ZERO-LEDGER P5-001 step 3 (DESIGN-control-epochs §6 commits 3 and 6).
/// Oracle: the divider's closed form at the last point of each `.step`,
/// and for the three nested steps at every point in order (the harness
/// reads a plot a pass), the points being those Xyce 7.10 and LTspice
/// 26.1.1 stepped through by hand
/// (research/impl/evidence/P5-001-s3/RESULTS.txt): Xyce ends at its grid's
/// last point, LTspice adds the stop value. Xyce 7.10 itself crashes on
/// two `.step` lines beside `.op`, so only ngspice runs here. Tolerance: a
/// linear solve of three elements, RELTOL class.
#[test]
fn p5_001_s3_a_step_runs_as_ngspice_foreach_loops() {
    // The source, the rest of the deck from R2's card on, v(out) at the
    // last point.
    const CASES: [(Dialect, &str, f64); 6] = [
        (Dialect::Xyce, "\n.step rr 1k 3k 1k\n", 0.75),
        // The grid 1k, 5k, 9k: Xyce ends at 9k, LTspice adds 10k.
        (Dialect::Xyce, "\n.step rr 1k 10k 4k\n", 0.9),
        (
            Dialect::Ltspice,
            "\n.step param rr 1k 10k 4k\n",
            10.0 / 11.0,
        ),
        // Two points a decade: 31.62k is Xyce's last (its v(out), x4.prn),
        // 50k LTspice's.
        (Dialect::Xyce, "\n.step dec rr 1k 50k 2\n", 0.969346570),
        (
            Dialect::Ltspice,
            "\n.step dec param rr 1k 50k 2\n",
            50.0 / 51.0,
        ),
        // A parameter, an element and the temperature: rr = 2k at 127 °C
        // is 4k, V1 = 2.
        (
            Dialect::Xyce,
            " tc1=0.01\n.step rr list 1k 2k\n.step V1 list 1 2\n.step temp list 27 127\n",
            1.6,
        ),
    ];
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    for (from, rest, want) in CASES {
        let deck = format!("{STEPPED}{rest}.op\n.end\n");
        let text = oracle::write(&deck, from, Dialect::Ngspice);
        let plots = oracle::run(Dialect::Ngspice, &text);
        let got = oracle::value(&plots[plots.len() - 1..], "out", oracle::At::First);
        assert!(
            tol.within(got, want),
            "{from:?}: v(out) = {got}, want {want}\n{text}"
        );
    }
    // The loops themselves: the first `.step` innermost (Xyce RG `.STEP`;
    // the eight points came in Xyce's order), the parameter before
    // `reset`, which would undo the temperature and the `alter`; after
    // it each loop's line in loop order (ngspice-45 gave the same eight
    // values with the two in either order: RESULTS.txt §4).
    let (from, rest, _) = CASES[5];
    let deck = format!("{STEPPED}{rest}.op\n.end\n");
    let text = oracle::write(&deck, from, Dialect::Ngspice);
    let script: Vec<&str> = text.lines().skip_while(|l| *l != ".control").collect();
    assert_eq!(
        script,
        [
            ".control",
            "foreach s3 27 127",
            "foreach s2 1 2",
            "foreach s1 1000 2000",
            "alterparam rr=$s1",
            "reset",
            "alter V1=$s2",
            "option temp=$s3",
            "op",
            "end",
            "end",
            "end",
            ".endc",
            ".end",
        ],
        "{text}"
    );
    // Every pass is a plot, in Xyce's order (RESULTS.txt §4): rr the
    // innermost, then V1, then the temperature.
    let plots = oracle::run(Dialect::Ngspice, &text);
    let want = [
        0.5,
        2.0 / 3.0,
        1.0,
        4.0 / 3.0,
        2.0 / 3.0,
        0.8,
        4.0 / 3.0,
        1.6,
    ];
    assert_eq!(plots.len(), want.len(), "a plot a pass\n{text}");
    for (k, w) in want.iter().enumerate() {
        let got = oracle::value(&plots[k..k + 1], "out", oracle::At::First);
        assert!(
            tol.within(got, *w),
            "pass {k}: v(out) = {got}, want {w}\n{text}"
        );
    }
    // A run with no simulator named never lands a stepped deck on
    // ngspice, whose run reads one pass back: `select`'s last resort
    // leaves a deck that steps out (it takes any other deck ngspice
    // writes).
    {
        use spicerack::backend::{BackendError, Kind, Sim, feature, select};
        let ngspice = |s: Sim| s == Sim::NGSPICE;
        let plain = select(&[Kind::Op], None, 0, ngspice);
        assert_eq!(plain.ok(), Some(Sim::NGSPICE));
        let stepped = select(&[Kind::Op], None, feature::STEP_PARAMS, ngspice);
        assert!(
            matches!(stepped, Err(BackendError::UnsupportedFeature { .. })),
            "{stepped:?}"
        );
    }
    // What the script has no command for stays refused (W0016): a device
    // parameter, a name the deck's top does not declare; and every step on
    // ESPice, which runs no `.control` command.
    for step in [".step R1:R 1k 2k 1k", ".step nosuch 1 2 1"] {
        let deck = format!("{STEPPED}\n{step}\n.op\n.end\n");
        let codes = oracle::refusal(&deck, Dialect::Xyce, Dialect::Ngspice);
        assert!(codes.contains(&Code::W0016), "{step}: {codes:?}");
    }
    let deck = format!("{STEPPED}\n.step rr 1k 3k 1k\n.op\n.end\n");
    let codes = oracle::refusal(&deck, Dialect::Xyce, Dialect::Espice);
    assert!(codes.contains(&Code::W0016), "{codes:?}");
}

/// ZERO-LEDGER P5-001 step 4 (DESIGN-control-epochs §6 commit 4). Oracle:
/// ngspice-45 running the source script itself (the deck written from
/// what was read equals the script, same simulator), and the closed form
/// v(out) = rr / (1k + rr); both by hand in
/// research/impl/evidence/P5-001-s4/RESULTS.txt §1, §4. Each case ends on
/// the run under test, read as the run's last plot. Tolerance: one
/// simulator's linear solve of a divider, twice.
#[test]
fn p5_001_s4_an_ngspice_alterparam_takes_effect_at_its_reset() {
    // The script, and v(out) of its last run.
    const CASES: [(&str, f64); 4] = [
        ("op\nalterparam rr = 2k\nreset\nop\n", 2.0 / 3.0),
        // No `reset` yet: the run is on the netlist as written
        // (ngspice manual §13.5.5).
        ("op\nalterparam rr = 2k\nop\n", 0.5),
        // Before any run; the last of two values wins.
        ("alterparam rr=3k\nalterparam rr=4k\nreset\nop\n", 0.8),
        (
            "op\nalterparam rr=2k\nreset\nop\nalterparam rr=3k\nreset\nop\nprint v(out)\n",
            0.75,
        ),
    ];
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    let last = |text: &str| {
        let plots = oracle::run(Dialect::Ngspice, text);
        oracle::value(&plots[plots.len() - 1..], "out", oracle::At::First)
    };
    for (script, want) in CASES {
        let deck = format!("{SCRIPTED}{script}.endc\n.end\n");
        let text = oracle::write(&deck, Dialect::Ngspice, Dialect::Ngspice);
        let (got, own) = (last(&text), last(&deck));
        assert!(
            tol.within(got, own),
            "written {got}, the source script {own}\n{text}"
        );
        assert!(tol.within(got, want), "v(out) = {got}, want {want}\n{text}");
    }
    // The script written from the plan (`spice/epochs.rs`), and the one
    // line left out: `print` changes no run (W0025).
    let deck = format!("{SCRIPTED}{}.endc\n.end\n", CASES[3].0);
    let w = oracle::written(&deck, Dialect::Ngspice, Dialect::Ngspice);
    let script: Vec<&str> = w.text.lines().skip_while(|l| *l != ".control").collect();
    assert_eq!(
        script,
        [
            ".control",
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
        "{}",
        w.text
    );
    let notes = oracle::notes(&w);
    let left_out = notes.iter().filter(|c| **c == Code::W0025).count();
    assert_eq!(left_out, 1, "{notes:?}");
    // A dot card after the block runs on the state the script left
    // (ngspice -b: RESULTS.txt §3, e7), which is the plan's order.
    let deck = format!("{SCRIPTED}alterparam rr=2k\nreset\n.endc\n.op\n.end\n");
    let text = oracle::write(&deck, Dialect::Ngspice, Dialect::Ngspice);
    let got = last(&text);
    assert!(tol.within(got, 2.0 / 3.0), "v(out) = {got}\n{text}");
}

/// ZERO-LEDGER P5-001 step 4 (and P3-018's rule): a script line that
/// changes the circuit and is not read refuses the deck on every target,
/// ngspice too (its writer writes the script from what was read); a
/// state that is read is refused where a deck is one circuit. Oracle:
/// ngspice-45, where each of these lines changed the next run's v(out)
/// (research/impl/evidence/P5-001-s4/RESULTS.txt §1 to §3); before this
/// step each was left out with a note and the deck written.
#[test]
fn p5_001_s4_an_unread_script_line_that_changes_the_circuit_is_refused() {
    const LINES: [&str; 8] = [
        "alter r2 = 2k",
        "altermod r2 r = 2k",
        "option temp=127",
        "set temp=127",
        "alterparam rr = 2*1k",
        "alterparam half f = 3k",
        "foreach v 1k 2k",
        "source other.sp",
    ];
    for line in LINES {
        let deck = format!("{SCRIPTED}op\n{line}\nop\n.endc\n.end\n");
        for target in [Dialect::Ngspice, Dialect::Xyce, Dialect::Vacask] {
            let codes = oracle::refusal(&deck, Dialect::Ngspice, target);
            assert!(
                codes.contains(&Code::W0160),
                "{line} on {target:?}: {codes:?}"
            );
        }
    }
    assert_eq!(info(Code::W0160).kind, Kind::Refusal);
    // A parameter change beside the deck's dot-card analyses: ngspice -b
    // runs those after the script and at each `run` (§3: e5, e6).
    let before = SCRIPTED.replace(".control\n", ".op\n.control\n");
    for deck in [
        format!("{before}alterparam rr=2k\nreset\n.endc\n.end\n"),
        format!("{SCRIPTED}run\nalterparam rr=2k\nreset\nrun\n.endc\n.op\n.end\n"),
    ] {
        let codes = oracle::refusal(&deck, Dialect::Ngspice, Dialect::Ngspice);
        assert!(codes.contains(&Code::W0160), "{codes:?}\n{deck}");
    }
    // The state that is read, running two analyses where the first runs
    // one: ESPice's cell refuses it and no table of states holds it on
    // HSPICE, Xyce or LTspice (step 5); the named targets keep the line
    // as text until their `alter` is written from the plan (ZERO-LEDGER
    // P5-050).
    let deck = format!("{SCRIPTED}op\nalterparam rr = 2k\nreset\nop\nop\n.endc\n.end\n");
    for (targets, code) in [
        (
            &[
                Dialect::Espice,
                Dialect::Hspice,
                Dialect::Xyce,
                Dialect::Ltspice,
            ][..],
            Code::W0130,
        ),
        (&[Dialect::Spectre, Dialect::Vacask][..], Code::W0001),
    ] {
        for &target in targets {
            let codes = oracle::refusal(&deck, Dialect::Ngspice, target);
            assert!(codes.contains(&code), "{target:?}: {codes:?}");
            assert!(!codes.contains(&Code::W0160), "{target:?}: {codes:?}");
        }
    }
    // The legal neighbour: lines that change no run are left out with a
    // note each, and the deck is written.
    let deck =
        format!("{SCRIPTED}set filetype=ascii\nop\nlet x = 1\nprint v(out)\nquit\n.endc\n.end\n");
    let w = oracle::written(&deck, Dialect::Ngspice, Dialect::Xyce);
    let notes = oracle::notes(&w);
    let left_out = notes.iter().filter(|c| **c == Code::W0025).count();
    assert_eq!(left_out, 4, "{notes:?}");
}

/// ZERO-LEDGER P5-001 step 5 (DESIGN-control-epochs §3 `StepStates`, §7).
/// Oracle: the divider's closed form at each circuit state, v(out) = R2 /
/// (1k + R2), which ngspice-45 gave on the source scripts and Xyce 7.10
/// (one plot a `.DATA` row), ESPice 0.3.0 reading HSPICE (one plot an
/// `.ALTER` block) and LTspice 26.1.1 gave on the written forms, each by
/// hand (research/impl/evidence/P5-001-s5/RESULTS.txt). LTspice is not in
/// `.#testing`: its lines are those of the deck that was run (§3).
/// Tolerance: a linear solve of three elements, RELTOL class.
#[test]
fn p5_001_s5_circuit_states_are_one_table_the_deck_runs_over() {
    // rr = 1k, 2k, 3k, 2k: out of order, a value repeated.
    const ONE: &str = "op\nalterparam rr=2k\nreset\nop\nalterparam rr=3k\nreset\nop\n\
        alterparam rr=2k\nreset\nop\n";
    // (a, b) = (1, 0), (2, 0), (2, 1), (1, 0), and R2 = a · 1k + b · 2k.
    const TWO: &str = "op\nalterparam a=2\nreset\nop\nalterparam b=1\nreset\nop\n\
        alterparam a=1\nalterparam b=0\nreset\nop\n";
    let two = SCRIPTED
        .replace(".param rr=1k", ".param a=1\n.param b=0")
        .replace("{rr}", "{a*1k+b*2k}");
    let cases = [
        (
            format!("{SCRIPTED}{ONE}.endc\n.end\n"),
            [0.5, 2.0 / 3.0, 0.75, 2.0 / 3.0],
        ),
        (
            format!("{two}{TWO}.endc\n.end\n"),
            [0.5, 2.0 / 3.0, 0.8, 0.5],
        ),
    ];
    let tol = oracle::Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    for (deck, want) in &cases {
        for target in [Dialect::Xyce, Dialect::Hspice] {
            let text = oracle::write(deck, Dialect::Ngspice, target);
            let plots = oracle::run(target, &text);
            assert_eq!(
                plots.len(),
                want.len(),
                "{target:?}: a plot a state\n{text}"
            );
            for (k, w) in want.iter().enumerate() {
                let got = oracle::value(&plots[k..k + 1], "out", oracle::At::First);
                assert!(
                    tol.within(got, *w),
                    "{target:?} state {k}: v(out) = {got}, want {w}\n{text}"
                );
            }
        }
    }
    // The table itself: Xyce's rows in state order, HSPICE's blocks each
    // setting every state parameter, LTspice's index and its tables.
    let text = oracle::write(&cases[0].0, Dialect::Ngspice, Dialect::Xyce);
    let table = "\n.param rr=1000\n";
    assert!(text.contains(table), "{text}");
    let table = "\n.data spicerack_states\n+ rr\n+ 1000\n+ 2000\n+ 3000\n+ 2000\n.enddata\n\
        .step data=spicerack_states\n";
    assert!(text.contains(table), "{text}");
    let text = oracle::write(&cases[1].0, Dialect::Ngspice, Dialect::Hspice);
    let blocks = "\n.alter\n.param a=2 b=0\n.alter\n.param a=2 b=1\n.alter\n.param a=1 b=0\n.end";
    assert!(text.contains(blocks), "{text}");
    let text = oracle::write(&cases[1].0, Dialect::Ngspice, Dialect::Ltspice);
    for line in [
        ".param a=table(spicerack_state,0,1,1,2,2,2,3,1)",
        ".param b=table(spicerack_state,0,0,1,0,2,1,3,0)",
        ".step param spicerack_state 0 3 1",
    ] {
        assert!(text.lines().any(|l| l == line), "no `{line}`\n{text}");
    }
    // An alter before the first run is the one state's own value: no step.
    let deck = format!("{SCRIPTED}alterparam rr=3k\nreset\nop\n.endc\n.end\n");
    let text = oracle::write(&deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(text.contains("\n.param rr=3000\n"), "{text}");
    assert!(!text.contains(".step") && !text.contains(".data"), "{text}");
    let got = oracle::op(Dialect::Xyce, &text, "out");
    assert!(tol.within(got, 0.75), "v(out) = {got}\n{text}");
    // What no table holds is refused where the states are a table
    // (W0130): a parameter whose card is an expression, and on HSPICE a
    // conditional block beside the blocks (ESPice keeps its first branch:
    // RESULTS.txt §2). The legal neighbour: the same deck with one state.
    let expr = cases[0].0.replace(".param rr=1k", ".param rr={2*500}");
    for target in [Dialect::Xyce, Dialect::Hspice, Dialect::Ltspice] {
        let codes = oracle::refusal(&expr, Dialect::Ngspice, target);
        assert!(codes.contains(&Code::W0130), "{target:?}: {codes:?}");
    }
    let branches = ".if (rr > 1500)\nR2 out 0 {rr}\n.else\nR2 out 0 1k\n.endif\n";
    let cond = cases[0].0.replace("R2 out 0 {rr}\n", branches);
    let codes = oracle::refusal(&cond, Dialect::Ngspice, Dialect::Hspice);
    assert!(codes.contains(&Code::W0130), "{codes:?}");
    let one = cond.replace(ONE, "op\n");
    let one = oracle::write(&one, Dialect::Ngspice, Dialect::Hspice);
    assert!(one.contains(".if") && !one.contains(".alter"), "{one}");
    // ESPice runs one circuit a deck: its cell refuses.
    let codes = oracle::refusal(&cases[0].0, Dialect::Ngspice, Dialect::Espice);
    assert!(codes.contains(&Code::W0130), "{codes:?}");
}
