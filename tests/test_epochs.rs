//! A deck that changes its circuit between analyses (VACASK `alter` in a
//! `control` block) written for another simulator: the oracle is each
//! circuit state's closed form, met by real VACASK on the source deck and
//! by real ngspice on the deck written from it; a target whose deck is one
//! circuit refuses by its code.
//!
//! Unsupported fails, never skips: without `vacask` and `ngspice` on PATH
//! every run fails; run the tests in `nix develop .#testing`.
//!
//! Cites: DESIGN-control-epochs §5 (O1, O2), ZERO-LEDGER P5-001; VACASK
//! 36be560 `docs/cmd-alter.md`; ngspice manual §13.5.5 (`alterparam`),
//! §13.5.65 (`reset`).
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
