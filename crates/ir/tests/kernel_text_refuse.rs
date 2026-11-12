//! The IR text reader refuses a text that states an invalid design (R61/A36's finding on the
//! trunk's mutation law, `worked.ir` seed 9): what `check` would refuse in what `parse` built is
//! one `TEXT_SYNTAX` row per item and no item, the rest read. `parse_unchecked` is the door that
//! builds such a text as written, for `check`'s own fixtures.
//!
//! Oracles: `check` of what was read; hand-written cases, each the shrunk text a mutation survey
//! of generated designs found; and that survey kept as a law (the dump of a generated design,
//! mutated line by line under fixed seeds, reads without a panic to a design `check` accepts).
//!
//! Contents: none (tests only).

#![cfg(feature = "gen")]

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{DefKind, Design, Diags, FeatureSet, check, diag::TEXT_SYNTAX, dump, parse};

/// Generated designs the law mutates, and mutants of each.
const SEEDS: u64 = 300;
const MUTANTS: u32 = 30;

/// splitmix64.
struct Rng(u64);

// ---- private ----

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// `text` read: the design, the reader's codes, and what `check` finds in what it built.
fn read_checked(text: &str) -> (Design, Vec<spicerack_ir::diag::Code>, Diags) {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(text, &mut d.builder(), &mut diags);
    let mut checked = Diags::new();
    check(&d, &mut checked);
    (d, diags.rows().iter().map(|r| r.code).collect(), checked)
}

/// What is wrong with reading `text`: a panic's first line or `check`'s codes; `None` when it
/// reads to a valid design.
fn wrong(text: &str) -> Option<String> {
    let run = std::panic::catch_unwind(|| read_checked(text).2.rows().to_vec());
    match run {
        Ok(rows) if rows.is_empty() => None,
        Ok(rows) => Some(format!(
            "{:?}",
            rows.iter().map(|r| r.code.to_string()).collect::<Vec<_>>()
        )),
        Err(e) => {
            let msg = e.downcast_ref::<String>().cloned();
            let msg = msg.or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
            Some(format!("panic: {}", msg.unwrap_or_default()))
        }
    }
}

/// One mutation of `text`'s lines: a line duplicated, moved, re-indented, a word dropped, two
/// words swapped, or a word replaced by a word of another line.
fn mutate(text: &str, r: &mut Rng) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let n = lines.len();
    let k = r.below(n);
    let mut words: Vec<String> = lines[k].split(' ').map(String::from).collect();
    let i = r.below(words.len());
    match r.below(6) {
        0 => {
            let l = lines[k].clone();
            lines.insert(r.below(n + 1), l);
        }
        1 => {
            let l = lines.remove(k);
            lines.insert(r.below(n), l);
        }
        2 => {
            words.remove(i);
            lines[k] = words.join(" ");
        }
        3 => {
            let j = (i + 1).min(words.len() - 1);
            words.swap(i, j);
            lines[k] = words.join(" ");
        }
        4 => {
            let other: Vec<&str> = lines[r.below(n)].split(' ').collect();
            words[i] = other[r.below(other.len())].to_string();
            lines[k] = words.join(" ");
        }
        _ => lines[k] = format!("  {}", lines[k]),
    }
    lines.join("\n") + "\n"
}

/// `text` with every line dropped that `bad` stays true without (a line-by-line shrink, one
/// pass: bounded by the line count).
fn shrink(text: &str, bad: &dyn Fn(&str) -> bool) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    let mut k = 1;
    while k < lines.len() {
        let mut fewer = lines.clone();
        fewer.remove(k);
        if bad(&(fewer.join("\n") + "\n")) {
            lines = fewer;
        } else {
            k += 1;
        }
    }
    lines.join("\n") + "\n"
}

#[test]
fn a_text_that_states_an_invalid_design_is_refused() {
    // texts a mutation survey of generated designs found building what `check` refuses
    // (oracle: `check`; each is one `TEXT_SYNTAX` row and no item); the matching good line stays
    let cases = [
        // StringOrListField: an `if` condition is no string (A25 item 4)
        (
            "file \"f0.sp\" deck\n  if \"f0.sp\"\n    param a = 1\n  param ok = 1\n",
            1,
        ),
        // FieldDomain: a measure line read in part (its value left out) is no measure
        (
            "file \"f0.sp\" deck\n  measure g13 tf param $tstop\n  param ok = 1\n",
            1,
        ),
        // FieldDomain after a clean read: an `env` that sets nothing (the row's own check)
        ("file \"f0.sp\" deck\n  env\n  param ok = 1\n", 1),
        // StringOrListBody: a function body is a number
        (
            "file \"f0.sp\" deck\n  func g2(g0, g1) = \"f1.sp\"\n  param ok = 1\n",
            1,
        ),
        // FieldDomain of the whole run: a Fourier analysis with no count and no `nfreqs` in
        // force (`state_order`'s design-level rule)
        (
            "file \"f0.sp\" deck\n  analysis four fundamental=g15 periods=1 (i im (g19))\n  param ok = 1\n",
            1,
        ),
        // trailing words: the line is skipped whole
        ("file \"f0.sp\" deck\n  param p = 1 2\n  param ok = 1\n", 1),
        // DefinitionCycle: a parameter reading itself, and one reading through a second
        (
            "file \"f0.sp\" deck\n  param g = ln($temp) >= g\n  param ok = 1\n  param r = g + 1\n",
            1,
        ),
        (
            "file \"f0.sp\" deck\n  param a = b\n  param b = a\n  param ok = 1\n",
            2,
        ),
    ];
    let mut wrong = Vec::new();
    for (text, refused) in cases {
        let (d, codes, checked) = read_checked(text);
        let top = d.file(d.files().next().unwrap()).top;
        let ok = d.lookup(top, DefKind::Param, d.find_name("ok"));
        let lines = text
            .lines()
            .filter(|l| l.starts_with("  ") && !l.starts_with("    "));
        let items = d.items(top).len();
        if codes != vec![TEXT_SYNTAX; refused] {
            wrong.push(format!("{text}: read {codes:?}"));
        }
        if !checked.is_empty() {
            wrong.push(format!("{text}: check {:?}", checked.rows()));
        }
        if ok.is_none() {
            wrong.push(format!("{text}: the good line is not read"));
        }
        if items != lines.count() - refused {
            wrong.push(format!("{text}: {items} items, a refused line leaves none"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_mutated_text_reads_to_a_valid_design() {
    let size = Size {
        items: 16,
        scope_depth: 3,
        expr_depth: 3,
        files: 2,
    };
    for seed in 0..SEEDS {
        let d = generate(seed, &FeatureSet::ALL, size);
        let mut text = String::new();
        for f in d.files() {
            dump(&d, f, &mut text);
        }
        let mut r = Rng(seed);
        for m in 0..MUTANTS {
            let mutant = mutate(&text, &mut r);
            if let Some(why) = wrong(&mutant) {
                let small = shrink(&mutant, &|t| wrong(t).is_some());
                panic!("seed {seed}, mutant {m}: {why}\n{small}");
            }
        }
    }
}
