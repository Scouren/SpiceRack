//! The pair test (`rewrite/SPINE.md` §8a): a deck in dialect A read by A's reader, legalized
//! and written for B through the spine, run on A's simulator and B's, and compared. One test
//! per cell, a (source, target) pair:
//!
//! | cell | source runs on | target runs on |
//! |---|---|---|
//! | `ngspice-vacask` | ngspice-45 (`-b -r`, R49) | VACASK |
//! | `vacask-ngspice` | VACASK | ngspice-45 |
//!
//! Oracles per deck, in this order; the first that fails names the component (REWRITE-PLAN
//! §7.4): 1 read, strict (no refusal, `ir::check` clean, no `Unread`) → the source reader;
//! 2 `legalize` (no refusal) → a pass or the target's `FEATURES`; 3 the written text read back
//! by the target's reader means the legalized design (`eq_meaning` in normal form) → the
//! target dialect; 4 the numbers: every vector of every plot of the source simulator's run on
//! the source deck against the target simulator's on the written text, at `common/cmp.rs`'s
//! tolerances → the target writer, or the simulators; 5 the names: every source vector has
//! its counterpart in the target's run under the rename map (`name_with_letter`'s U0311
//! notes) → the writer or the pass that renamed; 6 the written bytes equal under 1 and 8
//! jobs → whatever holds state across conversions. A step whose input exists runs after a
//! failure (3 does not stop 4: a wrong number behind a known step-3 failure stays visible);
//! a refusal stops the deck (no text).
//!
//! A deck is `tests/fixtures/pair/<cell>/<deck>.<ext>` (`.sp` ngspice, `.sim` VACASK), with
//! an optional `<deck>.pair` beside it: `key = value` a line, `#` comments; `owner`,
//! `proves`, the tightenings `reltol`, `vntol`, `abstol`, `tube` (a looser value fails the
//! cell: `cmp::Tol::tighten`), and `vectors = <name> …` (compare these only). The failing
//! steps are listed in `tests/fixtures/pair/<cell>.txt`, `<deck> <step> <class> <proof>`
//! (`#` to the end a comment), compared by (deck, step) both ways: an unlisted failure fails,
//! and so does a listed step that passes or fails in another class. A cell whose rows are
//! not both in `dialect::BUILTIN` with the sides it needs, a missing simulator, a cell with
//! no deck: each fails, never skips. Every simulator run has a CPU and a wall-clock budget
//! (`sim::run`).
//!
//!   nix develop .#testing -c cargo test --test test_pair
//!
//! `CROSS_SIM_KEEP=1` keeps every run's directory and prints where (`sim::run`).
//!
//! Cites: `rewrite/SPINE.md` §8a; REWRITE-PLAN §7.4; R49 (an ngspice deck means `ngspice -b
//! -r`); AMEND-A12 A32 (the rename map is notes).
//!
//! Contents: none (tests only).

#[path = "common/cmp.rs"]
mod cmp;
#[allow(dead_code)]
#[path = "common/laws.rs"]
mod laws;
#[allow(dead_code)]
#[path = "common/sim.rs"]
mod sim;
#[allow(dead_code)]
#[path = "test_import_suite/tube.rs"]
mod tube;
#[allow(dead_code)]
#[path = "test_cross_sim/vacask.rs"]
mod vacask;

#[path = "test_pair/files.rs"]
mod files;
#[path = "test_pair/plots.rs"]
mod plots;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;
use spicerack::ir::ItemRow as _;

use files::PairFile;
use sim::{Ran, Sim};

/// One (source, target) pair: the dialect rows by name, the simulator each runs on, and the
/// extension of each dialect's text.
struct Cell {
    name: &'static str,
    from: &'static str,
    to: &'static str,
    from_sim: Sim,
    to_sim: Sim,
    from_ext: &'static str,
    to_ext: &'static str,
}

/// What the spine made of one deck: the legalized design, the written files (the entry
/// first), the rename map (new name → source name, lower case) and the names of its devices
/// (every element but a subcircuit instance, under both names, lower case).
struct Spine {
    legal: ir::Design,
    files: Vec<(String, String)>,
    renames: HashMap<String, String>,
    devices: BTreeSet<String>,
}

