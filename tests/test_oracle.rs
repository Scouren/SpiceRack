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
