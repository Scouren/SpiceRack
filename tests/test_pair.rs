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
#[path = "test_pair/plots.rs"]
mod plots;
#[path = "test_pair/run.rs"]
mod run;

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

// ---- private ----

#[cfg(feature = "corpus")]
#[test]
fn spice_bench_to_vacask() {
    // The spice-bench decks the cross-sim cell keeps (`test_cross_sim/main.rs` SPICE_BENCH).
    let spice_bench = corpus::Corpus {
        var: "SPICE_BENCH",
        keep: |p| !p.contains("/src") && !p.contains("/."),
        source_list: "spice-bench",
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
    };
    let wrong = corpus::census(&CELLS[1], &vacask);
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
        .join(c.name);
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
