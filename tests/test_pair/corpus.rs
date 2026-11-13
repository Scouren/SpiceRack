//! The corpus cells of the pair test (`--features corpus`, SPINE §8a): every deck of a corpus
//! the old cross-sim cells run (spice-bench for `ngspice-vacask`, the VACASK corpus for
//! `vacask-ngspice`), read leniently with every file it names, through the six steps on
//! `PAIR_JOBS` workers (6), step 6 the bytes of one job against theirs (by hash) on every tenth written deck. A deck the
//! corpus's cross-sim source list quarantines as `slow` is neither converted nor simulated (step 1
//! `quarantined`). Each deck's line is also appended to `pair-<cell>-census.partial.txt` as it
//! finishes, so a census cut short keeps what ran. Triage: `PAIR_ONLY=<a|b|…>` (the decks whose
//! path holds one, quarantine lifted), `PAIR_NOSIM=1` (no simulator).
//!
//! In: the corpus root (an environment variable `.#testing` sets). Out: the census, one line a
//! deck (`<path> <step>:<class> …`) in `$CARGO_TARGET_TMPDIR/pair-<cell>-census.txt`, and its
//! table on stderr: decks by the first step that fails and its class, and the step-2
//! refusals by feature (a deck once per feature it is refused for) and its refused opaques by
//! dialect, class and the reader's code. The failing steps are
//! compared with `tests/fixtures/pair/<cell>-corpus.txt` as an authored cell's are.
//!
//! Cites: `rewrite/SPINE.md` §8a; `tests/test_cross_sim/main.rs` (the corpora and their keep
//! rules, the source lists).
//!
//! Contents:
//! - `pub struct Corpus`
//! - `pub fn census(c: &'static Cell, corpus: &Corpus) -> Vec<String>`

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use spicerack::dialect::{DialectRow, Policy, Sources};
use spicerack::ir;

use crate::files::PairFile;
use crate::run::{self, Deck, Done, Fail};
use crate::{Cell, rows};

/// A corpus: the variable that holds its root, the decks it keeps, and the name of its
/// cross-sim source list (`tests/fixtures/cross_sim/<name>-source.txt`).
pub struct Corpus {
    /// The environment variable naming the root.
    pub var: &'static str,
    /// Which paths (absolute, as the cross-sim cell's rule reads them) are decks of the cell.
    pub keep: fn(&str) -> bool,
    /// The cross-sim corpus name.
    pub source_list: &'static str,
    /// The census's name: its expected list `tests/fixtures/pair/<list>.txt` and its file
    /// `pair-<list>-census.txt` (a cell may run several corpora).
    pub list: &'static str,
}

/// Step 6 runs on every this-many-th written deck of a corpus.
// ponytail: a sample; one job over all of spice-bench is an hour on one core. The whole set
// when a nightly has the hour.
const STRIDE_SIX: usize = 10;

/// A deck's conversion budget in seconds: past it the deck is `converter-slow` at step 1 (named,
/// never a pass or a skip; the class must be empty before a release).
const BUDGET: u64 = 120;

/// Bytes free under the temp dir (where every simulator run's directory goes) below which no
/// deck starts: it is `environment` at step 1, named. The peak a census adds is its workers
/// times the files one run writes times `sim`'s 512 MiB cap per file, each run's directory
/// removed when the run returns.
const DISK_MIN: u64 = 20 << 30;

/// A deck still running after this many seconds is named on stderr while it runs.
const SLOW: u64 = 120;

