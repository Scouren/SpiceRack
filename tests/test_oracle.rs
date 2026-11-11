//! The test templates' own arithmetic (`tests/common/oracle.rs`): the
//! tolerance bound and where `value` reads a vector, on plots built by hand,
//! so a template cannot pass a wrong number by construction.
//!
//! Oracle: the numbers below, by hand (a ramp v = 2·t read at points and
//! between them; the bound's edge cases from its formula).
//!
//! Contents: none (tests only).

#[path = "common/oracle.rs"]
mod oracle;

use std::path::Path;

use oracle::{At, Tol, value};
use spicerack::result::{RawData, VarInfo, VarType};
use spicerack::syntax::Dialect;

// ---- private ----

/// A transient plot: time 0, 1, 2, 4 and `v(out)` = 2·time.
fn ramp() -> RawData {
    let var = |index, name: &str, var_type| VarInfo {
        index,
        name: name.into(),
        var_type,
    };
    RawData {
        plot_name: "Transient Analysis".into(),
        variables: vec![
            var(0, "time", VarType::Time),
            var(1, "v(out)", VarType::Voltage),
        ],
        real_data: vec![vec![0.0, 1.0, 2.0, 4.0], vec![0.0, 2.0, 4.0, 8.0]],
        ..RawData::default()
    }
}

#[test]
fn tol_is_relative_to_want_down_to_its_floor() {
    let t = Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    assert!(t.within(1.001, 1.0) && !t.within(1.0011, 1.0));
    assert!(
        t.within(0.0, 0.0) && !t.within(1e-300, 0.0),
        "floor 0: exact zero"
    );
    let f = Tol {
        rel: 1e-3,
        floor: 1.0,
    };
    assert!(
        f.within(1e-3, 0.0) && !f.within(1.1e-3, 0.0),
        "floor 1: abs 1e-3"
    );
    assert!(
        !t.within(f64::NAN, 1.0) && !t.within(1.0, f64::NAN),
        "NaN never meets"
    );
}

#[test]
fn value_reads_first_last_and_between_points() {
    let p = [ramp()];
    assert_eq!(value(&p, "out", At::First), 0.0);
    assert_eq!(value(&p, "OUT", At::Last), 8.0, "case-folded");
    assert_eq!(value(&p, "out", At::Time(2.0)), 4.0, "on a point");
    assert_eq!(value(&p, "out", At::Time(3.0)), 6.0, "between 2 and 4");
    assert_eq!(value(&p, "v(out)", At::Time(0.5)), 1.0, "the full name");
}

#[test]
#[should_panic(expected = "after")]
fn value_refuses_a_time_past_the_axis() {
    value(&[ramp()], "out", At::Time(5.0));
}

#[test]
#[should_panic(expected = "no v(x)")]
fn value_names_a_missing_vector() {
    value(&[ramp()], "x", At::First);
}

#[test]
#[should_panic(expected = "never a deck extension")]
fn golden_refuses_a_deck_extension_the_golden_gate_would_read() {
    oracle::golden("spectre/x.scs", "by hand", "");
}

/// The two tolerance forms the migrated crates used, written as they were
/// (`test_xyce_write.rs` `same_op` and `both_op` before `oracle.rs`), equal
/// to their `Tol` on every edge: zero, the bound itself, either side of
/// it, sign, NaN and infinity.
#[test]
fn tol_reproduces_each_migrated_form_exactly() {
    let same_op = |got: f64, want: f64| (got - want).abs() <= 1e-3 * want.abs();
    let both_op = |got: f64, want: f64| (got - want).abs() <= 1e-9 * want.abs().max(1e-12);
    let forms = [
        (
            same_op as fn(f64, f64) -> bool,
            Tol {
                rel: 1e-3,
                floor: 0.0,
            },
        ),
        (
            both_op,
            Tol {
                rel: 1e-9,
                floor: 1e-12,
            },
        ),
    ];
    let wants = [
        0.0,
        -0.0,
        1e-15,
        1e-12,
        1.0,
        -2.5,
        3.3e6,
        f64::NAN,
        f64::INFINITY,
    ];
    let steps = [
        0.0,
        1e-21,
        1e-15,
        9.99e-4,
        1e-3,
        1.0001e-3,
        1e-9,
        1.0,
        -1e-3,
        f64::NAN,
    ];
    let mut differ = Vec::new();
    for (i, (old, tol)) in forms.iter().enumerate() {
        for &want in &wants {
            for &d in &steps {
                for got in [want + d, want - d, want * (1.0 + d), want * (1.0 - d)] {
                    if old(got, want) != tol.within(got, want) {
                        differ.push((i, got, want));
                    }
                }
            }
        }
    }
    assert_eq!(differ, [], "form, got, want where Tol differs");
}

