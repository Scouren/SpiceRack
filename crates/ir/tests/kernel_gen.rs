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
    ItemKind, ScopeKind, UnOp, check, dump, features_used, parse,
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
    // a title stands only in a deck's top, so no placement of one exists (A15 K1)
    let titles = ScopeKind::ALL[1..]
        .iter()
        .map(|&s| Feature::Placed(s, ItemKind::Title));
    // (A28, R51: an uncalled function body counts static, so `Func(Ddt)` and `Func(Idt)` are
    // reached through one; they were unreachable while a body counted dynamic always)
    let unreachable: Vec<Feature> = titles.collect();
    // known gaps, a marker: none left (the run owner's generators reach `LoopOverOp`)
    let not_yet: [Feature; 0] = [];
    let goal = unreachable
        .iter()
        .chain(&not_yet)
        .fold(FeatureSet::ALL, |s, f| s.minus(FeatureSet::EMPTY.with(*f)));
    // reaching the rare kinds takes more seeds than the properties need; stops once reached
    for seed in 0..4000u64 {
        if goal.minus(reached).is_empty() {
            break;
        }
        reached = reached.union(features_used(&generate(seed, &set, SIZE)));
    }
    let missing: Vec<Feature> = FeatureSet::ALL.minus(reached).iter().collect();
    let mut want: Vec<Feature> = unreachable.iter().chain(&not_yet).copied().collect();
    want.sort_by_key(|f| f.index());
    assert_eq!(missing, want, "features no seed generated");
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

#[test]
fn refs_and_map_refs_list_the_same_references_in_the_same_order() {
    // M5: the read-only twin drifts from the visitor by no reference, kind or position, for
    // every item of every kind (FeatureSet::ALL reaches all; oracle: map_refs itself)
    let mut kinds_seen = FeatureSet::EMPTY;
    for seed in 0..300u64 {
        let d = generate(seed, &FeatureSet::ALL, SIZE);
        let items: Vec<_> = d.reachable().into_iter().map(|(item, _)| item).collect();
        let mut scratch = d.clone();
        for item in items {
            let mut read = Vec::new();
            d.item_refs(item, &mut read);
            let mut visited = Vec::new();
            scratch
                .editor()
                .map_refs(item, &mut |r, kind| visited.push((*r, kind)));
            assert_eq!(read, visited, "seed {seed}, {item:?}");
            if !read.is_empty() {
                kinds_seen = kinds_seen.with(Feature::Item(item.kind));
            }
        }
    }
    // the kinds that hold references were exercised with some
    for kind in [
        ItemKind::Element,
        ItemKind::Model,
        ItemKind::Alter,
        ItemKind::Loop,
        ItemKind::Elaborate,
    ] {
        assert!(
            kinds_seen.contains(Feature::Item(kind)),
            "{kind:?} with a reference"
        );
    }
}

#[test]
fn a_generated_load_names_a_file_of_its_kind() {
    // A25 (R48): VACASK decides a load's kind by the extension, so the generator draws what a
    // simulator could mean: `.va` for a Verilog-A source, `.osdi` for a compiled binary
    let mut seen = [false; 2];
    for seed in 0..300u64 {
        let d = generate(seed, &FeatureSet::ALL, SIZE);
        for (item, _) in d.reachable() {
            if item.kind != ItemKind::Load {
                continue;
            }
            let row: spicerack_ir::item::Load = d.row(item.index);
            let path = d.str(row.path);
            let want = match row.kind {
                spicerack_ir::LoadKind::VerilogA => ".va",
                spicerack_ir::LoadKind::Osdi => ".osdi",
            };
            assert!(
                path.ends_with(want),
                "seed {seed}: {:?} at {path}",
                row.kind
            );
            seen[row.kind as usize] = true;
        }
    }
    assert_eq!(seen, [true; 2], "both kinds generated");
}

#[test]
fn a_set_without_a_default_rule_draws_no_default_under_it() {
    // R62/A37 (oracle: `features_used`, which reads each default's references): a set holding
    // `FormalNotConstant` but not `FormalOuterRead` (`FormalUnboundRead`) draws no default
    // bound outside its subcircuit (unbound) over 300 seeds; with the rule, some seed draws one
    for rule in [Extra::FormalOuterRead, Extra::FormalUnboundRead] {
        let without = FeatureSet::ALL.minus(FeatureSet::EMPTY.with(Feature::Extra(rule)));
        let mut drawn = false;
        for seed in 0..300u64 {
            let f = Feature::Extra(rule);
            assert!(
                !features_used(&generate(seed, &without, SIZE)).contains(f),
                "{rule:?}, seed {seed}"
            );
            drawn |= features_used(&generate(seed, &FeatureSet::ALL, SIZE)).contains(f);
        }
        assert!(drawn, "{rule:?} is drawn when the set holds it");
    }
}
