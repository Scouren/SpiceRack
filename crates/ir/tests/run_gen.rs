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

#[test]
fn a_named_only_set_draws_no_unnamed_row_and_names_split_exactly() {
    // R59/A35: each row sets exactly one of its pair; a set holding only the named member draws
    // named rows only (VACASK's laws)
    let drop = [
        RunExtra::AnalysisUnnamed,
        RunExtra::DcAxisUnnamed,
        RunExtra::LoopUnnamed,
    ];
    let named_only = drop.iter().fold(run_set(), |s, &x| {
        s.minus(FeatureSet::EMPTY.with(Feature::Run(x)))
    });
    let mut analyses = 0;
    for seed in 0..200u64 {
        let d = generate(seed, &named_only, SIZE);
        let used = features_used(&d);
        for x in drop {
            assert!(!used.contains(Feature::Run(x)), "seed {seed}: {x:?}");
        }
        let text = text_of(&d);
        analyses += text.matches("analysis ").count();
        // every analysis line names itself
        for line in text
            .lines()
            .filter(|l| l.trim_start().starts_with("analysis "))
        {
            assert!(line.contains(" name="), "seed {seed}: {line}");
        }
    }
    assert!(analyses > 0, "the named-only set still draws analyses");
}

#[test]
fn every_generated_named_row_sets_exactly_one_of_its_pair() {
    // R59/A35 as a property over generated designs: `check_legal` against the empty set reports
    // each item's features at it
    use spicerack_ir::{ItemRef, check_legal};
    let pair = |named, unnamed| [Feature::Run(named), Feature::Run(unnamed)];
    let analysis = pair(RunExtra::AnalysisNamed, RunExtra::AnalysisUnnamed);
    let lp = pair(RunExtra::LoopNamed, RunExtra::LoopUnnamed);
    let axis = pair(RunExtra::DcAxisNamed, RunExtra::DcAxisUnnamed);
    let mut seen = 0;
    for seed in 0..200u64 {
        let d = generate(seed, &run_set(), SIZE);
        let mut diags = Diags::new();
        check_legal(&d, &FeatureSet::EMPTY, &mut diags);
        let at = |item: ItemRef, of: &[Feature; 2]| {
            of.iter()
                .filter(|f| {
                    diags
                        .rows()
                        .iter()
                        .any(|r| r.item == item && r.arg == f.index() as u32)
                })
                .count()
        };
        for (item, _) in d.reachable() {
            match item.kind {
                ItemKind::Analysis => {
                    assert_eq!(at(item, &analysis), 1, "seed {seed}, {item:?}");
                    let row: spicerack_ir::Analysis = d.row(item.index);
                    if row.kind == AnalysisKind::Dc {
                        // one per axis: at least one, both only with several axes
                        let n = at(item, &axis);
                        let dc: spicerack_ir::dc_ac::Dc = d.detail(row.detail);
                        assert!(n >= 1 && (n == 1 || dc.axes.len > 1), "seed {seed}");
                    }
                    seen += 1;
                }
                ItemKind::Loop => {
                    assert_eq!(at(item, &lp), 1, "seed {seed}, {item:?}");
                    seen += 1;
                }
                _ => {} // else: only analyses, DC axes and loops carry a name pair
            }
        }
    }
    assert!(seen > 100, "the property ran on {seen} rows");
}