/// A failed step: its class (what the expected list names) and what was seen.
struct Fail {
    class: String,
    said: String,
}

/// A deck's written files, the entry first: (path, text).
type Written = Vec<(String, String)>;

/// The cells, `ngspice-vacask` and `vacask-ngspice` first (SPINE §8a).
const CELLS: &[Cell] = &[
    Cell {
        name: "ngspice-vacask",
        from: "ngspice",
        to: "vacask",
        from_sim: Sim::Ngspice,
        to_sim: Sim::Vacask,
        from_ext: "sp",
        to_ext: "sim",
    },
    Cell {
        name: "vacask-ngspice",
        from: "vacask",
        to: "ngspice",
        from_sim: Sim::Vacask,
        to_sim: Sim::Ngspice,
        from_ext: "sim",
        to_ext: "sp",
    },
];

/// CPU seconds per simulator run (the wall clock gets ten times as much: `sim::run`).
const SECS: u64 = 60;

/// The jobs step 6 converts the cell's decks on, against one.
const JOBS: usize = 8;

/// `name_with_letter`'s note at a renamed element, `arg` the old name (the pass's `RENAMED`,
/// `src/passes/structure/name_with_letter.rs`; the passes are not public items).
const RENAMED: ir::Code = ir::Code::new(b'U', 311);

// ---- private ----

#[test]
fn ngspice_to_vacask() {
    cell(&CELLS[0]);
}

#[test]
fn vacask_to_ngspice() {
    cell(&CELLS[1]);
}

#[test]
fn a_pair_file_tightens_and_never_loosens() {
    // Oracle: the defaults of `cmp.rs` (RELTOL 1e-3, VNTOL 1e-6, ABSTOL 1e-12, tube 0.01),
    // written out by hand. Checks: 5.
    let ok = files::pair_text("owner = me\nreltol = 1e-4 # tighter\nvectors = v(out) in\n");
    let ok = ok.expect("a tightening reads");
    assert_eq!(ok.tightened, [("reltol".to_string(), 1e-4)]);
    let names: Vec<&str> = ok.vectors.iter().flatten().map(String::as_str).collect();
    assert_eq!(names, ["in", "out"]);
    assert!(
        files::pair_text("reltol = 1e-2\n").is_err(),
        "a looser reltol"
    );
    assert!(files::pair_text("tube = 0.05\n").is_err(), "a wider tube");
    assert!(files::pair_text("speed = 2\n").is_err(), "an unknown key");
}

#[test]
fn a_cell_names_rows_with_the_sides_it_needs() {
    // Oracle: `dialect::BUILTIN` as `src/dialect/mod.rs` lists it. Checks: one per cell.
    for c in CELLS {
        rows(c);
    }
}

