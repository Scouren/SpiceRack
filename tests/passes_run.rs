//! The meaning of the run passes (`src/passes/run/`), on the IR alone, until the trunk's pair
//! decks prove them on the simulators (`PENDING_PAIRS`).
//!
//! Oracle: the state in force at each analysis (`passes_run/state.rs`, a model written from
//! the IR's docs, never from a pass): for every analysis, the same options, temperatures, node
//! conditions and saves before and after a pass, over generated designs (`ir::r#gen::generate`)
//! holding the pass's feature; and every row reference unchanged (`Design::item_refs`).
//!
//! Contents: tests only.

#[path = "passes_run/state.rs"]
mod state;
#[path = "passes_run/walk.rs"]
mod walk;

use std::path::Path;

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes;

/// The run passes whose simulator proof (a pair deck naming it in `proves =`) is to come:
/// compared by name with the decks, both ways.
const PENDING_PAIRS: &[&str] = &[
    "whole_run_state_first",
    "whole_run_to_from_here",
    "whole_run_save_to_from_here",
    "run_items_together",
];

const SEEDS: std::ops::Range<u64> = 0..96;

const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

// ---- private ----

#[test]
fn whole_run_state_first_keeps_the_state_at_every_analysis() {
    keeps_state("whole_run_state_first", ir::RunExtra::WholeRunMidRun);
}

#[test]
fn whole_run_to_from_here_keeps_the_state_at_every_analysis() {
    keeps_state("whole_run_to_from_here", ir::RunExtra::StateWholeRun);
}

#[test]
fn whole_run_save_to_from_here_keeps_the_saves_at_every_analysis() {
    keeps_state("whole_run_save_to_from_here", ir::RunExtra::SaveWholeRun);
}

#[test]
fn run_items_together_keeps_the_state_at_every_analysis() {
    keeps_state("run_items_together", ir::RunExtra::RunInterleaved);
}

/// The chain for a target with neither whole-run state nor interleaved run items: legalize
/// moves, then makes positional, and leaves nothing to refuse.
#[test]
fn legalize_makes_whole_run_state_positional() {
    let text = "file \"a.sp\" deck\n  analysis op\n  element resistor r1 (a 0) value=1k\n  option rel_tol value=1m whole_run\n  save (v (a))\n  analysis op\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let before = state::states(&design);
    let lacks = [
        ir::RunExtra::WholeRunMidRun,
        ir::RunExtra::StateWholeRun,
        ir::RunExtra::SaveWholeRun,
        ir::RunExtra::RunInterleaved,
    ]
    .into_iter()
    .fold(ir::FeatureSet::EMPTY, |s, x| s.with(ir::Feature::Run(x)));
    let target = ir::FeatureSet::ALL.minus(lacks);
    passes::legalize(&mut design, &target, &mut diags);
    assert!(diags.is_empty(), "nothing to refuse: {:?}", diags.rows());
    assert_eq!(state::states(&design), before);
    let mut out = String::new();
    ir::dump(&design, design.entry(), &mut out);
    assert_eq!(
        out,
        "file \"a.sp\" deck\n  element resistor r1 (a 0) value=1k\n  option rel_tol value=1m\n  save from_here (v (a))\n  analysis op\n  analysis op\n"
    );
}

#[test]
fn every_run_pass_has_its_pair_deck_or_a_pending_marker_and_not_both() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    let mut proven = Vec::new();
    for cell in std::fs::read_dir(root).into_iter().flatten().flatten() {
        for deck in std::fs::read_dir(cell.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if deck.path().extension().is_some_and(|e| e == "pair") {
                let text = std::fs::read_to_string(deck.path()).unwrap_or_default();
                for line in text.lines() {
                    if let Some((k, v)) = line.split('#').next().unwrap_or("").split_once('=')
                        && k.trim() == "proves"
                    {
                        proven.push(v.trim().to_string());
                    }
                }
            }
        }
    }
    let group = passes::GROUPS
        .iter()
        .find(|g| g.name == "run")
        .expect("the group");
    let mut wrong = Vec::new();
    for pass in group.passes {
        if PENDING_PAIRS.contains(&pass.name) == proven.iter().any(|p| p == pass.name) {
            wrong.push(pass.name);
        }
    }
    wrong.extend(
        PENDING_PAIRS
            .iter()
            .filter(|p| !group.passes.iter().any(|q| q.name == **p)),
    );
    assert!(
        wrong.is_empty(),
        "pair deck against PENDING_PAIRS: {wrong:?}"
    );
}

/// The pass `name` over every generated design using `feature`: `ir::check` clean, every
/// analysis's state and every row reference unchanged.
fn keeps_state(name: &str, feature: ir::RunExtra) {
    let pass = passes::all().find(|p| p.name == name).expect("registered");
    let mut ran = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(ir::Feature::Run(feature)) {
            continue;
        }
        ran += 1;
        let before = state::states(&design);
        let refs_before = refs(&design);
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "{name} seed {seed}: {:?}", found.rows());
        let mut after = state::states(&design);
        let mut before = before;
        after.sort_by_key(|a| (a.0.kind as u8, a.0.index));
        before.sort_by_key(|a| (a.0.kind as u8, a.0.index));
        assert_eq!(
            after, before,
            "{name} seed {seed}: the state at an analysis changed"
        );
        let mut refs_after = refs(&design);
        let mut refs_before = refs_before;
        refs_after.sort_by_key(|a| (a.0.kind as u8, a.0.index));
        refs_before.sort_by_key(|a| (a.0.kind as u8, a.0.index));
        assert_eq!(
            refs_after, refs_before,
            "{name} seed {seed}: a reference moved"
        );
    }
    assert!(ran > 0, "{name}: no generated design had the feature");
    eprintln!("{name}: {ran} generated designs");
}

fn refs(design: &ir::Design) -> Vec<(ir::ItemRef, Vec<(ir::Ref, ir::DefKind)>)> {
    let reachable = design.reachable().into_iter();
    reachable
        .map(|(item, _)| {
            let mut out = Vec::new();
            design.item_refs(item, &mut out);
            (item, out)
        })
        .collect()
}
