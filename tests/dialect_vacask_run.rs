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
//! Contents: tests only (`settle`, the settle decks' facts; `save_plans`, the seeded save
//! property).

#[path = "common/memcap.rs"]
mod memcap;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "dialect_vacask_run/currents.rs"]
mod currents;
#[path = "dialect_vacask_run/save_plans.rs"]
mod save_plans;
#[path = "dialect_vacask_run/settle.rs"]
mod settle;

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
const KNOWN: &[(&str, &str)] = &[];

/// Writer fixtures that fail for a cause outside this area, each with its cause; strict both
/// ways.
const KNOWN_WRITE: &[(&str, &str)] = &[];

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
        let Ok((got, diags)) = std::panic::catch_unwind(|| read(&file, &text, "vacask")) else {
            failed.push((name, "the reader panicked".to_string()));
            continue;
        };
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
    assert_known(&failed, &[]);
}

#[test]
fn every_reader_fixture_is_a_fixpoint_of_write_and_read() {
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let file = deck.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&deck).unwrap();
        let Ok((mut once, _)) = std::panic::catch_unwind(|| read(&file, &text, "vacask")) else {
            failed.push((stem(&deck), "the reader panicked".to_string()));
            continue;
        };
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
    assert_known(&failed, KNOWN);
}

#[test]
fn every_reader_fixture_uses_only_vacasks_features() {
    let row = dialect::by_name("vacask").expect("the vacask row");
    let features = row.write.expect("a writer").features();
    let mut failed = Vec::new();
    for deck in files(READ, "sim") {
        let file = deck.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&deck).unwrap();
        let Ok((design, _)) = std::panic::catch_unwind(|| read(&file, &text, "vacask")) else {
            failed.push((stem(&deck), "the reader panicked".to_string()));
            continue;
        };
        let lacked: Vec<ir::Feature> = ir::features_used(&design)
            .iter()
            .filter(|f| !features.contains(*f))
            .collect();
        if !lacked.is_empty() {
            failed.push((stem(&deck), format!("{lacked:?}")));
        }
    }
    assert_known(&failed, KNOWN);
}

// ---- writer fixtures ----

#[test]
fn every_writer_fixture_writes_its_text() {
    let mut failed = Vec::new();
    for path in files(WRITE, "ir") {
        let want = std::fs::read_to_string(path.with_extension("sim")).expect("a .sim beside");
        let (mut design, _) = read("want.ir", &std::fs::read_to_string(&path).unwrap(), "ir");
        match catch_unwind(AssertUnwindSafe(|| write_entry(&mut design))) {
            Ok(got) if got == want => {}
            Ok(got) => failed.push((stem(&path), format!("--- got\n{got}--- want\n{want}"))),
            Err(_) => failed.push((stem(&path), "legalize or write refused".to_string())),
        }
    }
    let names: Vec<&str> = failed.iter().map(|f| f.0.as_str()).collect();
    let known: Vec<&str> = KNOWN_WRITE.iter().map(|k| k.0).collect();
    assert_eq!(names, known, "{failed:#?}");
}

// ---- the simulator round trip ----

#[test]
fn every_settle_deck_runs_the_same_after_read_and_write() {
    let mut failed = Vec::new();
    for deck in files(SETTLE, "sim") {
        let text = std::fs::read_to_string(&deck).unwrap();
        let Ok((mut design, diags)) =
            std::panic::catch_unwind(|| read("deck.sim", &text, "vacask"))
        else {
            failed.push((stem(&deck), "the reader panicked".to_string()));
            continue;
        };
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

// ---- the writer's refusals ----

#[test]
fn a_sweep_of_a_waveform_sources_dc_is_refused_and_a_dc_sources_is_not() {
    // Oracle: settle run/sweep_source's negative row (VACASK leaves a pulse source at its wave
    // while its `dc` is swept: e = 0 0 where ngspice-45 gives 2 4), so the writer refuses the
    // source-DC target of a source whose DC is its wave at zero (W2307), on a DC axis and on a
    // loop; a dc source and R42's fold (given DC beside a wave) are swept as written.
    let head = r#"file "w.ir" deck
  title "swept sources"
  model vsource source_v
"#;
    let pulse = "wave=pulse(v1=0i v2=1i delay=0 rise=1u fall=0 width=1u period=0 law=vacask)";
    let cases: [(String, &[&str]); 2] = [
        (
            format!(
                "  element vsource v1 (a 0) model=vsource dc=1i
  element vsource v3 (e 0) model=vsource {pulse} dc_rule=wave_at_zero
  analysis dc name=d1 (source_dc v1) (list values=[1i, 2i]) name=s1
  analysis dc name=d2 (source_dc v3) (list values=[1i, 2i]) name=s2
  loop sweep name=s4 (source_dc v3) (list values=[1i, 2i])
    analysis tran name=t1 stop=1m cap=span_over50
"
            ),
            &["W2307 d2", "W2307 s4"],
        ),
        // R42's fold: the line is type dc during the DC analyses, so the sweep moves it.
        (
            format!(
                "  element vsource v4 (f 0) model=vsource dc=2i {pulse}
  analysis dc name=d3 (source_dc v4) (list values=[1i, 2i]) name=s3
  analysis tran name=t2 stop=1m cap=span_over50
"
            ),
            &[],
        ),
    ];
    let row = dialect::by_name("vacask").expect("the vacask row");
    for (body, want) in cases {
        let (mut design, _) = read("w.ir", &format!("{head}{body}"), "ir");
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row, &mut diags);
        let mut refused: Vec<String> = diags
            .rows()
            .iter()
            .filter(|d| dialect::refuses(d.code))
            .map(|d| {
                let at = d.item;
                assert_ne!(at, ir::ItemRef::NONE, "a refusal at its item");
                let name = match at.kind {
                    ir::ItemKind::Analysis => design.row::<ir::Analysis>(at.index).name,
                    ir::ItemKind::Loop => design.row::<ir::Loop>(at.index).name,
                    _ => ir::NameId::NONE,
                };
                let name = if name.is_none() {
                    "?"
                } else {
                    design.str(name)
                };
                format!("{}{:04} {name}", d.code.letter as char, d.code.number)
            })
            .collect();
        refused.sort();
        assert_eq!(refused, want, "{body}");
    }
}

#[test]
fn a_given_print_step_never_reaches_the_writer() {
    // R67, A41: VACASK has no output interval (run: `outstep=`, `tstep=` are "Parameter … not
    // found."), so `FEATURES` lacks `TranPrintStep` and `legalize` refuses a transient that
    // gives one (until the run group's rule drops it with its note); the writer asserts it.
    let text = "file \"p.ir\" deck\n  title \"print step\"\n  analysis tran name=t1 stop=40n print_step=0.1n max_step=1n\n";
    let (mut design, _) = read("p.ir", text, "ir");
    let row = dialect::by_name("vacask").expect("the vacask row");
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut design, row, &mut diags);
    let want = ir::Feature::Run(ir::RunExtra::TranPrintStep);
    let converted = !ir::features_used(&design).contains(want);
    let refused = diags
        .rows()
        .iter()
        .any(|d| dialect::refuses(d.code) && ir::Feature::from_index(d.arg as u16) == want);
    assert!(converted || refused, "{:?}", diags.rows());
}
