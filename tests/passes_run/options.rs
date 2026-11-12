//! `inert_options` on the simulator: each option the rule drops leaves ngspice-45's raw file as
//! it is (`decks/options_probe.sp`: op, dc, ac, tran and a `.four` over a diode, an inductor,
//! an RC and a MOSFET), every plot, vector and value equal; and `interp`, which the rule keeps,
//! changes it (the control: the comparison sees a change).
//!
//! Contents: `pub fn inert_options_leave_the_raw_file_alone()`

use std::path::Path;

use spicerack::result::RawData;

use crate::sim::{self, Ran, Sim};

/// Each dropped option's spelling, as the probe deck's `.options` line takes it.
const DROPPED: &[&str] = &[
    "noacct",
    "acct",
    "list",
    "nomod",
    "nopage",
    "node",
    "opts",
    "warn=1",
    "maxwarns=5",
    "norefvalue",
    "noinit",
    "nfreqs=12",
    "fourgridsize=300",
];

/// Runs the probe with each option and compares the plots with the probe's own.
pub fn inert_options_leave_the_raw_file_alone() {
    assert_eq!(
        DROPPED.len(),
        spicerack::passes::run::inert_options::INERT.len(),
        "one spelling a row"
    );
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/passes_run/decks/options_probe.sp");
    let text = std::fs::read_to_string(&path).expect("the deck");
    let run = |line: &str| -> Vec<RawData> {
        let deck = text.replace("*OPTION", line);
        match sim::run(Sim::Ngspice, &path, &deck, &[], 60) {
            Ran::Plots(p) => p,
            Ran::Timeout => panic!("{line}: past the CPU budget"),
            Ran::Failed(why) => panic!("{line}: {why}"),
        }
    };
    let base = run("");
    assert!(base.len() >= 4, "op, dc, ac and tran plots");
    for o in DROPPED {
        let with = run(&format!(".options {o}"));
        assert!(same(&base, &with), "{o}: the raw file changed");
    }
    assert!(
        !same(&base, &run(".options interp")),
        "interp: the control changes nothing"
    );
}

// ---- private ----

fn same(a: &[RawData], b: &[RawData]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            x.plot_name == y.plot_name
                && x.variables
                    .iter()
                    .map(|v| &v.name)
                    .eq(y.variables.iter().map(|v| &v.name))
                && x.real_data == y.real_data
                && x.complex_data == y.complex_data
        })
}
