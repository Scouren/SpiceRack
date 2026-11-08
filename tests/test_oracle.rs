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
#[should_panic(expected = "request one from remote-oracle")]
fn a_missing_pin_says_who_records_it() {
    oracle::pinned(Dialect::Spectre, "* no such deck\n.end\n", "out", At::First);
}
