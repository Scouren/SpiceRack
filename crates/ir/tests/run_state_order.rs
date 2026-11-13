//! Which state item holds where (A28): a deck in, `in_force` at each analysis out, and the walks
//! `run_walks` lists. Oracle: ngspice's runs as `ng-control` settled them (S1, S3:
//! `state_option_temp.sp`, `state_reset.sp`): a script `option` beats an `.options` card until
//! `reset`, which drops the script's and keeps the card's; the expected item per analysis was
//! worked by hand from those facts.
//!
//! Cites: `rewrite/AMEND-A12.md` A28; `crates/ir/src/state_order.rs`, `crates/ir/src/option.rs`
//! (`Applies`).
//!
//! Contents: tests only.

use spicerack_ir::{
    Design, Diags, ItemKind, ItemRef, Listed, OptionKey, Setting, in_force, listed,
    parse_unchecked, run_walks,
};

// ---- private ----

fn read(text: &str) -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse_unchecked(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    d
}

/// The item in force for `what` at each analysis of the deck's walk, as the deck's top lists
/// them (`k` = the top's index of the holder, `None` = the default).
fn at_analyses(d: &Design, what: Setting) -> Vec<Option<usize>> {
    let (_, order) = run_walks(d).into_iter().next().expect("one walk");
    let top = d.items(d.file(d.entry()).top).to_vec();
    let index = |i: ItemRef| top.iter().position(|&t| t == i);
    order
        .iter()
        .enumerate()
        .filter(|(_, (i, _))| i.kind == ItemKind::Analysis)
        .map(|(k, _)| {
            let held = in_force(d, &order, k, what);
            (held != ItemRef::NONE).then(|| index(held).expect("a top item"))
        })
        .collect()
}

#[test]
fn a_from_here_option_beats_the_card_until_a_reset() {
    let d = read(
        "file \"t.sp\" deck
  analysis op
  option rel_tol value=1m
  analysis op
  reset options
  analysis op
  option rel_tol value=1u whole_run
",
    );
    // the card (item 5) holds wherever it stands; the script option (item 1) from where it
    // stands until the reset (item 3)
    assert_eq!(
        at_analyses(&d, Setting::Option(OptionKey::RelTol)),
        [Some(5), Some(1), Some(5)]
    );
    assert_eq!(
        at_analyses(&d, Setting::Option(OptionKey::AbsTol)),
        [None, None, None]
    );
}

#[test]
fn a_reset_of_another_class_keeps_the_option_and_a_later_from_here_wins() {
    let d = read(
        "file \"t.sp\" deck
  option rel_tol value=1m
  reset temp saves
  analysis op
  option rel_tol value=2m
  analysis op
",
    );
    assert_eq!(
        at_analyses(&d, Setting::Option(OptionKey::RelTol)),
        [Some(0), Some(3)]
    );
}

#[test]
fn temp_and_tnom_are_separate_settings() {
    let d = read(
        "file \"t.sp\" deck
  env temp=27 whole_run
  env tnom=30
  analysis op
  env temp=85
  analysis op
  reset temp
  analysis op
",
    );
    assert_eq!(at_analyses(&d, Setting::Temp), [Some(0), Some(3), Some(0)]);
    // the tnom-only env sets no temperature; the reset of temp keeps the tnom
    assert_eq!(at_analyses(&d, Setting::Tnom), [Some(1), Some(1), Some(1)]);
}

#[test]
fn the_last_whole_run_item_of_a_setting_holds() {
    let d = read(
        "file \"t.sp\" deck
  option rel_tol value=1m whole_run
  analysis op
  option rel_tol value=2m whole_run
",
    );
    assert_eq!(
        at_analyses(&d, Setting::Option(OptionKey::RelTol)),
        [Some(2)]
    );
}

#[test]
fn a_walk_reads_bodies_in_place_and_the_classes_are_the_irs() {
    let d = read(
        "file \"t.sp\" deck
  analysis op
  loop sweep (temp) (list values=[1])
    analysis op
  save (v (out))
",
    );
    let (_, order) = run_walks(&d).into_iter().next().expect("one walk");
    let kinds: Vec<ItemKind> = order.iter().map(|(i, _)| i.kind).collect();
    assert_eq!(
        kinds,
        [
            ItemKind::Analysis,
            ItemKind::Loop,
            ItemKind::Analysis,
            ItemKind::Save
        ]
    );
    assert_eq!(listed(ItemKind::Reset), Listed::Run);
    assert_eq!(listed(ItemKind::Opaque), Listed::Neutral);
    assert_eq!(listed(ItemKind::Param), Listed::Other);
}

/// `check`'s rows for `text` (the IR text of a whole design).
fn faults(text: &str) -> usize {
    let d = read(text);
    let mut diags = Diags::new();
    spicerack_ir::check(&d, &mut diags);
    diags.len()
}

