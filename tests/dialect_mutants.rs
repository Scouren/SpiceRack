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

#[path = "common/memcap.rs"]
mod memcap;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Command;

use spicerack::dialect::{BUILTIN, DialectRow, Sources};
use spicerack::ir;

/// Mutants per deck (the merge tier).
const MUTANTS: u64 = 32;

/// The most reads one shrink makes (each a reader run on a candidate); past it the smallest
/// mutant reached is reported, marked as not minimal (a large fixture shrank for minutes).
const SHRINK_STEPS: usize = 400;

/// The rows that fail today: (law, row, why). Strict both ways.
const KNOWN: &[(&str, &str, &str)] = &[];

/// splitmix64.
struct Rng(u64);

/// Set in the child run of the memory cap's self-test.
const MEMCAP_CHILD: &str = "SPICERACK_MEMCAP_CHILD";

// The self-test's child reserves 64 GiB: past the cap by construction.
const _: () = assert!(memcap::CAP_BYTES < 64 << 30);

// ---- private ----

/// The memory cap's self-test (`common/memcap.rs`): this binary run again as a child that
/// reserves 1 GiB at a time, untouched (no page is resident, so the machine is never at risk),
/// up to 64 GiB. Under the cap it aborts with Rust's allocation-failure message before 8 GiB;
/// with no cap it would reserve all 64 and exit 0.
#[test]
fn an_allocation_past_the_memory_cap_aborts_the_binary() {
    const NAME: &str = "an_allocation_past_the_memory_cap_aborts_the_binary";
    if std::env::var_os(MEMCAP_CHILD).is_some() {
        // no core file for the abort this child is for (it took seconds to write)
        let none = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: `setrlimit` reads one initialized `rlimit` through a valid pointer.
        unsafe { libc::setrlimit(libc::RLIMIT_CORE, &none) };
        let mut held = Vec::new();
        for _ in 0..64 {
            held.push(std::hint::black_box(Vec::<u8>::with_capacity(1 << 30)));
        }
        println!("reserved {} GiB: no cap", held.len());
        return;
    }
    let exe = std::env::current_exe().expect("the test binary's path");
    let out = Command::new(exe)
        .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
        .env(MEMCAP_CHILD, "1")
        .output()
        .expect("the test binary runs again");
    let err = String::from_utf8_lossy(&out.stderr);
    let text = format!("{}{err}", String::from_utf8_lossy(&out.stdout));
    assert!(!out.status.success(), "the child ran past the cap:\n{text}");
    assert!(
        err.contains("memory allocation of 1073741824 bytes failed"),
        "the child died of something else:\n{text}"
    );
}

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

#[test]
fn reasons_that_differ_only_by_a_name_or_number_are_one_group() {
    assert_eq!(
        masked("panic: analysis name t3 used twice"),
        masked("panic: analysis name a5 used twice")
    );
    assert_ne!(
        masked("ir::check: X0016"),
        masked("ir::check: X0015"),
        "codes are kept"
    );
    assert_ne!(
        masked("panic: a name used twice"),
        masked("panic: a name not found")
    );
}

#[test]
fn a_shrink_that_hits_its_bound_says_it_is_not_minimal() {
    // Every candidate stays bad but only a line-by-line walk could tell: on a text of more lines
    // than the bound, the shrink stops and says so.
    let text: String = (0..SHRINK_STEPS + 50)
        .map(|k| format!("line{k} a=1\n"))
        .collect();
    let reads = std::cell::Cell::new(0);
    let bad = |t: &str| {
        reads.set(reads.get() + 1);
        t.contains("line0 ").then(|| "bad".to_string())
    };
    let small = shrink(&text, &bad);
    assert!(
        small.ends_with("bound)\n"),
        "{}",
        &small[small.len().saturating_sub(80)..]
    );
    assert!(reads.get() <= SHRINK_STEPS, "{} reads", reads.get());
}

/// Every mutant of every fixture deck of `row`, read and checked; each distinct failure (by
/// its reason, names and numbers masked: [`masked`]) once, with the first seed and deck that showed it, its mutant shrunk.
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
            // One bug, one shrink: reasons are grouped with the names and numbers in them masked.
            let group = masked(&why);
            if found.iter().any(|(w, _)| *w == group) {
                continue;
            }
            let small = shrink(&mutant, &bad);
            found.push((
                group,
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

/// `why` with every word holding a digit masked, a diagnostic code (`X0016`) kept
/// (`analysis name t3 used twice` and `… a5 …` are one reason; `X0016` and `X0015` two): the
/// key failures are grouped by.
fn masked(why: &str) -> String {
    let code = |w: &str| {
        let b = w.as_bytes();
        b.len() == 5 && b[0].is_ascii_uppercase() && b[1..].iter().all(u8::is_ascii_digit)
    };
    let word = |w: &str| match w.chars().any(|c| c.is_ascii_digit()) && !code(w) {
        true => "#".to_string(),
        false => w.to_string(),
    };
    why.split_whitespace()
        .map(word)
        .collect::<Vec<_>>()
        .join(" ")
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
        // a reader code no catalogue holds is shown as a bare number: a failure too
        let bare: Vec<ir::Code> = diags.rows()[..at]
            .iter()
            .map(|d| d.code)
            .filter(|&c| spicerack::dialect::info(c).is_none())
            .collect();
        (diags.rows()[at..].to_vec(), bare)
    }));
    match read {
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned();
            let msg = msg.or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            Some(format!("panic: {}", msg.unwrap_or_default()))
        }
        Ok((_, bare)) if !bare.is_empty() => {
            let codes: Vec<String> = bare.iter().map(|c| c.to_string()).collect();
            Some(format!("codes no catalogue holds: {}", codes.join(" ")))
        }
        Ok((found, _)) => {
            let codes: Vec<String> = found.iter().map(|d| d.code.to_string()).collect();
            (!codes.is_empty()).then(|| format!("ir::check: {}", codes.join(" ")))
        }
    }
}

/// The text with lines removed one at a time while it stays bad: the smallest failing mutant
/// this greedy walk finds. Bounded: at most [`SHRINK_STEPS`] reads; at the bound the smallest
/// text reached is returned with a line saying it is not minimal.
fn shrink(text: &str, bad: &dyn Fn(&str) -> Option<String>) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    let join = |ls: &[&str]| ls.iter().map(|l| format!("{l}\n")).collect::<String>();
    let mut changed = true;
    let mut sweeps = 0;
    let mut steps = 0;
    while changed && sweeps <= lines.len() {
        changed = false;
        sweeps += 1;
        let mut i = lines.len();
        while i > 0 {
            if steps == SHRINK_STEPS {
                let note =
                    format!("(not minimal: the shrink stopped at its {SHRINK_STEPS}-read bound)\n");
                return join(&lines) + &note;
            }
            steps += 1;
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
