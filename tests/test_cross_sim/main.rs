//! Cross-simulator oracles: a corpus deck run on its own simulator against
//! the deck SpiceRack writes from it run on the target's simulator, every
//! vector of every plot compared (`cmp.rs`, the tolerance each simulator's
//! RELTOL allows). One test per (source corpus, target) cell:
//!
//! | source | runs on | target | runs on |
//! |---|---|---|---|
//! | Xyce corpus | Xyce | ngspice | ngspice |
//! | spice-bench | ngspice | Xyce, ESPice, VACASK | Xyce, ESPice, VACASK |
//! | spice-bench | ngspice | HSPICE | ESPice `--tokenizer=hspice`, ngspice `hsa` |
//! | VACASK corpus | VACASK | ngspice | ngspice |
//! | HSPICE corpus | ESPice `--tokenizer=hspice` | ngspice | ngspice |
//!
//!   nix develop .#testing -c cargo test --release --test test_cross_sim                       # merge: a sample
//!   nix develop .#testing -c cargo test --release --features corpus --test test_cross_sim     # nightly: every deck
//!
//! Every deck that does not pass is named, with its class, in
//! `tests/fixtures/cross_sim/<cell>.txt`, and the lists are compared by
//! name: an unlisted failure fails, and so does a listed deck that passes or
//! fails in another class, until the line goes. A `mismatch` line is a
//! wrong number in a written deck; `simulator-differs` a mismatch the
//! target's simulator also gives on the original (ESPice reads ngspice), so
//! the simulators disagree and the writer is cleared; `target-fails` the
//! written deck does not run; `simulator-fails` neither does the original on
//! the target's simulator (ESPice lacks what the deck uses); `slow` a deck past the CPU budget, quarantined by name and not
//! simulated. An original that does not run on its own simulator
//! (`source-fails`, or `slow`) is named once per corpus, in
//! `<corpus>-source.txt`, and no target of it is judged. A deck the writer refuses
//! is counted per code, not failed: the refusal census covers it. A missing
//! simulator or corpus fails, never skips.
//!
//! `CROSS_SIM_BLESS=<dir>` writes each cell's observed list to `<dir>`, for
//! review against the tree's; `CROSS_SIM_JOBS` sets the workers (one per CPU);
//! `CROSS_SIM_ONLY=<text>` runs only the decks whose path holds it;
//! `CROSS_SIM_KEEP=1` keeps every run's directory (the decks as run, the
//! simulator's output) and prints where.
//!
//! Cites: the real simulators, run on both decks.
//!
//! Contents: none (tests only).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use spicerack::syntax::Dialect;
use spicerack::syntax::resolve::Form;

mod cmp;
mod kit;
mod sim;
#[path = "../test_import_suite/tube.rs"]
mod tube;

use sim::{Ran, Sim};

/// One target of a corpus: the dialect written, the simulator that runs it,
/// and the cell's name (its known-failure list).
struct Target {
    cell: &'static str,
    dialect: Dialect,
    sim: Sim,
    /// The target's simulator in a mode that reads the source too: a
    /// mismatch it also gives on the original is the simulator's, not the
    /// writer's (`simulator-differs`).
    control: Option<Sim>,
}

/// A corpus, the dialect it is read as, the simulator its originals run on,
/// and the targets each deck is written for.
struct Corpus {
    /// The name of the list of originals that do not run (`<name>-source`).
    name: &'static str,
    var: &'static str,
    ext: &'static str,
    keep: fn(&str) -> bool,
    dialect: Dialect,
    sim: Sim,
    targets: &'static [Target],
}

/// A deck's verdict in one cell.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    Pass,
    /// The writer refused: its codes.
    Refused(String),
    /// The original did not run: the corpus's list names it.
    Source,
    Fails(&'static str, String),
}

/// CPU seconds per run.
const SECS: u64 = 60;

/// Bytes free under the temp dir a run needs before it starts: 12 workers,
/// each run's files capped at `sim::FSIZE`, with room to spare.
const DISK_MIN: u64 = 30 << 30;

/// Decks per cell in the merge tier's sample.
const SAMPLE: usize = 48;

const XYCE: Corpus = Corpus {
    name: "xyce",
    var: "XYCE_CORPUS",
    ext: "cir",
    keep: |_| true,
    dialect: Dialect::Xyce,
    sim: Sim::Xyce,
    targets: &[Target {
        cell: "xyce-ngspice",
        dialect: Dialect::Ngspice,
        sim: Sim::Ngspice,
        control: None,
    }],
};