#[test]
#[should_panic(expected = "no recorded pin")]
fn p1_043_a_missing_pin_names_the_d43_oracles() {
    // ZERO D43: no Spectre run can be recorded any more, so the message
    // names what is left (the manual page, another simulator, the marker).
    oracle::pinned(Dialect::Spectre, "* no such deck\n.end\n", "out", At::First);
}

#[test]
fn p1_043_a_recorded_spectre_run_checks_the_written_deck() {
    // The divider swept to 2 V: out = 2/3 at the last point (by hand).
    // ngspice runs the source; Spectre's number is the pin of the deck the
    // writer writes from it, which Spectre 25.1 ran unchanged on iccad4
    // (settle-remote "Spectre writer check", p_dc_sweep: 0.6666667, seven
    // digits recorded, hence 1e-6). A writer change that moves one byte
    // of the deck loses the pin and fails here with the D43 message.
    let deck = "a dc sweep\nv1 in 0 0\nr1 in out 2k\nr2 out 0 1k\n.dc v1 0 2 0.5\n.end\n";
    let tol = Tol {
        rel: 1e-6,
        floor: 0.0,
    };
    let on = [Dialect::Ngspice, Dialect::Spectre];
    oracle::closed_form(Dialect::Ngspice, deck, &on, "out", At::Last, 2.0 / 3.0, tol);
}

#[test]
fn p1_050_hspice_pins_are_the_recorded_runs_by_name() {
    // research/settle-remote.md and REMOTE-ORACLE.md: the HSPICE
    // Y-2026.03-SP2-1 runs remote-oracle made on iccad4, 2026-11-07. No
    // other run exists and none can be added (ZERO D43), so the names are
    // compared, never counted. Checks: 1 + 2 per pin.
    let want = [
        "bsim1", "bsim2", "disto", "divider", "ekv", "gaas", "gates", "hb", "hbosc", "lstb",
        "mextram", "nint", "opamp", "sn", "u", "vccap", "w",
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hspice/recorded");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "pin"))
        .map(|p| {
            p.file_stem()
                .expect("a stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, want);
    for n in want {
        let text = std::fs::read_to_string(dir.join(format!("{n}.pin"))).expect("listed above");
        let tool = "provenance: HSPICE Y-2026.03-SP2-1, iccad4, 2026-11-07 (research/";
        assert!(text.starts_with(tool), "{n}: {text}");
        assert!(
            text.lines().nth(1).is_some_and(|l| l.starts_with("deck: ")),
            "{n}: {text}"
        );
    }
}

#[test]
fn p1_050_recorded_reads_a_key_of_a_named_run() {
    // settle-remote, row P6-002: `.disto rl 1` on the diode stage gave
    // hd2 = 23.01, hd3 = 391.5, im2d = 46.02 in the .lis. Checks: 3.
    assert_eq!(oracle::recorded(Dialect::Hspice, "disto", "hd2"), 23.01);
    assert_eq!(oracle::recorded(Dialect::Hspice, "disto", "hd3"), 391.5);
    assert_eq!(
        oracle::recorded(Dialect::Hspice, "nint", "nint(-2.5)"),
        -3.0
    );
}

#[test]
#[should_panic(expected = "no recorded run")]
fn p1_050_a_missing_recorded_run_names_the_d43_oracles() {
    oracle::recorded(Dialect::Hspice, "no_such_run", "x");
}

#[test]
fn p1_050_the_hspice_writer_meets_the_recorded_divider() {
    // REMOTE-ORACLE.md's smoke test: 3 V across 1k + 2k is V(b) = 2.0000
    // on HSPICE 2026.03 (five digits recorded, hence 5e-5). The deck that
    // ran was not kept, so the writer's HSPICE text for the same circuit
    // runs on the simulator here that reads HSPICE (ESPice
    // `--tokenizer=hspice`, ZERO D43 tier c) and must give the number real
    // HSPICE recorded; ngspice on the source vouches for the circuit.
    let deck = "t\nv1 a 0 3\nr1 a b 1k\nr2 b 0 2k\n.op\n.end\n";
    let want = oracle::recorded(Dialect::Hspice, "divider", "v(b)");
    let tol = Tol {
        rel: 5e-5,
        floor: 0.0,
    };
    let on = [Dialect::Ngspice, Dialect::Hspice];
    oracle::closed_form(Dialect::Ngspice, deck, &on, "b", At::First, want, tol);
}
