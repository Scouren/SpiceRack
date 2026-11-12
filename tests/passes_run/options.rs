//! `inert_options` on the simulator: each option the rule drops leaves ngspice-45's raw file as
//! it is (`decks/options_probe.sp`: op, dc, ac, tran and a `.four` over a diode, an inductor,
//! an RC and a MOSFET), every plot, vector and value equal; and `interp`, which the rule keeps,
//! changes it (the control: the comparison sees a change). `klu` within its stated bound, 1e-7
//! relative above ABSTOL (R83); and `drop_print`'s four combinations of `.print` and `.save`, the raw file
//! identical.
//!
//! Contents: `pub fn inert_options_leave_the_raw_file_alone()`,
//! `pub fn prints_leave_the_raw_file_alone()`, `pub fn klu_within_its_rounding()`

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
    "sparse",
];

/// Runs the probe with each option and compares the plots with the probe's own.
pub fn inert_options_leave_the_raw_file_alone() {
    assert_eq!(
        DROPPED.len() + 1,
        spicerack::passes::run::inert_options::INERT.len(),
        "one spelling a row, and klu"
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
        let with = probe(&format!(".options {o}"));
        assert!(same(&base, &with), "{o}: the raw file changed");
    }
    assert!(
        !same(&base, &probe(".options interp")),
        "interp: the control changes nothing"
    );
}

/// `.print` cards against none, and beside a `.save` against the `.save` alone.
pub fn prints_leave_the_raw_file_alone() {
    let base = probe("");
    let one = ".print tran v(out)";
    let three = ".print tran v(out) v(mid)\n.print ac vdb(out)\n.print dc i(vdd)";
    assert!(same(&base, &probe(one)), "one print changed the raw file");
    assert!(
        same(&base, &probe(three)),
        "three prints changed the raw file"
    );
    let save = probe(".save v(out)");
    assert!(
        !same(&base, &save),
        "the control: a save changes the saved set"
    );
    assert!(
        same(&save, &probe(&format!(".save v(out)\n{one}"))),
        "a print beside a save changed the raw file"
    );
}

/// `klu` against the default: every value within 1e-7 relative of the default's, above an
/// absolute floor of ABSTOL (1e-12: a current of 5e-31 A is zero to either solver).
pub fn klu_within_its_rounding() {
    let (base, klu) = (probe(""), probe(".options klu"));
    assert_eq!(base.len(), klu.len(), "the same plots");
    for (a, b) in base.iter().zip(&klu) {
        assert_eq!(a.plot_name, b.plot_name);
        for (x, y) in a
            .real_data
            .iter()
            .flatten()
            .zip(b.real_data.iter().flatten())
        {
            let bound = 1e-7 * x.abs().max(y.abs()) + 1e-12;
            assert!((x - y).abs() <= bound, "{}: {x} against {y}", a.plot_name);
        }
        for (x, y) in a
            .complex_data
            .iter()
            .flatten()
            .zip(b.complex_data.iter().flatten())
        {
            let bound = 1e-7 * x.norm().max(y.norm()) + 1e-12;
            assert!((x - y).norm() <= bound, "{}: {x} against {y}", a.plot_name);
        }
    }
}

// ---- private ----

/// The probe deck with `line` in place of its `*OPTION` comment, run on ngspice-45.
fn probe(line: &str) -> Vec<RawData> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/passes_run/decks/options_probe.sp");
    let text = std::fs::read_to_string(&path).expect("the deck");
    let deck = text.replace("*OPTION", line);
    match sim::run(Sim::Ngspice, &path, &deck, &[], 60) {
        Ran::Plots(p) => p,
        Ran::Timeout => panic!("{line}: past the CPU budget"),
        Ran::Failed(why) => panic!("{line}: {why}"),
    }
}

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