const SPICE_BENCH: Corpus = Corpus {
    name: "spice-bench",
    var: "SPICE_BENCH",
    ext: "sp",
    keep: |p| !p.contains("/src") && !p.contains("/."),
    dialect: Dialect::Ngspice,
    sim: Sim::Ngspice,
    targets: &[
        Target {
            cell: "ngspice-xyce",
            dialect: Dialect::Xyce,
            sim: Sim::Xyce,
            control: None,
        },
        Target {
            cell: "ngspice-espice",
            dialect: Dialect::Espice,
            sim: Sim::Espice,
            control: Some(Sim::Espice),
        },
        Target {
            cell: "ngspice-vacask",
            dialect: Dialect::Vacask,
            sim: Sim::Vacask,
            control: None,
        },
        Target {
            cell: "ngspice-hspice-espice",
            dialect: Dialect::Hspice,
            sim: Sim::EspiceHspice,
            control: Some(Sim::Espice),
        },
        Target {
            cell: "ngspice-hspice-hsa",
            dialect: Dialect::Hspice,
            sim: Sim::NgspiceHsa,
            control: None,
        },
    ],
};

const VACASK: Corpus = Corpus {
    name: "vacask",
    var: "VACASK_CORPUS",
    ext: "sim",
    keep: |_| true,
    dialect: Dialect::Vacask,
    sim: Sim::Vacask,
    targets: &[Target {
        cell: "vacask-ngspice",
        dialect: Dialect::Ngspice,
        sim: Sim::Ngspice,
        control: None,
    }],
};

const HSPICE: Corpus = Corpus {
    name: "hspice",
    var: "HSPICE_CORPUS",
    ext: "sp",
    keep: |p| p.contains("hspice") && !p.contains("invalid"),
    dialect: Dialect::Hspice,
    sim: Sim::EspiceHspice,
    targets: &[Target {
        cell: "hspice-ngspice",
        dialect: Dialect::Ngspice,
        sim: Sim::Ngspice,
        control: None,
    }],
};

// ---- private ----

#[test]
fn xyce_corpus_sample() {
    sweep(&XYCE, Some(SAMPLE));
}

#[test]
fn spice_bench_sample() {
    sweep(&SPICE_BENCH, Some(SAMPLE));
}

#[test]
fn vacask_corpus_sample() {
    sweep(&VACASK, Some(SAMPLE));
}

#[test]
fn hspice_corpus_sample() {
    sweep(&HSPICE, Some(SAMPLE));
}

#[cfg(feature = "corpus")]
#[test]
fn xyce_corpus() {
    sweep(&XYCE, None);
}

#[cfg(feature = "corpus")]
#[test]
fn spice_bench() {
    sweep(&SPICE_BENCH, None);
}

#[cfg(feature = "corpus")]
#[test]
fn vacask_corpus() {
    sweep(&VACASK, None);
}

#[cfg(feature = "corpus")]
#[test]
fn hspice_corpus() {
    sweep(&HSPICE, None);
}

