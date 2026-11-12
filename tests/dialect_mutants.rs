//! Laws L1 and L2 on card-shaped text, for every row of `dialect::BUILTIN` with a reader: each
//! of the row's fixture decks (`tests/fixtures/dialect/<row>/read/**`, and its L6 corpus under
//! `--features corpus`) mutated token by token under fixed seeds (a `k=v` duplicated, a key
//! renamed to another key of the same deck, a token dropped, two neighbouring tokens swapped, a
//! line duplicated, a line moved, into or out of a block). For each mutant `read` returns (a
//! panic is caught and reported) and `ir::check` adds no `X` row. Random bytes (L1 in
//! `dialect_laws.rs`) never look like a card, so no reader path past the tokenizer runs there;
//! these do. A separate binary from `dialect_laws.rs` for the 500-line cap; its own [`KNOWN`].
//!
//! Oracles: `read` returning and `ir::check`; the failure names the deck, the seed and the
//! mutant shrunk line by line to the smallest text that still fails. "Every statement is
//! accounted for" is not asserted: the IR exposes no statement count.
//!
//! Contents: tests only.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use spicerack::dialect::{BUILTIN, DialectRow, Sources};
use spicerack::ir;

/// Mutants per deck (the merge tier).
const MUTANTS: u64 = 32;

/// The rows that fail today: (law, row, why). Strict both ways.
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "l1-mutants",
        "ir",
        "ir::parse pushes element rows ir::check refuses (X0016 a second element of one name, X0015 \
     a key of another kind, X0006 a terminal count out of range) instead of a P9001; reported to \
     the circuit owner",
    ),
    (
        "l1-mutants",
        "ngspice",
        "elements: two B cards of one name push two elements (X0016); a J card with fewer than its \
         nodes (`jfet_mes_hfet` read as a card, ` j1 njf beta=1e-4`) indexes past its words in \
         read_elements/semi.rs; run: `.options fourgridsize=400` with `.four` pushes a key of \
         another kind (X0015); reported to ng-elements and ng-run",
    ),
];

/// splitmix64.
struct Rng(u64);

// ---- private ----

#[test]
fn l1_l2_card_shaped_mutants_read_and_check_clean() {
    let mut failing = Vec::new();
    let mut messages = Vec::new();
    for row in BUILTIN.iter().filter(|r| r.read.is_some()) {
        if let Err(m) = mutants_of(row) {
            failing.push(row.name);
            messages.push(format!("{}: {m}", row.name));
        }
    }
    let known: Vec<&str> = KNOWN
        .iter()
        .filter(|k| k.0 == "l1-mutants")
        .map(|k| k.1)
        .collect();
    assert_eq!(
        failing,
        known,
        "l1-mutants against KNOWN\n{}",
        messages.join("\n\n")
    );
}

#[test]
fn a_mutant_that_breaks_the_reader_is_caught_and_shrunk() {
    // The harness's own oracle: a reader that panics on a duplicated line is reported with
    // its seed and a shrunk mutant of the one line that triggers it.
    let text = "alpha a=1\nbeta b=2\ngamma c=3\n";
    let breaks = |t: &str| {
        let lines: Vec<&str> = t.lines().collect();
        let dup = lines
            .windows(2)
            .any(|w| w[0] == w[1] && w[0].starts_with("beta"));
        dup.then(|| "panic: duplicated beta".to_string())
    };
    let found = (0..MUTANTS).find_map(|seed| breaks(&mutate(text, seed)).map(|_| seed));
    let seed = found.expect("some seed duplicates the beta line");
    let small = shrink(&mutate(text, seed), &breaks);
    assert_eq!(small, "beta b=2\nbeta b=2\n");
}

/// Every mutant of every fixture deck of `row`, read and checked; each distinct failure (by
/// its reason) once, with the first seed and deck that showed it, its mutant shrunk.
fn mutants_of(row: &DialectRow) -> Result<(), String> {
    let decks = decks(row.name);
    if decks.is_empty() {
        return Err("no fixture deck to mutate (unsupported fails)".to_string());
    }
    let mut found: Vec<(String, String)> = Vec::new();
    for (path, text) in &decks {
        for seed in 0..MUTANTS {
            let mutant = mutate(text, seed);
            let bad = |t: &str| bad_read(row, t);
            let Some(why) = bad(&mutant) else { continue };
            if found.iter().any(|(w, _)| *w == why) {
                continue;
            }
            let small = shrink(&mutant, &bad);
            found.push((
                why.clone(),
                format!("{path} seed {seed}: {why}\nshrunk mutant:\n{small}"),
            ));
        }
    }
    match found.is_empty() {
        true => Ok(()),
        false => Err(found
            .into_iter()
            .map(|(_, m)| m)
            .collect::<Vec<_>>()
            .join("\n")),
    }
}

