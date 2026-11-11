//! The zero gates' checked-in lists. Z1: `tests/fixtures/zero/untranslated.txt`,
//! every golden output that carries `W0001` (a statement kept as another
//! dialect's text) or `W0032` (text that did not parse), by name.
//!
//! The list is generated from a golden record and compared with it, strict
//! both ways, by `golden zero <record> tests/fixtures/zero/untranslated.txt`
//! (`src/bin/golden/zero.rs`; TESTING.md, "Golden gate"), which needs the
//! corpora. Here, in the merge tier: the checked-in list is in the form that
//! command writes (`<set>/<relpath> <target>`, sorted by bytes, each name
//! once), so a line edited by hand into another shape fails before a record
//! is taken.
//!
//! Cites: ZERO-LEDGER P1-029 (gate Z1).
//!
//! Contents: none (tests only).

use spicerack::syntax::Dialect;

// ---- private ----

/// A checked-in list of `tests/fixtures/zero/`, as text.
fn list(name: &str) -> String {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/zero");
    let path = std::path::Path::new(dir).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn p1_029_the_z1_list_is_sorted_names() {
    // Oracle: the list's format as `golden zero <record>` prints it (a spec
    // example: `<set>/<relpath> <target>`, the target a dialect's name).
    let text = list("untranslated.txt");
    let targets: Vec<&str> = Dialect::ALL.iter().map(|d| d.name()).collect();
    let lines: Vec<&str> = text.lines().collect();
    for &l in &lines {
        let (input, target) = l.rsplit_once(' ').unwrap_or((l, ""));
        assert!(targets.contains(&target), "no target in `{l}`");
        assert!(input.contains('/'), "no <set>/<relpath> in `{l}`");
    }
    let unsorted = lines.windows(2).find(|w| w[0] >= w[1]);
    assert_eq!(unsorted, None, "sorted by bytes, each name once");
}