#[test]
fn every_generated_transient_sets_exactly_one_print_step_member() {
    // R67/A41: `TranPrintStep` and `TranSimulatorStep` split the transients exactly; a set with
    // only one of them draws only that one (VACASK's laws hold no print step)
    use spicerack_ir::{ItemRef, check_legal};
    let [print, chosen] = [RunExtra::TranPrintStep, RunExtra::TranSimulatorStep];
    let without = |x: RunExtra| run_set().minus(FeatureSet::EMPTY.with(Feature::Run(x)));
    let mut seen = [0usize; 3];
    for (k, set) in [run_set(), without(print), without(chosen)]
        .iter()
        .enumerate()
    {
        for seed in 0..400u64 {
            let d = generate(seed, set, SIZE);
            let mut diags = Diags::new();
            check_legal(&d, &FeatureSet::EMPTY, &mut diags);
            let at = |item: ItemRef, f: RunExtra| {
                let f = Feature::Run(f).index() as u32;
                diags.rows().iter().any(|r| r.item == item && r.arg == f)
            };
            for (item, _) in d.reachable() {
                let row = (item.kind == ItemKind::Analysis)
                    .then(|| d.row::<spicerack_ir::Analysis>(item.index));
                if row.is_none_or(|a| a.kind != AnalysisKind::Tran) {
                    continue;
                }
                let (p, c) = (at(item, print), at(item, chosen));
                assert!(p != c, "seed {seed}, set {k}, {item:?}: exactly one");
                assert!(
                    k != 1 || c,
                    "seed {seed}: only TranSimulatorStep in the set"
                );
                assert!(k != 2 || p, "seed {seed}: only TranPrintStep in the set");
                seen[k] += 1;
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 20), "transients per set: {seen:?}");
}

#[test]
fn a_set_without_the_r68_features_draws_neither() {
    // R68/A42 (2), (4): without `AnalysisParamArgument` no analysis argument or sweep range
    // reads a parameter; without `PssArgumentOmitted` every periodic steady state states all
    // four of its optional arguments (VACASK's and ngspice's laws)
    let drop = [
        RunExtra::AnalysisParamArgument,
        RunExtra::PssArgumentOmitted,
    ];
    let set = drop.iter().fold(run_set(), |s, &x| {
        s.minus(FeatureSet::EMPTY.with(Feature::Run(x)))
    });
    let (mut analyses, mut psses) = (0, 0);
    for seed in 0..200u64 {
        let d = generate(seed, &set, SIZE);
        let used = features_used(&d);
        for x in drop {
            assert!(!used.contains(Feature::Run(x)), "seed {seed}: {x:?}");
        }
        let text = text_of(&d);
        analyses += text.matches("analysis ").count();
        psses += text.matches("analysis pss").count();
    }
    assert!(
        analyses > 200 && psses > 5,
        "{analyses} analyses, {psses} pss"
    );
}

#[test]
fn every_generated_parameter_alter_states_one_effect() {
    // R70/A43: `AlterParamAtOnce` and `AlterParamAtReset` split the parameter alters exactly; a
    // set with only one of them draws only that one (VACASK's and ngspice's laws)
    use spicerack_ir::plan::{Alter, AlterEffect};
    let [now, later] = [RunExtra::AlterParamAtOnce, RunExtra::AlterParamAtReset];
    let without = |x: RunExtra| run_set().minus(FeatureSet::EMPTY.with(Feature::Run(x)));
    let mut seen = [0usize; 3];
    for (k, set) in [run_set(), without(later), without(now)].iter().enumerate() {
        for seed in 0..400u64 {
            let d = generate(seed, set, SIZE);
            let used = features_used(&d);
            for (item, _) in d.reachable() {
                if item.kind != ItemKind::Alter {
                    continue;
                }
                let row: Alter = d.row(item.index);
                if !row.target.kind.is_parameter() {
                    assert_eq!(row.effect, AlterEffect::AtOnce, "seed {seed}, set {k}");
                    continue;
                }
                assert!(k != 1 || row.effect == AlterEffect::AtOnce, "seed {seed}");
                assert!(k != 2 || row.effect == AlterEffect::AtReset, "seed {seed}");
                seen[k] += 1;
            }
            let x = if k == 1 { later } else { now };
            assert!(
                k == 0 || !used.contains(Feature::Run(x)),
                "seed {seed}, set {k}: {x:?}"
            );
        }
    }
    assert!(
        seen.iter().all(|&n| n > 10),
        "parameter alters per set: {seen:?}"
    );
}

#[test]
fn every_generated_outer_dc_axis_states_one_continuation() {
    // R75: `DcOuterCarries` and `DcOuterAfresh` split the axes above the innermost exactly; a set
    // with only one of them draws only that one (ngspice's set holds the afresh twin only)
    use spicerack_ir::dc_ac::Dc;
    let [carries, afresh] = [RunExtra::DcOuterCarries, RunExtra::DcOuterAfresh];
    let without = |x: RunExtra| run_set().minus(FeatureSet::EMPTY.with(Feature::Run(x)));
    let mut seen = [0usize; 3];
    for (k, set) in [run_set(), without(afresh), without(carries)]
        .iter()
        .enumerate()
    {
        for seed in 0..400u64 {
            let d = generate(seed, set, SIZE);
            for (item, _) in d.reachable() {
                if item.kind != ItemKind::Analysis {
                    continue;
                }
                let row: spicerack_ir::Analysis = d.row(item.index);
                if row.kind != AnalysisKind::Dc {
                    continue;
                }
                let dc: Dc = d.detail(row.detail);
                for axis in &d.dc_axes(dc.axes)[1..] {
                    assert!(
                        k != 1 || !axis.afresh,
                        "seed {seed}: only the carrying twin"
                    );
                    assert!(k != 2 || axis.afresh, "seed {seed}: only the afresh twin");
                    seen[k] += 1;
                }
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 10), "outer axes per set: {seen:?}");
}

#[test]
fn generated_element_targets_name_real_elements_above_a_floor() {
    // the lead's reach finding: with only fresh names no generated alter, sweep or DC axis ever
    // set a real element's parameter, so the laws never saw one. Floor: of the element targets
    // without a path in 400 designs, at least 40 name an element of the design that
    // `unbound_targets` does not list
    use spicerack_ir::dc_ac::Dc;
    use spicerack_ir::plan::{SweepLoop, TableLoop};
    use spicerack_ir::{Alter, Element, Loop, LoopKind, Target, TargetKind, unbound_targets};
    let set = FeatureSet::ALL; // elements and run items together
    let (mut real, mut all) = (0usize, 0usize);
    for seed in 0..400u64 {
        let d = generate(seed, &set, SIZE);
        let mut targets: Vec<Target> = Vec::new();
        let mut elements = std::collections::HashSet::new();
        for (item, _) in d.reachable() {
            match item.kind {
                ItemKind::Element => {
                    elements.insert(d.row::<Element>(item.index).name);
                }
                ItemKind::Alter => targets.push(d.row::<Alter>(item.index).target),
                ItemKind::Loop => {
                    let l: Loop = d.row(item.index);
                    match l.kind {
                        LoopKind::Sweep => targets.push(d.detail::<SweepLoop>(l.detail).target),
                        LoopKind::Table => {
                            let t: TableLoop = d.detail(l.detail);
                            targets.extend_from_slice(d.targets(t.columns));
                        }
                        LoopKind::MonteCarlo => {}
                    }
                }
                ItemKind::Analysis => {
                    let a: spicerack_ir::Analysis = d.row(item.index);
                    if a.kind == AnalysisKind::Dc {
                        let dc: Dc = d.detail(a.detail);
                        targets.extend(d.dc_axes(dc.axes).iter().map(|x| x.target));
                    }
                }
                _ => {}
            }
        }
        let unbound: Vec<_> = unbound_targets(&d).iter().map(|u| u.name).collect();
        let element_kinds = [
            TargetKind::SourceDc,
            TargetKind::ElementValue,
            TargetKind::DeviceParam,
        ];
        for t in targets
            .iter()
            .filter(|t| element_kinds.contains(&t.kind) && t.path.len == 0)
        {
            all += 1;
            if elements.contains(&t.a.name) && !unbound.contains(&t.a.name) {
                real += 1;
            }
        }
    }
    assert!(
        real >= 40,
        "{real} of {all} element targets name a real element"
    );
}