/// Every deck of `c` through the six steps, the failures compared with the cell's list.
fn cell(c: &Cell) {
    let (from, to) = rows(c);
    for s in [c.from_sim, c.to_sim] {
        assert!(
            s.on_path(),
            "{}: {} not on PATH: run in `nix develop .#testing`",
            c.name,
            s.name()
        );
    }
    let decks = decks(c);
    assert!(!decks.is_empty(), "{}: a cell with no deck", c.name);
    let mut seen: BTreeMap<(String, u8), Fail> = BTreeMap::new();
    let mut written: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for deck in &decks {
        let name = stem(deck);
        let text = std::fs::read_to_string(deck).expect("a deck");
        let pair =
            files::pair_file(deck).unwrap_or_else(|e| panic!("{}: {name}.pair: {e}", c.name));
        let (fails, last) = steps(c, from, to, deck, &text, &pair, &mut written);
        // A step with no input (no design, no text, no plots) did not run: `-`. Step 6 runs on
        // every deck that was written.
        let ran = |s: u8| s <= last || (s == 6 && written.contains_key(&name));
        let line: Vec<String> = (1..=6u8)
            .map(|s| match fails.get(&s) {
                Some(f) => format!("{s}:{}", f.class),
                None if ran(s) => format!("{s}:ok"),
                None => format!("{s}:-"),
            })
            .collect();
        eprintln!("{} {name}: {}", c.name, line.join(" "));
        for (s, f) in fails {
            seen.insert((name.clone(), s), f);
        }
    }
    for (name, why) in determinism(c, from, to, &decks, &written) {
        let said = why.clone();
        seen.insert((name, 6), fail("nondeterministic", said));
    }
    let listed = files::listed(c.name);
    let mut wrong = Vec::new();
    for ((deck, step), f) in &seen {
        match listed.get(&(deck.clone(), *step)) {
            Some(class) if *class == f.class => {}
            Some(class) => wrong.push(format!(
                "{deck} {step}: listed {class}, fails {}: {}",
                f.class, f.said
            )),
            None => wrong.push(format!("{deck} {step} {}: {}", f.class, f.said)),
        }
    }
    for ((deck, step), class) in &listed {
        if !seen.contains_key(&(deck.clone(), *step)) {
            wrong.push(format!(
                "{deck} {step} {class}: listed, and the step passes: remove the line"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{}: {} differ from tests/fixtures/pair/{}.txt:\n{}",
        c.name,
        wrong.len(),
        c.name,
        wrong.join("\n")
    );
}

/// The two rows of `c`; panics unless the source has a reader and the target a writer and a
/// reader (step 3 reads the written text back).
fn rows(c: &Cell) -> (&'static DialectRow, &'static DialectRow) {
    let row = |n: &str| dialect::by_name(n).unwrap_or_else(|| panic!("{}: no row {n}", c.name));
    let (from, to) = (row(c.from), row(c.to));
    assert!(from.read.is_some(), "{}: {} has no reader", c.name, c.from);
    assert!(to.write.is_some(), "{}: {} has no writer", c.name, c.to);
    assert!(to.read.is_some(), "{}: {} has no reader", c.name, c.to);
    (from, to)
}

/// The decks of `c`, sorted: every file of the cell's folder with the source's extension.
fn decks(c: &Cell) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pair")
        .join(c.name);
    let rd = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut out: Vec<PathBuf> = rd
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == c.from_ext))
        .collect();
    out.sort();
    out
}

/// Steps 1 to 5 of one deck: each failing step with its class, and the last step that ran.
/// Puts the written files of a deck that reached step 3 in `written` (step 6's one job).
fn steps(
    c: &Cell,
    from: &DialectRow,
    to: &DialectRow,
    deck: &Path,
    text: &str,
    pair: &PairFile,
    written: &mut BTreeMap<String, Vec<(String, String)>>,
) -> (BTreeMap<u8, Fail>, u8) {
    let mut out = BTreeMap::new();
    let spine = match spine(from, to, deck, text) {
        Ok(s) => s,
        Err((step, f)) => {
            out.insert(step, f);
            return (out, step);
        }
    };
    written.insert(stem(deck), spine.files.clone());
    if let Err(f) = read_back(to, &spine) {
        out.insert(3, f);
    }
    let run = |sim: Sim, path: &Path, text: &str, files: &[(String, String)]| {
        sim::run(sim, path, text, files, SECS)
    };
    let a = run(c.from_sim, deck, text, &[]);
    let (entry, rest) = spine.files.split_first().expect("an entry file");
    let b = run(c.to_sim, &deck.with_extension(c.to_ext), &entry.1, rest);
    let (a, b) = match (a, b) {
        (Ran::Plots(a), Ran::Plots(b)) => (
            plots::only(a, pair.vectors.as_ref()),
            plots::only(plots::renamed(b, &spine.renames), pair.vectors.as_ref()),
        ),
        (a, b) => {
            out.insert(4, ran_fail(a, b));
            return (out, 4);
        }
    };
    let mut tol = cmp::Tol::of(&[text, &entry.1]);
    for (k, v) in &pair.tightened {
        tol.tighten(k, *v).expect("pair_file checked it");
    }
    if let Err(why) = cmp::compare(&a, &b, tol) {
        out.insert(4, fail("mismatch", why));
    }
    if let Err(why) = plots::names(&a, &b, &spine.devices) {
        out.insert(5, fail("missing-vector", why));
    }
    (out, 5)
}

