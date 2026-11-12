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

/// Settle decks whose round trip fails today, by name, and why. Strict both ways.
const SETTLE_KNOWN: &[(&str, &str)] = &[
    (
        "atan2_zero.sim",
        "VACASK's parameter atan2(0, 0) is NaN (the analysis then aborts on it); the reader's \
     expansion (R76a) divides 0 by 0 there, which VACASK's `/` refuses (\"Division by zero.\"): \
     the IR has no NaN to write, so the deck fails at another place",
    ),
    (
        "grid_fold_atan2.sim",
        "atan2(y, -0.0): the cited exception of grid.rs's KNOWN_GRID (R76a)",
    ),
    (
        "grid_static.sim",
        "atan2(y, -0.0): the cited exception of grid.rs's KNOWN_GRID (R76a)",
    ),
];

// ---- private ----

/// The entry file of `design` written by VACASK's writer, after `legalize`.
fn write_entry(design: &mut ir::Design) -> String {
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
    let text = dialect::write(design, design.entry(), row, &mut diags);
    assert!(
        !diags.rows().iter().any(|d| dialect::refuses(d.code)),
        "refused: {:?}",
        diags.rows()
    );
    text
}

/// `text` read as `row`, and whether the read refused it.
fn read_checked(path: &str, text: &str, row: &str) -> (ir::Design, bool) {
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
    let refused = diags.rows().iter().any(|d| dialect::refuses(d.code));
    (design, refused)
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

/// What VACASK printed after its banner, each raw file it wrote
/// (sorted by name), and whether it reported an error at a place in the deck, for `deck` run in a
/// fresh directory.
fn vacask(deck: &str) -> (String, Vec<(String, Vec<u8>)>, bool) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("deck.sim"), deck).unwrap();
    let done = Command::new("vacask")
        .arg("deck.sim")
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let mut text = String::from_utf8_lossy(&done.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&done.stderr));
    // The banner ends with the thread counts; what follows is the run's.
    let printed = text
        .find("OpenBLAS CPUs:")
        .and_then(|k| text[k..].find('\n').map(|e| k + e + 1))
        .map_or(text.clone(), |k| text[k..].to_string());
    let printed = without_tmp_names(&printed);
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
    // A parse or elaboration error has a location line; a run-time one names the analysis.
    let error = printed.contains(") in ")
        || (printed.contains("Analysis '") && printed.contains("' aborted."));
    (messages(&printed), raws, error)
}

/// `printed` without where VACASK pointed in the deck: each message's location line (`9:10
/// (0xad) in …`), its echo of the source line and the caret under it, which move when the
/// writer spaces a statement otherwise; and the run's clock (`Elapsed time:`).
fn messages(printed: &str) -> String {
    let lines: Vec<&str> = printed.lines().collect();
    let caret = |l: &str| l.trim() == "^";
    let located = |l: &str| {
        let t = l.trim_start();
        t.split_once(' ').is_some_and(|(pos, rest)| {
            pos.split_once(':').is_some_and(|(a, b)| {
                a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit())
            }) && rest.starts_with("(0x")
        })
    };
    let mut out = String::new();
    for (k, l) in lines.iter().enumerate() {
        let echo = lines.get(k + 1).is_some_and(|n| caret(n));
        let clock = l.trim_start().starts_with("Elapsed time:");
        if !(caret(l) || located(l) || echo || clock) {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
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
    let mut failing: Vec<String> = Vec::new();
    for deck in decks {
        let name = deck.file_name().unwrap().to_string_lossy().into_owned();
        if let Err(why) = round_trip(&deck) {
            eprintln!("{name}: {why}");
            failing.push(name);
        }
    }
    let known: Vec<&str> = SETTLE_KNOWN.iter().map(|(d, _)| *d).collect();
    assert_eq!(
        failing, known,
        "settle round trips failing, against SETTLE_KNOWN"
    );
}

/// One settle deck: VACASK on it and on `write(read(it))` print and write the same, or the
/// reader refuses a deck VACASK rejects.
fn round_trip(deck: &Path) -> Result<(), String> {
    {
        let text = std::fs::read_to_string(deck).unwrap();
        let (mut design, refused) = read_checked("deck.sim", &text, "vacask");
        let a = vacask(&text);
        if refused {
            // A deck the reader refuses is one VACASK itself rejects.
            return match a.2 {
                true => Ok(()),
                false => Err("refused, but VACASK reported no error".to_string()),
            };
        }
        let again =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| write_entry(&mut design)))
                .map_err(|e| {
                    e.downcast_ref::<String>()
                        .cloned()
                        .unwrap_or_else(|| "the writer panicked".to_string())
                })?;
        let b = vacask(&again);
        if a.0 != b.0 {
            return Err(format!("printed\n{}\nagainst\n{}\nfor\n{again}", a.0, b.0));
        }
        if a.1 != b.1 {
            return Err(format!("raw files differ for\n{again}"));
        }
        Ok(())
    }
}

/// `text` with each OpenVAF temporary directory's random suffix (`.tmpE3haeq`) cut to `.tmp`:
/// a compile error names the file it wrote there (settle `beh_int`).
fn without_tmp_names(text: &str) -> String {
    let (mut out, mut rest) = (String::with_capacity(text.len()), text);
    while let Some(k) = rest.find(".tmp") {
        out.push_str(&rest[..k + 4]);
        rest = &rest[k + 4..];
        let suffix = rest.bytes().take_while(u8::is_ascii_alphanumeric).count();
        rest = &rest[suffix..];
    }
    out.push_str(rest);
    out
}
