//! The laws every registered pass meets (`rewrite/SPINE.md` §4), generic over
//! `passes::GROUPS`, over generated designs that use the pass's `removes` feature and over ones
//! that do not:
//! - P1: `ir::check` is clean after it;
//! - P2: every item still carrying `removes` (by `ir::check_legal`, per `Diag::item`) has a
//!   coded note of that pass (`Pass::notes`) at that item (`Diag::item`, not the span: a
//!   generated item has none), and every other item has lost it (`check_legal` reports the run
//!   owner's design-level features at their items too);
//! - P3: its output uses nothing outside its input's features plus `requires`, a function,
//!   operator or draw compared without its form (static or dynamic: R65, A39);
//! - P4: running it twice equals running it once (IR text equal);
//! - P5: on a design without the feature nothing changes (IR text equal).
//!
//! And each pass's fixtures, `tests/fixtures/passes/<group>/<pass>/<case>.ir` read, passed and
//! written back equal to `<case>.out.ir`, the codes the pass appended equal, in order, to
//! `<case>.notes` (one code a line, `#` comments; A26). A case whose pass appends a row and has
//! no `.notes` fails, and so does a `.notes` that lists what the pass did not append; a fixture
//! folder naming no registered pass fails.
//!
//! Failing passes are compared with [`KNOWN`] by name, strictly both ways; a law that ran on no
//! generated design for a pass fails for it (a registered pass with no law run is a failure).
//! Meaning (the reference evaluator, or a simulator on both forms) is each pass's own test.
//!
//! Oracles: generated designs (`ir::gen::generate`, valid by construction), `ir::check`,
//! `ir::check_legal` (which items carry a feature), the canonical IR text (`ir::dump`), and
//! hand-written fixtures.
//!
//! Contents: tests only.

#[path = "common/memcap.rs"]
mod memcap;

use std::path::Path;

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes::{self, GROUPS, Pass};

#[path = "passes_run/gen_sets.rs"]
mod gen_sets;

/// The seeds every law draws designs from (printed on failure).
const SEEDS: std::ops::Range<u64> = 0..96;

/// A moderate design: the merge tier's size (REWRITE-PLAN §7.5).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// The laws a pass fails today: (law, pass, why). Strict both ways: a pass failing a law it
/// is not listed for fails, and so does a listed pass that passes.
const KNOWN: &[(&str, &str, &str)] = &[];