/// Steps 1 and 2 and the writing: the legalized design and its files, or the step that
/// refused with its codes. A statement the source reader keeps as `InvalidInSource` (its
/// simulator rejects it) fails step 1, class `invalid-in-source`: the deck, not a pass, is at
/// fault, though `legalize` would refuse it too. A panic is the step's failure, class `panic`: in the read step 1,
/// in `legalize` step 2, in the writer step 3 (the target dialect).
fn spine(from: &DialectRow, to: &DialectRow, deck: &Path, text: &str) -> Result<Spine, (u8, Fail)> {
    let path = deck.file_name().expect("a deck file").to_string_lossy();
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    let src = Sources::one(&path, text);
    guard(1, || {
        dialect::read(&src, from, Policy::Strict, &mut design, &mut diags)
    })?;
    if let Some(f) = refusal(&diags, 0, text) {
        return Err((1, f));
    }
    let table = ir::item::Opaque::table(&design);
    let invalid = table
        .rows()
        .iter()
        .filter(|o| o.class == ir::OpaqueClass::InvalidInSource);
    if invalid.count() > 0 {
        let notes: Vec<&ir::Diag> = diags.rows().iter().collect();
        return Err((1, fail("invalid-in-source", describe(&notes, text))));
    }
    let read = diags.len();
    guard(2, || dialect::legalize(&mut design, to, &mut diags))?;
    if let Some(f) = refusal(&diags, read, text) {
        return Err((2, f));
    }
    let renames: HashMap<String, String> = diags.rows()[read..]
        .iter()
        .filter(|d| d.code == RENAMED)
        .map(|d| {
            let e = design.row::<ir::Element>(d.item.index);
            let new = design.str(e.name).to_ascii_lowercase();
            (new, design.str(ir::NameId(d.arg)).to_ascii_lowercase())
        })
        .collect();
    let files = guard(3, || laws::write_all(&design, to).files().to_vec())?;
    let mut devices = BTreeSet::new();
    for (item, _) in design.reachable() {
        if item.kind != ir::ItemKind::Element {
            continue;
        }
        let e = design.row::<ir::Element>(item.index);
        if e.kind != ir::ElemKind::Instance {
            let name = design.str(e.name).to_ascii_lowercase();
            devices.extend(renames.get(&name).cloned());
            devices.insert(name);
        }
    }
    Ok(Spine {
        legal: design,
        files,
        renames,
        devices,
    })
}

/// Step 3: the written files read back by `to`'s reader, leniently, mean the legalized design.
fn read_back(to: &DialectRow, spine: &Spine) -> Result<(), Fail> {
    let mut src = Sources::new();
    for (path, text) in &spine.files {
        src.add(path, text);
    }
    let mut back = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, to, Policy::Lenient, &mut back, &mut diags);
    if let Some(f) = refusal(&diags, 0, &spine.files[0].1) {
        return Err(fail("refused-back", f.said));
    }
    if laws::same_meaning(&spine.legal, &back) {
        return Ok(());
    }
    let ir_row = dialect::by_name("ir").expect("the ir row");
    let normal =
        |d: &ir::Design| laws::canonical(&laws::read_all(ir_row, &laws::write_all(d, ir_row)));
    let (want, got) = (normal(&spine.legal), normal(&back));
    // Every differing line, the first four: a known difference must not hide another.
    let pairs = want.lines().zip(got.lines()).filter(|(w, g)| w != g);
    let lines: Vec<String> = pairs
        .take(4)
        .map(|(w, g)| format!("legalized `{}`, read back `{}`", w.trim(), g.trim()))
        .collect();
    let said = match lines.is_empty() {
        false => lines.join(" | "),
        true => format!(
            "{} lines legalized, {} read back",
            want.lines().count(),
            got.lines().count()
        ),
    };
    Err(fail("differs", said))
}

/// `f`'s value, or step `step` failed with class `panic` and the panic's message.
fn guard<T>(step: u8, f: impl FnOnce() -> T) -> Result<T, (u8, Fail)> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|e| {
        let said = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "a panic with no message".into());
        (step, fail("panic", said))
    })
}

