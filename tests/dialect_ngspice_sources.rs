//! The ngspice sources area's fixtures (`tests/fixtures/dialect/ngspice/{read,write}/sources/`)
//! and its simulator round trip.
//!
//! - `read/sources/<clause>.sp` + `<clause>.ir`: a deck of `V`/`I` cards and the IR it reads to,
//!   written by hand from the oracle its second line names (the ngspice-45 source line, a run,
//!   a ruling); the `.ir`'s `# codes:` line lists the notes the read gives, in order. The cards
//!   ngspice rejects are `invalid`, the ones the IR cannot state yet `unread`.
//! - `write/sources/<name>.ir` + `<name>.sp`: a design in IR text and the ngspice text it writes
//!   to, by hand.
//! - Every read fixture is a fixpoint of read → write → read (law L6 on these decks).
//! - The simulator round trip: each deck of [`SIM`] and its `write(read(deck))` print the same
//!   values on ngspice-45 (the oracle is ngspice on the original deck; R1: the writer spells the
//!   reader's digits, so the doubles are the same and the printed lines equal). The values the
//!   rulings rest on are pinned in [`PINNED`] (R36: the negative-delay SFFM, TRRANDOM's p2).
//!   ngspice comes from `nix develop .#testing`; a missing simulator fails.
//!
//! Fixture lists are compared by name, never by count.
//!
//! Contents: tests only.

use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The read fixtures, by name.
const READ: &[&str] = &[
    "ac_forms",
    "am",
    "dc_forms",
    "distof_port",
    "exp",
    "invalid",
    "noise",
    "notes",
    "pulse",
    "pwl",
    "sffm",
    "sin",
    "unread",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &["waves"];

/// Read fixtures that run on ngspice and print values.
const SIM: &[&str] = &[
    "ac_forms",
    "am",
    "dc_forms",
    "distof_port",
    "exp",
    "noise",
    "pulse",
    "pwl",
    "sffm",
    "sin",
];

/// Values ngspice-45 prints for the original decks, pinned (fixture, printed line).
const PINNED: &[(&str, &str)] = &[
    ("sffm", "v(a) = 0.000000e+00"),
    ("sffm", "v(b) = 5.000000e-01"),
    ("sffm", "v(c) = -1.34147e+00"),
    ("noise", "v(a) = 5.000000e-01"),
    ("noise", "v(b) = 1.000000e+00"),
    ("noise", "v(c) = 0.000000e+00"),
    ("sin", "v(a) = 1.000000e+00"),
    ("dc_forms", "v(d) = 1.000000e+00"),
    ("dc_forms", "v(f) = 0.000000e+00"),
    ("am", "v(a) = 0.000000e+00"),
    ("distof_port", "v(b) = 9.900990e-03"),
];

// ---- private ----

#[test]
fn the_fixture_lists_are_the_files() {
    assert_eq!(names("read", "sp"), READ);
    assert_eq!(names("read", "ir"), READ);
    assert_eq!(names("write", "ir"), WRITE);
    assert_eq!(names("write", "sp"), WRITE);
}

#[test]
fn each_read_fixture_gives_the_codes_its_ir_lists() {
    for name in READ {
        let (_, diags) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let got: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        let ir = text("read", name, "ir");
        let line = ir
            .lines()
            .find_map(|l| l.strip_prefix("# codes:"))
            .expect("a codes line");
        let want: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn each_read_fixture_reads_to_its_ir() {
    for name in READ {
        let (got, _) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let want = parse_ir(&text("read", name, "ir"));
        let mut dumped = String::new();
        ir::dump(&got, got.entry(), &mut dumped);
        assert!(got.eq_ignoring_spans(&want), "{name}: read gives\n{dumped}");
    }
}

#[test]
fn each_read_fixture_is_a_fixpoint_of_read_write_read() {
    for name in READ {
        let path = format!("{name}.sp");
        let (first, _) = read_deck(&path, &text("read", name, "sp"));
        let once = write_entry(&first);
        let (second, _) = read_deck(&path, &once);
        assert!(first.eq_ignoring_spans(&second), "{name}:\n{once}");
        assert_eq!(write_entry(&second), once, "{name}: the text is a fixpoint");
    }
}

#[test]
fn each_write_fixture_writes_to_its_text() {
    for name in WRITE {
        let design = parse_ir(&text("write", name, "ir"));
        assert_eq!(write_entry(&design), text("write", name, "sp"), "{name}");
    }
}

#[test]
fn written_sources_print_what_ngspice_prints_for_the_original() {
    for name in SIM {
        let deck = text("read", name, "sp");
        let (design, _) = read_deck(&format!("{name}.sp"), &deck);
        let written = write_entry(&design);
        let (want, got) = (printed(&deck), printed(&written));
        assert!(!want.is_empty(), "{name}: the deck prints");
        assert_eq!(got, want, "{name}:\n{written}");
    }
}

#[test]
fn ngspice_prints_the_pinned_values() {
    for (name, line) in PINNED {
        let lines = printed(&text("read", name, "sp"));
        assert!(
            lines.iter().any(|l| l == line),
            "{name}: {line} in {lines:?}"
        );
    }
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// The fixture directory's `<sub>/sources/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir(sub).join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice/<sub>/sources`.
fn dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect/ngspice")
        .join(sub)
        .join("sources")
}

/// The sorted names of the `<sub>/sources/*.<ext>` files.
fn names(sub: &str, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir(sub))
        .expect("the fixture directory")
        .filter_map(|e| {
            let p = e.expect("an entry").path();
            (p.extension()? == ext)
                .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))?
        })
        .collect();
    out.sort();
    out
}

/// One deck read by the ngspice row, leniently; no `X` row (the spine checks).
fn read_deck(path: &str, text: &str) -> (ir::Design, ir::Diags) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    let src = Sources::one(path, text);
    dialect::read(&src, row(), Policy::Lenient, &mut design, &mut diags);
    assert!(
        !diags.rows().iter().any(|d| d.code.letter == b'X'),
        "{path}: {diags:?}"
    );
    (design, diags)
}

/// The entry file written by the ngspice row; no refusal.
fn write_entry(design: &ir::Design) -> String {
    let mut diags = ir::Diags::new();
    let text = dialect::write(design, design.entry(), row(), &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    text
}

/// A design from IR text.
fn parse_ir(text: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    design
}

/// The `name = value` lines `deck` prints when run by `ngspice -b`.
fn printed(deck: &str) -> Vec<String> {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("deck.cir");
    std::fs::write(&path, deck).expect("the deck is written");
    let out = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&path)
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains(" = "))
        .map(|l| l.trim().to_string())
        .collect()
}
