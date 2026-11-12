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
//!   ngspice-45 give the same raw file, byte for byte but the date lines (the oracle is ngspice on
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
    "four_gridsize",
    "four_nfreqs",
    "four_script",
    "ic",
    "ic_dotted_net",
    "ic_subckt",
    "interleaved",
    "nodeset",
    "noise",
    "noise_current_refused",
    "op",
    "options",
    "options_form",
    "pss",
    "pss_short",
    "pz",
    "run_in_subckt",
    "sens",
    "sp",
    "state_after_analysis",
    "temp",
    "temp_options",
    "temp_unusable",
    "tf",
    "tnom",
    "tran",
    "tran_tmax_zero",
    "tran_word_refused",
];

/// The write fixtures, by name.
const WRITE: &[&str] = &[
    "analyses",
    "analyses_outputs",
    "interleaved",
    "state",
    "temp_scale",
    "temps",
];

/// The fixtures that fail today, by name, with why. Strict both ways: a listed fixture that
/// passes fails the test until its line goes.
const PENDING: &[(&str, &str)] = &[];

/// Decks whose raw file ngspice must reproduce after `write(read(deck))`. The element cards are
/// another area's (copied as text while it is a stub); the run cards are this area's, some in
/// upper case so that the written text differs from the deck (ngspice folds case).
const DECKS: &[(&str, &str)] = &[
    (
        "rc_tran",
        "rc tran\nV1 a 0 pulse(0 1 0 1u 1u 5u 10u)\nR1 a b 1k\nC1 b 0 1n\n.TRAN 0.1U 20U 1U 0.2U\n.Tran 1u 30u UIC\n.IC V(b)=0.5\n.OPTIONS RELTOL=1e-4 method=gear\n.end\n",
    ),
    (
        "rc_interleaved",
        "rc interleaved\n.OPTIONS RELTOL=1e-4\n.TEMP 50\nV1 a 0 pulse(0 1 0 1u 1u 5u 10u)\n.tran 0.1u 20u\nR1 a b 1k tc1=0.01\n.ic v(b)=0.2\nC1 b 0 1n\n.op\n.end\n",
    ),
    (
        "rc_pss",
        "rc pss\nV1 a 0 dc 0 sin(0 1 1k)\nR1 a b 1k\nC1 b 0 1u\n.PSS 1k 1m b 64 10 50 1e-3\n.end\n",
    ),
    (
        "rc_small_signal",
        "rc small signal\nV1 a 0 dc 1 ac 1\nR1 a b 1k tc1=0.01\nR2 b 0 2k\nC1 b 0 1u\n.NODESET v(b)=0.6\n.options gmin=1e-11 gminfactor=5\n.OP\n.DC V1 0 2 0.25 R2 1K 3K 1K\n.dc temp -40 85 25\n.ac dec 5 1 1meg\n.ac lin 4 10 100\n.noise v(b) v1 oct 2 10 1k 2\n.tf v(b) v1\n.sens v(b)\n.pz a 0 b 0 vol pz\n.TEMP 85\n.options tnom=30\n.end\n",
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
fn each_read_fixture_gives_the_codes_and_the_ir_it_lists() {
    let mut failed = Vec::new();
    for name in READ {
        let (got, diags) = read_deck(&format!("{name}.sp"), &text("read", name, "sp"));
        let codes: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        let ir = text("read", name, "ir");
        let line = ir
            .lines()
            .find_map(|l| l.strip_prefix("# codes:"))
            .expect("a codes line");
        let want: Vec<&str> = line.split_whitespace().collect();
        let same = got.eq_ignoring_spans(&parse_ir(name, &ir));
        if codes != want || !same {
            eprintln!("{name}: codes {codes:?}, read as\n{}", dump(&got));
            failed.push(*name);
        }
    }
    assert_eq!(
        failed,
        pending("read/run"),
        "the failing fixtures are the pending ones"
    );
}

#[test]
fn each_read_fixture_is_a_fixpoint_of_read_write_read() {
    for name in READ {
        if PENDING.iter().any(|(n, _)| n == name) {
            continue; // read as `Unread` until S3.3b: a fixpoint, but of the stub's text
        }
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
    let mut failed = Vec::new();
    for name in WRITE {
        if PENDING.iter().any(|(n, _)| n == name) {
            // a stub's `unreachable!` cannot be caught by name; listed, not run
            failed.push(*name);
            continue;
        }
        let design = parse_ir(name, &text("write", name, "ir"));
        assert_eq!(write_entry(&design), text("write", name, "sp"), "{name}");
    }
    assert_eq!(
        failed,
        pending("write/run"),
        "the failing fixtures are the pending ones"
    );
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
        let at = got.iter().zip(&want).position(|(a, b)| a != b);
        assert!(
            got == want,
            "{name}: the written deck runs to another raw file (lengths {} and {}, first difference at \
             byte {at:?}; both kept in {})\n{written}",
            got.len(),
            want.len(),
            dir.display()
        );
    }
}

/// The names of `PENDING` whose fixture is under `<sub>`, in list order.
fn pending(sub: &str) -> Vec<&'static str> {
    let names = if sub.starts_with("read") { READ } else { WRITE };
    PENDING
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| names.contains(n))
        .collect()
}

