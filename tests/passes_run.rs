//! The meaning of the run passes (`src/passes/run/`), on the IR alone, until the trunk's pair
//! decks prove them on the simulators (`PENDING_PAIRS`).
//!
//! Oracle: the state in force at each analysis (`passes_run/state.rs`, a model written from
//! the IR's docs, never from a pass): for every analysis, the same options, temperatures, node
//! conditions and saves before and after a pass, over generated designs (`ir::r#gen::generate`)
//! holding the pass's feature; and every row reference unchanged (`Design::item_refs`).
//!
//! Contents: tests only.

#[path = "common/memcap.rs"]
mod memcap;

#[path = "passes_structure/bindings.rs"]
mod bindings;
#[path = "passes_run/first_step.rs"]
mod first_step;
#[path = "passes_run/loops.rs"]
mod loops;
#[path = "passes_run/names.rs"]
mod names;
#[path = "passes_run/params.rs"]
mod params;
#[path = "passes_run/print_step.rs"]
mod print_step;
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
    "whole_run_option_first",
    "whole_run_env_first",
    "whole_run_node_set_first",
    "whole_run_save_first",
    "whole_run_option_to_from_here",
    "whole_run_env_to_from_here",
    "whole_run_node_set_to_from_here",
    "whole_run_save_to_from_here",
    "run_items_together",
    "tran_cap_ngspice_to_given",
    "tran_cap_span50_to_given",
    "tran_cap_span10_to_given",
    "tran_cap_none_to_given",
    "ic_beyond_next_to_next_only",
    "ic_next_only_to_beyond",
    "analysis_name_dropped",
    "dc_axis_name_dropped",
    "loop_name_dropped",
    "analysis_name_given",
    "dc_axis_name_given",
    "loop_name_given",
    "tran_print_step_to_simulator",
    "tran_simulator_step_to_print",
    "tran_first_step_to_print",
    "loop_restore_to_keep_last",
    "loop_keep_last_to_restore",
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
    keeps_state("whole_run_option_first", ir::RunExtra::OptionWholeRunMidRun);
    keeps_state("whole_run_env_first", ir::RunExtra::EnvWholeRunMidRun);
    keeps_state(
        "whole_run_node_set_first",
        ir::RunExtra::NodeSetWholeRunMidRun,
    );
    keeps_state("whole_run_save_first", ir::RunExtra::SaveWholeRunMidRun);
}

#[test]
fn whole_run_to_from_here_keeps_the_state_at_every_analysis() {
    keeps_state(
        "whole_run_option_to_from_here",
        ir::RunExtra::OptionWholeRun,
    );
    keeps_state("whole_run_env_to_from_here", ir::RunExtra::EnvWholeRun);
    keeps_state(
        "whole_run_node_set_to_from_here",
        ir::RunExtra::NodeSetWholeRun,
    );
}

#[test]
fn whole_run_save_to_from_here_keeps_the_saves_at_every_analysis() {
    keeps_state("whole_run_save_to_from_here", ir::RunExtra::SaveWholeRun);
}

#[test]
fn ic_beyond_next_to_next_only_keeps_the_state_at_every_analysis() {
    keeps_state("ic_beyond_next_to_next_only", ir::RunExtra::IcBeyondNext);
}

#[test]
fn ic_next_only_to_beyond_keeps_the_state_at_every_analysis() {
    keeps_state("ic_next_only_to_beyond", ir::RunExtra::IcNextOnly);
}

#[test]
fn names_dropped_keep_the_state_at_every_analysis() {
    keeps_state("analysis_name_dropped", ir::RunExtra::AnalysisNamed);
    keeps_state("dc_axis_name_dropped", ir::RunExtra::DcAxisNamed);
    keeps_state("loop_name_dropped", ir::RunExtra::LoopNamed);
}

#[test]
fn names_given_keep_the_state_at_every_analysis() {
    keeps_state("analysis_name_given", ir::RunExtra::AnalysisUnnamed);
    keeps_state("dc_axis_name_given", ir::RunExtra::DcAxisUnnamed);
    keeps_state("loop_name_given", ir::RunExtra::LoopUnnamed);
}

/// After each naming row (R59, A35), over generated designs holding its feature: every
/// statement of its kind has a name, but a loop it noted (an opaque body), no two share one
/// without case (DC axes and loops as one set), and each note's argument is the name now at its item. Each design runs twice:
/// as generated, and with its first named statement renamed to the upper case of the first
/// name the rule gave (a name the rule must then skip). Oracle: the rows' names read back,
/// against the rule's statement.
#[test]
fn names_given_name_every_statement_of_a_kind_once() {
    for (name, feature) in [
        ("analysis_name_given", ir::RunExtra::AnalysisUnnamed),
        ("dc_axis_name_given", ir::RunExtra::DcAxisUnnamed),
        ("loop_name_given", ir::RunExtra::LoopUnnamed),
    ] {
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let (mut ran, mut planted) = (0, 0);
        for seed in SEEDS {
            let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
            if !ir::features_used(&design).contains(ir::Feature::Run(feature)) {
                continue;
            }
            ran += 1;
            let Some(first) = names::named_once(pass, feature, &mut design, seed) else {
                continue;
            };
            let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
            if names::rename_first_named(&mut design, feature, &first.to_ascii_uppercase()) {
                planted += 1;
                names::named_once(pass, feature, &mut design, seed);
            }
        }
        assert!(ran > 0, "{name}: no generated design had the feature");
        assert!(
            planted > 0,
            "{name}: no design had a named statement to rename"
        );
    }
}

