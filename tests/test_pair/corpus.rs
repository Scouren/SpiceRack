//! The corpus cells of the pair test (`--features corpus`, SPINE §8a): every deck of a corpus
//! the old cross-sim cells run (spice-bench for `ngspice-vacask`, the VACASK corpus for
//! `vacask-ngspice`), read leniently with every file it names, through the six steps on
//! `PAIR_JOBS` workers (6). A deck the corpus's cross-sim source list quarantines as `slow` is
//! converted and not simulated.
//!
//! In: the corpus root (an environment variable `.#testing` sets). Out: the census, one line a
//! deck (`<path> <step>:<class> …`) in `$CARGO_TARGET_TMPDIR/pair-<cell>-census.txt`, and its
//! table on stderr: decks by the first step that fails and its class, and the step-2
//! refusals by feature (a deck once per feature it is refused for). The failing steps are
//! compared with `tests/fixtures/pair/<cell>-corpus.txt` as an authored cell's are.
//!
//! Cites: `rewrite/SPINE.md` §8a; `tests/test_cross_sim/main.rs` (the corpora and their keep
//! rules, the source lists).
//!
//! Contents:
//! - `pub struct Corpus`
//! - `pub fn census(c: &Cell, corpus: &Corpus) -> Vec<String>`

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use spicerack::dialect::{Policy, Sources};
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
}

/// Every deck of `corpus` through `c`'s six steps: the census written, its table printed, and
/// the differences from the cell's corpus list returned (empty: the list holds).
pub fn census(c: &Cell, corpus: &Corpus) -> Vec<String> {
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
    let decks: Vec<Deck> = paths
        .iter()
        .map(|p| {
            let dir = p.parent().expect("a deck in a folder").to_path_buf();
            let name = p
                .file_name()
                .expect("a file")
                .to_string_lossy()
                .into_owned();
            let mut load = |f: &str| std::fs::read_to_string(dir.join(f)).ok();
            let src = Sources::load(&[&name], ir::Role::Deck, reader, &mut load);
            Deck {
                name: rel(p),
                path: p.clone(),
                src,
                policy: Policy::Lenient,
                simulate: !slow.contains(&rel(p)),
            }
        })
        .collect();
    let jobs = std::env::var("PAIR_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6usize);
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Done)>> = Mutex::new(Vec::new());
    let none = PairFile::default();
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| {
                // Bounded: each index is taken once.
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(d) = decks.get(k) else { break };
                    let v = run::steps(c, from, to, d, &none);
                    done.lock().expect("no worker panicked").push((k, v));
                }
            });
        }
    });
    let mut done = done.into_inner().expect("no worker panicked");
    done.sort_by_key(|(k, _)| *k);
    assert_eq!(done.len(), decks.len(), "{}: every deck ran", c.name);
    let once: BTreeMap<String, run::Written> = done
        .iter()
        .filter_map(|(k, v)| v.written.clone().map(|w| (decks[*k].name.clone(), w)))
        .collect();
    let mut seen: BTreeMap<(String, u8), Fail> = BTreeMap::new();
    let mut lines = Vec::new();
    for (k, v) in done {
        let name = &decks[k].name;
        lines.push(format!(
            "{name} {}",
            crate::line(&v, once.contains_key(name))
        ));
        for (step, f) in v.fails {
            seen.insert((name.clone(), step), f);
        }
    }
    for (name, why) in run::determinism(from, to, &decks, &once) {
        seen.insert((name, 6), run::fail("nondeterministic", why));
    }
    let file = format!("{}/pair-{}-census.txt", env!("CARGO_TARGET_TMPDIR"), c.name);
    std::fs::write(&file, lines.join("\n") + "\n").unwrap_or_else(|e| panic!("{file}: {e}"));
    eprintln!(
        "{}: census of {} decks in {file}\n{}",
        c.name,
        decks.len(),
        table(&seen, decks.len())
    );
    crate::judge(&format!("{}-corpus", c.name), &seen)
}

// ---- private ----

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

/// The census table: decks by their first failing step and its class (`6:pass` for none), and
/// the step-2 refusals by feature.
fn table(seen: &BTreeMap<(String, u8), Fail>, decks: usize) -> String {
    let mut first: BTreeMap<String, &Fail> = BTreeMap::new();
    for ((deck, _), f) in seen {
        first.entry(deck.clone()).or_insert(f);
    }
    let mut by_class: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_feature: BTreeMap<String, usize> = BTreeMap::new();
    for ((deck, step), f) in seen {
        if std::ptr::eq(first[deck], f) {
            let class = f.class.split(':').take(2).collect::<Vec<_>>().join(":");
            *by_class.entry(format!("{step} {class}")).or_default() += 1;
        }
        if *step == 2 {
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
    let mut out = format!("pass all six: {pass}\nfirst failing step and class:\n");
    for (k, n) in &by_class {
        out += &format!("  {n:5} {k}\n");
    }
    out += "step-2 refusals by feature:\n";
    let mut feats: Vec<(&String, &usize)> = by_feature.iter().collect();
    feats.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (f, n) in feats {
        out += &format!("  {n:5} {f}\n");
    }
    out
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
