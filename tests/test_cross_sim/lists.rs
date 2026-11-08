//! A cell's list of decks that do not pass (`tests/fixtures/cross_sim/<cell>.txt`):
//! each line `<deck> <class>`, a `simulator-differs:<kind>` line with its
//! rule and proof (ZERO-LEDGER P1-030), `#` to the end a comment.
//!
//! Cites: mismatches.md §4.3 (the kinds of simulator-differs).
//!
//! Contents:
//! - `pub fn listed(cell: &str) -> BTreeMap<String, String>`
//! - `pub const KINDS: [&str; 6]`, `pub const HARNESS_KINDS: [&str; 2]`

use std::collections::BTreeMap;
use std::path::Path;

/// The kinds of `simulator-differs` (mismatches.md §4.3).
pub const KINDS: [&str; 6] = [
    "third-simulator",
    "reference-data",
    "closed-form",
    "source",
    "refinement",
    "self-inconsistency",
];

/// The kinds the harness proves by its own runs: the target's simulator on
/// the original (`source`), both refined (`refinement`).
pub const HARNESS_KINDS: [&str; 2] = ["source", "refinement"];

/// A cell's list: deck path -> class, `#` to the end of a line a comment.
/// A `simulator-differs:<kind>` line names its rule and its proof
/// (`<deck> simulator-differs:<kind> reason=<rule-id> proof=<file>`, ZERO
/// P1-030): a kind outside [`KINDS`], or no rule or proof, fails the gate;
/// `proof=harness` only for the kinds the harness runs itself, any other a
/// file under `tests/fixtures/cross_sim/proof/`.
pub fn listed(cell: &str) -> BTreeMap<String, String> {
    let dir = format!("{}/tests/fixtures/cross_sim", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(format!("{dir}/{cell}.txt")).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split('#').next()?.split_whitespace().collect();
            let (deck, class) = (*w.first()?, *w.get(1)?);
            if let Some(kind) = class.strip_prefix("simulator-differs") {
                let kind = kind.strip_prefix(':').unwrap_or("");
                let field = |k: &str| w.iter().find_map(|x| x.strip_prefix(k));
                let (rule, proof) = (field("reason="), field("proof="));
                assert!(
                    KINDS.contains(&kind) && rule.is_some_and(|r| !r.is_empty()),
                    "{cell}: {deck}: a simulator-differs line needs a kind of {KINDS:?} and reason=<rule-id>"
                );
                match proof {
                    Some("harness") => assert!(
                        HARNESS_KINDS.contains(&kind),
                        "{cell}: {deck}: proof=harness only for {HARNESS_KINDS:?}"
                    ),
                    Some(f) => assert!(
                        Path::new(&format!("{dir}/proof/{f}")).is_file(),
                        "{cell}: {deck}: proof file tests/fixtures/cross_sim/proof/{f} missing"
                    ),
                    None => panic!("{cell}: {deck}: a simulator-differs line needs proof=<file>"),
                }
            }
            Some((deck.to_string(), class.to_string()))
        })
        .collect()
}