/// Every deck of `corpus` through `c`'s six steps: the census written, its table printed, and
/// the differences from the cell's corpus list returned (empty: the list holds).
pub fn census(c: &'static Cell, corpus: &Corpus) -> Vec<String> {
    let (from, to) = rows(c);
    let root = std::env::var(corpus.var)
        .unwrap_or_else(|_| panic!("{} not set: run in `nix develop .#testing`", corpus.var));
    let root = PathBuf::from(root);
    let mut paths = Vec::new();
    walk(&root, c.from_ext, &mut paths);
    let rel = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .into_owned()
    };
    paths.retain(|p| (corpus.keep)(&p.to_string_lossy()));
    // Triage: `PAIR_ONLY=<a|b|…>` keeps the decks whose path holds one and converts them even
    // when quarantined; `PAIR_NOSIM=1` runs no simulator (step 4 `slow`).
    let only = std::env::var("PAIR_ONLY").ok();
    if let Some(o) = &only {
        paths.retain(|p| o.split('|').any(|w| p.to_string_lossy().contains(w)));
    }
    let nosim = std::env::var_os("PAIR_NOSIM").is_some();
    paths.sort();
    assert!(
        !paths.is_empty(),
        "{}: no *.{} under {}",
        c.name,
        c.from_ext,
        root.display()
    );
    let slow = quarantined(corpus.source_list);
    let reader = from.read.expect("rows checked it");
    // A deck is loaded in the worker that runs it and dropped after: a corpus does not fit in
    // memory at once. Step 6 keeps a hash of the written bytes, not the bytes.
    // Paths relative to the corpus root, so an include above the deck's folder
    // (`include "../mosmm.inc"`) stays inside what `Sources::load` confines a kit to.
    let load = |p: &Path| {
        let mut missing = Vec::new();
        let mut read = |f: &str| {
            let text = std::fs::read_to_string(root.join(f)).ok();
            if text.is_none() {
                missing.push(f.to_string());
            }
            text
        };
        let src = Sources::load(&[&rel(p)], ir::Role::Deck, reader, &mut read);
        Deck {
            name: rel(p),
            path: p.to_path_buf(),
            src,
            policy: Policy::Lenient,
            simulate: !nosim && !slow.contains(&rel(p)),
            missing,
        }
    };
    let jobs = std::env::var("PAIR_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6usize);
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Done, Option<u64>)>> = Mutex::new(Vec::new());
    let none = PairFile::default();
    // Each worker's deck and its start, so a deck that runs long is named while it runs.
    let running: Mutex<Vec<Option<(usize, std::time::Instant)>>> =
        Mutex::new(vec![None; jobs.max(1)]);
    let workers_left = AtomicUsize::new(jobs.max(1));
    let partial_path = format!(
        "{}/pair-{}-census.partial.txt",
        env!("CARGO_TARGET_TMPDIR"),
        corpus.list
    );
    let partial =
        std::fs::File::create(&partial_path).unwrap_or_else(|e| panic!("{partial_path}: {e}"));
    std::thread::scope(|s| {
        for w in 0..jobs.max(1) {
            let (running, workers_left, next, done) = (&running, &workers_left, &next, &done);
            let (load, none, paths, slow, partial, only) =
                (&load, &none, &paths, &slow, &partial, &only);
            s.spawn(move || {
                // Bounded: each index is taken once.
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = paths.get(k) else { break };
                    running.lock().expect("no worker panicked")[w] =
                        Some((k, std::time::Instant::now()));
                    let free = free_bytes(&std::env::temp_dir());
                    let mut v = match only.is_none() && slow.contains(&rel(p)) {
                        // The disk guard: no deck starts under DISK_MIN free.
                        _ if free < DISK_MIN => Done {
                            fails: [(
                                1,
                                run::fail(
                                    "environment",
                                    format!("{} GB free under the temp dir, 20 needed", free >> 30),
                                ),
                            )]
                            .into(),
                            last: 1,
                            written: None,
                        },
                        // Quarantined: its conversion alone can run for hours (the
                        // powergrid decks), so it is named, not run.
                        true => Done {
                            fails: [(
                                1,
                                run::fail("quarantined", "slow in the source list".into()),
                            )]
                            .into(),
                            last: 1,
                            written: None,
                        },
                        false => budgeted(c, from, to, load(p), none.clone()),
                    };
                    running.lock().expect("no worker panicked")[w] = None;
                    let h = v.written.take().map(|w| hash(&w));
                    let mut all = done.lock().expect("no worker panicked");
                    // The line as the deck finishes: a census cut short keeps what ran.
                    let line = format!("{} {}\n", rel(p), crate::line(&v, h.is_some()));
                    let _ = std::io::Write::write_all(&mut &*partial, line.as_bytes());
                    all.push((k, v, h));
                    if all.len().is_multiple_of(200) {
                        eprintln!("{}: {} of {} decks", c.name, all.len(), paths.len());
                    }
                }
                workers_left.fetch_sub(1, Ordering::Relaxed);
            });
        }
        // The watch: every 30 s, each deck past SLOW seconds is named (once a minute at most).
        s.spawn(|| {
            let mut told = std::collections::BTreeSet::new();
            while workers_left.load(Ordering::Relaxed) > 0 {
                std::thread::sleep(std::time::Duration::from_secs(30));
                for (k, t) in running.lock().expect("no worker panicked").iter().flatten() {
                    let secs = t.elapsed().as_secs();
                    if secs > SLOW && told.insert((*k, secs / 60)) {
                        eprintln!("{}: still on {} after {secs} s", c.name, rel(&paths[*k]));
                    }
                }
            }
        });
    });
    let mut done = done.into_inner().expect("no worker panicked");
    done.sort_by_key(|(k, _, _)| *k);
    assert_eq!(done.len(), paths.len(), "{}: every deck ran", c.name);
    let mut seen: BTreeMap<(String, u8), Fail> = BTreeMap::new();
    let mut lines = Vec::new();
    let mut again = Vec::new();
    for (k, v, h) in done {
        let name = rel(&paths[k]);
        lines.push(format!("{name} {}", crate::line(&v, h.is_some())));
        for (step, f) in v.fails {
            seen.insert((name.clone(), step), f);
        }
        if let Some(h) = h
            && k % STRIDE_SIX == 0
        {
            again.push((k, name, h));
        }
    }
    let file = format!(
        "{}/pair-{}-census.txt",
        env!("CARGO_TARGET_TMPDIR"),
        corpus.list
    );
    std::fs::write(&file, lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{file}: {e}"));
    // Step 6, on every STRIDE_SIX-th written deck: converted again on one job, against the
    // bytes the workers wrote.
    for (k, name, h) in again {
        if run::convert(from, to, &load(&paths[k])).map(|w| hash(&w)) != Some(h) {
            let said = format!("the written bytes differ between {jobs} jobs and 1");
            seen.insert((name, 6), run::fail("nondeterministic", said));
        }
    }
    eprintln!(
        "{}: census of {} decks in {file}\n{}",
        c.name,
        paths.len(),
        table(&seen, &paths.iter().map(|p| rel(p)).collect::<Vec<_>>())
    );
    crate::judge(corpus.list, &seen)
}

