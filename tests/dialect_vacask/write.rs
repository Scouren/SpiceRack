//! Writer fixtures (`tests/fixtures/dialect/vacask/write/<name>.ir` and `.sim`): each design,
//! written by the VACASK writer, is the text written by hand from the VACASK clauses its header
//! names; and the simulator round trip of the settle decks: `vacask(write(read(d)))` prints and
//! writes what `vacask(d)` does (the oracle is VACASK on the original deck).
//!
//! Contents: tests only.

use std::path::Path;
use std::process::Command;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

/// The writer fixtures, relative to the crate root.
const DIR: &str = "tests/fixtures/dialect/vacask/write";
/// The settle decks.
const SETTLE: &str = "tests/fixtures/dialect/vacask/settle";

// ---- private ----

/// The entry file of `design` written by VACASK's writer, after `legalize`.
fn write_entry(design: &mut ir::Design) -> String {
    let row = dialect::by_name("vacask").expect("the vacask row");
    let mut diags = ir::Diags::new();
    dialect::legalize(design, row, &mut diags);
    let text = dialect::write(design, design.entry(), row, &mut diags);
    assert!(
        !diags.rows().iter().any(|d| dialect::refuses(d.code)),
        "refused: {:?}",
        diags.rows()
    );
    text
}

/// `text` read as `row`.
fn read(path: &str, text: &str, row: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    let row = dialect::by_name(row).expect("a row");
    dialect::read(
        &Sources::one(path, text),
        row,
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    design
}

/// What VACASK printed after its banner (its `Simulating:` line on), and each raw file it wrote,
/// sorted by name, for `deck` run in a fresh directory.
fn vacask(deck: &str) -> (String, Vec<(String, Vec<u8>)>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("deck.sim"), deck).unwrap();
    let done = Command::new("vacask")
        .arg("deck.sim")
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let mut text = String::from_utf8_lossy(&done.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&done.stderr));
    let printed = text
        .find("Simulating:")
        .map_or(String::new(), |k| text[k..].to_string());
    let mut raws: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "raw"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let bytes = std::fs::read(&p).unwrap();
            // A raw file's `Date:` line is the run's clock, not its result.
            let kept: Vec<u8> = String::from_utf8_lossy(&bytes)
                .lines()
                .filter(|l| !l.starts_with("Date:"))
                .flat_map(|l| l.bytes().chain(std::iter::once(b'\n')))
                .collect();
            (name, kept)
        })
        .collect();
    raws.sort();
    (
        printed.replace(&dir.path().display().to_string(), "<dir>"),
        raws,
    )
}

#[test]
fn every_writer_fixture_writes_its_text() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR);
    let mut failed = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for ir_path in entries
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "ir"))
    {
        let want = std::fs::read_to_string(ir_path.with_extension("sim")).expect("a .sim beside");
        let mut design = read("want.ir", &std::fs::read_to_string(ir_path).unwrap(), "ir");
        let got = write_entry(&mut design);
        if got != want {
            failed.push(format!(
                "{}:\n--- got\n{got}--- want\n{want}",
                ir_path.display()
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn the_settle_decks_run_the_same_after_read_and_write() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SETTLE);
    let mut decks: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sim"))
        .collect();
    decks.sort();
    for deck in decks {
        let text = std::fs::read_to_string(&deck).unwrap();
        let mut design = read("deck.sim", &text, "vacask");
        let again = write_entry(&mut design);
        let (a, b) = (vacask(&text), vacask(&again));
        assert_eq!(a.0, b.0, "{}: printed\n{again}", deck.display());
        assert_eq!(a.1, b.1, "{}: raw files\n{again}", deck.display());
    }
}