#[test]
fn a_fourier_count_comes_from_the_nfreqs_option_in_force_in_its_walk() {
    // spine's L8 finding (seed 41): an option in an unrelated file gives the count nothing,
    // whatever the file order
    let four = "  analysis tran stop=1u print_step=1n cap=ngspice_rule\n  analysis four fundamental=1k periods=1 (v (out))\n";
    let lib = "file \"l.sp\" library\n  option n_freqs value=10 whole_run\n";
    let alone = format!("file \"t.sp\" deck\n{four}{lib}");
    assert_eq!(faults(&alone), 1, "{alone}");
    let included = format!("file \"t.sp\" deck\n  include \"l.sp\"\n{four}{lib}");
    assert_eq!(faults(&included), 0, "{included}");
    let reversed = format!("{lib}file \"t.sp\" deck\n  include \"l.sp\"\n{four}");
    assert_eq!(faults(&reversed), 0, "{reversed}");
    // a from-here option holds after it, and a reset of options ends it
    let before = format!("file \"t.sp\" deck\n  option n_freqs value=10\n{four}");
    assert_eq!(faults(&before), 0, "{before}");
    let after = format!("file \"t.sp\" deck\n{four}  option n_freqs value=10\n");
    assert_eq!(faults(&after), 1, "{after}");
    let reset = format!("file \"t.sp\" deck\n  option n_freqs value=10\n  reset options\n{four}");
    assert_eq!(faults(&reset), 1, "{reset}");
}

#[test]
fn a_next_only_and_a_standing_condition_of_one_node_at_one_analysis_are_refused() {
    // no simulator states both (ngspice's are standing, VACASK's next-only): refused
    let next = "  nodeset ic transient next_only node=(a) value=1\n";
    let card = "  nodeset ic transient node=(a) value=2 whole_run\n";
    let script = "  nodeset ic transient node=(a) value=2\n";
    let op = "  analysis op\n";
    let deck = |lines: &[&str]| format!("file \"t.sp\" deck\n{}", lines.concat());
    assert_eq!(faults(&deck(&[card, next, op])), 1, "a card anywhere");
    assert_eq!(
        faults(&deck(&[next, op, card])),
        1,
        "a card after it still holds"
    );
    assert_eq!(
        faults(&deck(&[script, next, op])),
        1,
        "a script one before it"
    );
    assert_eq!(
        faults(&deck(&[next, script, op])),
        1,
        "a script one before its analysis"
    );
    assert_eq!(
        faults(&deck(&[next, op, script, op])),
        0,
        "a script one after its analysis"
    );
    assert_eq!(
        faults(&deck(&[card, next])),
        0,
        "no analysis: nothing is in force"
    );
    let other = "  nodeset ic transient node=(b) value=2 whole_run\n";
    assert_eq!(faults(&deck(&[other, next, op])), 0, "another node");
    let guess = "  nodeset nodeset every_op node=(a) value=2 whole_run\n";
    assert_eq!(faults(&deck(&[guess, next, op])), 0, "another kind");
}

#[test]
fn check_legal_reports_a_design_level_run_feature_at_its_item() {
    // pass-structure's finding 1: the item between two run items, the state item after one
    let d = read(
        "file \"t.sp\" deck
  analysis op
  param p = 1
  option klu whole_run
",
    );
    let top: Vec<ItemRef> = d.items(d.file(d.entry()).top).to_vec();
    let legal = spicerack_ir::features_used(&d)
        .minus(
            spicerack_ir::FeatureSet::EMPTY.with(spicerack_ir::Feature::Run(
                spicerack_ir::RunExtra::RunInterleaved,
            )),
        )
        .minus(
            spicerack_ir::FeatureSet::EMPTY.with(spicerack_ir::Feature::Run(
                spicerack_ir::RunExtra::OptionWholeRunMidRun,
            )),
        );
    let mut diags = Diags::new();
    spicerack_ir::check_legal(&d, &legal, &mut diags);
    let at: Vec<ItemRef> = diags.rows().iter().map(|r| r.item).collect();
    assert_eq!(
        at,
        [top[2], top[1]],
        "the option after the analysis, the param between"
    );
}

#[test]
fn a_parameter_alter_at_reset_holds_from_the_next_full_reset() {
    // R70/A43 (the lead's ruling): an `AtReset` alter of a parameter holds from the first full
    // reset after it, never before; a partial reset does not apply it (a definition: no dialect
    // writes a partial reset after one, ngspice's is all or nothing). A full reset undoes the
    // device alters before it (ngspice's `reset` re-reads the deck) and applies the waiting
    // parameter alter instead. An `AtOnce` one holds from where it stands. Expected items
    // worked by hand (the top's index of the holder, `None` = the definition's own value).
    let d = read(
        "file \"t.sp\" deck
  param p = 1
  element resistor r1 (a 0) value=1k
  alter (element_value r1) value=2k
  alter (param p) value=2 at_reset
  analysis op
  reset values
  analysis op
  reset values options temp tnom saves
  analysis op
  alter (param p) value=3
  analysis op
",
    );
    let p = d.lookup(
        d.file(d.entry()).top,
        spicerack_ir::DefKind::Param,
        d.find_name("p"),
    );
    // 0 p, 1 r1, 2 device alter, 3 parameter alter at reset, 4 op, 5 partial reset, 6 op,
    // 7 full reset, 8 op, 9 parameter alter at once, 10 op
    assert_eq!(
        at_analyses(&d, Setting::Param(p)),
        [None, None, Some(3), Some(9)],
        "the parameter"
    );
    let r1 = d.find_name("r1");
    assert_eq!(
        at_analyses(&d, Setting::Device(r1, spicerack_ir::NameId::NONE)),
        [Some(2), None, None, None],
        "the device: undone by a reset of values, partial or full"
    );
}