/// Every deck of `c` (or `sample` of them, evenly spaced in path order)
/// through every target, the verdicts checked against each cell's list and
/// the corpus's list of originals that do not run.
fn sweep(c: &Corpus, sample: Option<usize>) {
    let root = std::env::var(c.var).unwrap_or_else(|_| {
        panic!(
            "{} not set: run the tests in `nix develop .#testing`",
            c.var
        )
    });
    let free = free_bytes(&std::env::temp_dir());
    assert!(
        free >= DISK_MIN,
        "{} GB free under the temp dir, {} needed (GAPS-BRIEF \"Disk\")",
        free >> 30,
        DISK_MIN >> 30
    );
    for s in std::iter::once(c.sim).chain(c.targets.iter().map(|t| t.sim)) {
        assert!(
            s.on_path(),
            "{} not on PATH: run in `nix develop .#testing`",
            s.name()
        );
    }
    let mut all = Vec::new();
    files(Path::new(&root), c.ext, &mut all);
    let only = std::env::var("CROSS_SIM_ONLY").unwrap_or_default();
    all.retain(|p| (c.keep)(&p.to_string_lossy()) && p.to_string_lossy().contains(&only));
    all.sort();
    assert!(!all.is_empty(), "no *.{} under {root}", c.ext);
    if let Some(n) = sample {
        let stride = all.len().div_ceil(n).max(1);
        all = all.into_iter().step_by(stride).collect();
    }
    let rel = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .into_owned()
    };
    let source = listed(&format!("{}-source", c.name));
    let lists: Vec<BTreeMap<String, String>> = c.targets.iter().map(|t| listed(t.cell)).collect();
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    let jobs = std::env::var("CROSS_SIM_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(deck) = all.get(i) else { break };
                    let r = rel(deck);
                    let is_slow =
                        |l: &BTreeMap<String, String>| l.get(&r).is_some_and(|c| c == "slow");
                    let slow: Vec<bool> = lists.iter().map(is_slow).collect();
                    let v = check(c, Path::new(&root), deck, is_slow(&source), &slow);
                    results.lock().unwrap().push((r, v));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));
    let decks: BTreeSet<&String> = results.iter().map(|r| &r.0).collect();
    let mut bad = Vec::new();
    let mut seen = BTreeMap::new();
    for (deck, (src, _)) in &results {
        if let Some((class, why)) = src {
            seen.insert(deck.clone(), (*class, why.clone()));
        }
    }
    eprintln!(
        "{}: {} decks, {} originals listed",
        c.name,
        results.len(),
        seen.len()
    );
    let name = format!("{}-source", c.name);
    against(&name, &source, &seen, &decks, sample.is_none(), &mut bad);
    for (k, t) in c.targets.iter().enumerate() {
        let (mut pass, mut refused) = (0, BTreeMap::<String, usize>::new());
        let mut seen = BTreeMap::new();
        for (deck, (_, v)) in &results {
            match &v[k] {
                Verdict::Pass => pass += 1,
                Verdict::Source => {}
                Verdict::Refused(codes) => *refused.entry(codes.clone()).or_default() += 1,
                Verdict::Fails(class, why) => {
                    seen.insert(deck.clone(), (*class, why.clone()));
                }
            }
        }
        eprintln!(
            "{}: {pass} pass, {} refused, {} listed; refusals by codes: {refused:?}",
            t.cell,
            refused.values().sum::<usize>(),
            seen.len()
        );
        against(t.cell, &lists[k], &seen, &decks, sample.is_none(), &mut bad);
    }
    assert!(
        bad.is_empty(),
        "cross-simulator lists out of date (tests/fixtures/cross_sim/):\n{}",
        bad.join("\n")
    );
}