#[test]
fn a_node_set_that_holds_from_its_place_on_is_refused() {
    // Oracle: ngspice-45 commands.c (the script commands: no `ic` or `nodeset`; a node's
    // condition is the `.ic`/`.nodeset` card, which holds for the whole run, R24). The law L5's
    // seed 0 wrote one as a card, which read back whole-run.
    for item in [
        "nodeset nodeset every_op value=1",
        "nodeset ic transient node=(b) value=0.5",
    ] {
        let mut design = parse_ir(
            "from_here",
            &format!("file \"f.sp\" deck\n  title \"f\"\n  {item}\n"),
        );
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row(), &mut diags);
        let codes: Vec<String> = diags.rows().iter().map(|r| r.code.to_string()).collect();
        assert_eq!(codes, ["W1303"], "{item}");
    }
}

#[test]
fn a_temp_or_nfreqs_in_another_file_is_not_seen_today() {
    // Oracle: none yet; pins the `ponytail:` limit of `read_run` (`.temp` and `nfreqs` are found
    // in the statement's own file only). ngspice would read `temp=50` as beaten by the library's
    // `.temp` and give `.four` five rows; today the deck's items say otherwise. A change here is
    // the corpus run showing the limit bites: fix the detection, then this test.
    let mut src = Sources::new();
    src.add(
        "deck.sp",
        "deck\n.include \"lib.sp\"\n.options temp=50\n.four 1k v(a)\n",
    );
    src.add("lib.sp", "* lib\n.temp 85\n.options nfreqs=5\n");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, row(), Policy::Lenient, &mut design, &mut diags);
    let top = design.file(design.entry()).top;
    let kinds: Vec<ir::ItemKind> = design.items(top).iter().map(|i| i.kind).collect();
    use ir::ItemKind::{Analysis, Env, Include, Title};
    assert_eq!(kinds, [Title, Include, Env, Analysis]);
    let env: ir::Env = design.row(design.items(top)[2].index);
    assert!(
        !env.temp.is_none(),
        "temp=50 is read as setting the temperature"
    );
    let a: ir::Analysis = design.row(design.items(top)[3].index);
    assert_eq!(a.kind, ir::AnalysisKind::Four);
    let f: ir::tran::Four = design.detail(a.detail);
    let nine = matches!(design.expr(f.harmonics), ir::Expr::Num(n) if design.num(n).value() == 9.0);
    assert!(nine, "the default count, not the library's nfreqs");
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
fn parse_ir(name: &str, text: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{name}: {diags:?}");
    design
}

/// The raw file ngspice-45 writes for `deck` (`ngspice -b -r`), without its `Date:` lines.
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
    // each plot's header has a `Date:` line (the second plot's sits after the first's binary
    // data); a run that crosses a second differs there and only there
    let mut kept = Vec::with_capacity(bytes.len());
    let mut at = 0;
    // Bounded: each step moves `at` forward.
    while at < bytes.len() {
        let line_start = at == 0 || bytes[at - 1] == b'\n';
        if line_start && bytes[at..].starts_with(b"Date:") {
            at += bytes[at..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len() - at, |k| k + 1);
            continue;
        }
        kept.push(bytes[at]);
        at += 1;
    }
    kept
}