#[cfg(feature = "gen")]
#[test]
fn a_generated_parameter_in_force_stands_before_and_is_applied() {
    // R70/A43 over generated walks (the evaluator's rule exercised with partial resets the
    // generator draws): what `in_force` names for a parameter is an alter of it that stands
    // before the point, and an `AtReset` one has a full reset between it and the point.
    // Oracle: those two invariants, read off the walk, not the rule's own loop.
    use spicerack_ir::r#gen::{Size, generate};
    use spicerack_ir::plan::{Alter, AlterEffect, TargetKind};
    use spicerack_ir::reset::{Reset, ResetSet};
    let size = Size {
        items: 40,
        scope_depth: 3,
        expr_depth: 3,
        files: 1,
    };
    let (mut checked, mut partial_after) = (0, 0);
    for seed in 0..300u64 {
        let d = generate(seed, &spicerack_ir::FeatureSet::ALL, size);
        for (_, order) in run_walks(&d) {
            let alters: Vec<(usize, Alter)> = order
                .iter()
                .enumerate()
                .filter(|(_, (i, _))| i.kind == ItemKind::Alter)
                .map(|(k, (i, _))| (k, d.row::<Alter>(i.index)))
                .filter(|(_, a)| a.target.kind == TargetKind::Param && !a.target.a.def.is_none())
                .collect();
            let reset_at = |k: usize| {
                let (i, _) = order[k];
                (i.kind == ItemKind::Reset).then(|| d.row::<Reset>(i.index).restores)
            };
            for &(k, a) in &alters {
                let later = (k + 1..order.len()).filter_map(reset_at);
                partial_after += usize::from(
                    a.effect == AlterEffect::AtReset && later.clone().any(|r| r != ResetSet::ALL),
                );
            }
            for at in 0..order.len() {
                for &(_, a) in &alters {
                    let held = in_force(&d, &order, at, Setting::Param(a.target.a.def));
                    if held == ItemRef::NONE {
                        continue;
                    }
                    let k = order[..at].iter().position(|(i, _)| *i == held);
                    let k = k.expect("the holder stands before the point");
                    let row: Alter = d.row(held.index);
                    assert_eq!(row.target.a.def, a.target.a.def, "seed {seed}");
                    if row.effect == AlterEffect::AtReset {
                        let full = (k + 1..at).filter_map(reset_at).any(|r| r == ResetSet::ALL);
                        assert!(full, "seed {seed}: applied with no full reset between");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 50, "{checked} points checked");
    assert!(
        partial_after > 0,
        "the generator drew a partial reset after an at-reset alter"
    );
}

#[test]
fn a_nested_dc_point_is_seeded_by_the_outermost_axis_that_advanced() {
    // R75 (F8): each DC axis says whether the solve after it advances starts from the previous
    // solve or from the initial guess; the outermost axis that advanced decides; the first
    // point is the initial guess. Oracle: a three-by-three grid (points 0..9, the inner axis
    // fastest: point = 3 × outer + inner) with each point's seed written out by hand, under
    // the four combinations (VACASK's `continuation=` per level, latch_c0_*.sim at
    // step3-runrules 8379e74f; ngspice is inner carries, outer afresh, latch_desc.sp)
    use spicerack_ir::dc_ac::dc_seeds;
    type Seeds = [Option<usize>; 9];
    let n = None;
    let cases: [((bool, bool), Seeds); 4] = [
        // (inner afresh, outer afresh)
        // both carry (VACASK's default): every point from the one before
        (
            (false, false),
            [
                n,
                Some(0),
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
            ],
        ),
        // outer afresh only (ngspice; VACASK continuation=0 on the outer sweep): each outer
        // step's first point starts over, the inner points carry
        (
            (false, true),
            [
                n,
                Some(0),
                Some(1),
                n,
                Some(3),
                Some(4),
                n,
                Some(6),
                Some(7),
            ],
        ),
        // inner afresh only (VACASK continuation=0 on the inner sweep): an outer step's first
        // point carries the previous step's last, every later inner point starts over
        ((true, false), [n, n, n, Some(2), n, n, Some(5), n, n]),
        // both afresh: every point from the initial guess
        ((true, true), [n; 9]),
    ];
    for ((inner, outer), want) in cases {
        assert_eq!(
            dc_seeds(&[3, 3], &[inner, outer]),
            want,
            "inner {inner}, outer {outer}"
        );
    }
    // one axis: the innermost bit alone (`DcRestart`)
    assert_eq!(dc_seeds(&[3], &[false]), [n, Some(0), Some(1)]);
    assert_eq!(dc_seeds(&[3], &[true]), [n, n, n]);
}
