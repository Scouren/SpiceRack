//! The generator over the run rows: a seed and a feature set holding every run feature in, a
//! design out that passes `check`, uses only features of its set, round-trips through IR text
//! exactly, and is the same for the same seed; random subsets of the run features restrict it.
//! Oracle: the checker (tested against hand-broken designs in `run_check.rs`), the inverse
//! (`parse(dump(d)) == d`), byte compare.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (round-trip and generator rows); `rewrite/IR-DECISIONS.md`
//! kernel M4.
//!
//! Contents: tests only.

#![cfg(feature = "gen")]

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{
    AnalysisKind, BinOp, Design, Diags, Dist, DrawPer, DrawWhen, ExprTag, Extra, Feature,
    FeatureSet, Func, ItemKind, OptionKey, RunExtra, UnOp, check, dump, features_used, parse,
};

const SIZE: Size = Size {
    items: 40,
    scope_depth: 3,
    expr_depth: 3,
    files: 2,
};

/// The kernel harness's size (`kernel_gen.rs`), for the reach test.
const KERNEL_SIZE: Size = Size {
    items: 40,
    scope_depth: 3,
    expr_depth: 4,
    files: 2,
};

/// The run owner's item kinds.
const RUN_KINDS: [ItemKind; 12] = [
    ItemKind::Statistics,
    ItemKind::Analysis,
    ItemKind::Loop,
    ItemKind::Alter,
    ItemKind::Option,
    ItemKind::Env,
    ItemKind::NodeSet,
    ItemKind::Save,
    ItemKind::Print,
    ItemKind::Measure,
    ItemKind::Elaborate,
    ItemKind::Reset,
];

// ---- private ----

/// Every expression feature, the kernel's structure kinds and `Extra`s, and every run feature.
fn run_set() -> FeatureSet {
    let mut set = FeatureSet::EMPTY;
    for k in [
        ItemKind::Comment,
        ItemKind::Subckt,
        ItemKind::If,
        ItemKind::Param,
    ] {
        set = set.with(Feature::Item(k));
    }
    for k in RUN_KINDS {
        set = set.with(Feature::Item(k));
    }
    for t in ExprTag::ALL {
        set = set.with(Feature::Expr(t));
    }
    for f in Func::ALL {
        set = set.with(Feature::Func(f)).with(Feature::FuncDyn(f));
    }
    for op in BinOp::ALL {
        set = set.with(Feature::Binary(op)).with(Feature::BinaryDyn(op));
    }
    for op in UnOp::ALL {
        set = set.with(Feature::Unary(op)).with(Feature::UnaryDyn(op));
    }
    for d in Dist::ALL {
        set = set.with(Feature::Dist(d));
    }
    for w in DrawWhen::ALL {
        set = set.with(Feature::DrawWhen(w));
    }
    for p in DrawPer::ALL {
        set = set.with(Feature::DrawPer(p));
    }
    for e in Extra::ALL {
        set = set.with(Feature::Extra(e));
    }
    for a in AnalysisKind::ALL {
        set = set.with(Feature::Analysis(a));
    }
    for x in RunExtra::ALL {
        set = set.with(Feature::Run(x));
    }
    for k in OptionKey::ALL {
        set = set.with(Feature::Option(k));
    }
    set
}

fn text_of(d: &Design) -> String {
    let mut out = String::new();
    for f in d.files() {
        dump(d, f, &mut out);
    }
    out
}

/// Why `d` fails: its `check` rows, or the features outside `set`; empty when it passes.
fn faults(d: &Design, set: &FeatureSet) -> String {
    let mut diags = Diags::new();
    check(d, &mut diags);
    let outside: Vec<Feature> = features_used(d).minus(*set).iter().collect();
    if diags.is_empty() && outside.is_empty() {
        return String::new();
    }
    format!("check {:?}; outside the set {outside:?}", diags.rows())
}

fn round_trip(d: &Design) -> Option<String> {
    let text = text_of(d);
    let mut back = Design::new();
    let mut diags = Diags::new();
    parse(&text, &mut back.builder(), &mut diags);
    (!diags.is_empty() || !back.eq_ignoring_spans(d)).then(|| {
        format!(
            "{:?}\n{text}\n--- read back ---\n{}",
            diags.rows(),
            text_of(&back)
        )
    })
}

#[test]
fn generated_run_designs_are_valid_and_within_their_set() {
    let set = run_set();
    for seed in 0..300u64 {
        let d = generate(seed, &set, SIZE);
        let why = faults(&d, &set);
        assert!(why.is_empty(), "seed {seed}: {why}\n{}", text_of(&d));
    }
}

#[test]
fn every_run_kind_is_generated() {
    let set = run_set();
    let mut seen = Vec::new();
    for seed in 0..300u64 {
        let used = features_used(&generate(seed, &set, SIZE));
        for k in RUN_KINDS {
            if used.contains(Feature::Item(k)) && !seen.contains(&k) {
                seen.push(k);
            }
        }
    }
    seen.sort_by_key(|k| *k as u8);
    assert_eq!(seen, RUN_KINDS);
}

#[test]
fn random_run_subsets_restrict_the_generator() {
    let full: Vec<Feature> = run_set().iter().collect();
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for k in 0..20 {
        let mut set = FeatureSet::EMPTY;
        for &f in &full {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            if state >> 62 != 0 {
                set = set.with(f);
            }
        }
        for seed in 0..25u64 {
            let d = generate(seed, &set, SIZE);
            let why = faults(&d, &set);
            assert!(
                why.is_empty(),
                "subset {k}, seed {seed}: {why}\n{}",
                text_of(&d)
            );
        }
    }
}

#[test]
fn generated_run_designs_round_trip_through_ir_text() {
    let set = run_set();
    for seed in 0..2_000u64 {
        let d = generate(seed, &set, SIZE);
        if let Some(why) = round_trip(&d) {
            panic!("seed {seed}: parse(dump(d)) differs: {why}");
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_run_bytes() {
    let set = run_set();
    for seed in [0u64, 7, 99, 12_345] {
        assert_eq!(
            text_of(&generate(seed, &set, SIZE)),
            text_of(&generate(seed, &set, SIZE))
        );
    }
}

#[test]
fn every_run_feature_is_reached_on_a_second_seed_list() {
    // The kernel's full-set test reaches every feature over seeds 0..4000; the same must hold on
    // a disjoint list, so reach does not hang on one seed list. Run features only (the kernel's
    // own are its test's): every run item kind, analysis kind, run field feature and option key,
    // under every feature of the crate, as the kernel's harness runs it.
    let set = FeatureSet::ALL;
    let mut reached = FeatureSet::EMPTY;
    for seed in 50_000..54_000u64 {
        reached = reached.union(features_used(&generate(seed, &set, KERNEL_SIZE)));
    }
    let ours = |f: &Feature| match f {
        Feature::Item(k) => RUN_KINDS.contains(k),
        Feature::Analysis(_) | Feature::Run(_) | Feature::Option(_) => true,
        _ => false,
    };
    let missing: Vec<Feature> = set.minus(reached).iter().filter(ours).collect();
    assert!(
        missing.is_empty(),
        "run features no seed generated: {missing:?}"
    );
}
