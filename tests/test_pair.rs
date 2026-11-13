//! The pair test (`rewrite/SPINE.md` §8a): a deck in dialect A read by A's reader, legalized
//! and written for B through the spine, run on A's simulator and B's, and compared. One test
//! per cell, a (source, target) pair:
//!
//! | cell | source runs on | target runs on |
//! |---|---|---|
//! | `ngspice-vacask` | ngspice-45 (`-b -r`, R49) | VACASK |
//! | `vacask-ngspice` | VACASK | ngspice-45 |
//! | `ngspice-ngspice` | ngspice-45 | ngspice-45 (step 4 bit for bit: `test_pair/same.rs`) |
//! | `vacask-vacask` | VACASK | VACASK (the same) |
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
//! What step 4 does not compare: measures. Under the stated meaning of an ngspice deck
//! (`ngspice -b -r`, R49) no `.measure` card is evaluated at all: ngspice-45 prints "No
//! .measure possible in batch mode (-b) with -r rawfile set!" and writes the vectors only. So
//! no cell compares a measure today; VACASK has no measure statement (`Item(Measure)` is
//! refused toward it and never read from it). A cell with a third dialect that has measures
//! makes the measure comparison a step of its own, with its own source mode.
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
//! Corpus cells (`--features corpus`, `test_pair/corpus.rs`): spice-bench through
//! `ngspice-vacask`, the VACASK corpus through `vacask-ngspice`, leniently, with a census.
//!
//! Contents: none (tests only).

#[path = "common/memcap.rs"]
mod memcap;

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
#[path = "test_pair/numbers.rs"]
mod numbers;
#[path = "test_pair/plots.rs"]
mod plots;
#[path = "test_pair/run.rs"]
mod run;
#[path = "test_pair/same.rs"]
mod same;

#[cfg(feature = "corpus")]
#[path = "test_pair/corpus.rs"]
mod corpus;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, DialectRow, Policy, Sources};

use run::{Deck, Done, Fail};
use sim::Sim;

/// One (source, target) pair: the dialect rows by name, the simulator each runs on, and the
/// extension of each dialect's text.
struct Cell {
    name: &'static str,
    /// The folder of its authored decks under `tests/fixtures/pair` (a same-dialect cell reads
    /// the decks of the cell whose source dialect it shares).
    dir: &'static str,
    from: &'static str,
    to: &'static str,
    from_sim: Sim,
    to_sim: Sim,
    from_ext: &'static str,
    to_ext: &'static str,
}

/// The cells, `ngspice-vacask` and `vacask-ngspice` first (SPINE §8a).
const CELLS: &[Cell] = &[
    Cell {
        name: "ngspice-vacask",
        dir: "ngspice-vacask",
        from: "ngspice",
        to: "vacask",
        from_sim: Sim::Ngspice,
        to_sim: Sim::Vacask,
        from_ext: "sp",
        to_ext: "sim",
    },
    Cell {
        name: "vacask-ngspice",
        dir: "vacask-ngspice",
        from: "vacask",
        to: "ngspice",
        from_sim: Sim::Vacask,
        to_sim: Sim::Ngspice,
        from_ext: "sim",
        to_ext: "sp",
    },
    // The same-dialect cells (`test_pair/same.rs`): one simulator runs both decks, so step 4
    // asks for the same bits.
    Cell {
        name: "ngspice-ngspice",
        dir: "ngspice-vacask",
        from: "ngspice",
        to: "ngspice",
        from_sim: Sim::Ngspice,
        to_sim: Sim::Ngspice,
        from_ext: "sp",
        to_ext: "sp",
    },
    Cell {
        name: "vacask-vacask",
        dir: "vacask-ngspice",
        from: "vacask",
        to: "vacask",
        from_sim: Sim::Vacask,
        to_sim: Sim::Vacask,
        from_ext: "sim",
        to_ext: "sim",
    },
];

// ---- private ----

