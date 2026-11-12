//! The VACASK control block (sub-slice S4.3): `options`, analyses, `sweep`, `save`, `alter`,
//! `elaborate`, `mc … endmc`, `print`/`abort`/`postprocess`, `clear`, `var`, `ic=`/`nodeset=`.
//!
//! Oracles, each named by a fixture's header:
//! - reader fixtures (`tests/fixtures/dialect/vacask/read/run/<clause>.sim` + `.ir`): the IR
//!   text written by hand from the VD clause or the run the header names, with the codes its
//!   `# codes:` line lists;
//! - writer fixtures (`…/write/run/<name>.ir` + `.sim`): the text written by hand from the VD
//!   clauses its header names;
//! - the settle decks (`…/settle/run/*.sim`): what VACASK 36be560 printed and wrote, asserted
//!   per deck; and the simulator round trip, `vacask(write(read(d)))` against `vacask(d)`:
//!   the same messages and the same raw files, byte for byte (VACASK on the original is the
//!   oracle);
//! - each reader fixture is a fixpoint of read, write, read, and uses only `FEATURES`.
//!
//! Unsupported fails, never skips: without `vacask` the settle tests fail; run them in
//! `nix develop .#testing`. Known failures are named in `KNOWN` (compared as a list: an
//! unexpected pass fails until its line goes).
//!
//! Contents: tests only.

use std::path::{Path, PathBuf};
use std::process::Command;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;
use spicerack::rawfile::parse_raw;
use spicerack::result::RawData;

/// The reader fixtures.
const READ: &str = "tests/fixtures/dialect/vacask/read/run";
/// The writer fixtures.
const WRITE: &str = "tests/fixtures/dialect/vacask/write/run";
/// The settle decks of this area.
const SETTLE: &str = "tests/fixtures/dialect/vacask/settle/run";

/// Reader fixtures that fail for a cause outside this area, each with its cause; strict both
/// ways.
const KNOWN: &[(&str, &str)] = &[
    (
        "save",
        "A22: Save states its set of analysis kinds (run owner)",
    ),
    ("sweep_op", "A22: DcAxis gains an optional name (run owner)"),
];

/// Settle decks the round trip cannot run yet, each with its cause; strict both ways.
const KNOWN_TRIP: &[(&str, &str)] = &[];

/// One VACASK run: what it printed after its banner, its raw files (each without its clock
/// line, sorted by name) and the directory it ran in.
struct Run {
    out: String,
    raws: Vec<(String, Vec<u8>)>,
    dir: tempfile::TempDir,
}

// ---- private ----

/// The files of `dir` (relative to the crate root) with extension `ext`, sorted.
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

/// A path's file stem.
fn stem(p: &Path) -> String {
    p.file_stem().unwrap().to_string_lossy().into_owned()
}

/// `text` (at `path`) read by `row`, leniently.
fn read(path: &str, text: &str, row: &str) -> (ir::Design, ir::Diags) {
    let row = dialect::by_name(row).expect("a dialect row");
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one(path, text),
        row,
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    (design, diags)
}

/// `text` read as IR text, or `None` when the IR reader panics (a marked fixture states syntax
/// the run owner has not written yet: `KNOWN`).
fn read_ir(text: &str) -> Option<(ir::Design, ir::Diags)> {
    std::panic::catch_unwind(|| read("want.ir", text, "ir")).ok()
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

/// The entry file of `design` written by VACASK's writer, after `legalize`; panics on a
/// refusal.
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
        "writing refused: {:?}",
        diags.rows()
    );
    text
}

/// `deck` run by VACASK in a fresh directory.
fn vacask(deck: &str) -> Run {
    let dir = tempfile::tempdir().expect("a temporary directory");
    std::fs::write(dir.path().join("deck.sim"), deck).unwrap();
    let done = Command::new("vacask")
        .arg("deck.sim")
        .current_dir(dir.path())
        .output()
        .expect("vacask not runnable: run in `nix develop .#testing`");
    let mut text = String::from_utf8_lossy(&done.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&done.stderr));
    let out = text
        .find("OpenBLAS CPUs:")
        .and_then(|k| text[k..].find('\n').map(|e| k + e + 1))
        .map_or(text.clone(), |k| text[k..].to_string());
    let mut raws: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "raw"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let kept: Vec<u8> = String::from_utf8_lossy(&std::fs::read(&p).unwrap())
                .lines()
                .filter(|l| !l.starts_with("Date:"))
                .flat_map(|l| l.bytes().chain(std::iter::once(b'\n')))
                .collect();
            (name, kept)
        })
        .collect();
    raws.sort();
    Run { out, raws, dir }
}

/// Settle deck `name` run by VACASK.
fn settle(name: &str) -> Run {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(SETTLE)
        .join(name);
    vacask(&std::fs::read_to_string(path).expect("the settle deck"))
}

/// What a run printed without where VACASK pointed in the deck (the location line, its echo
/// of the source line and the caret), which move when a statement is spaced otherwise, and
/// without its clock.
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

