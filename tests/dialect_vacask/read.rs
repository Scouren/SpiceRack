//! Reader fixtures (`tests/fixtures/dialect/vacask/read/<clause>.sim` and `.ir`): each deck read
//! by the VACASK reader is the design its `.ir` file states, written by hand from the clause or
//! run its header names (the oracle), with the codes its `# codes:` line lists. A file
//! `<clause>--<name>` beside a deck is a library the deck includes, read as `<clause>--<name>`.
//! Each fixture is also a fixpoint of read, write, read (law L6 on the fixture).
//!
//! Contents: tests only.

use std::path::{Path, PathBuf};

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

/// The reader fixtures, relative to the crate root.
const DIR: &str = "tests/fixtures/dialect/vacask/read";

// ---- private ----

/// Every fixture deck, sorted.
fn decks() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR);
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the fixture directory")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "sim"))
        .collect();
    out.sort();
    out
}

/// The deck at `path` and its libraries as sources: the deck first (an entry), then each
/// `<stem>--<name>` file beside it.
fn sources(path: &Path) -> Sources {
    let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
    let mut src = Sources::new();
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
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
        let name = lib.file_name().unwrap().to_string_lossy().into_owned();
        src.add(&name, &std::fs::read_to_string(&lib).unwrap());
    }
    src
}

/// `src` read by `row`, leniently.
fn read(src: &Sources, row: &'static str) -> (ir::Design, ir::Diags) {
    let row = dialect::by_name(row).expect("a dialect row");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    (design, diags)
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

/// Every file of `design` written by VACASK's writer, as sources under the same paths.
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
    assert!(
        !diags.rows().iter().any(|d| dialect::refuses(d.code)),
        "writing refused: {:?}",
        diags.rows()
    );
    src
}

#[test]
fn every_reader_fixture_reads_to_its_ir() {
    let mut failed = Vec::new();
    for deck in decks() {
        let ir_path = deck.with_extension("ir");
        let want_text = std::fs::read_to_string(&ir_path).expect("a .ir beside the deck");
        let (want, _) = read(&Sources::one("want.ir", &want_text), "ir");
        let (got, diags) = read(&sources(&deck), "vacask");
        let name = deck.file_name().unwrap().to_string_lossy().into_owned();
        if canonical(&got) != canonical(&want) {
            failed.push(format!(
                "{name}:\n--- got\n{}--- want\n{}",
                canonical(&got),
                canonical(&want)
            ));
        }
        if codes(&diags) != expected_codes(&want_text) {
            failed.push(format!("{name}: codes {:?}", codes(&diags)));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn every_reader_fixture_is_a_fixpoint_of_write_and_read() {
    for deck in decks() {
        let (mut once, _) = read(&sources(&deck), "vacask");
        let before = canonical(&once);
        let (twice, _) = read(&written(&mut once), "vacask");
        assert_eq!(canonical(&twice), before, "{}", deck.display());
    }
}

/// Every fixture deck under the reader fixtures, subdirectories included, sorted.
fn all_decks() -> Vec<PathBuf> {
    let mut todo = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join(DIR)];
    let mut out = Vec::new();
    // Bounded: each directory is taken once (the fixture tree is acyclic).
    while let Some(dir) = todo.pop() {
        for e in std::fs::read_dir(&dir).expect("a fixture directory") {
            let p = e.expect("an entry").path();
            match (p.is_dir(), p.extension().is_some_and(|e| e == "sim")) {
                (true, _) => todo.push(p),
                (false, true) => out.push(p),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Oracle: settle unclosed_* (VACASK rejects a statement whose `(`, `[` or `{` never closes, or
/// whose string a line break ends: "Parser syntax error …", "unexpected newline in string
/// literal"). Mutants of every reader fixture: one line's last closing bracket deleted, or a
/// line's closing quote; each must be refused by the reader (the mutation law's edits never
/// make one: the pair agent's paren deck read Private).
#[test]
fn a_deleted_closing_bracket_or_quote_is_refused() {
    let mut failed = Vec::new();
    let (mut brackets, mut quotes) = (0, 0);
    for deck in all_decks() {
        let text = std::fs::read_to_string(&deck).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let mut embed = false;
        for (k, line) in lines.iter().enumerate().skip(1) {
            let t = line.trim_start();
            // An embed's body (`<<<MARK` … `>>>MARK`) is another language's text.
            if line.contains("<<<") || t.starts_with(">>>") {
                embed = line.contains("<<<");
                continue;
            }
            if embed || t.starts_with("//") || line.contains("/*") || line.contains("*/") {
                continue;
            }
            let quote_count = line.matches('"').count();
            let cut = if quote_count == 0 {
                line.rfind([')', ']', '}']).map(|at| (at, &mut brackets))
            } else if quote_count == 2 && !line.contains('\\') {
                line.rfind('"').map(|at| (at, &mut quotes))
            } else {
                None
            };
            let Some((at, count)) = cut else {
                continue;
            };
            *count += 1;
            let mut mutant: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
            mutant[k].remove(at);
            let mutant = mutant.join("\n") + "\n";
            let (_, diags) = read(&Sources::one("mutant.sim", &mutant), "vacask");
            if !diags.rows().iter().any(|d| dialect::refuses(d.code)) {
                failed.push(format!(
                    "{}:{}: {}",
                    deck.display(),
                    k + 1,
                    mutant.lines().nth(k).unwrap()
                ));
            }
        }
    }
    assert!(
        brackets >= 40 && quotes >= 20,
        "{brackets} bracket and {quotes} quote mutants"
    );
    assert!(
        failed.is_empty(),
        "{} not refused:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