#[cfg(feature = "corpus")]
#[test]
fn spice_bench_to_vacask() {
    // The spice-bench decks the cross-sim cell keeps (`test_cross_sim/main.rs` SPICE_BENCH).
    let spice_bench = corpus::Corpus {
        var: "SPICE_BENCH",
        keep: |p| !p.contains("/src") && !p.contains("/."),
        source_list: "spice-bench",
        list: "ngspice-vacask-corpus",
    };
    let wrong = corpus::census(&CELLS[0], &spice_bench);
    assert!(
        wrong.is_empty(),
        "{} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[cfg(feature = "corpus")]
#[test]
fn spice_bench_twinless_to_vacask() {
    // The spice-bench decks under `src.assets` that no adapted twin under ngspice/examples or
    // ngspice/paranoia carries, by the list (its header states the rule): every other
    // `/src` deck stays out, as in `spice_bench_to_vacask`.
    let twinless = corpus::Corpus {
        var: "SPICE_BENCH",
        keep: twinless,
        source_list: "spice-bench",
        list: "ngspice-vacask-twinless",
    };
    let wrong = corpus::census(&CELLS[0], &twinless);
    assert!(
        wrong.is_empty(),
        "{} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[cfg(feature = "corpus")]
#[test]
fn vacask_corpus_to_ngspice() {
    // The corpus's benchmarks carry each circuit for several simulators, each in its own
    // folder (`benchmark/rc/{ngspice,xyce,gnucap,vacask}/runme.sim`): only VACASK's text is
    // this cell's source.
    let vacask = corpus::Corpus {
        var: "VACASK_CORPUS",
        keep: |p| {
            let other = ["ngspice", "xyce", "gnucap"];
            !p.split('/')
                .any(|seg| other.iter().any(|o| seg.starts_with(o)))
        },
        source_list: "vacask",
        list: "vacask-ngspice-corpus",
    };
    let wrong = corpus::census(&CELLS[1], &vacask);
    assert!(
        wrong.is_empty(),
        "{} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[cfg(feature = "corpus")]
#[test]
fn spice_bench_to_ngspice() {
    // The same-dialect cell over every spice-bench deck the cross-sim cell keeps (as
    // `spice_bench_to_vacask`): stage 1 of the IR's set (R100).
    let spice_bench = corpus::Corpus {
        var: "SPICE_BENCH",
        keep: |p| !p.contains("/src") && !p.contains("/."),
        source_list: "spice-bench",
        list: "ngspice-ngspice-bench",
    };
    let wrong = corpus::census(&CELLS[2], &spice_bench);
    assert!(
        wrong.is_empty(),
        "{} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[cfg(feature = "corpus")]
#[test]
fn vacask_corpus_to_vacask() {
    // The same-dialect cell over the VACASK corpus, VACASK's own text only (as
    // `vacask_corpus_to_ngspice`).
    let vacask = corpus::Corpus {
        var: "VACASK_CORPUS",
        keep: |p| {
            let other = ["ngspice", "xyce", "gnucap"];
            !p.split('/')
                .any(|seg| other.iter().any(|o| seg.starts_with(o)))
        },
        source_list: "vacask",
        list: "vacask-vacask-corpus",
    };
    let wrong = corpus::census(&CELLS[3], &vacask);
    assert!(
        wrong.is_empty(),
        "{} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn ngspice_to_vacask() {
    cell(&CELLS[0]);
}

#[test]
fn vacask_to_ngspice() {
    cell(&CELLS[1]);
}

#[test]
fn ngspice_to_ngspice() {
    cell(&CELLS[2]);
}

#[test]
fn vacask_to_vacask() {
    cell(&CELLS[3]);
}

#[test]
fn a_pair_file_tightens_and_never_loosens() {
    // Oracle: the defaults of `cmp.rs` (RELTOL 1e-3, VNTOL 1e-6, ABSTOL 1e-12, tube 0.01),
    // written out by hand; ABSTOL and VNTOL are floors as well (cmp.rs). Checks: 6.
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
    assert!(
        files::pair_text("abstol = 1e-15\n").is_err(),
        "below the solvers' floor"
    );
    assert!(files::pair_text("speed = 2\n").is_err(), "an unknown key");
}

#[test]
fn an_include_is_named_from_the_decks_folder() {
    // Oracle: the paths written out by hand (VACASK's demo/bsim3-ptm/amp.sim includes
    // demo/cmos180n.lib). Checks: 4.
    let base = Path::new("demo/bsim3-ptm");
    let named = |p: &str| files::relative(base, p);
    assert_eq!(named("demo/bsim3-ptm/x.inc").as_deref(), Some("x.inc"));
    assert_eq!(
        named("demo/cmos180n.lib").as_deref(),
        Some("../cmos180n.lib")
    );
    assert_eq!(named("lib/a/b.lib").as_deref(), Some("../../lib/a/b.lib"));
    assert_eq!(
        named("demo/bsim3-ptm/sub/y.inc").as_deref(),
        Some("sub/y.inc")
    );
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
    let decks: Vec<Deck> = decks(c)
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("a deck");
            let file = path
                .file_name()
                .expect("a deck file")
                .to_string_lossy()
                .into_owned();
            Deck {
                name: stem(&path),
                src: Sources::one(&file, &text),
                path,
                policy: Policy::Strict,
                simulate: true,
                missing: Vec::new(),
            }
        })
        .collect();
    assert!(!decks.is_empty(), "{}: a cell with no deck", c.name);
    let mut seen: BTreeMap<(String, u8), Fail> = BTreeMap::new();
    let mut once = BTreeMap::new();
    for deck in &decks {
        let pair = files::pair_file(&deck.path)
            .unwrap_or_else(|e| panic!("{}: {}.pair: {e}", c.name, deck.name));
        let done = run::steps(c, from, to, deck, &pair);
        eprintln!(
            "{} {}: {}",
            c.name,
            deck.name,
            line(&done, done.written.is_some())
        );
        if let Some(w) = done.written {
            once.insert(deck.name.clone(), w);
        }
        for (s, f) in done.fails {
            seen.insert((deck.name.clone(), s), f);
        }
    }
    for (name, why) in run::determinism(from, to, &decks, &once) {
        seen.insert((name, 6), run::fail("nondeterministic", why));
    }
    let wrong = judge(c.name, &seen);
    assert!(
        wrong.is_empty(),
        "{}: {} differ from tests/fixtures/pair/{}.txt:\n{}",
        c.name,
        wrong.len(),
        c.name,
        wrong.join("\n")
    );
}

/// `seen` (each failing (deck, step)) against the list `tests/fixtures/pair/<list>.txt`, both
/// ways: each unlisted failure, each listed step that fails in another class or passes.
fn judge(list: &str, seen: &BTreeMap<(String, u8), Fail>) -> Vec<String> {
    let listed = files::listed(list);
    let mut wrong = Vec::new();
    for ((deck, step), f) in seen {
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
    wrong
}

/// A deck's six steps on one line, `<step>:<class>`, `ok`, or `-` for a step with no input (no
/// design, no text, no plots); step 6 ran when the deck was `written`.
fn line(done: &Done, written: bool) -> String {
    let ran = |s: u8| s <= done.last || (s == 6 && written);
    let steps: Vec<String> = (1..=6u8)
        .map(|s| match done.fails.get(&s) {
            Some(f) => format!("{s}:{}", f.class),
            None if ran(s) => format!("{s}:ok"),
            None => format!("{s}:-"),
        })
        .collect();
    steps.join(" ")
}

/// The two rows of `c`; panics unless the source has a reader and the target a writer and a
/// reader (step 3 reads the written text back), and both simulators are on `PATH`.
fn rows(c: &Cell) -> (&'static DialectRow, &'static DialectRow) {
    let row = |n: &str| dialect::by_name(n).unwrap_or_else(|| panic!("{}: no row {n}", c.name));
    let (from, to) = (row(c.from), row(c.to));
    assert!(from.read.is_some(), "{}: {} has no reader", c.name, c.from);
    assert!(to.write.is_some(), "{}: {} has no writer", c.name, c.to);
    assert!(to.read.is_some(), "{}: {} has no reader", c.name, c.to);
    for s in [c.from_sim, c.to_sim] {
        assert!(
            s.on_path(),
            "{}: {} not on PATH: run in `nix develop .#testing`",
            c.name,
            s.name()
        );
    }
    (from, to)
}

/// The decks of `c`, sorted: every file of the cell's folder with the source's extension.
fn decks(c: &Cell) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pair")
        .join(c.dir);
    let rd = std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let mut out: Vec<PathBuf> = rd
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == c.from_ext))
        .collect();
    out.sort();
    out
}

/// A path's file stem.
fn stem(p: &Path) -> String {
    p.file_stem()
        .expect("a file name")
        .to_string_lossy()
        .into_owned()
}

/// Whether the absolute path `p` is one of `tests/fixtures/pair/spice-bench-twinless.txt`'s
/// decks (paths relative to the corpus root).
#[cfg(feature = "corpus")]
fn twinless(p: &str) -> bool {
    static LIST: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let list = LIST.get_or_init(|| {
        let f = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/pair/spice-bench-twinless.txt"
        );
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}"));
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .map(|l| format!("/{}", l.trim()))
            .collect()
    });
    list.iter().any(|rel| p.ends_with(rel.as_str()))
}
