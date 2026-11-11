//! The reader (`render/read/`): what a deck's statements become, by the
//! seams a reader row edits (`elements::Rd::component`, `control::Rd::control`,
//! `cards::Rd::dialect_card`, `refused::Rd::kept_construct`). Each reader row of
//! `research/ZERO-LEDGER.md` whose oracle is a code or a spelling from a
//! manual clause adds its red-first test here, next to these.
//!
//! The tests below are characterization tests (`legacy-code`): they pin what
//! the reader does at the C3 cleanup. Their oracle is the golden gate's
//! record (`baseline-daa0ad0-fx14`, TESTING.md "Golden gate"), which this
//! output matched byte for byte when they were written.
//!
//! Cites: per test.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{refusal, write};
use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;

// ---- private ----

/// The codes of the notes on `text` (read as `from`) written for `to`;
/// panics on a refusal.
fn notes(text: &str, from: Dialect, to: Dialect) -> Vec<Code> {
    oracle::notes(&oracle::written(text, from, to))
}

#[test]
fn each_linear_element_family_reads_as_a_typed_card() {
    // One card per `component` arm the cleanup split out: K (`coupling`),
    // T (`tline`), E/G/F/H (`controlled`). A typed card is spelled by the
    // Spectre writer as its master; a raw one would be refused. Checks: 6.
    let deck = "t\nV1 a 0 1\nL1 a 0 1u\nL2 b 0 1u\nK1 L1 L2 0.5\n\
        T1 a 0 c 0 Z0=50 TD=1n\nR1 c 0 50\n\
        E1 e 0 a 0 2\nG1 g 0 a 0 1m\nF1 f 0 V1 3\nH1 h 0 V1 4\n\
        R2 e 0 1k\nR3 g 0 1k\nR4 f 0 1k\nR5 h 0 1k\n.op\n.end\n";
    let out = write(deck, Dialect::Ngspice, Dialect::Spectre);
    for master in ["mutual_inductor", "tline", "vcvs", "vccs", "cccs", "ccvs"] {
        assert!(out.contains(master), "{master} in\n{out}");
    }
}

