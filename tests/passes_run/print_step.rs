//! `tran_print_step_to_simulator`'s proof (R67, A41): the interval only resamples the output,
//! so the state at every analysis and every row reference stay (`keeps_state`, the model in
//! `passes_run/state.rs`), and over generated designs each transient's print step goes with a
//! `U0412` note carrying the old interval, or stays with `U0413` exactly where something reads
//! it (ngspice's ceiling rule, a `$tstep` leaf, further segments). Oracle: the rows read back
//! against the rule's statement.
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::ir::r#gen::generate;
use spicerack::ir::tran::{StepCap, StepCeiling, Tran};
use spicerack::passes;

use super::{SEEDS, SIZE, keeps_state};

// ---- private ----

#[test]
fn tran_print_step_to_simulator_keeps_the_state_at_every_analysis() {
    keeps_state("tran_print_step_to_simulator", ir::RunExtra::TranPrintStep);
}

#[test]
fn tran_print_step_to_simulator_drops_exactly_the_unread_intervals() {
    let name = "tran_print_step_to_simulator";
    let pass = passes::all().find(|p| p.name == name).expect("registered");
    let (mut dropped, mut kept) = (0, 0);
    // every feature, and the set without what reads an interval (else most are kept)
    let readers = ir::FeatureSet::EMPTY
        .with(ir::Feature::Expr(ir::ExprTag::TStep))
        .with(ir::Feature::Run(ir::RunExtra::TranCapNgspice))
        .with(ir::Feature::Run(ir::RunExtra::TranSegments));
    let sets = [ir::FeatureSet::ALL, ir::FeatureSet::ALL.minus(readers)];
    for (set, seed) in sets.iter().flat_map(|s| SEEDS.map(move |k| (s, k))) {
        let mut design = generate(seed, set, SIZE);
        if !ir::features_used(&design).contains(ir::Feature::Run(ir::RunExtra::TranPrintStep)) {
            continue;
        }
        let tstep = ir::features_used(&design).contains(ir::Feature::Expr(ir::ExprTag::TStep));
        let before = transients(&design);
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        for ((item, old), (_, new)) in before.iter().zip(transients(&design)) {
            if old.print_step.is_none() {
                assert_eq!(new.print_step, old.print_step, "seed {seed}: {item:?}");
                continue;
            }
            let read = tstep
                || old.ceiling == StepCeiling::Rule(StepCap::NgspiceRule)
                || old.segments.len > 0;
            let notes: Vec<&ir::Diag> = diags.rows().iter().filter(|d| d.item == *item).collect();
            assert_eq!(notes.len(), 1, "seed {seed}: one note at {item:?}");
            match read {
                true => {
                    assert_eq!(new.print_step, old.print_step, "seed {seed}: kept");
                    assert_eq!(notes[0].code.number, 413, "seed {seed}");
                    kept += 1;
                }
                false => {
                    assert!(new.print_step.is_none(), "seed {seed}: dropped");
                    assert_eq!(notes[0].code.number, 412, "seed {seed}");
                    assert_eq!(
                        notes[0].arg, old.print_step.0,
                        "seed {seed}: the old interval"
                    );
                    dropped += 1;
                }
            }
        }
    }
    eprintln!("{name}: {dropped} dropped, {kept} kept");
    assert!(dropped > 0, "no generated interval was dropped");
    assert!(kept > 0, "no generated interval was read");
}

/// Every reachable transient with its detail, in listed order.
fn transients(design: &ir::Design) -> Vec<(ir::ItemRef, Tran)> {
    let items = design.reachable().into_iter().map(|(i, _)| i);
    items
        .filter(|i| i.kind == ir::ItemKind::Analysis)
        .map(|i| (i, design.row::<ir::Analysis>(i.index)))
        .filter(|(_, a)| a.kind == ir::AnalysisKind::Tran)
        .map(|(i, a)| (i, design.detail::<Tran>(a.detail)))
        .collect()
}