/// The plot of raw file `name` a run wrote.
fn raw(run: &Run, name: &str) -> RawData {
    let bytes = std::fs::read(run.dir.path().join(name))
        .unwrap_or_else(|_| panic!("no {name}; the run said:\n{}", run.out));
    parse_raw(&bytes).expect("a raw file")
}

/// The names of `plot`'s vectors, in order.
fn vectors(plot: &RawData) -> Vec<String> {
    plot.variables.iter().map(|v| v.name.clone()).collect()
}

/// Column `var` of `plot` at point `k`.
fn value(plot: &RawData, var: &str, k: usize) -> f64 {
    let col = plot
        .variables
        .iter()
        .position(|v| v.name == var)
        .unwrap_or_else(|| panic!("no vector {var}"));
    plot.real_data[col][k]
}

/// Whether `a` is `b` within `tol` relative (absolute below 1).
fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

/// The names of `failed` (fixture names) against `known`'s, as sorted lists.
fn assert_known(failed: &[(String, String)], known: &[(&str, &str)]) {
    let mut got: Vec<&str> = failed.iter().map(|(n, _)| n.as_str()).collect();
    got.sort_unstable();
    let mut want: Vec<&str> = known.iter().map(|(n, _)| *n).collect();
    want.sort_unstable();
    let detail: Vec<String> = failed.iter().map(|(n, d)| format!("{n}:\n{d}")).collect();
    assert_eq!(got, want, "\n{}", detail.join("\n"));
}

// ---- reader fixtures ----

#[test]
fn every_reader_fixture_reads_to_its_ir() {
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let name = stem(&deck);
        let want_text = std::fs::read_to_string(deck.with_extension("ir")).expect("a .ir");
        let Some((want, _)) = read_ir(&want_text) else {
            failed.push((name, "the expected IR text does not read".to_string()));
            continue;
        };
        let text = std::fs::read_to_string(&deck).unwrap();
        let file = deck.file_name().unwrap().to_string_lossy().into_owned();
        let (got, diags) = read(&file, &text, "vacask");
        let mut why = String::new();
        if canonical(&got) != canonical(&want) {
            why += &format!("--- got\n{}--- want\n{}", canonical(&got), canonical(&want));
        }
        if codes(&diags) != expected_codes(&want_text) {
            why += &format!(
                "codes {:?}, want {:?}\n",
                codes(&diags),
                expected_codes(&want_text)
            );
        }
        if !why.is_empty() {
            failed.push((name, why));
        }
    }
    assert_known(&failed, KNOWN);
}

#[test]
fn every_fixtures_ir_text_parses_cleanly() {
    let mut failed = Vec::new();
    for path in files(READ, "ir").into_iter().chain(files(WRITE, "ir")) {
        let text = std::fs::read_to_string(&path).unwrap();
        let Some((_, diags)) = read_ir(&text) else {
            failed.push((stem(&path), "the IR reader panicked".to_string()));
            continue;
        };
        let refused: Vec<_> = diags
            .rows()
            .iter()
            .filter(|d| dialect::refuses(d.code))
            .collect();
        if !refused.is_empty() {
            failed.push((stem(&path), format!("{refused:?}")));
        }
    }
    // The marked fixtures state syntax the run owner has not written yet (KNOWN).
    assert_known(&failed, KNOWN);
}

