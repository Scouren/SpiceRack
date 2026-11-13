//! A law's failures per (row, seed) against its KNOWN rows, strictly both ways by name: a row of
//! the KNOWN list names the law, the row (a dialect or a pass), the labels it fails at (`seed
//! <n>`, or the name of a check that is no seed's) and the reason. A listed label that passes
//! fails the test, and so does a failing label no row lists: a known failure at one seed never
//! hides another at the next (a whole-row entry hid two unrelated failures at VACASK's L5 seed 0
//! for hours, ir-next 3e565243).
//!
//! Used by `tests/dialect_laws.rs`, `tests/pass_laws.rs`, `tests/dialect_mutants.rs`.
//!
//! Contents: `pub type Known`, `pub struct Fails`, `Fails::one(&str, String) -> Fails`,
//! `Fails::seed(&mut self, u64, String)`,
//! `Fails::check(&mut self, &str, String)`, `Fails::ran(&mut self, Result<(), String>)`,
//! `Fails::evidence(&self) -> String`,
//! `pub fn mismatches(law: &str, failing: &[(&str, Fails)], known: &[Known]) -> Vec<String>`,
//! `pub fn compare(law: &str, failing: &[(&str, Fails)], known: &[Known])`

// each including binary uses a part: the mutants binary labels by deck, never by seed
#![allow(dead_code)]

use std::collections::BTreeMap;

/// A KNOWN row: (law, row, the labels it fails at, why).
pub type Known = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
);

/// One law's failures on one row: label → the first evidence seen for it. Each seed is one
/// label whatever the settings it fails under.
#[derive(Default, Debug)]
pub struct Fails(BTreeMap<String, String>);

impl Fails {
    /// One failure, labelled `label`.
    pub fn one(label: &str, evidence: String) -> Fails {
        let mut fails = Fails::default();
        fails.check(label, evidence);
        fails
    }

    /// A failure at `seed`, label `seed <seed>`; a second one at the same seed keeps the first
    /// evidence.
    pub fn seed(&mut self, seed: u64, evidence: String) {
        self.check(&format!("seed {seed}"), evidence);
    }

    /// A failure of a check that is no seed's (a stale citation, a panic, a law run on nothing),
    /// labelled `label`.
    pub fn check(&mut self, label: &str, evidence: String) {
        assert!(!label.is_empty(), "a failure has a label");
        self.0.entry(label.to_string()).or_insert(evidence);
    }

    /// A `ran on nothing` failure when `ran` is one (`Tally::some`, `ran_on`).
    pub fn ran(&mut self, ran: Result<(), String>) {
        if let Err(m) = ran {
            self.check("ran on nothing", m);
        }
    }

    /// Every label with its evidence, one per line block.
    pub fn evidence(&self) -> String {
        let lines: Vec<String> = self.0.iter().map(|(l, m)| format!("{l}: {m}")).collect();
        lines.join("\n")
    }
}

/// The differences between what failed and the KNOWN rows of `law`, one line each: a failing
/// label no row lists (with its evidence), and a listed label that passed. Empty when they agree.
pub fn mismatches(law: &str, failing: &[(&str, Fails)], known: &[Known]) -> Vec<String> {
    let mut out = Vec::new();
    for (row, fails) in failing {
        let listed: &[&str] = known
            .iter()
            .find(|k| k.0 == law && k.1 == *row)
            .map_or(&[], |k| k.2);
        for (label, m) in &fails.0 {
            if !listed.contains(&label.as_str()) {
                out.push(format!("{row} {label}: fails, not listed: {m}"));
            }
        }
    }
    for (_, row, labels, _) in known.iter().filter(|k| k.0 == law) {
        let fails = failing.iter().find(|f| f.0 == *row).map(|f| &f.1);
        for label in *labels {
            if !fails.is_some_and(|f| f.0.contains_key(*label)) {
                out.push(format!("{row} {label}: listed, and passes: remove it"));
            }
        }
    }
    out
}

/// Asserts that `law` failed exactly at its KNOWN rows' labels, both ways, printing every
/// mismatch with its evidence.
pub fn compare(law: &str, failing: &[(&str, Fails)], known: &[Known]) {
    for (_, row, labels, _) in known.iter().filter(|k| k.0 == law) {
        assert!(
            !labels.is_empty(),
            "{law} {row}: a KNOWN row names its seeds"
        );
    }
    let wrong = mismatches(law, failing, known);
    assert!(
        wrong.is_empty(),
        "{law}: failures against KNOWN, by (row, seed)\n{}",
        wrong.join("\n")
    );
}