// ---- private ----

/// `deck` through the steps on a thread of its own: its conversion first, within [`BUDGET`]
/// seconds, else `converter-slow` (the thread is left to finish or to die with the process: a
/// thread cannot be stopped), then the steps, whose simulator runs have their own budgets.
fn budgeted(
    c: &'static Cell,
    from: &'static DialectRow,
    to: &'static DialectRow,
    deck: Deck,
    pair: PairFile,
) -> Done {
    let (tx, rx) = std::sync::mpsc::channel();
    let name = deck.name.clone();
    std::thread::spawn(move || {
        let _ = run::convert(from, to, &deck);
        if tx.send(None).is_ok() {
            let _ = tx.send(Some(run::steps(c, from, to, &deck, &pair)));
        }
    });
    let slow = || Done {
        fails: [(
            1,
            run::fail(
                "converter-slow",
                format!("{name}: past {BUDGET} s converting"),
            ),
        )]
        .into(),
        last: 1,
        written: None,
    };
    match rx.recv_timeout(std::time::Duration::from_secs(BUDGET)) {
        Ok(_) => rx.recv().ok().flatten().unwrap_or_else(slow),
        Err(_) => slow(),
    }
}

/// Bytes free on the file system that holds `dir` (`df`'s avail column).
fn free_bytes(dir: &Path) -> u64 {
    let out = std::process::Command::new("df")
        .args(["--output=avail", "-B1"])
        .arg(dir)
        .output()
        .expect("df on PATH");
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .nth(1)
        .and_then(|l| l.trim().parse().ok())
        .expect("df's avail column")
}

