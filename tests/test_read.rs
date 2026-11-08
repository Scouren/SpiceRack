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

use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::render_text;

// ---- private ----

/// The deck `text` read as `from`, written for `to`; panics on a refusal.
fn write(text: &str, from: Dialect, to: Dialect) -> String {
    render_text(text, from, "t", to)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.messages))
        .text
}

/// The codes the refusal of `text` (read as `from`, written for `to`) has.
fn refusal(text: &str, from: Dialect, to: Dialect) -> Vec<Code> {
    match render_text(text, from, "t", to) {
        Ok(w) => panic!("written, not refused:\n{}", w.text),
        Err(e) => e.diags.iter().map(|d| d.code).collect(),
    }
}

/// The codes of the notes on `text` (read as `from`) written for `to`;
/// panics on a refusal.
fn notes(text: &str, from: Dialect, to: Dialect) -> Vec<Code> {
    render_text(text, from, "t", to)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.messages))
        .notes
        .diags
        .iter()
        .map(|d| d.code)
        .collect()
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
    // the script's (W0025, `control::ControlLine::Script`). Checks: 2.
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