/// The naming rows and `names_dropped` are inverse pairs (R59): a target with both forms runs
/// neither; one with the named forms only gets every name. Oracle: the rule's statement, by hand.
#[test]
fn legalize_names_for_the_named_forms_and_runs_neither_pair_with_both() {
    let text = "file \"a.sp\" deck\n  title \"t\"\n  analysis op name=op1\n  analysis op\n  \
                loop sweep (temp) (list values=[1])\n    analysis dc (source_dc v1) (lin start=0 \
                stop=1 step=1)\n";
    let unnamed = [
        ir::RunExtra::AnalysisUnnamed,
        ir::RunExtra::DcAxisUnnamed,
        ir::RunExtra::LoopUnnamed,
    ];
    let named_only = unnamed.iter().fold(ir::FeatureSet::ALL, |s, &x| {
        s.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Run(x)))
    });
    for (target, want) in [
        (ir::FeatureSet::ALL, text.to_string()),
        (
            named_only,
            "file \"a.sp\" deck\n  title \"t\"\n  analysis op name=op1\n  analysis op name=op2\n  \
             loop sweep name=loop1 (temp) (list values=[1])\n    analysis dc name=dc1 (source_dc v1) \
             (lin start=0 stop=1 step=1) name=axis1\n"
                .to_string(),
        ),
    ] {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        passes::legalize(&mut design, &target, &mut diags);
        let refused: Vec<_> = diags.rows().iter().filter(|d| d.code == ir::diag::UNSUPPORTED).collect();
        assert!(refused.is_empty(), "{refused:?}");
        let mut out = String::new();
        ir::dump(&design, design.entry(), &mut out);
        assert_eq!(out, want);
    }
}

/// The loop-end rows keep the value every swept or altered parameter holds at every analysis
/// (`passes_run/params.rs`), over generated designs and the fixtures' cases.
#[test]
fn loop_ends_keep_every_parameter_value() {
    for (name, feature) in [
        ("loop_restore_to_keep_last", ir::RunExtra::LoopRestore),
        ("loop_keep_last_to_restore", ir::RunExtra::LoopKeepLast),
    ] {
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let mut ran = 0;
        let mut converted = 0;
        for seed in SEEDS {
            let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
            if !ir::features_used(&design).contains(ir::Feature::Run(feature)) {
                continue;
            }
            ran += 1;
            let before = params::values(&design);
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            converted += diags.rows().iter().filter(|d| d.code.number == 409).count();
            assert_eq!(params::values(&design), before, "{name} seed {seed}");
        }
        assert!(ran > 0, "{name}: no generated design");
        eprintln!("{name}: {ran} generated designs, {converted} loops left with U0409");
        // and the fixture's converted case, which the generated designs rarely reach
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/passes/run")
            .join(name);
        let text = std::fs::read_to_string(root.join("plain.ir")).expect("the plain case");
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(&text, &mut design.builder(), &mut diags);
        let before = params::values(&design);
        (pass.run)(&mut design, &mut diags);
        let feature = ir::Feature::Run(feature);
        assert!(
            !ir::features_used(&design).contains(feature),
            "{name}: plain"
        );
        assert_eq!(params::values(&design), before, "{name}: plain");
    }
}

/// `loop_end` over 300 seeded designs of the shape it converts (`passes_run/loops.rs`): the
/// value each parameter holds at each analysis (`passes_run/params.rs`) is unchanged, with
/// floors on the loops converted in each direction.
#[test]
fn loop_ends_keep_every_parameter_value_on_their_shape() {
    for (name, feature) in [
        ("loop_restore_to_keep_last", ir::RunExtra::LoopRestore),
        ("loop_keep_last_to_restore", ir::RunExtra::LoopKeepLast),
    ] {
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let (mut ran, mut converted, mut alters) = (0, 0, 0);
        for seed in 1..=300u64 {
            let text = loops::design(seed);
            let mut design = ir::Design::new();
            let mut diags = ir::Diags::new();
            ir::parse(&text, &mut design.builder(), &mut diags);
            assert!(diags.is_empty(), "seed {seed}: {:?}\n{text}", diags.rows());
            if !ir::features_used(&design).contains(ir::Feature::Run(feature)) {
                continue;
            }
            ran += 1;
            let before = params::values(&design);
            let count = |d: &ir::Design| {
                d.reachable()
                    .iter()
                    .filter(|(i, _)| i.kind == ir::ItemKind::Alter)
                    .count()
            };
            let alters_before = count(&design);
            (pass.run)(&mut design, &mut diags);
            let mut found = ir::Diags::new();
            ir::check(&design, &mut found);
            assert!(found.is_empty(), "{name} seed {seed}: {:?}", found.rows());
            assert_eq!(
                params::values(&design),
                before,
                "{name} seed {seed}\n{text}"
            );
            converted +=
                usize::from(!ir::features_used(&design).contains(ir::Feature::Run(feature)));
            alters += count(&design) - alters_before;
        }
        assert!(
            ran >= 100 && converted >= 50,
            "{name}: {ran} designs, {converted} converted"
        );
        eprintln!(
            "{name}: {ran} seeded designs, {converted} fully converted, {alters} alters added"
        );
    }
}