/// A hash of a deck's written files, paths and bytes.
fn hash(w: &run::Written) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    w.hash(&mut h);
    h.finish()
}

/// The decks a cross-sim source list names `slow` (corpus-relative paths).
fn quarantined(list: &str) -> Vec<String> {
    let path = format!(
        "{}/tests/fixtures/cross_sim/{list}-source.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            match (w.next(), w.next()) {
                (Some(deck), Some("slow")) => Some(deck.to_string()),
                _ => None,
            }
        })
        .collect()
}

/// The census table: decks by their first failing step and its class (`6:pass` for none), the
/// step-2 refusals by feature, and each kit family (`cmcqa/<kit>/`) once: its decks, how many
/// pass, and its top three step-2 features (thousands of near-identical decks are one fact).
fn table(seen: &BTreeMap<(String, u8), Fail>, names: &[String]) -> String {
    let decks = names.len();
    let mut first: BTreeMap<String, &Fail> = BTreeMap::new();
    for ((deck, _), f) in seen {
        first.entry(deck.clone()).or_insert(f);
    }
    let mut by_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_feature: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_opaque: BTreeMap<String, usize> = BTreeMap::new();
    for ((deck, step), f) in seen {
        if std::ptr::eq(first[deck], f) {
            let class = f.class.split(':').take(2).collect::<Vec<_>>().join(":");
            *by_class.entry(format!("{step} {class}")).or_default() += 1;
        }
        if *step == 2 {
            let kinds: BTreeSet<&String> = f.opaques.iter().collect();
            for o in kinds {
                *by_opaque.entry(o.clone()).or_default() += 1;
            }
            for feature in f
                .class
                .split(':')
                .nth(2)
                .into_iter()
                .flat_map(|x| x.split('+'))
            {
                *by_feature.entry(feature.to_string()).or_default() += 1;
            }
        }
    }
    let pass = decks - first.len();
    let slow = seen
        .values()
        .filter(|f| f.class == "converter-slow")
        .count();
    let mut out = format!(
        "pass all six: {pass}; converter-slow (must be 0 before a release): {slow}\nfirst failing step and class:\n"
    );
    for (k, n) in &by_class {
        out += &format!("  {n:5} {k}\n");
    }
    out += "step-2 refusals by feature:\n";
    let mut feats: Vec<(&String, &usize)> = by_feature.iter().collect();
    feats.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (f, n) in feats {
        out += &format!("  {n:5} {f}\n");
    }
    out += "step-2 refused opaques by dialect/class/reader code (decks):\n";
    let mut ops: Vec<(&String, &usize)> = by_opaque.iter().collect();
    ops.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (o, n) in ops {
        out += &format!("  {n:5} {o}\n");
    }
    // Kit families: one line each.
    let kit = |d: &str| {
        let w: Vec<&str> = d.split('/').collect();
        (w.len() > 2 && w[0] == "cmcqa").then(|| format!("{}/{}", w[0], w[1]))
    };
    let mut kits: BTreeMap<String, (usize, usize, BTreeMap<String, usize>)> = BTreeMap::new();
    for d in names {
        if let Some(k) = kit(d) {
            let e = kits.entry(k).or_default();
            e.0 += 1;
            e.1 += usize::from(!first.contains_key(d));
        }
    }
    for ((deck, step), f) in seen {
        if let (Some(k), 2) = (kit(deck), *step) {
            let e = kits.get_mut(&k).expect("a kit of a listed deck");
            for feature in f
                .class
                .split(':')
                .nth(2)
                .into_iter()
                .flat_map(|x| x.split('+'))
            {
                *e.2.entry(feature.to_string()).or_default() += 1;
            }
        }
    }
    out += "kit families (decks, pass all six, top three step-2 features by decks):\n";
    for (k, (n, pass, feats)) in &kits {
        let mut top: Vec<(&String, &usize)> = feats.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top: Vec<String> = top
            .iter()
            .take(3)
            .map(|(f, c)| format!("{f} {c}"))
            .collect();
        out += &format!("  {k}: {n} decks, {pass} pass; {}\n", top.join(", "));
    }
    out + &solves(seen)
}

