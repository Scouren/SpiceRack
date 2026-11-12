//! The VACASK sources area (S4.2, `rewrite/DIALECT-VACASK.md` §3.2 sources … mutual,
//! tline_ideal): reader fixtures (`tests/fixtures/dialect/vacask/read/sources/<clause>.sim` and
//! `.ir`, the IR written by hand from the clause or run each header names), writer fixtures
//! (`…/write/sources/<name>.ir` and `.sim`), the fixpoint of read, write, read on every reader
//! fixture, and the settle decks `…/settle/src_*.sim`: what VACASK printed or simulated for each
//! rule (the oracle: VACASK 36be560 itself, R5; a wave against VD's closed form at every time
//! point the run wrote). The simulator round trip of the settle decks is the scaffold's
//! (`dialect_vacask::write::the_settle_decks_run_the_same_after_read_and_write`).
//!
//! Unsupported fails, never skips: without `vacask` the settle tests fail; run them in
//! `nix develop .#testing`.
//!
//! Contents: tests only; the settle tests in `dialect_vacask_sources/settle.rs`, the covering
//! properties (R55) in `dialect_vacask_sources/property.rs`.

use std::path::{Path, PathBuf};

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

#[path = "dialect_vacask_sources/property.rs"]
mod property;
#[path = "dialect_vacask_sources/settle.rs"]
mod settle;

/// The reader fixtures, relative to the crate root.
const READ: &str = "tests/fixtures/dialect/vacask/read/sources";
/// The writer fixtures.
const WRITE: &str = "tests/fixtures/dialect/vacask/write/sources";
/// The settle decks.
const SETTLE: &str = "tests/fixtures/dialect/vacask/settle";

/// Known failures, by fixture and cause; a fixture that passes while listed fails the test.
const KNOWN: &[(&str, &str)] = &[];

/// Reader fixtures the write tests cannot take yet, by cause (compared by the reader test).
const KNOWN_WRITE: &[(&str, &str)] = &[];

// ---- private ----

/// The files of `dir` with extension `ext`, sorted.
fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the fixture directory")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    out.sort();
    out
}

/// `src` read by `row`, leniently.
fn read(src: &Sources, row: &'static str) -> (ir::Design, ir::Diags) {
    let row = dialect::by_name(row).expect("a dialect row");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    (design, diags)
}

/// The deck at `path` as sources, then each `<stem>--<name>` library beside it (sorted).
fn deck(path: &Path) -> Sources {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let mut src = Sources::new();
    src.add(&name, &std::fs::read_to_string(path).unwrap());
    let mut libs: Vec<PathBuf> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{stem}--")))
        })
        .collect();
    libs.sort();
    for lib in libs {
        let lib_name = lib.file_name().unwrap().to_string_lossy().into_owned();
        src.add(&lib_name, &std::fs::read_to_string(&lib).unwrap());
    }
    src
}

/// The canonical IR text of every file of `design`.
fn canonical(design: &ir::Design) -> String {
    let mut out = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut out);
    }
    out
}

/// The codes of `diags`, as `P2001`-style words, sorted.
fn codes(diags: &ir::Diags) -> Vec<String> {
    let mut out: Vec<String> = diags
        .rows()
        .iter()
        .map(|d| format!("{}{:04}", d.code.letter as char, d.code.number))
        .collect();
    out.sort();
    out
}

/// The codes a fixture's `# codes:` line lists, sorted.
fn expected_codes(ir_text: &str) -> Vec<String> {
    let line = ir_text
        .lines()
        .find_map(|l| l.strip_prefix("# codes:"))
        .expect("a `# codes:` line");
    let mut out: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    out.sort();
    out
}

/// `design` legalized for VACASK and each file written, as sources under the same paths.
fn written(design: &mut ir::Design) -> Sources {
    let row = dialect::by_name("vacask").expect("the vacask row");
    let mut diags = ir::Diags::new();
    dialect::legalize(design, row, &mut diags);
    let refused: Vec<String> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .map(|d| format!("{:?} {:?}", d.code, ir::Feature::from_index(d.arg as u16)))
        .collect();
    assert!(refused.is_empty(), "legalize refused: {refused:?}");
    let mut src = Sources::new();
    let files: Vec<ir::FileId> = design.files().collect();
    for f in files {
        let text = dialect::write(design, f, row, &mut diags);
        src.add(design.str(design.file(f).path), &text);
    }
    src
}

#[test]
fn every_reader_fixture_reads_to_its_ir() {
    let mut failed = Vec::new();
    for path in files(READ, "sim") {
        let ir_path = path.with_extension("ir");
        let want_text = std::fs::read_to_string(&ir_path).expect("a .ir beside the deck");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // A panic (a want the IR cannot hold yet, a reader assertion) is a failure too.
        let compared = std::panic::catch_unwind(|| {
            let (want, _) = read(&Sources::one("want.ir", &want_text), "ir");
            let (got, diags) = read(&deck(&path), "vacask");
            let mut bad = Vec::new();
            if canonical(&got) != canonical(&want) {
                bad.push(format!(
                    "--- got\n{}--- want\n{}",
                    canonical(&got),
                    canonical(&want)
                ));
            }
            if codes(&diags) != expected_codes(&want_text) {
                bad.push(format!("codes {:?}", codes(&diags)));
            }
            bad
        });
        let bad = compared.unwrap_or_else(|_| vec!["panicked".to_string()]);
        let known = KNOWN.iter().any(|(k, _)| *k == name);
        match (bad.is_empty(), known) {
            (true, true) => failed.push(format!("{name}: passes; drop its KNOWN line")),
            (false, false) => failed.push(format!("{name}:\n{}", bad.join("\n"))),
            _ => {}
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn every_reader_fixture_is_a_fixpoint_of_write_and_read() {
    for path in files(READ, "sim") {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if KNOWN.iter().chain(KNOWN_WRITE).any(|(k, _)| *k == name) {
            continue;
        }
        let (mut once, _) = read(&deck(&path), "vacask");
        let before = canonical(&once);
        let (twice, _) = read(&written(&mut once), "vacask");
        assert_eq!(canonical(&twice), before, "{name}");
    }
}

#[test]
fn every_writer_fixture_writes_its_text() {
    let mut failed = Vec::new();
    for ir_path in files(WRITE, "ir") {
        let want = std::fs::read_to_string(ir_path.with_extension("sim")).expect("a .sim beside");
        let (mut design, _) = read(&deck(&ir_path), "ir");
        let src = written(&mut design);
        let got = src.files()[0].1.clone();
        if got != want {
            failed.push(format!(
                "{}:\n--- got\n{got}--- want\n{want}",
                ir_path.display()
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