/// A pass's own generation set, for a feature the generator does not reach over every feature
/// (a competing feature always wins): the laws then also run on `generate(seed, set)`. The set
/// comes from the pass's row (its `removes`, the kinds that hold it, `Expr(Num)`, without the
/// competing features), as the IR owner who knows the generator states it; (pass name, set).
const GEN_SETS: &[(&str, ir::FeatureSet)] = &[
    (
        "pulse_spice_to_vacask",
        gen_sets::sources_only(
            &[ir::WaveKind::Pulse],
            ir::Feature::Circuit(ir::CircuitExtra::PulseVacask),
        ),
    ),
    (
        "given_zero_to_as_given",
        gen_sets::sources_only(
            &[
                ir::WaveKind::Pulse,
                ir::WaveKind::Sin,
                ir::WaveKind::Exp,
                ir::WaveKind::Sffm,
                ir::WaveKind::Am,
                ir::WaveKind::TrRandom,
            ],
            ir::Feature::Circuit(ir::CircuitExtra::GivenZeroAsGiven),
        ),
    ),
    (
        "pulse_vacask_to_spice",
        gen_sets::sources_only(
            &[ir::WaveKind::Pulse],
            ir::Feature::Circuit(ir::CircuitExtra::PulseSpice),
        ),
    ),
    (
        "am_pre_hold_to_zero",
        gen_sets::sources_only(
            &[ir::WaveKind::Am],
            ir::Feature::Circuit(ir::CircuitExtra::AmPreZero),
        ),
    ),
    (
        "as_given_to_given_zero",
        gen_sets::sources_only(
            &[
                ir::WaveKind::Pulse,
                ir::WaveKind::Sin,
                ir::WaveKind::Exp,
                ir::WaveKind::Sffm,
                ir::WaveKind::Am,
                ir::WaveKind::TrRandom,
            ],
            ir::Feature::Circuit(ir::CircuitExtra::GivenZeroSpiceDefault),
        ),
    ),
    (
        "exp_td2_relative",
        gen_sets::sources_only(
            &[ir::WaveKind::Exp],
            ir::Feature::Circuit(ir::CircuitExtra::ExpTd2FromTd1),
        ),
    ),
    (
        "exp_td2_absolute",
        gen_sets::sources_only(
            &[ir::WaveKind::Exp],
            ir::Feature::Circuit(ir::CircuitExtra::ExpTd2FromZero),
        ),
    ),
    (
        "sffm_ngspice_to_spice3",
        gen_sets::sources_only(
            &[ir::WaveKind::Sffm],
            ir::Feature::Circuit(ir::CircuitExtra::SffmSpice3),
        ),
    ),
    (
        "sffm_spice3_to_ngspice",
        gen_sets::sources_only(
            &[ir::WaveKind::Sffm],
            ir::Feature::Circuit(ir::CircuitExtra::SffmNgspice),
        ),
    ),
    ("hoist_enclosing_refs", HOIST_SET),
    ("hoist_enclosing_refs_default", HOIST_SET),
    (
        "dc_axis_device_param_to_loop",
        gen_sets::dc_loops(ir::RunExtra::DcAxisDeviceParam),
    ),
    (
        "dc_axis_model_param_to_loop",
        gen_sets::dc_loops(ir::RunExtra::DcAxisModelParam),
    ),
    (
        "tran_cap_ngspice_to_given",
        ir::FeatureSet::EMPTY
            .with(ir::Feature::Item(ir::ItemKind::Title))
            .with(ir::Feature::Item(ir::ItemKind::Analysis))
            .with(ir::Feature::Expr(ir::ExprTag::Num))
            .with(ir::Feature::Analysis(ir::AnalysisKind::Tran))
            .with(ir::Feature::Run(ir::RunExtra::TranCapNgspice))
            .with(ir::Feature::Run(ir::RunExtra::TranRecordStart)),
    ),
];

/// `hoist_enclosing_refs`' set (both rows): a subcircuit in a subcircuit reading its parent.
const HOIST_SET: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Title))
    .with(ir::Feature::Item(ir::ItemKind::Subckt))
    .with(ir::Feature::Item(ir::ItemKind::Param))
    .with(ir::Feature::Placed(
        ir::ScopeKind::Subckt,
        ir::ItemKind::Subckt,
    ))
    .with(ir::Feature::Placed(
        ir::ScopeKind::Subckt,
        ir::ItemKind::Param,
    ))
    .with(ir::Feature::Extra(ir::Extra::EnclosingRef))
    .with(ir::Feature::Expr(ir::ExprTag::Num))
    .with(ir::Feature::Expr(ir::ExprTag::Param))
    .with(ir::Feature::Extra(ir::Extra::SubcktConstantParam))
    .with(ir::Feature::Extra(ir::Extra::FormalNotConstant))
    .with(ir::Feature::Binary(ir::BinOp::Add))
    .with(ir::Feature::Binary(ir::BinOp::Mul));

/// The leaky self-test pass's note code.
const LEAKY: ir::Code = ir::Code::new(b'U', 399);

// ---- private ----

#[test]
fn p1_check_is_clean_after_the_pass() {
    laws("p1", |pass| {
        each_with(pass, |seed, mut design| {
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            let mut found = ir::Diags::new();
            ir::check(&design, &mut found);
            match found.is_empty() {
                true => Ok(()),
                false => Err(format!("seed {seed}: {:?}", found.rows())),
            }
        })
    });
}

#[test]
fn p2_every_item_left_carrying_the_feature_has_the_passes_note() {
    laws("p2", p2_on);
}

