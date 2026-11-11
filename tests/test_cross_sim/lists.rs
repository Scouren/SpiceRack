//! A cell's list of decks that do not pass (`tests/fixtures/cross_sim/<cell>.txt`):
//! each line `<deck> <class>`, a `simulator-differs:<kind>` line with its
//! rule and proof (ZERO-LEDGER P1-030), a `simulator-fails` line the
//! harness's control does not show with its rule and proof file (P1-037),
//! a `source` or `refinement` line whose proof is a file a certificate run
//! by hand (P1-045), `#` to the end a comment.
//!
//! Cites: mismatches.md §4.3 (the kinds of simulator-differs).
//!
//! Contents:
//! - `pub fn listed(cell: &str) -> BTreeMap<String, String>`
//! - `pub fn admits(class: &str, now: Option<&str>) -> bool`
//! - `pub const KINDS: [&str; 6]`, `pub const HARNESS_KINDS: [&str; 2]`
//! - `pub const PROVEN_FAILS: &str`, `pub const BY_HAND: &str`

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

/// The class [`listed`] gives a `simulator-fails` line that names a proof
/// file: the target's simulator fails on text the target's manual allows
/// and runs the original in its other mode, so the control does not show
/// the failure and the harness observes `target-fails` (ZERO P1-037).
pub const PROVEN_FAILS: &str = "simulator-fails:proof";

/// What [`listed`] appends to the class of a `source` or `refinement` line
/// that names a proof file: where the file's run is one the harness does
/// not make (a step finer than it refines to, a cell with no control) the
/// harness still observes a `mismatch` (ZERO P1-045); where its own run
/// agrees it observes the kind (the HICUM ac lines, P3-103).
pub const BY_HAND: &str = ":by-hand";

/// A cell's list: deck path -> class, `#` to the end of a line a comment.
/// A `simulator-differs:<kind>` line names its rule and its proof
/// (`<deck> simulator-differs:<kind> reason=<rule-id> proof=<file>`, ZERO
/// P1-030): a kind outside [`KINDS`], or no rule or proof, fails the gate;
/// `proof=harness` only for the kinds the harness runs itself, any other a
/// file under `tests/fixtures/cross_sim/proof/`, and a harness kind that
/// names a file is given its class with [`BY_HAND`] appended. A
/// `simulator-fails` line
/// with more words than its class names both too (`<deck> simulator-fails
/// reason=<rule-id> proof=<file>`, a file, never `harness`) and is given
/// the class [`PROVEN_FAILS`].
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
                // A proof file for a kind the harness proves by its own
                // runs: observed as the kind, or still as a mismatch.
                if HARNESS_KINDS.contains(&kind) && proof != Some("harness") {
                    return Some((deck.to_string(), format!("{class}{BY_HAND}")));
                }
            }
            if class == "simulator-fails" && w.len() > 2 {
                let field = |k: &str| w.iter().find_map(|x| x.strip_prefix(k));
                let file = |f: &str| Path::new(&format!("{dir}/proof/{f}")).is_file();
                assert!(
                    field("reason=").is_some_and(|r| !r.is_empty()),
                    "{cell}: {deck}: a simulator-fails line with a proof needs reason=<rule-id>"
                );
                assert!(
                    field("proof=").is_some_and(file),
                    "{cell}: {deck}: a simulator-fails line with a rule needs proof=<file> under tests/fixtures/cross_sim/proof/"
                );
                return Some((deck.to_string(), PROVEN_FAILS.to_string()));
            }
            Some((deck.to_string(), class.to_string()))
        })
        .collect()
}

