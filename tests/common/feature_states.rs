//! Law L7's states of a lacked feature (R57, A34), apart from `tests/dialect_laws.rs` for its
//! 500-line cap: for a row with a writer and each feature of the full set (the IR text's) the
//! writer lacks, it is legal (`passes::legal` over every registered pass: a pass whose
//! `requires` the target cannot meet does not count), cited (`Write::citations`), or missing.
//! A missing one is **owed** when another row of `BUILTIN` with a reader holds it in its
//! `FEATURES` (a design read from that dialect carries it here: a pass or a citation is owed
//! now), **unspoken** when none does (it waits for the dialect that will; the IR text row,
//! which holds everything, is no speaker). The missing ones and their states are compared with
//! the list in [`KNOWN_MISSING`], strictly both ways, so a feature that becomes owed the day a
//! new reader claims it fails the list until someone looks.
//!
//! Contents: `pub const KNOWN_MISSING: &str`,
//! `pub fn known_missing() -> Vec<(String, String, String)>`,
//! `pub fn missing(&DialectRow, &dyn Write) -> Vec<(String, String)>`,
//! `pub fn stale(&DialectRow, &dyn Write) -> Vec<String>`

#![allow(dead_code)]

use spicerack::dialect::{BUILTIN, DialectRow, Write, irtext::IrText};
use spicerack::passes::{self, Pass};

/// The L7 missing list's file, relative to the crate root: `<row> <feature> <state>` a line
/// (the feature as `Debug` prints it, spaces dropped; the state `owed from=<rows>` or
/// `unspoken`), `#` comments (the lead's sorting: the rule or citation owed, its owner).
pub const KNOWN_MISSING: &str = "tests/fixtures/dialect/l7_missing.txt";

/// The extensions of a settle deck named as evidence: such an evidence is a path relative to
/// `tests/fixtures/dialect/<row>/` that must exist.
const DECKS: &[&str] = &[".sp", ".cir", ".sim", ".va"];

/// The lacked features neither legal nor cited, as recorded: (row, feature, state), read from
/// [`KNOWN_MISSING`] (a fixture file: two dialects' lists outgrow a source file).
pub fn known_missing() -> Vec<(String, String, String)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(KNOWN_MISSING);
    let text = std::fs::read_to_string(&path).expect("the L7 missing list");
    text.lines()
        .map(|l| l.split('#').next().unwrap_or_default().trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut w = l.splitn(3, ' ');
            let row = w.next().expect("a row").to_string();
            let feature = w.next().expect("a feature").to_string();
            let state = w.next().expect("a state (`owed from=…` or `unspoken`)");
            (row, feature, state.trim().to_string())
        })
        .collect()
}

/// The features `writer` lacks that no registered pass makes legal and it does not cite, in
/// feature order: (feature as `Debug` prints it, spaces dropped; its state).
pub fn missing(row: &DialectRow, writer: &dyn Write) -> Vec<(String, String)> {
    let all: Vec<&Pass> = passes::all().collect();
    let legal = passes::legal(&all, writer.features());
    let full = *IrText.features();
    // the speakers: every other row with a reader, the IR text (it holds everything) aside
    let speakers: Vec<(&str, &dyn Write)> = BUILTIN
        .iter()
        .filter(|r| r.name != row.name && r.read.is_some())
        .filter_map(|r| r.write.map(|w| (r.name, w)))
        .filter(|(_, w)| w.features() != IrText.features())
        .collect();
    full.minus(legal)
        .iter()
        .filter(|&f| !writer.citations().iter().any(|c| c.feature == f))
        .map(|f| {
            let from: Vec<&str> = speakers
                .iter()
                .filter(|(_, w)| w.features().contains(f))
                .map(|(name, _)| *name)
                .collect();
            let state = match from.is_empty() {
                true => "unspoken".to_string(),
                false => format!("owed from={}", from.join(",")),
            };
            (format!("{f:?}").replace(' ', ""), state)
        })
        .collect()
}

/// Every citation of `writer` that is wrong: of a feature it has, of a feature a pass makes
/// legal, with no evidence, or naming a settle deck that does not exist.
pub fn stale(row: &DialectRow, writer: &dyn Write) -> Vec<String> {
    let all: Vec<&Pass> = passes::all().collect();
    let has = *writer.features();
    let legal = passes::legal(&all, &has);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect")
        .join(row.name);
    let mut out = Vec::new();
    for c in writer.citations() {
        let f = c.feature;
        if has.contains(f) {
            out.push(format!("{f:?}: cited, and the writer has it"));
        } else if legal.contains(f) {
            out.push(format!("{f:?}: cited, and a pass makes it legal"));
        }
        if c.evidence.trim().is_empty() {
            out.push(format!("{f:?}: cited with no evidence"));
        } else if DECKS.iter().any(|x| c.evidence.ends_with(x)) && !root.join(c.evidence).exists() {
            out.push(format!(
                "{f:?}: its settle deck {} does not exist",
                c.evidence
            ));
        }
    }
    out
}

// ---- private ----

/// L7's missing states (R57, A34): per row with a writer, the lacked features no registered
/// pass makes legal and the row does not cite, each owed or unspoken, against
/// [`KNOWN_MISSING`] by (row, feature, state), strictly both ways. The list is printed on every
/// run: its owed rows are the pair's pass-and-citation work list.
#[test]
fn l7_missing_features_are_the_known_list() {
    let mut got: Vec<(String, String, String)> = Vec::new();
    for row in BUILTIN {
        let Some(writer) = row.write else { continue };
        for (f, state) in missing(row, writer) {
            got.push((row.name.to_string(), f, state));
        }
    }
    for (row, f, state) in &got {
        eprintln!("l7 missing: {row} {f} {state}");
    }
    let known = known_missing();
    assert_eq!(got, known, "l7: missing features against {KNOWN_MISSING}");
}