/// P2's self-test (A25): a pass that notes the first comment and leaves every comment fails
/// P2 by item. By span it passed: every generated item's span is `NONE`, so one note anywhere
/// matched every item.
#[test]
fn p2_by_item_catches_a_pass_that_notes_one_item_and_leaves_two() {
    let leaky = Pass {
        name: "leaky",
        removes: ir::Feature::Item(ir::ItemKind::Comment),
        requires: ir::FeatureSet::EMPTY,
        notes: &[LEAKY],
        run: note_first_comment,
    };
    let found = p2_on(&leaky).expect_err("P2 sees the comments left with no note");
    assert!(found.contains("keeps it with no note"), "{found}");
    let design = generate(0, &ir::FeatureSet::ALL, SIZE);
    let spans: Vec<ir::Span> = carrying(&design, leaky.removes)
        .iter()
        .map(|&i| design.span_of(i))
        .collect();
    assert!(
        spans.iter().all(|s| s.is_none()),
        "generated items have no span"
    );
}

#[test]
fn p3_the_output_uses_only_the_inputs_features_and_requires() {
    laws("p3", |pass| {
        each_with(pass, |seed, design| {
            p3_on(pass, design).map_err(|m| format!("seed {seed}: {m}"))
        })
    });
}

/// P3's self-test (R65, A39): on `v(n) + 1` beside a static `2 * 3`, a pass that writes
/// `v(n) + 1 * 1` only moves `Mul` from its static into its dynamic member, which P3 passes; one
/// that writes `v(n) + (1 - 1)` adds `Sub`, which neither form used, and P3 catches it.
#[test]
fn p3_passes_a_changed_form_and_catches_a_new_operator() {
    let form_only = stub("form_only", mul_one);
    p3_on(&form_only, p3_design()).expect("a moved form is no new feature");
    let adds = stub("adds_sub", sub_one);
    let found = p3_on(&adds, p3_design()).expect_err("Sub is new in either form");
    assert!(found.contains("BinaryDyn(Sub)"), "{found}");
}

#[test]
fn p4_twice_is_once() {
    laws("p4", |pass| {
        each_with(pass, |seed, mut design| {
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            let once = canonical(&design);
            (pass.run)(&mut design, &mut diags);
            match canonical(&design) == once {
                true => Ok(()),
                false => Err(format!("seed {seed}: a second run changes it")),
            }
        })
    });
}

#[test]
fn p5_a_design_without_the_feature_is_untouched() {
    laws("p5", |pass| {
        let without = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(pass.removes));
        let mut ran = 0;
        for seed in SEEDS {
            let mut design = generate(seed, &without, SIZE);
            if ir::features_used(&design).contains(pass.removes) {
                continue;
            }
            ran += 1;
            let before = canonical(&design);
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            if canonical(&design) != before {
                return Err(format!(
                    "seed {seed}: it changed a design without {:?}",
                    pass.removes
                ));
            }
        }
        ran_on(ran)
    });
}

#[test]
fn every_fixture_reads_passes_and_writes_its_expected_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/passes");
    let mut failures = Vec::new();
    let mut cases = 0;
    for group in dirs(&root) {
        for name in dirs(&root.join(&group)) {
            let Some(pass) = registered(&group, &name) else {
                failures.push(format!("{group}/{name}: no registered pass of that name"));
                continue;
            };
            for case in cases_in(&root.join(&group).join(&name)) {
                cases += 1;
                if let Err(m) = fixture(pass, &case) {
                    let file = case.file_name().map(|f| f.to_string_lossy().into_owned());
                    failures.push(format!("{group}/{name}/{}: {m}", file.unwrap_or_default()));
                }
            }
        }
    }
    assert!(cases > 0, "the fixture walk found no case");
    assert!(failures.is_empty(), "fixtures:\n{}", failures.join("\n"));
}

