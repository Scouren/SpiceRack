//! The ngspice run area's fixtures (`tests/fixtures/dialect/ngspice/{read,write}/run/`) and its
//! simulator round trip.
//!
//! - `read/run/<clause>.sp` + `<clause>.ir`: a deck of run cards and the IR it reads to, written
//!   by hand from the oracle its second line names (the ngspice-45 source line, a manual clause,
//!   a run); the `.ir`'s `# codes:` line lists the notes the read gives, in order. A refusal twin
//!   sits beside each card form ngspice rejects.
//! - `write/run/<name>.ir` + `<name>.sp`: a design in IR text and the ngspice text it writes to,
//!   by hand.
//! - Every read fixture is a fixpoint of read → write → read (law L6 on these decks).
//! - The simulator round trip: each deck of [`DECKS`] and its `write(read(deck))` run on
//!   ngspice-45 give the same raw file, byte for byte but the date line (the oracle is ngspice on
//!   the original deck; R1: the writer spells the reader's digits, so the doubles are the same).
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
    "ac",
    "ac_defaults",
    "ac_word_refused",
    "cards_inert",
    "cards_refused",
    "dc",
    "dc_step_zero",
    "disto",
    "four",
    "four_nfreqs",
    "noise",
    "noise_current_refused",
    "op",
    "pss",
    "pz",
    "sens",
    "sp",
    "tf",
    "tran",
    "tran_tmax_zero",
    "tran_word_refused",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &["analyses"];

/// Decks whose raw file ngspice must reproduce after `write(read(deck))`. The element cards are
/// another area's (copied as text while it is a stub); the run cards are this area's, some in
/// upper case so that the written text differs from the deck (ngspice folds case).
const DECKS: &[(&str, &str)] = &[
    (
        "rc_tran",
        "rc tran\nV1 a 0 pulse(0 1 0 1u 1u 5u 10u)\nR1 a b 1k\nC1 b 0 1n\n.TRAN 0.1U 20U 1U 0.2U\n.Tran 1u 30u UIC\n.end\n",
    ),
    (
        "rc_small_signal",
        "rc small signal\nV1 a 0 dc 1 ac 1\nR1 a b 1k\nR2 b 0 2k\nC1 b 0 1u\n.OP\n.DC V1 0 2 0.25 R2 1K 3K 1K\n.dc temp -40 85 25\n.ac dec 5 1 1meg\n.ac lin 4 10 100\n.noise v(b) v1 oct 2 10 1k 2\n.tf v(b) v1\n.sens v(b)\n.pz a 0 b 0 vol pz\n.end\n",
    ),
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
        assert!(
            got.eq_ignoring_spans(&want),
            "{name}: read as\n{}",
            dump(&got)
        );
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
fn a_written_deck_runs_on_ngspice_to_the_original_raw_file() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("dialect_ngspice_run");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    for (name, deck) in DECKS {
        let (design, _) = read_deck(&format!("{name}.sp"), deck);
        let written = write_entry(&design);
        assert_ne!(
            &written, deck,
            "{name}: the run cards are written, not copied"
        );
        let want = raw(&dir, &format!("{name}_original"), deck);
        let got = raw(&dir, &format!("{name}_written"), &written);
        assert!(
            got == want,
            "{name}: the written deck runs to another raw file\n{written}"
        );
    }
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// The fixture directory's `<sub>/run/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir().join(sub).join("run").join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice`.
fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dialect/ngspice")
}

/// The sorted names of the `<sub>/run/*.<ext>` files.
fn names(sub: &str, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir().join(sub).join("run"))
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

/// The entry file of `design` as IR text.
fn dump(design: &ir::Design) -> String {
    let mut out = String::new();
    ir::dump(design, design.entry(), &mut out);
    out
}

/// A design from IR text.
fn parse_ir(text: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{diags:?}");
    design
}

/// The raw file ngspice-45 writes for `deck` (`ngspice -b -r`), without its `Date:` line.
/// Asserts the run exits 0 and writes one.
fn raw(dir: &std::path::Path, stem: &str, deck: &str) -> Vec<u8> {
    let cir = dir.join(format!("{stem}.cir"));
    let out = dir.join(format!("{stem}.raw"));
    std::fs::write(&cir, deck).expect("the deck is written");
    let _ = std::fs::remove_file(&out);
    let run = std::process::Command::new("ngspice")
        .arg("-b")
        .arg("-r")
        .arg(&out)
        .arg(&cir)
        .output()
        .expect("ngspice runs (nix develop .#testing)");
    assert!(
        run.status.success(),
        "{stem}: ngspice failed\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let bytes = std::fs::read(&out).unwrap_or_else(|e| panic!("{stem}: no raw file: {e}"));
    let mut kept = Vec::with_capacity(bytes.len());
    let mut lines = bytes.split_inclusive(|&b| b == b'\n');
    for line in lines.by_ref() {
        if !line.starts_with(b"Date:") {
            kept.extend_from_slice(line);
        }
        if line.starts_with(b"Binary:") {
            break;
        }
    }
    for line in lines {
        kept.extend_from_slice(line);
    }
    kept
}