#[test]
fn a_control_blocks_analysis_reads_and_its_script_is_noted() {
    // ngspice manual ch.17: `op` in `.control` runs the analysis; `let` is
    // the script's (W0025, `control::ControlLine::Skipped`). Checks: 2.
    let deck = "t\nR1 a 0 1k\n.control\nlet x=1\nop\n.endc\n.end\n";
    assert!(notes(deck, Dialect::Ngspice, Dialect::Ngspice).contains(&Code::W0025));
    let out = write(deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(out.to_ascii_lowercase().contains(".op"), "{out}");
}

#[test]
fn an_ltspice_backanno_is_left_out_with_a_note() {
    // LTspice help: `.backanno` changes no number (`cards::Rd::dialect_card`,
    // the LTspice arm). Checks: 2.
    let deck = "* t\nR1 a 0 1k\n.backanno\n.op\n.end\n";
    assert!(notes(deck, Dialect::Ltspice, Dialect::Ngspice).contains(&Code::W0036));
    let out = write(deck, Dialect::Ltspice, Dialect::Ngspice);
    assert!(!out.to_ascii_lowercase().contains("backanno"), "{out}");
}

#[test]
fn a_control_pre_osdi_is_a_load_ltspice_refuses() {
    // ngspice manual, `pre_osdi file` (`control::ControlLine::PreOsdi`): a
    // load, which LTspice has no form for (W0004, as the `load` statement
    // in test_spice_write.rs). Checks: 1.
    let deck = "t\nR1 a 0 1k\n.control\npre_osdi m.osdi\n.endc\n.op\n.end\n";
    assert!(refusal(deck, Dialect::Ngspice, Dialect::Ltspice).contains(&Code::W0004));
}

// ZERO-LEDGER P3-028: HSPICE `.OP format time …` reports the operating
// point at each transient time (HSPICE Reference Manual: Commands and
// Control Options E-2010.12 pp.232-233). Oracle: the manual pages; and
// ESPice 0.3.0 `--tokenizer=hspice` on this deck publishes v(out) =
// 0.18119 V at 1 ns and 0.63210 V at 5 ns (1 - exp(-t / 5 ns)), where
// ngspice-45 runs `.op 1n 5n` as `.op`: v(out) = 0 V.
#[test]
fn p3_028_an_hspice_op_at_times_is_kept() {
    let deck = "* t\nv1 in 0 pulse(0 1 0 1p 1p 1 2)\nr1 in out 1k\nc1 out 0 5p\n\
                .tran 0.1n 10n\n.op 1n 5n\n.end\n";
    let hs = Dialect::Hspice;
    oracle::spec_example("HSPICE `.OP format time`", deck, hs, hs, &[".op 1n 5n"]);
    // ngspice manual §11.3.5, Xyce RG `.OP`, LTspice help `.op`, Spectre
    // Reference `dc`, VACASK `op`: none takes a time.
    for to in [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Xyce,
        Dialect::Ltspice,
        Dialect::Spectre,
        Dialect::Vacask,
    ] {
        oracle::spec_refusal("no timed `.op` on the target", deck, hs, to, &[Code::W0015]);
    }
}

// ZERO-LEDGER P3-017: a parameter defined twice. Oracle: each simulator on
// the divider v(b) = 4 V · 1k / (rx + 1k). ngspice-45 keeps the last
// definition (rx = 3k: 1 V), Xyce 7.10 the first (rx = 1k: 2 V), VACASK
// refuses a second one ("Parameter redefinition."): every target is
// written the source's winner only (Xyce_Regression
// `ISSUE_202/first_last.cir` is the corpus deck of the Xyce half).
#[test]
fn p3_017_a_parameter_defined_twice_keeps_its_sources_winner() {
    let deck = "* t\n.param rx=1k\n.param rx=3k\nv1 a 0 4\nr1 a b {rx}\nr2 b 0 1k\n.op\n.end\n";
    // A linear divider: the simulators' answers are exact to rounding.
    let tol = oracle::Tol {
        rel: 1e-9,
        floor: 0.0,
    };
    let (ng, xy, va) = (Dialect::Ngspice, Dialect::Xyce, Dialect::Vacask);
    // 4 V · 1k / (3k + 1k) = 1 V: the last definition.
    oracle::closed_form(ng, deck, &[ng, xy, va], "b", oracle::At::First, 1.0, tol);
    // 4 V · 1k / (1k + 1k) = 2 V: the first definition.
    oracle::closed_form(xy, deck, &[xy, ng, va], "b", oracle::At::First, 2.0, tol);
}

// ZERO-LEDGER P3-106: HSPICE `.option delmax=` is the largest step of the
// deck's transients (HSPICE Reference Manual: Commands and Control Options
// E-2010.12 p.443); ngspice's is `.tran`'s fourth number (manual §11.3.10,
// TMAX), and ngspice-45 reads `.options delmax=` without a word. Oracle:
// ngspice's own count of time points on this deck, 1021 with the 1 µs cap
// and 129 without (Xyce 7.10: 1028 and 168).
#[test]
fn p3_106_hspice_delmax_is_tran_max_step() {
    let deck = "t\nV1 in 0 PULSE(0 1 0 1u 1u 0.5m 1m)\nR1 in out 1k\nC1 out 0 100n\n\
                .option delmax=1u\n.tran 10u 1m\n.end\n";
    let out = write(deck, Dialect::Hspice, Dialect::Ngspice);
    let plots = oracle::run(Dialect::Ngspice, &out);
    let points = plots
        .iter()
        .filter_map(|p| p.real_data.first())
        .map(Vec::len)
        .max();
    assert!(
        points > Some(1000),
        "{points:?} time points under a 1 µs cap\n{out}"
    );
    // HSPICE's own writer spells the cap back, on the line before its
    // `.tran` (`spice/analyses.rs`): written twice, the text is the same.
    let back = write(deck, Dialect::Hspice, Dialect::Hspice);
    let lines: Vec<String> = back.lines().map(str::to_ascii_lowercase).collect();
    let capped = |w: &[String]| w[0].starts_with(".option delmax=") && w[1].starts_with(".tran ");
    assert!(lines.windows(2).any(capped), "{back}");
    assert_eq!(write(&back, Dialect::Hspice, Dialect::Hspice), back);
}

#[test]
fn p1_057_a_rejected_lines_fragment_is_no_unparsed_text() {
    // Xyce 7.10 `-norun` stops on `.FOUR {` (".FOUR line needs at least 3
    // arguments"), so the reader leaves the line out (P0010, P1-022); its
    // unclosed `{` (P0008) leaves with it and is no text another dialect
    // has to refuse (W0032). Oracle: Xyce 7.10 `-norun` on this deck, and
    // the golden gate's fx15 record of Xyce_Regression
    // FOURIER/bad_dot_four_line1.cir, which CDL writes. Checks: 3 per target.
    let deck = "t\nV1 1 0 1\nR1 1 0 1\n.TRAN 1N 100N\n.FOUR {\n.PRINT TRAN V(1)\n.END\n";
    for to in [Dialect::Cdl, Dialect::Ngspice] {
        let w = oracle::written(deck, Dialect::Xyce, to);
        let codes = oracle::notes(&w);
        assert!(codes.contains(&Code::P0010), "{to:?}: {codes:?}");
        assert!(codes.contains(&Code::P0008), "{to:?}: {codes:?}");
        assert!(!w.text.contains('{'), "{to:?}: {}", w.text);
    }
}