/// The census's solve differences (rulings R96 (3)), one block a class: its name, its decks
/// by name, and the first deck's message as the run printed it. Classified from what the
/// steps captured, nothing run again: a step-4 run that failed with a solver's message (which
/// side, which analysis), a transient that agrees only with the step capped
/// (`simulator-differs:refinement`), a step-2 refusal for continuation (`Run(DcOuter…)`). A
/// step-4 run failure that is no solver message is listed as `…-fails:not-a-solve` with its
/// message, so a pattern this misses shows.
fn solves(seen: &BTreeMap<(String, u8), Fail>) -> String {
    let mut classes: BTreeMap<String, Vec<(&String, String)>> = BTreeMap::new();
    for ((deck, step), f) in seen {
        let class = match (*step, f.class.as_str()) {
            (4, "simulator-differs:refinement") => "step-control:refined-step".to_string(),
            (4, side @ ("target-fails" | "source-fails")) => match solve_analysis(&f.said) {
                Some(a) => format!("solve:{side}:{a}"),
                None => format!("{side}:not-a-solve"),
            },
            (2, c) if c.contains("Run(DcOuter") => "continuation".to_string(),
            _ => continue,
        };
        // A refusal quotes every refused item: its continuation lines alone.
        let said = if class == "continuation" {
            let lines: Vec<&str> = f
                .said
                .split(" | ")
                .filter(|l| l.contains("DcOuter"))
                .collect();
            lines.join(" | ")
        } else {
            f.said.clone()
        };
        classes.entry(class).or_default().push((deck, said));
    }
    let mut out =
        "solve differences (R96), by class: decks, the first one's message:\n".to_string();
    for (class, decks) in &classes {
        let names: Vec<&str> = decks.iter().map(|(d, _)| d.as_str()).collect();
        out += &format!("  {class}: {}\n    said: {}\n", names.join(" "), decks[0].1);
    }
    out
}

/// The analysis a solver's failure message names (`tran`, `ac`, `dc`, `op`, `noise`, or
/// `unnamed`), or `None` when `said` is no solve failure. The forms, as ngspice-45 and VACASK
/// print them: a step too small, a singular matrix, gmin or source stepping, an iteration limit,
/// no convergence, homotopy.
fn solve_analysis(said: &str) -> Option<&'static str> {
    let low = said.to_ascii_lowercase();
    let solver = [
        "timestep too small",
        "time step too small",
        "singular",
        "gmin stepping",
        "source stepping",
        "iteration limit",
        "convergence",
        "converge",
        "homotopy",
    ];
    if !solver.iter().any(|w| low.contains(w)) {
        return None;
    }
    let words: Vec<&str> = low
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let named = ["tran", "ac", "dc", "op", "noise"].into_iter().find(|a| {
        words.iter().any(|w| {
            w.strip_prefix(a)
                .is_some_and(|r| r.chars().all(|c| c.is_ascii_digit()))
        })
    });
    Some(named.unwrap_or("unnamed"))
}

/// Files ending in `.ext` under `dir`, recursively.
fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::solve_analysis;

    #[test]
    fn a_solver_message_names_its_analysis_and_nothing_else_is_a_solve() {
        // Oracle: the forms written out by hand (ngspice-45's "doAnalyses: TRAN:  Timestep too
        // small", VACASK's analysis name `tran1`), and two failures that are no solve.
        let tran = "Error: doAnalyses: TRAN:  Timestep too small; time = 1e-9 | tran simulation(s) aborted";
        assert_eq!(solve_analysis(tran), Some("tran"));
        assert_eq!(
            solve_analysis("Analysis tran1 failed: Homotopy failed."),
            Some("tran")
        );
        assert_eq!(
            solve_analysis("Error: doAnalyses: matrix is singular"),
            Some("unnamed")
        );
        assert_eq!(solve_analysis("Device '__behavioral_B2' not found"), None);
        assert_eq!(
            solve_analysis("Unexpected format: Unexpected EOF in header"),
            None
        );
    }
}