/// Whether a deck listed as `class` may be observed as `now` (`None`: it
/// passes or is refused). A class is observed as itself; a
/// `simulator-differs` kind a proof file settles (a third simulator,
/// reference data, a closed form, a self-inconsistent source) as a
/// `mismatch`; a harness kind with a proof file ([`BY_HAND`]) as the kind
/// or as a `mismatch`; [`PROVEN_FAILS`] as `target-fails`.
pub fn admits(class: &str, now: Option<&str>) -> bool {
    assert!(!class.is_empty(), "a listed class is a word");
    assert!(
        now != Some(PROVEN_FAILS),
        "the harness never observes {PROVEN_FAILS}"
    );
    let seen = match class.strip_prefix("simulator-differs:") {
        Some(kind) if !HARNESS_KINDS.contains(&kind) => "mismatch",
        _ if class == PROVEN_FAILS => "target-fails",
        _ => class,
    };
    // A proof file beside a harness kind: its own run may agree too.
    let own = class.strip_suffix(BY_HAND);
    now == Some(class) || now == Some(seen) || (own.is_some() && now == own)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p1_060_the_originals_xyce_rejects_are_named() {
        // P1-022 leaves out a line Xyce rejects, so the ngspice target
        // writes these 12 decks and the harness runs their originals,
        // which Xyce stops on. Oracle: Xyce 7.10 on each original, run as
        // `sim.rs` runs it (no raw file, exit 1), its verdict recorded here
        // by name. Checks: 12.
        let source = listed("xyce-source");
        for deck in [
            "Certification_Tests/BUG_204/bug204.cir",
            "Certification_Tests/BUG_281/bug_281.cir",
            "Certification_Tests/BUG_630_SON/vpwlRepeatFail.cir",
            "Certification_Tests/BUG_636_SON/bug636.cir",
            "Certification_Tests/BUG_80_SON/Bad_PWL_Source.cir",
            "Message/Input/AC_extractACData1.cir",
            "Message/Input/DC_excessArgs.cir",
            "Message/Input/NOISE_extractNOISEData1.cir",
            "Output/MPDE/startup_printline_for_mpde-probe-defaults-to-prn.cir",
            "Output/MPDE/startup_printline_for_mpde-probe-fallback-defaults-to-prn.cir",
            "Output/MPDE/startup_printline_for_mpde-raw-defaults-to-prn.cir",
            "Output/MPDE/startup_printline_for_mpde-raw-fallback-defaults-to-prn.cir",
        ] {
            let class = source.get(deck).map(String::as_str);
            assert_eq!(class, Some("source-fails"), "{deck}");
        }
    }

    #[test]
    fn p1_037_a_proof_file_admits_a_reader_failure() {
        // ESPice's HSPICE reader stops on text HSPICE reads (a node voltage
        // in a resistor value: HSPICE User Guide E-2010.12 p.170, p.173)
        // and its own reader runs the original, so the harness observes
        // `target-fails` where the list says `simulator-fails` with a proof.
        // Oracle: the list file's own lines, and the class table below
        // written out by hand (what `against` accepted before, plus the
        // one new pair). Checks: 2 + 7.
        let list = listed("ngspice-hspice-espice");
        let class = |deck: &str| list.get(deck).map(String::as_str);
        let proven = class("ngspice/tests/regression/misc/asrc-tc-2.sp");
        assert_eq!(proven, Some(PROVEN_FAILS));
        let plain = class("ngspice/tests/regression/parser/bxpressn-1.sp");
        assert_eq!(plain, Some("simulator-fails"));
        let third = "simulator-differs:third-simulator";
        let refined = "simulator-differs:refinement";
        for (line, now, ok) in [
            (PROVEN_FAILS, Some("target-fails"), true),
            (PROVEN_FAILS, Some("simulator-fails"), false),
            (PROVEN_FAILS, None, false),
            ("simulator-fails", Some("target-fails"), false),
            ("target-fails", Some("target-fails"), true),
            (third, Some("mismatch"), true),
            (refined, Some("mismatch"), false),
        ] {
            assert_eq!(admits(line, now), ok, "{line} observed as {now:?}");
        }
    }

    #[test]
    fn p1_045_a_proof_file_beside_a_harness_kind() {
        // A `refinement` or `source` line that names a proof file: where
        // the file's run is one the harness does not make (SUBCKT/subckt_a5
        // agrees at TMAX 100 ps, proof/refined-by-hand.md; the hsa cell has
        // no control, proof/hsa-dc-restart.md) the harness still observes
        // a mismatch; where its own control agrees it observes the kind
        // (the HICUM ac lines, proof/hicum-l2-ac-tm50.md). A
        // `proof=harness` line stays strict.
        // Oracle: the list files' own lines, and the class table below
        // written out by hand. Checks: 4 + 6.
        let by_hand = "simulator-differs:refinement:by-hand";
        let source = "simulator-differs:source:by-hand";
        let class = |cell: &str, deck: &str| listed(cell).get(deck).cloned();
        let a5 = class("xyce-ngspice", "SUBCKT/subckt_a5.cir");
        assert_eq!(a5.as_deref(), Some(by_hand));
        let osc = "Certification_Tests/BUG_805/colpitts_osc1.cir";
        let harness = class("xyce-ngspice", osc);
        assert_eq!(harness.as_deref(), Some("simulator-differs:refinement"));
        let schmitt = class("ngspice-hspice-hsa", "circuitsim90/MOS2/schmitfast.sp");
        assert_eq!(schmitt.as_deref(), Some(source));
        let hicum = "cmcqa/hicum/hicum_CBE_npn_1D_ac_Tm50.sp";
        assert_eq!(class("ngspice-espice", hicum).as_deref(), Some(source));
        for (line, now, ok) in [
            (by_hand, Some("mismatch"), true),
            (by_hand, Some("simulator-differs:refinement"), true),
            (by_hand, Some("target-fails"), false),
            (by_hand, None, false),
            (source, Some("simulator-differs:source"), true),
            (source, Some("simulator-differs:refinement"), false),
        ] {
            assert_eq!(admits(line, now), ok, "{line} observed as {now:?}");
        }
    }
}
