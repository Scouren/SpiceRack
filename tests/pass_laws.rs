//! The laws every registered pass meets (`rewrite/SPINE.md` §4), generic over
//! `passes::GROUPS`, over generated designs that use the pass's `removes` feature and over ones
//! that do not:
//! - P1: `ir::check` is clean after it;
//! - P2: every item still carrying `removes` (by `ir::check_legal`, per `Diag::item`) has a
//!   coded note of that pass (`Pass::notes`) at that item (`Diag::item`, not the span: a
//!   generated item has none), and every other item has lost it; a feature `check_legal`
//!   reports at no item (design-level) is noted by any note of the pass;
//! - P3: its output uses nothing outside its input's features plus `requires`;
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

use std::path::Path;

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes::{self, GROUPS, Pass};

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
        each_with(pass, |seed, mut design| {
            let allowed = ir::features_used(&design).union(pass.requires);
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            let extra: Vec<ir::Feature> =
                ir::features_used(&design).minus(allowed).iter().collect();
            match extra.is_empty() {
                true => Ok(()),
                false => Err(format!("seed {seed}: it adds {extra:?}")),
            }
        })
    });
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

/// `law` on every generated design that uses `pass.removes`; an error when none does.
fn each_with(
    pass: &Pass,
    law: impl Fn(u64, ir::Design) -> Result<(), String>,
) -> Result<(), String> {
    let mut ran = 0;
    for seed in SEEDS {
        let design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(pass.removes) {
            continue;
        }
        ran += 1;
        law(seed, design)?;
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
            // A design-level feature (`check_legal` reports it at no item, e.g. a state item
            // mid-run) is noted by any note of the pass, at whichever items the pass names.
            let design_level = item.index == u32::MAX;
            let noted = diags
                .rows()
                .iter()
                .any(|d| (design_level || d.item == item) && pass.notes.contains(&d.code));
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