#[test]
fn run_items_together_keeps_the_state_at_every_analysis() {
    keeps_state("run_items_together", ir::RunExtra::RunInterleaved);
}

/// The ceiling rows against `StepCap`'s docs: for a grid of transients (stops, record starts
/// and print steps of several magnitudes, record start absent or set), the given ceiling
/// evaluates (reference evaluator) to the doc's arithmetic on the fields' values, bit for bit.
#[test]
fn tran_ceilings_are_the_rules_arithmetic() {
    use spicerack::ir::tran::{StepCap, StepCeiling, Tran};
    let rows = [
        (
            "tran_cap_ngspice_to_given",
            StepCap::NgspiceRule,
            "ngspice_rule",
        ),
        (
            "tran_cap_span50_to_given",
            StepCap::SpanOver50,
            "span_over50",
        ),
        (
            "tran_cap_span10_to_given",
            StepCap::SpanOver10,
            "span_over10",
        ),
        ("tran_cap_none_to_given", StepCap::None, "none"),
    ];
    let stops = ["1u", "3.3e-6", "1", "7.31e-9", "1e3", "0.1"];
    let starts = ["", "1n", "1.1e-7", "0.05"];
    let steps = ["1n", "1e-12", "2.5e-6", "0.01"];
    for (name, cap, word) in rows {
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let mut points = 0;
        for stop in stops {
            for start in starts {
                for step in steps {
                    let rs = if start.is_empty() {
                        String::new()
                    } else {
                        format!(" record_start={start}")
                    };
                    let text = format!(
                        "file \"a.sp\" deck\n  analysis tran stop={stop}{rs} print_step={step} cap={word}\n"
                    );
                    let mut design = ir::Design::new();
                    let mut diags = ir::Diags::new();
                    ir::parse(&text, &mut design.builder(), &mut diags);
                    if !diags.is_empty() {
                        continue; // a record start past the stop: outside the analysis's domain
                    }
                    (pass.run)(&mut design, &mut diags);
                    let a = design.row::<ir::Analysis>(
                        design.items(design.file(design.entry()).top)[0].index,
                    );
                    let t = design.detail::<Tran>(a.detail);
                    let StepCeiling::Given(g) = t.ceiling else {
                        panic!("{name}: {text} keeps its rule");
                    };
                    let env = bindings::ByDefinition(&design);
                    let value =
                        |e: ir::ExprId| ir::eval(&design, e, &env).expect("a literal field");
                    let span = if t.record_start.is_none() {
                        value(t.stop)
                    } else {
                        value(t.stop) - value(t.record_start)
                    };
                    let want = match cap {
                        StepCap::None => span,
                        StepCap::SpanOver10 => span / 10.0,
                        StepCap::SpanOver50 => span / 50.0,
                        StepCap::NgspiceRule => value(t.print_step).min(span / 50.0),
                    };
                    assert_eq!(value(g).to_bits(), want.to_bits(), "{name}: {text}");
                    points += 1;
                }
            }
        }
        assert!(points > 0, "{name}: no point");
        eprintln!("{name}: {points} points");
    }
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
        ir::RunExtra::OptionWholeRunMidRun,
        ir::RunExtra::EnvWholeRunMidRun,
        ir::RunExtra::NodeSetWholeRunMidRun,
        ir::RunExtra::SaveWholeRunMidRun,
        ir::RunExtra::OptionWholeRun,
        ir::RunExtra::EnvWholeRun,
        ir::RunExtra::NodeSetWholeRun,
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
        // every item kept holds the same references; an item added or dropped holds none
        let refs_after = refs(&design);
        let all = || refs_before.iter().chain(&refs_after);
        for (item, r) in all() {
            let changed = all().any(|(i, x)| i == item && x != r);
            assert!(
                !changed,
                "{name} seed {seed}: {item:?}'s references changed"
            );
            let kept = refs_before.iter().any(|(i, _)| i == item)
                && refs_after.iter().any(|(i, _)| i == item);
            assert!(
                kept || r.is_empty(),
                "{name} seed {seed}: {item:?} came or went"
            );
        }
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