/// Runs `law` on every registered pass and compares the passes it fails on with [`KNOWN`],
/// strictly: an unexpected failure and an unexpected pass both fail, each with its message.
fn laws(name: &str, law: impl Fn(&Pass) -> Result<(), String>) {
    let mut failing = Vec::new();
    let mut messages = Vec::new();
    for pass in passes::all() {
        if let Err(m) = law(pass) {
            failing.push(pass.name);
            messages.push(format!("{}: {m}", pass.name));
        }
    }
    let known: Vec<&str> = KNOWN.iter().filter(|k| k.0 == name).map(|k| k.1).collect();
    assert_eq!(
        failing,
        known,
        "{name}: failing passes against KNOWN\n{}",
        messages.join("\n")
    );
}

/// `law` on every generated design that uses `pass.removes`, over every feature and then over
/// the pass's own [`GEN_SETS`] set if it has one; an error when none does.
fn each_with(
    pass: &Pass,
    law: impl Fn(u64, ir::Design) -> Result<(), String>,
) -> Result<(), String> {
    let mut ran = 0;
    let own = GEN_SETS
        .iter()
        .find(|(name, _)| *name == pass.name)
        .map(|(_, set)| *set);
    for set in std::iter::once(ir::FeatureSet::ALL).chain(own) {
        for seed in SEEDS {
            let design = generate(seed, &set, SIZE);
            if !ir::features_used(&design).contains(pass.removes) {
                continue;
            }
            ran += 1;
            law(seed, design)?;
        }
    }
    ran_on(ran)
}

/// `Ok` when the law ran on at least one design.
fn ran_on(ran: usize) -> Result<(), String> {
    match ran {
        0 => Err("no generated design to run on: the law ran on nothing".to_string()),
        _ => Ok(()),
    }
}

/// The spans of the items that carry `feature`: `check_legal` against every feature but it.
fn carrying(design: &ir::Design, feature: ir::Feature) -> Vec<ir::ItemRef> {
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let rows = rows
        .rows()
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index);
    rows.map(|d| d.item).collect()
}

/// P2 on one pass: every item still carrying `removes` after it has one of its notes at that
/// item (`Diag::item`).
fn p2_on(pass: &Pass) -> Result<(), String> {
    each_with(pass, |seed, mut design| {
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        for item in carrying(&design, pass.removes) {
            let noted = diags
                .rows()
                .iter()
                .any(|d| d.item == item && pass.notes.contains(&d.code));
            if !noted {
                return Err(format!(
                    "seed {seed}: the item {item:?} keeps it with no note"
                ));
            }
        }
        Ok(())
    })
}

/// The leaky self-test pass: one note, at the first comment, and no change.
fn note_first_comment(design: &mut ir::Design, diags: &mut ir::Diags) {
    let first = design
        .reachable()
        .into_iter()
        .find(|(i, _)| i.kind == ir::ItemKind::Comment);
    if let Some((item, _)) = first {
        diags.push_at(LEAKY, design, item, 0);
    }
}

/// P3 on one design: what `pass` leaves uses nothing outside what the design used plus
/// `requires`, an expression feature compared without its form (R65, A39).
fn p3_on(pass: &Pass, mut design: ir::Design) -> Result<(), String> {
    let allowed = ir::features_used(&design).union(pass.requires);
    let allowed: Vec<ir::Feature> = allowed.iter().map(static_member).collect();
    let mut diags = ir::Diags::new();
    (pass.run)(&mut design, &mut diags);
    let used = ir::features_used(&design);
    let extra: Vec<ir::Feature> = used
        .iter()
        .filter(|&f| !allowed.contains(&static_member(f)))
        .collect();
    match extra.is_empty() {
        true => Ok(()),
        false => Err(format!("it adds {extra:?}")),
    }
}