/// Why `text` is a bad read for `row` (a panic, or an `X` row), or `None`.
fn bad_read(row: &DialectRow, text: &str) -> Option<String> {
    // The reader and `ir::check` called apart (not through `dialect::read`, whose debug
    // assertion on an `X` row would hide the code).
    let reader = row.read.expect("a row with a reader");
    let read = catch_unwind(AssertUnwindSafe(|| {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        reader.read(
            &Sources::one("mutant", text),
            &mut design.builder(),
            &mut diags,
        );
        let at = diags.len();
        ir::check(&design, &mut diags);
        diags.rows()[at..].to_vec()
    }));
    match read {
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned();
            let msg = msg.or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            Some(format!("panic: {}", msg.unwrap_or_default()))
        }
        Ok(found) => {
            let codes: Vec<String> = found.iter().map(|d| d.code.to_string()).collect();
            (!codes.is_empty()).then(|| format!("ir::check: {}", codes.join(" ")))
        }
    }
}

/// The text with lines removed one at a time while it stays bad: the smallest failing mutant
/// this greedy walk finds. Bounded: at most one removal per line per sweep, sweeps until none.
fn shrink(text: &str, bad: &dyn Fn(&str) -> Option<String>) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    let join = |ls: &[&str]| ls.iter().map(|l| format!("{l}\n")).collect::<String>();
    let mut changed = true;
    let mut sweeps = 0;
    while changed && sweeps <= lines.len() {
        changed = false;
        sweeps += 1;
        let mut i = lines.len();
        while i > 0 {
            i -= 1;
            let mut fewer = lines.clone();
            fewer.remove(i);
            if !fewer.is_empty() && bad(&join(&fewer)).is_some() {
                lines = fewer;
                changed = true;
            }
        }
    }
    join(&lines)
}

/// One mutant of `text` for `seed`: one token-level edit, chosen and placed by the seed.
fn mutate(text: &str, seed: u64) -> String {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ text.len() as u64);
    let mut lines: Vec<Vec<String>> = text.lines().map(words).collect();
    if lines.is_empty() {
        return text.to_string();
    }
    let keys: Vec<String> = lines.iter().flatten().filter_map(|w| key(w)).collect();
    let at = rng.below(lines.len());
    match seed % 6 {
        0 | 1 if lines[at].iter().any(|w| key(w).is_some()) => {
            let kv: Vec<usize> = (0..lines[at].len())
                .filter(|&i| key(&lines[at][i]).is_some())
                .collect();
            let i = kv[rng.below(kv.len())];
            if seed.is_multiple_of(6) || keys.len() < 2 {
                let copy = lines[at][i].clone();
                lines[at].insert(i + 1, copy);
            } else {
                let other = &keys[rng.below(keys.len())];
                let value = lines[at][i]
                    .split_once('=')
                    .map_or("", |(_, v)| v)
                    .to_string();
                lines[at][i] = format!("{other}={value}");
            }
        }
        3 if lines[at].len() > 2 => {
            let i = 1 + rng.below(lines[at].len() - 2);
            lines[at].swap(i, i + 1);
        }
        4 => {
            let copy = lines[at].clone();
            lines.insert(at, copy);
        }
        5 if lines.len() > 1 => {
            let line = lines.remove(at);
            let to = rng.below(lines.len() + 1);
            lines.insert(to, line);
        }
        _ if !lines[at].is_empty() => {
            let i = rng.below(lines[at].len());
            lines[at].remove(i);
        }
        _ => lines.remove(at).clear(),
    }
    lines
        .iter()
        .map(|ws| format!("{}\n", ws.concat()))
        .collect()
}

/// A line as words that keep their spacing: each word with the blanks before it.
fn words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    for c in line.chars() {
        if c.is_whitespace() && in_word {
            out.push(std::mem::take(&mut cur));
            in_word = false;
        }
        in_word |= !c.is_whitespace();
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The key of a `k=v` word (blanks before it kept), or `None`.
fn key(word: &str) -> Option<String> {
    let (k, _) = word.split_once('=')?;
    let lead = &k[..k.len() - k.trim_start().len()];
    let k = k.trim_start();
    (!k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_'))
        .then(|| format!("{lead}{k}"))
}

/// The row's fixture decks: every file under `tests/fixtures/dialect/<row>/read/` but the
/// expected IR (`.ir`; for the `ir` row its decks are its `.ir` files), and its L6 corpus under
/// `--features corpus`.
fn decks(row: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect")
        .join(row)
        .join("read");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    let deck = |p: &PathBuf| (p.extension().is_some_and(|e| e == "ir")) == (row == "ir");
    #[cfg_attr(not(feature = "corpus"), allow(unused_mut))]
    let mut out: Vec<(String, String)> = files
        .iter()
        .filter(|p| deck(p))
        .filter_map(|p| {
            let name = p
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap_or(p)
                .display()
                .to_string();
            Some((name, std::fs::read_to_string(p).ok()?))
        })
        .collect();
    #[cfg(feature = "corpus")]
    out.extend(
        corpus(row)
            .into_iter()
            .enumerate()
            .map(|(k, t)| (format!("corpus {k}"), t)),
    );
    out
}

/// Every file under `dir`, recursively.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// A row's corpus texts under `--features corpus` (each dialect branch adds its own).
#[cfg(feature = "corpus")]
fn corpus(_row: &str) -> Vec<String> {
    Vec::new()
}

impl Rng {
    /// A number in `0..n` (`n > 0`).
    fn below(&mut self, n: usize) -> usize {
        assert!(n > 0, "below: a non-empty range");
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z % n as u64) as usize
    }
}