#[test]
fn every_reader_fixture_is_a_fixpoint_of_write_and_read() {
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let file = deck.file_name().unwrap().to_string_lossy().into_owned();
        let (mut once, _) = read(&file, &std::fs::read_to_string(&deck).unwrap(), "vacask");
        let before = canonical(&once);
        let text = write_entry(&mut once);
        let (twice, _) = read(&file, &text, "vacask");
        if canonical(&twice) != before {
            failed.push((
                stem(&deck),
                format!("written:\n{text}--- again\n{}", canonical(&twice)),
            ));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn every_reader_fixture_uses_only_vacasks_features() {
    let row = dialect::by_name("vacask").expect("the vacask row");
    let features = row.write.expect("a writer").features();
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let file = deck.file_name().unwrap().to_string_lossy().into_owned();
        let (design, _) = read(&file, &std::fs::read_to_string(&deck).unwrap(), "vacask");
        let lacked: Vec<ir::Feature> = ir::features_used(&design)
            .iter()
            .filter(|f| !features.contains(*f))
            .collect();
        if !lacked.is_empty() {
            failed.push((stem(&deck), format!("{lacked:?}")));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

// ---- writer fixtures ----

#[test]
fn every_writer_fixture_writes_its_text() {
    let mut failed = Vec::new();
    for path in files(WRITE, "ir") {
        let want = std::fs::read_to_string(path.with_extension("sim")).expect("a .sim beside");
        let (mut design, _) = read("want.ir", &std::fs::read_to_string(&path).unwrap(), "ir");
        let got = write_entry(&mut design);
        if got != want {
            failed.push((stem(&path), format!("--- got\n{got}--- want\n{want}")));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

// ---- settle decks: VACASK's own facts ----

#[test]
fn ic_next_an_analysis_list_holds_for_that_analysis_only() {
    // Run e1: `ic={"b", 1}` with icmode="uic" starts t1 at 1 V; t2 starts at 0.
    let r = settle("ic_next.sim");
    assert!(
        close(value(&raw(&r, "t1.raw"), "b", 0), 1.0, 1e-12),
        "{}",
        r.out
    );
    assert!(
        close(value(&raw(&r, "t2.raw"), "b", 0), 0.0, 1e-12),
        "{}",
        r.out
    );
}

#[test]
fn save_scope_v_governs_op_and_tran_not_ac() {
    // Run e5: `default` saves every node and every branch current (an inductor's too);
    // `save v(b)` limits op and tran to b, the ac keeps its default; `full` adds nothing here.
    let r = settle("save_scope.sim");
    let all = ["a", "b", "c", "l1:flow(br)", "v1:flow(br)"];
    assert_eq!(vectors(&raw(&r, "opd.raw")), all);
    assert_eq!(vectors(&raw(&r, "opv.raw")), ["b"]);
    assert_eq!(vectors(&raw(&r, "trv.raw")), ["time", "b"]);
    let ac = vectors(&raw(&r, "acv.raw"));
    assert_eq!(ac[0], "frequency");
    assert_eq!(&ac[1..], all);
    assert_eq!(vectors(&raw(&r, "opf.raw")), all);
}

#[test]
fn maxstep0_a_zero_ceiling_is_none() {
    // Run e7: `maxstep=0` writes the same raw file as no maxstep (the span/50 cap of tr1).
    let r = settle("maxstep0.sim");
    let get = |n: &str| r.raws.iter().find(|(k, _)| k == n).expect(n).1.clone();
    assert_eq!(get("tq.raw"), get("tw.raw"));
}

#[test]
fn two_blocks_one_control_block_per_file() {
    let r = settle("two_blocks.sim");
    assert!(
        r.out.contains("Only one control block is allowed."),
        "{}",
        r.out
    );
}

#[test]
fn unknown_option_is_refused() {
    let r = settle("unknown_option.sim");
    assert!(r.out.contains("Parameter 'foo' not found."), "{}", r.out);
}

#[test]
fn sweep_bound_a_sweep_binds_the_next_analysis() {
    let r = settle("sweep_bound.sim");
    assert!(r.out.contains("expecting sweep or analysis"), "{}", r.out);
}

#[test]
fn options_mid_an_option_holds_from_where_it_stands() {
    // `options temp=127` between op1 and op2: r1 = 1k (1 + 1m (127 - 27)) = 1.1k from op2 on.
    let r = settle("options_mid.sim");
    let i = |n: &str| value(&raw(&r, n), "v1:flow(br)", 0).abs();
    assert!(close(i("op1.raw"), 1e-3, 1e-9), "{}", r.out);
    assert!(close(i("op2.raw"), 1.0 / 1100.0, 1e-9), "{}", r.out);
    assert!(close(i("op3.raw"), 1.0 / 1100.0, 1e-9), "{}", r.out);
}

#[test]
fn plan_an_alter_persists() {
    // Run e2: after `alter instance("r2") r=3k` every later op sees the 1k/3k divider.
    let r = settle("plan.sim");
    let b = |n: &str| value(&raw(&r, n), "b", 0);
    assert!(close(b("op1.raw"), 0.5, 1e-9), "{}", r.out);
    assert!(close(b("op2.raw"), 0.75, 1e-9), "{}", r.out);
    assert!(close(b("op3.raw"), 0.75, 1e-9), "{}", r.out);
}

#[test]
fn ac_points_a_lin_count_is_intervals() {
    // VD cmd-analysis-ac: `mode="lin" points=2` is 3 frequencies; dec 10..1k at 2 per decade
    // is 5.
    let r = settle("ac_points.sim");
    assert_eq!(raw(&r, "a1.raw").re(0).len(), 3, "{}", r.out);
    assert_eq!(raw(&r, "a2.raw").re(0).len(), 5, "{}", r.out);
}

// ---- the simulator round trip ----

#[test]
fn every_settle_deck_runs_the_same_after_read_and_write() {
    let mut failed = Vec::new();
    for deck in files(SETTLE, "sim") {
        let text = std::fs::read_to_string(&deck).unwrap();
        let (mut design, diags) = read("deck.sim", &text, "vacask");
        let a = vacask(&text);
        if diags.rows().iter().any(|d| dialect::refuses(d.code)) {
            // A deck the reader refuses is one VACASK rejects.
            if !a.out.contains(") in ") {
                failed.push((stem(&deck), "refused, but VACASK ran it".to_string()));
            }
            continue;
        }
        let again = write_entry(&mut design);
        let b = vacask(&again);
        if messages(&a.out) != messages(&b.out) || a.raws != b.raws {
            failed.push((
                stem(&deck),
                format!(
                    "written:\n{again}--- printed\n{}--- original printed\n{}",
                    b.out, a.out
                ),
            ));
        }
    }
    assert_known(&failed, KNOWN_TRIP);
}