/// The refusing rows of `diags` from `from` on, as a class `refused:<codes>` (and, for `U0001`,
/// `:<features>`: the cause is the feature, so a row whose feature changes fails its line) and
/// a message naming each with its title and source line; `None` when none refuses.
fn refusal(diags: &ir::Diags, from: usize, text: &str) -> Option<Fail> {
    let rows: Vec<&ir::Diag> = diags.rows()[from..]
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    if rows.is_empty() {
        return None;
    }
    let codes: BTreeSet<String> = rows.iter().map(|d| d.code.to_string()).collect();
    let features: BTreeSet<String> = rows
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED)
        .map(|d| format!("{:?}", ir::Feature::from_index(d.arg as u16)).replace(' ', ""))
        .collect();
    let mut class = format!(
        "refused:{}",
        codes.into_iter().collect::<Vec<_>>().join(",")
    );
    if !features.is_empty() {
        class += ":";
        class += &features.into_iter().collect::<Vec<_>>().join("+");
    }
    Some(fail(&class, describe(&rows, text)))
}

/// `rows`, each with its source line in `text`, its code, title and (for `U0001`) the feature.
fn describe(rows: &[&ir::Diag], text: &str) -> String {
    let said: Vec<String> = rows
        .iter()
        .map(|d| {
            let title = dialect::info(d.code).map_or("(no catalogue row)", |i| i.title);
            let line = 1 + text.as_bytes()[..(d.span.start as usize).min(text.len())]
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
            let arg = match d.code == ir::diag::UNSUPPORTED {
                true => format!(" {:?}", ir::Feature::from_index(d.arg as u16)),
                false => String::new(),
            };
            format!("line {line}: {} {title}{arg}", d.code)
        })
        .collect();
    said.join(" | ")
}

/// Step 4's verdict when a run gave no plots: whose run, and why.
fn ran_fail(a: Ran, b: Ran) -> Fail {
    match (a, b) {
        (Ran::Timeout, _) | (_, Ran::Timeout) => fail("slow", "past the CPU budget".into()),
        (Ran::Failed(why), _) => fail("source-fails", why),
        (_, Ran::Failed(why)) => fail("target-fails", why),
        (Ran::Plots(_), Ran::Plots(_)) => unreachable!("a run without plots"),
    }
}

/// Step 6: every deck of `decks` that was written once (`written`, one job) converted again on
/// [`JOBS`] threads at once; the decks whose bytes differ, with the first file that does.
fn determinism(
    c: &Cell,
    from: &DialectRow,
    to: &DialectRow,
    decks: &[PathBuf],
    written: &BTreeMap<String, Vec<(String, String)>>,
) -> Vec<(String, String)> {
    let again: Vec<(String, Option<Written>)> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..JOBS)
            .map(|j| {
                s.spawn(move || {
                    let mine = decks.iter().skip(j).step_by(JOBS);
                    mine.map(|d| {
                        let text = std::fs::read_to_string(d).expect("a deck");
                        (stem(d), spine(from, to, d, &text).ok().map(|s| s.files))
                    })
                    .collect::<Vec<_>>()
                })
            })
            .collect();
        jobs.into_iter()
            .flat_map(|j| j.join().expect("a job"))
            .collect()
    });
    assert_eq!(
        again.len(),
        decks.len(),
        "{}: every deck converted again",
        c.name
    );
    let mut out = Vec::new();
    for (name, files) in again {
        let once = written.get(&name);
        if once != files.as_ref() {
            let file = once
                .zip(files.as_ref())
                .and_then(|(x, y)| {
                    x.iter()
                        .zip(y)
                        .find(|(p, q)| p != q)
                        .map(|(p, _)| p.0.clone())
                })
                .unwrap_or_else(|| "the file list".into());
            out.push((name, format!("{file} differs between 1 and {JOBS} jobs")));
        }
    }
    out
}

/// A failure of class `class`.
fn fail(class: &str, said: String) -> Fail {
    Fail {
        class: class.to_string(),
        said,
    }
}

/// A path's file stem.
fn stem(p: &Path) -> String {
    p.file_stem()
        .expect("a file name")
        .to_string_lossy()
        .into_owned()
}
