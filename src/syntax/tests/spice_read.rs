//! The SPICE-family reader's phase-7 readings (stream spice-read): each
//! construct it now reads, written by every target in its own form or
//! refused with the code naming what the target lacks. Each test names its
//! clause, the text derived by hand from it, and how many checks it makes.
//!
//! Contents: none (tests only).

use crate::syntax::Dialect;
use crate::syntax::diag::Code;
use crate::syntax::render::render_text;

// ---- private ----

#[test]
fn an_ltspice_startup_transient_is_ltspices_and_refused_elsewhere() {
    // LTspice help, `.TRAN`: `.tran <Tstop> [modifiers]`, the modifier
    // `startup` (sources ramped on over the first 20 µs) and `uic` in any
    // order. LTspice writes its own card (`Tstep` 0: LTspice's default);
    // every other simulator has no such ramp (W0037). Checks: 1 + 6.
    let text = "t\nR1 a 0 1k\nV1 a 0 1\n.tran 750u startup uic\n.end\n";
    let lt = render_text(text, Dialect::Ltspice, "t", Dialect::Ltspice).expect("written");
    assert!(
        lt.text.lines().any(|l| l == ".tran 0 750u UIC startup"),
        "{}",
        lt.text
    );
    for d in [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Spectre,
        Dialect::Vacask,
    ] {
        let e = render_text(text, Dialect::Ltspice, "t", d).expect_err("refused");
        assert!(
            e.diags.iter().any(|g| g.code == Code::W0037),
            "{d:?}: {}",
            e.messages
        );
    }
}

#[test]
fn an_ltspice_backanno_is_left_out_with_its_note() {
    // LTspice help, `.BACKANNO`: the viewer's port-current names, no number
    // of the run. Left out on every target, noted W0036. Checks: 2 per
    // target, 3 targets.
    let text = "t\nR1 a 0 1k\nV1 a 0 1\n.op\n.backanno\n.end\n";
    for d in [Dialect::Ltspice, Dialect::Ngspice, Dialect::Spectre] {
        let w = render_text(text, Dialect::Ltspice, "t", d).expect("written");
        assert!(
            !w.text.to_ascii_lowercase().contains("backanno"),
            "{d:?}: {}",
            w.text
        );
        assert!(w.notes.diags.iter().any(|g| g.code == Code::W0036), "{d:?}");
    }
}
