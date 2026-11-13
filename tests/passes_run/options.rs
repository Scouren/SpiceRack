//! `inert_options` on the simulator: each option the rule drops leaves ngspice-45's raw file as
//! it is (`decks/options_probe.sp`: op, dc, ac, tran and a `.four` over a diode, an inductor,
//! an RC and a MOSFET), every plot, vector and value equal; and `interp`, which the rule keeps,
//! changes it (the control: the comparison sees a change). `klu` within its stated bound, 1e-7
//! relative above ABSTOL (R83); and `drop_print`'s four combinations of `.print` and `.save`, the raw file
//! identical. And, on the IR alone, for every row and every Fourier analysis that may read its
//! key (none, one with its count and grid, one without a count, one without a grid): `ir::check`
//! clean after the row, and the option kept exactly where an IR row reads it.
//!
//! Contents: `pub fn inert_options_leave_the_raw_file_alone()`,
//! `pub fn inert_options_keep_what_an_ir_row_reads()`, `pub fn prints_leave_the_raw_file_alone()`, `pub fn klu_within_its_rounding()`

use std::path::Path;

use spicerack::ir;
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
        DROPPED.len() + 2,
        spicerack::passes::run::inert_options::INERT.len(),
        "one spelling a row, and klu (bounded) and trtol (a hint, R84)"
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

/// Each row over the Fourier forms that read or do not read its key: check clean, the option
/// kept (`OPTION_KEPT`) exactly where a Four reads it.
pub fn inert_options_keep_what_an_ir_row_reads() {
    use spicerack::passes::run::inert_options::{INERT, OPTION_DROPPED, OPTION_KEPT};
    let fours = [
        ("", None),
        (
            "  analysis four fundamental=1k harmonics=9 periods=1 grid=100 (v (out))\n",
            None,
        ),
        (
            "  analysis four fundamental=1k periods=1 grid=100 (v (out))\n",
            Some(ir::OptionKey::NFreqs),
        ),
        (
            "  analysis four fundamental=1k harmonics=9 periods=1 (v (out))\n",
            Some(ir::OptionKey::FourGridSize),
        ),
    ];
    let mut ran = 0;
    for &(name, key, run) in INERT {
        for (four, reader) in fours {
            let value = if key.is_flag() {
                String::new()
            } else {
                " value=5".to_string()
            };
            // a count-less Four needs an nfreqs in force to be valid at all
            let nfreqs = if reader == Some(ir::OptionKey::NFreqs) && key != ir::OptionKey::NFreqs {
                "  option n_freqs value=5 whole_run\n"
            } else {
                ""
            };
            let text = format!(
                "file \"a.sp\" deck\n  option {}{value} whole_run\n{nfreqs}  analysis tran stop=1u print_step=1n cap=none\n{four}",
                snake(&format!("{key:?}"))
            );
            let mut design = ir::Design::new();
            let mut diags = ir::Diags::new();
            ir::parse(&text, &mut design.builder(), &mut diags);
            ir::check(&design, &mut diags);
            assert!(
                diags.is_empty(),
                "{name}: the input\n{text}{:?}",
                diags.rows()
            );
            run(&mut design, &mut diags);
            let codes: Vec<ir::Code> = diags.rows().iter().map(|d| d.code).collect();
            let mut found = ir::Diags::new();
            ir::check(&design, &mut found);
            assert!(
                found.is_empty(),
                "{name}: check after the row\n{text}{:?}",
                found.rows()
            );
            let want = if reader == Some(key) {
                OPTION_KEPT
            } else {
                OPTION_DROPPED
            };
            assert_eq!(codes, [want], "{name}\n{text}");
            ran += 1;
        }
    }
    assert_eq!(ran, INERT.len() * fours.len(), "every row over every form");
}

// ---- private ----

/// The IR's word for a key: snake case of its name (`NoAcct` → `no_acct`).
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (k, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && k > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

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
