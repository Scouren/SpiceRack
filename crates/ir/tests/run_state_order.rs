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
    let four = "  analysis tran stop=1u print_step=1n cap=none\n  analysis four fundamental=1k periods=1 (v (out))\n";
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
