//! The generator's properties: every generated design passes `check` and uses only the
//! requested features (for the kernel set and for 20 random subsets); IR text round-trips it
//! exactly (`parse(dump(d))` equals `d`, 2,000 seeds); the same seed gives the same bytes. Over
//! every feature (`FeatureSet::ALL`, M5), 400 seeds are valid and round-trip, and 4,000 between
//! them use every feature a valid design can hold.
//!
//! Oracles: the checker (tested against hand-broken designs in `check_tests`); the inverse
//! (text, tested against hand-built rows in `kernel_text`); byte compare. Seeds are fixed and
//! printed on failure with the smallest failing size (shrinking by `Size`).
//!
//! Contents: none (tests only).

#![cfg(feature = "gen")]

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{
    BinOp, Design, Diags, Dist, DrawPer, DrawWhen, ExprTag, Extra, Feature, FeatureSet, Func,
    ItemKind, UnOp, check, dump, features_used, parse,
};

const SIZE: Size = Size {
    items: 40,
    scope_depth: 3,
    expr_depth: 4,
    files: 2,
};

// ---- private ----

/// Every feature the kernel generates: its 11 item kinds, every expression tag and member, its
/// `Extra`s.
fn kernel_set() -> FeatureSet {
    let kinds = [
        ItemKind::Title,
        ItemKind::Comment,
        ItemKind::Include,
        ItemKind::Global,
        ItemKind::Load,
        ItemKind::Opaque,
        ItemKind::Subckt,
        ItemKind::Section,
        ItemKind::If,
        ItemKind::Param,
        ItemKind::Func,
    ];
    let mut set = FeatureSet::EMPTY;
    for k in kinds {
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
    set
}

fn text_of(d: &Design) -> String {
    let mut out = String::new();
    for f in d.files() {
        dump(d, f, &mut out);
    }
    out
}

/// The first property `holds` fails for, shrunk: the smallest item budget that still fails.
fn shrink(seed: u64, set: &FeatureSet, holds: impl Fn(&Design) -> bool) -> Option<Size> {
    let failing = |items| !holds(&generate(seed, set, Size { items, ..SIZE }));
    if !failing(SIZE.items) {
        return None;
    }
    (1..=SIZE.items)
        .find(|&n| failing(n))
        .map(|items| Size { items, ..SIZE })
}

fn valid_and_within(d: &Design, set: &FeatureSet) -> bool {
    let mut diags = Diags::new();
    check(d, &mut diags);
    diags.is_empty() && features_used(d).minus(*set).is_empty()
}

fn round_trips(d: &Design) -> bool {
    let mut back = Design::new();
    let mut diags = Diags::new();
    parse(&text_of(d), &mut back.builder(), &mut diags);
    diags.is_empty() && back.eq_ignoring_spans(d)
}

/// splitmix64 for the random subsets (the test's own PRNG).
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[test]
fn generated_designs_are_valid_and_within_their_set() {
    let set = kernel_set();
    for seed in 0..300u64 {
        let d = generate(seed, &set, SIZE);
        if !valid_and_within(&d, &set) {
            let small = shrink(seed, &set, |d| valid_and_within(d, &set));
            panic!("seed {seed}: invalid or outside its set; smallest failing size {small:?}");
        }
    }
}

#[test]
fn random_subsets_restrict_the_generator() {
    let full = kernel_set();
    let mut state = 0x005E_B5E7_u64;
    for k in 0..20u64 {
        let mut set = FeatureSet::EMPTY;
        for f in full.iter() {
            if next(&mut state).is_multiple_of(2) {
                set = set.with(f);
            }
        }
        for seed in 0..25u64 {
            let d = generate(seed, &set, SIZE);
            assert!(
                valid_and_within(&d, &set),
                "subset {k}, seed {seed}: {:?}",
                features_used(&d).minus(set).iter().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn ir_text_round_trips_generated_designs() {
    let set = kernel_set();
    for seed in 0..2_000u64 {
        let d = generate(seed, &set, SIZE);
        if !round_trips(&d) {
            let small = shrink(seed, &set, round_trips);
            panic!(
                "seed {seed}: parse(dump(d)) differs; smallest failing size {small:?}\n{}",
                text_of(&d)
            );
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_bytes() {
    let set = kernel_set();
    for seed in [0u64, 7, 99, 12_345] {
        assert_eq!(
            text_of(&generate(seed, &set, SIZE)),
            text_of(&generate(seed, &set, SIZE))
        );
    }
    let a = text_of(&generate(1, &set, SIZE));
    let b = text_of(&generate(2, &set, SIZE));
    assert_ne!(a, b, "different seeds give different designs");
}

#[test]
fn the_full_set_runs_every_property() {
    // M5: the owners' kinds generate, so the properties run over every feature
    let set = FeatureSet::ALL;
    let mut reached = FeatureSet::EMPTY;
    for seed in 0..400u64 {
        let d = generate(seed, &set, SIZE);
        if !valid_and_within(&d, &set) {
            let small = shrink(seed, &set, |d| valid_and_within(d, &set));
            panic!(
                "seed {seed}: invalid; smallest failing size {small:?}\n{}",
                text_of(&d)
            );
        }
        if !round_trips(&d) {
            let small = shrink(seed, &set, round_trips);
            panic!(
                "seed {seed}: parse(dump(d)) differs; smallest failing size {small:?}\n{}",
                text_of(&d)
            );
        }
    }
    // a `ddt`/`idt` makes its tree read the circuit (class `Circuit`), so it only ever counts as
    // `FuncDyn`: no valid design holds these two
    let unreachable = [Feature::Func(Func::Ddt), Feature::Func(Func::Idt)];
    let goal = unreachable
        .iter()
        .fold(FeatureSet::ALL, |s, f| s.minus(FeatureSet::EMPTY.with(*f)));
    // reaching the rare kinds takes more seeds than the properties need; stops once reached
    for seed in 0..4000u64 {
        if goal.minus(reached).is_empty() {
            break;
        }
        reached = reached.union(features_used(&generate(seed, &set, SIZE)));
    }
    let missing: Vec<Feature> = FeatureSet::ALL.minus(reached).iter().collect();
    assert_eq!(missing, unreachable, "features no seed generated");
}

#[test]
fn a_set_without_items_generates_files_only() {
    let d = generate(3, &FeatureSet::EMPTY, SIZE);
    assert!(features_used(&d).is_empty());
    assert!(d.files().count() >= 1);
}

#[test]
fn check_legal_agrees_with_features_used() {
    // check_legal is empty exactly when features_used(d).minus(legal) is, and reports exactly
    // that difference (compared as sets of feature indices)
    let full = kernel_set();
    let mut state = 0xC4EC_u64;
    for seed in 0..40u64 {
        let d = generate(seed, &full, SIZE);
        let used = features_used(&d);
        for k in 0..20 {
            let mut legal = FeatureSet::EMPTY;
            for f in full.iter() {
                if next(&mut state).is_multiple_of(3) {
                    legal = legal.with(f);
                }
            }
            let mut diags = Diags::new();
            spicerack_ir::check_legal(&d, &legal, &mut diags);
            let mut got: Vec<u32> = diags.rows().iter().map(|r| r.arg).collect();
            got.sort_unstable();
            got.dedup();
            let want: Vec<u32> = used.minus(legal).iter().map(|f| f.index() as u32).collect();
            assert_eq!(got, want, "seed {seed}, subset {k}");
        }
    }
}
