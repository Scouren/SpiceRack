//! The ngspice corpus gate (S3.5, first line): every `.cir` and `.sp` deck of the pinned
//! ngspice-45 source's `examples/` (`NGSPICE_EXAMPLES`, set by `nix develop .#testing`), each
//! through four steps in order, the first failing one named:
//!
//! - `read`: read with its includes (`Sources::load`), no panic (the reader asserts law L1:
//!   every statement accounted for);
//! - `check`: `ir::check` clean;
//! - `legalize`: `dialect::legalize` for ngspice refuses nothing: no `U0001` (every feature a
//!   plain ngspice deck produces is in ngspice's `FEATURES`) and no `admits` code;
//! - `round_trip`: written, read back, equal to the legalized design by `eq_meaning` (L5).
//!
//! Oracle: the decks are ngspice's own examples, so each step's expectation is ngspice reading
//! its own text. The failing decks are compared by name and step with [`KNOWN`], strictly both
//! ways. A missing corpus fails, never skips. Nightly tier: built with `--features corpus`.
//!
//! Contents: tests only.

#![cfg(feature = "corpus")]

#[path = "common/memcap.rs"]
mod memcap;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, DialectRow, Sources};
use spicerack::ir;

/// The decks that fail today: (deck relative to `examples/`, step, why). Strict both ways.
#[rustfmt::skip] // a table: one row per line
const KNOWN: &[(&str, &str, &str)] = &[
    ("osdi/bsimbulk/bsimbulk_inverter.sp", "legalize", "W1305: `.dc V1 0 'vcc' 'vcc/100'` in a deck the writer writes as a script (R49), where a command drops the braces (settle script_tran_brace.sp); waits for a ruling on keeping card analyses in a scripted deck"),
    ("osdi/bsimbulk-local/bsimbulk_inverter.sp", "legalize", "as osdi/bsimbulk/bsimbulk_inverter.sp (W1305)"),
    ("osdi/mixed-models/bsimbulk_inverter.sp", "legalize", "as osdi/bsimbulk/bsimbulk_inverter.sp (W1305)"),
    ("osdi/mixed-models/psp_inverter.sp", "legalize", "as osdi/bsimbulk/bsimbulk_inverter.sp (W1305)"),
    ("osdi/psp103/psp_inverter.sp", "legalize", "as osdi/bsimbulk/bsimbulk_inverter.sp (W1305)"),
];

/// The step names, in order.
const STEPS: [&str; 4] = ["read", "check", "legalize", "round_trip"];

/// The corpus's decks at most (ngspice-45 has 419): a bound on the walk.
const DECKS_MAX: usize = 4096;

// ---- private ----

#[test]
fn every_example_deck_reads_checks_legalizes_and_round_trips() {
    let root = examples();
    let decks = decks(&root);
    assert!(
        root.is_dir() && !decks.is_empty(),
        "{}: no deck (unsupported fails)",
        root.display()
    );
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let mut failing: Vec<(String, &str, String)> = Vec::new();
    let mut counts = [0usize; 5];
    for deck in &decks {
        let name = deck
            .strip_prefix(&root)
            .expect("a deck under the root")
            .to_string_lossy()
            .into_owned();
        match run(row, deck) {
            Ok(()) => counts[4] += 1,
            Err((step, why)) => {
                counts[STEPS.iter().position(|s| *s == step).expect("a step")] += 1;
                failing.push((name, step, why));
            }
        }
    }
    println!(
        "corpus: {} decks; first failing step: read {}, check {}, legalize {}, round_trip {}; passed {}",
        decks.len(),
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts[4]
    );
    let got: Vec<(&str, &str)> = failing.iter().map(|(n, s, _)| (n.as_str(), *s)).collect();
    let want: Vec<(&str, &str)> = KNOWN.iter().map(|(n, s, _)| (*n, *s)).collect();
    let why: Vec<String> = failing
        .iter()
        .map(|(n, s, w)| format!("{n} {s}: {w}"))
        .collect();
    assert_eq!(
        got,
        want,
        "failing decks of ngspice {} examples against KNOWN\n{}",
        version(&root),
        why.join("\n")
    );
}