/// A function, operator or draw's static member; every other feature itself. A root's form is
/// a fact about the root (R51), so a pass that merges two roots or drops a caller moves what it
/// did not write between the two members (R65).
fn static_member(feature: ir::Feature) -> ir::Feature {
    match feature {
        ir::Feature::FuncDyn(f) => ir::Feature::Func(f),
        ir::Feature::BinaryDyn(op) => ir::Feature::Binary(op),
        ir::Feature::UnaryDyn(op) => ir::Feature::Unary(op),
        ir::Feature::DistDyn(d) => ir::Feature::Dist(d),
        other => other,
    }
}

/// A self-test pass of one rule, no `requires` and no notes.
fn stub(name: &'static str, run: fn(&mut ir::Design, &mut ir::Diags)) -> Pass {
    Pass {
        name,
        removes: ir::Feature::Item(ir::ItemKind::Comment),
        requires: ir::FeatureSet::EMPTY,
        notes: &[],
        run,
    }
}

/// P3's self-test design: a static `Mul` and a dynamic `Add` over a literal.
fn p3_design() -> ir::Design {
    let text = "file \"d.sp\" deck\n  param a = 2 * 3\n  element behav b1 (n 0) expr=(v(n) + 1)\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    design
}

/// Every literal of a dynamic root `x` becomes `x * x`.
fn mul_one(design: &mut ir::Design, _: &mut ir::Diags) {
    literal_to(design, ir::BinOp::Mul);
}

/// Every literal of a dynamic root `x` becomes `x - x`.
fn sub_one(design: &mut ir::Design, _: &mut ir::Diags) {
    literal_to(design, ir::BinOp::Sub);
}

fn literal_to(design: &mut ir::Design, op: ir::BinOp) {
    let changed = ir::rewrite_exprs_in(design, ir::Form::Dynamic, &mut |b, e| {
        let ir::Expr::Num(_) = b.design().expr(e) else {
            return None;
        };
        Some(b.binary(op, e, e))
    });
    assert_eq!(changed, 1, "one dynamic root holds a literal");
}

/// `≅`: the canonical IR text of every file.
fn canonical(design: &ir::Design) -> String {
    let mut out = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut out);
    }
    out
}

/// The pass `name` of group `group`, if registered.
fn registered(group: &str, name: &str) -> Option<&'static Pass> {
    let g = GROUPS.iter().find(|g| g.name == group)?;
    g.passes.iter().find(|p| p.name == name)
}

/// The sub-directories of `dir`, by name, sorted; none when it does not exist.
fn dirs(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

/// The `<case>.ir` inputs of a pass's folder (not the `.out.ir` files), sorted.
fn cases_in(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            n.ends_with(".ir") && !n.ends_with(".out.ir")
        })
        .collect();
    out.sort();
    out
}

/// One fixture: `case` read, checked, passed, checked, and written equal to its `.out.ir`.
fn fixture(pass: &Pass, case: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(case).map_err(|e| e.to_string())?;
    let expected_path = case.with_extension("out.ir");
    let expected =
        std::fs::read_to_string(&expected_path).map_err(|e| format!("{e}: its .out.ir"))?;
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(&text, &mut design.builder(), &mut diags);
    if !diags.is_empty() {
        return Err(format!("the input does not read clean: {:?}", diags.rows()));
    }
    (pass.run)(&mut design, &mut diags);
    let noted: Vec<String> = diags.rows().iter().map(|d| d.code.to_string()).collect();
    let listed = match std::fs::read_to_string(case.with_extension("notes")) {
        Ok(t) => t
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) if noted.is_empty() => Vec::new(),
        Err(_) => {
            return Err(format!(
                "the pass noted {noted:?} and the case has no .notes"
            ));
        }
    };
    if noted != listed {
        return Err(format!("notes {noted:?}, its .notes lists {listed:?}"));
    }
    ir::check(&design, &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|d| d.code.letter == b'X')
        .collect();
    if !refused.is_empty() {
        return Err(format!("check after the pass: {refused:?}"));
    }
    let got = canonical(&design);
    match got == expected {
        true => Ok(()),
        false => Err(format!("written\n{got}expected\n{expected}")),
    }
}