/// The observed verdicts `seen` of one list against its file `list`, over
/// the `decks` that ran (`full`: the whole corpus, so a listed deck it lacks
/// is stale). Each difference goes to `bad`; `CROSS_SIM_BLESS` writes `seen`.
fn against(
    name: &str,
    list: &BTreeMap<String, String>,
    seen: &BTreeMap<String, (&'static str, String)>,
    decks: &BTreeSet<&String>,
    full: bool,
    bad: &mut Vec<String>,
) {
    if let Ok(dir) = std::env::var("CROSS_SIM_BLESS") {
        let text: String = seen
            .iter()
            .map(|(d, (class, why))| {
                let why: String = why.replace('\n', " ").chars().take(160).collect();
                format!("{d} {class} # {why}\n")
            })
            .collect();
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(Path::new(&dir).join(format!("{name}.txt")), text).unwrap();
    }
    if full {
        let stale: Vec<&String> = list.keys().filter(|d| !decks.contains(d)).collect();
        if !stale.is_empty() {
            bad.push(format!("{name}: listed decks not in the corpus: {stale:?}"));
        }
    }
    for (d, class) in list.iter().filter(|(d, _)| decks.contains(d)) {
        let now = seen.get(d).map(|s| s.0);
        if now != Some(class.as_str()) {
            let now = now.unwrap_or("pass or refused");
            bad.push(format!("{name}: {d} listed {class}, now {now}"));
        }
    }
    for (d, (class, why)) in seen {
        if !list.contains_key(d) {
            bad.push(format!("{name}: {d} {class}, not listed: {why}"));
        }
    }
}

/// One deck through every target of `c`: the original's failure, if it
/// does not run (`slow_source`: listed past the budget, so not run at all),
/// and each target's verdict; a target whose deck is `slow` is not
/// simulated. The original runs once, and only if some target wrote.
fn check(
    c: &Corpus,
    root: &Path,
    deck: &Path,
    slow_source: bool,
    slow: &[bool],
) -> (Option<(&'static str, String)>, Vec<Verdict>) {
    let src = match std::fs::read(deck) {
        Ok(b) => spicerack::import::decode(&b),
        Err(e) => {
            let all = c.targets.iter().map(|_| Verdict::Source).collect();
            return (Some(("source-fails", e.to_string())), all);
        }
    };
    let kit = spicerack::import::kit(&[deck.to_path_buf()], Form::Deck, c.dialect, root);
    let written: Vec<Result<kit::Files, String>> = c
        .targets
        .iter()
        .map(|t| kit::write(&kit, deck, t.dialect))
        .collect();
    let mut original: Option<Ran> = None;
    let mut source = None;
    let mut out = Vec::new();
    for (k, t) in c.targets.iter().enumerate() {
        let text = match &written[k] {
            Err(codes) => {
                out.push(Verdict::Refused(codes.clone()));
                continue;
            }
            Ok(_) if slow_source => {
                source = Some(("slow", "quarantined".to_string()));
                out.push(Verdict::Source);
                continue;
            }
            Ok(_) if slow[k] => {
                out.push(fails("slow", &"quarantined"));
                continue;
            }
            Ok(files) => files,
        };
        let a = match original
            .get_or_insert_with(|| uic_start(c.sim, &src, sim::run(c.sim, deck, &src, &[], SECS)))
        {
            Ran::Plots(p) => p,
            Ran::Timeout => {
                source = Some(("slow", format!("{} timed out", c.sim.name())));
                out.push(Verdict::Source);
                continue;
            }
            Ran::Failed(why) => {
                source = Some(("source-fails", why.clone()));
                out.push(Verdict::Source);
                continue;
            }
        };
        out.push(match sim::run(t.sim, deck, &text.0, &text.1, 2 * SECS) {
            Ran::Plots(b) => match cmp::compare(a, &b) {
                Ok(()) => Verdict::Pass,
                Err(why) => match t.control.map(|s| sim::run(s, deck, &src, &[], 2 * SECS)) {
                    Some(Ran::Plots(o)) if cmp::compare(&o, &b).is_ok() => {
                        fails("simulator-differs", &why)
                    }
                    _ => fails("mismatch", &why),
                },
            },
            Ran::Timeout => fails(
                "slow",
                &format!("{} timed out on the written deck", t.sim.name()),
            ),
            Ran::Failed(why) => match t.control.map(|s| sim::run(s, deck, &src, &[], 2 * SECS)) {
                Some(Ran::Failed(_)) => fails("simulator-fails", &why),
                _ => fails("target-fails", &why),
            },
        });
    }
    (source, out)
}

fn fails(class: &'static str, why: &dyn std::fmt::Display) -> Verdict {
    Verdict::Fails(class, why.to_string())
}

/// A cell's list: deck path -> class, `#` to the end of a line a comment.
fn listed(cell: &str) -> BTreeMap<String, String> {
    let path = format!(
        "{}/tests/fixtures/cross_sim/{cell}.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let mut w = l.split('#').next()?.split_whitespace();
            Some((w.next()?.to_string(), w.next()?.to_string()))
        })
        .collect()
}

/// Files ending in `.ext` under `dir`.
fn files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, ext, out);
        } else if p.extension() == Some(ext.as_ref()) {
            out.push(p);
        }
    }
}

/// `ran` without its transient's t = 0 row when `src` is a Xyce deck whose
/// `.tran` has UIC or NOOP, or a VACASK deck's `tran` with `icmode="uic"`:
/// Xyce runs NOOP as UIC (the same `.prn` bytes) and both print the zero
/// start state at t = 0 even on a source's node (VACASK
/// docs/cmd-analysis-tran.md: "the timepoint at t=0 is not correct"),
/// where ngspice prints t = 0 with the sources applied; after t = 0 all
/// start from that state.
fn uic_start(sim: sim::Sim, src: &str, ran: Ran) -> Ran {
    let xyce = |l: &str| {
        l.starts_with(".tran")
            && l.split(|c: char| c.is_whitespace() || c == ',')
                .any(|w| w == "uic" || w == "noop")
    };
    let vacask = |l: &str| l.contains(" tran ") && l.contains("icmode=\"uic\"");
    let skip = src.lines().any(|l| {
        let l = l.trim_start().to_ascii_lowercase();
        match sim {
            sim::Sim::Xyce => xyce(&l),
            sim::Sim::Vacask => vacask(&l),
            _ => false,
        }
    });
    let Ran::Plots(mut plots) = ran else {
        return ran;
    };
    if skip {
        for p in &mut plots {
            let at0 = p
                .real_data
                .first()
                .is_some_and(|t| t.first() == Some(&0.0) && t.len() > 1);
            let timed = p
                .variables
                .first()
                .is_some_and(|v| v.name.eq_ignore_ascii_case("time"));
            if at0 && timed {
                p.real_data.iter_mut().for_each(|c| {
                    c.remove(0);
                });
                p.complex_data
                    .iter_mut()
                    .filter(|c| !c.is_empty())
                    .for_each(|c| {
                        c.remove(0);
                    });
            }
        }
    }
    Ran::Plots(plots)
}

/// Bytes available on the file system holding `dir`, by `df`.
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