/// `NGSPICE_EXAMPLES`: unsupported fails.
fn examples() -> PathBuf {
    let dir = std::env::var_os("NGSPICE_EXAMPLES")
        .expect("NGSPICE_EXAMPLES is set by `nix develop .#testing` (unsupported fails)");
    PathBuf::from(dir)
}

/// The ngspice version the examples belong to: `ngspice_major_version` and
/// `ngspice_minor_version` as the source's `configure.ac` defines them (`m4_define([...], [45])`),
/// so a pin bump that changes the decks shows in the failure text.
fn version(root: &Path) -> String {
    let text = std::fs::read_to_string(root.join("../configure.ac")).unwrap_or_default();
    let defined = |name: &str| {
        let head = format!("m4_define([{name}],");
        let line = text.lines().find(|l| l.starts_with(&head))?;
        let arg = line[head.len()..].trim_matches(|c: char| "[]() ".contains(c));
        Some(arg.to_string())
    };
    match (
        defined("ngspice_major_version"),
        defined("ngspice_minor_version"),
    ) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => "(unknown version)".to_string(),
    }
}

/// Every `.cir` and `.sp` file under `root`, sorted.
fn decks(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    // Bounded: each directory is entered once, and the decks are capped.
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("cir") || ext.eq_ignore_ascii_case("sp") {
                out.push(path);
            }
        }
        assert!(
            out.len() <= DECKS_MAX,
            "the corpus has at most DECKS_MAX decks"
        );
    }
    out.sort();
    out
}

/// The four steps on `deck`: the first failing step and why.
fn run(row: &DialectRow, deck: &Path) -> Result<(), (&'static str, String)> {
    let path = deck.to_string_lossy().into_owned();
    let reader = row.read.expect("ngspice reads");
    let src = Sources::load(&[&path], ir::Role::Deck, reader, None, &mut |p| {
        std::fs::read(p)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    });
    let read = catch_unwind(AssertUnwindSafe(|| {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        reader.read(&src, &mut design.builder(), &mut diags);
        design
    }));
    let design = read.map_err(|p| ("read", panic_text(&p)))?;
    let mut found = ir::Diags::new();
    ir::check(&design, &mut found);
    if !found.is_empty() {
        return Err(("check", codes(&found)));
    }
    let mut legal = design.clone();
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut legal, row, &mut diags);
    // a lacked feature by name, any other refusal (an `admits` code) by its code
    let lacked: Vec<String> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .map(|d| match d.code == ir::diag::UNSUPPORTED {
            true => feature_name(d.arg),
            false => d.code.to_string(),
        })
        .collect();
    if !lacked.is_empty() {
        return Err(("legalize", lacked.join(" ")));
    }
    let trip = catch_unwind(AssertUnwindSafe(|| {
        let mut written = Sources::new();
        let entry = legal.entry();
        for f in std::iter::once(entry).chain(legal.files().filter(|&f| f != entry)) {
            let mut wd = ir::Diags::new();
            let text = dialect::write(&legal, f, row, &mut wd);
            written.add(legal.str(legal.file(f).path), &text);
        }
        written.set_form(ir::Role::Deck);
        let mut back = ir::Design::new();
        let mut bd = ir::Diags::new();
        reader.read(&written, &mut back.builder(), &mut bd);
        back.eq_meaning(&legal)
    }));
    match trip {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            "round_trip",
            "read(write(d)) differs by eq_meaning".to_string(),
        )),
        Err(p) => Err(("round_trip", panic_text(&p))),
    }
}

/// The codes of `diags`, joined.
fn codes(diags: &ir::Diags) -> String {
    let codes: Vec<String> = diags.rows().iter().map(|d| d.code.to_string()).collect();
    codes.join(" ")
}

/// A feature by its index, as `Debug` prints it.
fn feature_name(index: u32) -> String {
    ir::FeatureSet::ALL
        .iter()
        .find(|f| u32::from(f.index()) == index)
        .map_or_else(|| format!("#{index}"), |f| format!("{f:?}"))
}

/// A panic's message.
fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    let msg = p.downcast_ref::<String>().cloned();
    let msg = msg.or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()));
    format!("panic: {}", msg.unwrap_or_default())
}
