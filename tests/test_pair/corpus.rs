//! The corpus cells of the pair test (`--features corpus`, SPINE §8a): every deck of a corpus
//! the old cross-sim cells run (spice-bench for `ngspice-vacask`, the VACASK corpus for
//! `vacask-ngspice`), read leniently with every file it names, through the six steps on
//! `PAIR_JOBS` workers (6), step 6 the bytes of one job against theirs (by hash) on every tenth written deck. A deck the
//! corpus's cross-sim source list quarantines as `slow` is converted and not simulated.
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
//! - `pub fn census(c: &Cell, corpus: &Corpus) -> Vec<String>`

use std::collections::{BTreeMap, BTreeSet};
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

/// Step 6 runs on every this-many-th written deck of a corpus.
// ponytail: a sample; one job over all of spice-bench is an hour on one core. The whole set
// when a nightly has the hour.
const STRIDE_SIX: usize = 10;

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
    // A deck is loaded in the worker that runs it and dropped after: a corpus does not fit in
    // memory at once. Step 6 keeps a hash of the written bytes, not the bytes.
    let load = |p: &Path| {
        let dir = p.parent().expect("a deck in a folder").to_path_buf();
        let name = p
            .file_name()
            .expect("a file")
            .to_string_lossy()
            .into_owned();
        let mut read = |f: &str| std::fs::read_to_string(dir.join(f)).ok();
        Deck {
            name: rel(p),
            path: p.to_path_buf(),
            src: Sources::load(&[&name], ir::Role::Deck, reader, &mut read),
            policy: Policy::Lenient,
            simulate: !slow.contains(&rel(p)),
        }
    };
    let jobs = std::env::var("PAIR_JOBS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6usize);
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Done, Option<u64>)>> = Mutex::new(Vec::new());
    let none = PairFile::default();
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| {
                // Bounded: each index is taken once.
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(p) = paths.get(k) else { break };
                    let mut v = run::steps(c, from, to, &load(p), &none);
                    let h = v.written.take().map(|w| hash(&w));
                    let mut all = done.lock().expect("no worker panicked");
                    all.push((k, v, h));
                    if all.len().is_multiple_of(200) {
                        eprintln!("{}: {} of {} decks", c.name, all.len(), paths.len());
                    }
                }
            });
        }
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
    let file = format!("{}/pair-{}-census.txt", env!("CARGO_TARGET_TMPDIR"), c.name);
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
        table(&seen, paths.len())
    );
    crate::judge(&format!("{}-corpus", c.name), &seen)
}

// ---- private ----

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

/// The census table: decks by their first failing step and its class (`6:pass` for none), and
/// the step-2 refusals by feature.
fn table(seen: &BTreeMap<(String, u8), Fail>, decks: usize) -> String {
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
    out += "step-2 refused opaques by dialect/class/reader code (decks):\n";
    let mut ops: Vec<(&String, &usize)> = by_opaque.iter().collect();
    ops.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (o, n) in ops {
        out += &format!("  {n:5} {o}\n");
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
