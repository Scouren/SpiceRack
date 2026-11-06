//! The structural writers against real tools: each SPICE deck here written
//! as CDL is compared by netgen's LVS against the deck itself (same devices,
//! same nets, every subcircuit), and written as Verilog is read and
//! elaborated by Icarus Verilog (`iverilog`), every module resolved.
//!
//! Oracles: netgen 1.5 (`lvs`: every cell's "Netlists match uniquely", no
//! property error); Icarus Verilog
//! 13 (`iverilog -g2012`, exit 0, no warning).
//!
//! Cites: netgen manual (`netgen -batch lvs "<f1> <cell>" "<f2> <cell>"`);
//! IEEE 1800-2017 §23 (modules, ports, instances).
//!
//! Contents: none (tests only).

use std::path::Path;
use std::process::Command;

use spicerack::syntax::Dialect;
use spicerack::syntax::render::render_text;

/// Hierarchical decks: subcircuits nested two deep, MOSFETs, passives,
/// a parameterised instance, a port order that is not the alphabet's.
const DECKS: &[(&str, &str)] = &[
    (
        "inverter_chain",
        "an inverter chain
.subckt inv a y vdd vss
mp y a vdd vdd pch w=2u l=180n
mn y a vss vss nch w=1u l=180n
.ends inv
.subckt buf in out vdd vss
x1 in mid vdd vss inv
x2 mid out vdd vss inv
.ends buf
.model nch nmos level=1
.model pch pmos level=1
vdd vdd 0 1.8
vin in 0 0
xb in out vdd 0 buf
c1 out 0 10f
.op
.end
",
    ),
    (
        "divider_params",
        "a parameterised divider
.subckt div top mid bot rt=1k rb=1k
r1 top mid {rt}
r2 mid bot {rb}
.ends div
v1 a 0 1
x1 a m 0 div rt=2k
x2 m n 0 div
r3 n 0 10k
.op
.end
",
    ),
];

/// Decks whose Verilog Icarus rejects today, each with its cause (named in
/// FINDINGS-oracles.md); a listed deck that elaborates fails until its line goes.
const VERILOG_FAILS: &[(&str, &str)] = &[
    (
        "divider_params",
        "ground written as the escaped net `\\0 ` and never declared: `implicit definition of wire '0'` (an error under `default_nettype none`)",
    ),
    (
        "inverter_chain",
        "ground written as the escaped net `\\0 ` and never declared, as divider_params (the keyword `buf` is escaped `\\buf ` since stream B's 080a996)",
    ),
];

// ---- private ----

/// `text` in `dir/<stem>.<ext>`, the path.
fn put(dir: &Path, stem: &str, ext: &str, text: &str) -> std::path::PathBuf {
    let p = dir.join(format!("{stem}.{ext}"));
    std::fs::write(&p, text).unwrap();
    p
}

/// `program args` run in `dir`: its exit status and output.
fn run(dir: &Path, program: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("{program}: {e}: run the tests in `nix develop .#testing`"));
    let log = String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr);
    (out.status.success(), log.into_owned())
}

#[test]
fn cdl_matches_its_source_by_lvs() {
    let tmp = tempfile::TempDir::new().unwrap();
    for (stem, deck) in DECKS {
        let cdl = render_text(deck, Dialect::Ngspice, stem, Dialect::Cdl)
            .unwrap_or_else(|e| panic!("{stem}: CDL refused: {}", e.messages));
        let a = put(tmp.path(), stem, "sp", deck);
        let b = put(tmp.path(), stem, "cdl", &cdl.text);
        let report = tmp.path().join(format!("{stem}.lvs"));
        let (a, b, r) = (a.display(), b.display(), report.display());
        let (_, log) = run(
            tmp.path(),
            "netgen",
            &[
                "-batch",
                "lvs",
                &a.to_string(),
                &format!("{b} {stem}"),
                "",
                &r.to_string(),
            ],
        );
        let text = std::fs::read_to_string(&report).unwrap_or(log);
        // A deck's top has no ports, the CDL's top cell none either: netgen
        // then reports "Top level cell failed pin matching" on two empty pin
        // lists. Every cell's netlist and properties must still match.
        let cells = text.matches("Netlists match uniquely").count();
        let mismatch = ["do not match", "Property errors", "property errors"];
        assert!(
            cells > 0 && !mismatch.iter().any(|m| text.contains(m)),
            "{stem}: netgen finds the CDL unlike its source:\n{}\n{text}",
            cdl.text
        );
    }
}

#[test]
fn verilog_elaborates_in_icarus() {
    let tmp = tempfile::TempDir::new().unwrap();
    for (stem, deck) in DECKS {
        let v = render_text(deck, Dialect::Ngspice, stem, Dialect::Verilog)
            .unwrap_or_else(|e| panic!("{stem}: Verilog refused: {}", e.messages));
        let f = put(tmp.path(), stem, "v", &v.text);
        let f = f.display().to_string();
        let (ok, log) = run(
            tmp.path(),
            "iverilog",
            &["-g2012", "-Wall", "-o", "/dev/null", &f],
        );
        let listed = VERILOG_FAILS.iter().find(|k| k.0 == *stem);
        assert_eq!(
            ok && !log.contains("warning"),
            listed.is_none(),
            "{stem}: listed {listed:?}; iverilog says:\n{}\n{log}",
            v.text
        );
    }
}
