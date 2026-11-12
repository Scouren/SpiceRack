//! `vacask.rs` against lines and numbers written out by hand: the step
//! cap VACASK ran with, and the noise plot ngspice wrote for the same
//! divider.
//!
//! Cites: research/impl/evidence/P1-066-vacask-harness/ and
//! P1-066-vacask-noted-drops/runs.txt (ZERO-LEDGER P1-066).
//!
//! Contents: none (tests only).

use spicerack::result::{RawData, VarInfo, VarType};

use crate::vacask::{refined, spectrum};

// ---- private ----

/// Oracle: the lines written out by hand: stop/n, or the deck's own
/// smaller cap; VACASK ran decks refined so (ring.sp: 20058 points at
/// n = 20000). Checks: 5.
#[test]
fn p1_066_a_vacask_transient_is_refined_by_its_maxstep() {
    let deck = "t\ncontrol\n  analysis tran1 tran step=1e-3 stop=4 maxstep=1e-3\nendc\n";
    let fine = "t\ncontrol\n  analysis tran1 tran step=1e-3 stop=4 maxstep=2e-4\nendc\n";
    assert_eq!(refined(deck, 20000.0).as_deref(), Some(fine));
    // At n = 2000 the cap is 2 ms: the deck's own 1 ms stays.
    assert_eq!(refined(deck, 2000.0).as_deref(), Some(deck));
    // No `maxstep=` of its own, an SI suffix, words after it.
    let open = "  analysis tran1 tran step=2u stop=20m icmode=\"op\"\n";
    let capped = refined(open, 2000.0).expect("a transient");
    let (head, max) = capped.trim_end().rsplit_once(" maxstep=").expect("a cap");
    assert_eq!(head, open.trim_end());
    let max: f64 = max.parse().expect("a number");
    assert!((max / 1e-5 - 1.0).abs() < 1e-12, "20 ms / 2000 is {max}");
    assert_eq!(
        refined("t\ncontrol\n  analysis op1 op\nendc\n", 2000.0),
        None
    );
}

/// Oracle: ngspice-45 and VACASK on one divider (1k/1k, `.noise v(2) v1
/// dec 10 1 1k`), the first point: VACASK `onoise` 8.28803e-18 V²/Hz,
/// `gain` 0.25; ngspice `onoise_spectrum` 2.87889e-9, `inoise_spectrum`
/// 5.75779e-9 V/√Hz (the numbers as printed: 1e-5). Checks: 3 + 1.
#[test]
fn p1_066_vacasks_noise_plot_reads_as_ngspices_spectrum() {
    let var = |index: usize, name: &str| VarInfo {
        index,
        name: name.to_string(),
        var_type: VarType::Voltage,
    };
    let plot = RawData {
        plot_name: "Small-Signal Noise Analysis".into(),
        variables: vec![
            var(0, "frequency"),
            var(1, "onoise"),
            var(2, "gain"),
            var(3, "n(r1)"),
        ],
        real_data: vec![vec![1.0], vec![8.28803e-18], vec![0.25], vec![4.14402e-18]],
        ..Default::default()
    };
    let s = spectrum(plot);
    let names: Vec<&str> = s.variables.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["frequency", "onoise_spectrum", "inoise_spectrum"]);
    let close = |got: f64, want: f64| {
        assert!((got / want - 1.0).abs() < 1e-5, "{got} against {want}");
    };
    close(s.real_data[1][0], 2.87889e-9);
    close(s.real_data[2][0], 5.75779e-9);
    let op = RawData {
        plot_name: "Operating Point".into(),
        ..Default::default()
    };
    assert_eq!(spectrum(op).plot_name, "Operating Point");
}
