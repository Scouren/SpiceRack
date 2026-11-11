//! A deck that changes its circuit between analyses (VACASK `alter` in a
//! `control` block) written for another simulator: the oracle is each
//! circuit state's closed form, met by real VACASK on the source deck and
//! by real ngspice on the deck written from it; a target whose deck is one
//! circuit refuses by its code. A deck that steps (`.step`) is the same
//! script's `foreach` loops on ngspice: the oracle is the closed form at
//! the points Xyce and LTspice stepped through.
//!
//! Unsupported fails, never skips: without `vacask` and `ngspice` on PATH
//! every run fails; run the tests in `nix develop .#testing`.
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

// ---- private ----

/// ZERO-LEDGER P5-001 step 2 (DESIGN-control-epochs O1, O2). Oracle: the
/// closed form of each circuit state, v(2) = i1 · R, which VACASK 36be560
/// and ngspice-45 both gave by hand (research/impl/evidence/P5-001-s2/).
/// The harness keeps a `.control` deck's last plot only, so each state is
/// read from the deck cut after its analysis (the epoch split: a run of
/// the multi-state deck equals that state's own run). Tolerance: the two
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

/// ZERO-LEDGER P5-001 step 2: a target whose deck is one circuit refuses
/// the second state by its cell (W0130), not as foreign text (W0001).
/// Oracle: each target's own rule. ESPice 0.3.0 ignores `.control`
/// commands (run, evidence/P5-001-s2/espice.log); Xyce RG `.STEP` and
/// LTspice help `.step` step every analysis; HSPICE `.ALTER` is not
/// written yet (step 5).
#[test]
fn p5_001_s2_a_second_circuit_state_is_refused_where_a_deck_is_one_circuit() {
    let deck = "t\nground 0\nload \"spice/resistor.osdi\"\nmodel resistor sp_resistor\n\
        model isource isource\nparameters a=1\ni1 (0 2) isource dc=1m\nr1 (2 0) resistor r=a*1k\n\
        control\n  analysis op1 op\n  alter instance(\"__topinst__\") a=2\n  analysis op2 op\nendc\n";
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
/// Oracle: the divider's closed form at the last point of each `.step`
/// (the harness keeps a `.control` deck's last plot), the points being
/// those Xyce 7.10 and LTspice 26.1.1 stepped through by hand
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
    // `reset`, which would undo the temperature and the `alter`.
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
            "option temp=$s3",
            "alter V1=$s2",
            "op",
            "end",
            "end",
            "end",
            ".endc",
            ".end",
        ],
        "{text}"
    );
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
