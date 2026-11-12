//! The ngspice dialect's fixtures (`tests/fixtures/dialect/ngspice/`) and its simulator check.
//!
//! - `read/<clause>.sp` + `<clause>.ir`: a deck and the IR it reads to, written by hand from the
//!   oracle its second line names (an INV row, a manual clause, the ngspice-45 source, a run); the
//!   `.ir`'s `# codes:` line lists the notes the read gives, in order. A refusal twin sits beside
//!   each clause that has an invalid form.
//! - `write/<name>.ir` + `<name>.sp`: a design in IR text and the ngspice text it writes to, by
//!   hand.
//! - Every read fixture is a fixpoint of read → write → read (law L6 on these decks).
//! - The written text of a deck of expressions runs on ngspice-45 and prints the values ngspice
//!   prints for the original (L9; ngspice is the oracle of its own dialect). ngspice comes from
//!   `nix develop .#testing`; a missing simulator fails.
//!
//! Fixture lists are compared by name, never by count.
//!
//! Contents: tests only.

use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The read fixtures, by name.
const READ: &[&str] = &[
    "expr_ops",
    "expr_power",
    "expr_ternary",
    "expr_ternary_deep",
    "func",
    "funcs_a11",
    "global_gnd",
    "global_ground_only",
    "if_branch_param_refused",
    "if_chain",
    "include_lib",
    "limit_three",
    "negation_refused",
    "numbers",
    "param_last_wins",
    "param_reads_circuit",
    "param_temp_class",
    "subckt_nested",
    "title_comments_end",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &["expr_parens", "numbers", "structure"];

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
fn each_read_fixture_reads_to_its_ir() {
    for name in READ {
        let (got, _) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let want = parse_ir(&text("read", name, "ir"));
        assert!(got.eq_ignoring_spans(&want), "{name}");
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
fn written_expressions_run_on_ngspice_to_the_original_values() {
    // Oracle: ngspice-45 itself; the values are the ones it printed for these texts (runs of
    // the S3.0 report) and closed forms of ngspice's rules (`xpressn.c`).
    let deck = "sim\n.param a={2**3**2} b={-2**2} c={2*-2**2} d={pow(0-2,3)} e={(-7)\\2}\n.param f={1?0?4:8:16} g={0==1<2} h={nint(2.5)} k={7%3} m={2mil*1k}\n.param n={1?0:1?3:4} p={-7%3} q={int(-1.5)} r={!0*5} s={1e3k/1meg}\nV1 n1 0 {a}\nV2 n2 0 {b}\nV3 n3 0 {c}\nV4 n4 0 {d}\nV5 n5 0 {e}\nV6 n6 0 {f}\nV7 n7 0 {g}\nV8 n8 0 {h}\nV9 n9 0 {k}\nV10 n10 0 {m}\nV11 n11 0 {n}\nV12 n12 0 {p}\nV13 n13 0 {q}\nV14 n14 0 {r}\nV15 n15 0 {s}\n.control\nop\nprint v(n1) v(n2) v(n3) v(n4) v(n5) v(n6) v(n7) v(n8) v(n9) v(n10) v(n11) v(n12) v(n13) v(n14) v(n15)\n.endc\n.end\n";
    let want = [
        64.0, -4.0, 8.0, -8.0, 3.0, 4.0, 1.0, 2.0, 1.0, 2.0, 0.0, -1.0, -1.0, 5.0, 1.0,
    ];
    let (design, _) = read_deck("sim.sp", deck);
    let written = write_entry(&design);
    assert_ne!(
        written, deck,
        "the parameters are written by the writer, not copied"
    );
    for (k, v) in ngspice_op(&written, want.len()).iter().enumerate() {
        let w = want[k];
        assert!(
            (v - w).abs() <= 1e-6 * w.abs().max(1.0),
            "v(n{}) = {v}, want {w}",
            k + 1
        );
    }
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

/// The fixture directory's `<sub>/<name>.<ext>`.
fn text(sub: &str, name: &str, ext: &str) -> String {
    let p = dir().join(sub).join(format!("{name}.{ext}"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `tests/fixtures/dialect/ngspice`.
fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dialect/ngspice")
}

/// The sorted names of the `<sub>/*.<ext>` files.
fn names(sub: &str, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir().join(sub))
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

/// The `print v(n1) … v(nk)` values of `deck` run by `ngspice -b`.
fn ngspice_op(deck: &str, k: usize) -> Vec<f64> {
    let dir = tempfile::tempdir().expect("a temp dir");
    let path = dir.path().join("deck.cir");
    std::fs::write(&path, deck).expect("the deck is written");
    let out = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&path)
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let values: Vec<f64> = (1..=k)
        .map(|n| {
            let key = format!("v(n{n}) = ");
            let line = stdout
                .lines()
                .find_map(|l| l.trim().strip_prefix(&key))
                .unwrap_or_else(|| panic!("no {key} in ngspice's output:\n{stdout}"));
            line.trim().parse().expect("a number")
        })
        .collect();
    values
}
