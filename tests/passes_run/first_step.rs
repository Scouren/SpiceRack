//! `tran_first_step_to_print`'s proof (R73): the state at every analysis kept (`keeps_state`);
//! over generated designs each first step moved into an absent print step, the note carrying
//! the old value, or kept exactly where something reads the new print step (a ceiling rule, a
//! `$tstep` leaf) or a print step stands, and a transient with neither noted; the bound pinned
//! on ngspice-45 by the settle decks of the rule's fixture folder (`-b -r`, the second time
//! point never past the print step); and sim(ngspice) on a moved deck (legalized for ngspice,
//! written, run: its first step taken no larger than the source's).
//!
//! And `tran_simulator_step_to_print`'s stop as the print step: ngspice-45 runs the same raw
//! file for every step at or past the stop (`decks/tss_*.sp`, each run in its title), and an
//! LTspice-shaped transient (no print step, a given ceiling) legalized and written for ngspice
//! runs the very file of the same deck with a step of 0.3 times its stop.
//!
//! Contents: tests only.

use std::path::Path;

use spicerack::dialect;
use spicerack::ir;
use spicerack::ir::r#gen::generate;
use spicerack::ir::tran::{StepCeiling, Tran};
use spicerack::passes;
use spicerack::rawfile::parse_raw;

use super::{SEEDS, keeps_state};

// ---- private ----

#[test]
fn tran_first_step_to_print_keeps_the_state_at_every_analysis() {
    keeps_state(
        "tran_simulator_step_to_print",
        ir::RunExtra::TranSimulatorStep,
    );
    keeps_state("tran_first_step_to_print", ir::RunExtra::TranInitialStep);
}

#[test]
fn tran_first_step_to_print_moves_exactly_the_unread_first_steps() {
    let pass = passes::all()
        .find(|p| p.name == "tran_first_step_to_print")
        .expect("registered");
    let tran = ir::FeatureSet::EMPTY
        .with(ir::Feature::Item(ir::ItemKind::Title))
        .with(ir::Feature::Item(ir::ItemKind::Analysis))
        .with(ir::Feature::Expr(ir::ExprTag::Num))
        .with(ir::Feature::Analysis(ir::AnalysisKind::Tran))
        .with(ir::Feature::Run(ir::RunExtra::AnalysisUnnamed))
        .with(ir::Feature::Run(ir::RunExtra::TranSimulatorStep))
        .with(ir::Feature::Run(ir::RunExtra::TranPrintStep))
        .with(ir::Feature::Run(ir::RunExtra::TranInitialStep))
        .with(ir::Feature::Run(ir::RunExtra::TranMaxStep))
        .with(ir::Feature::Run(ir::RunExtra::TranCapSpan50));
    let tstep = tran
        .with(ir::Feature::Item(ir::ItemKind::Param))
        .with(ir::Feature::Expr(ir::ExprTag::TStep));
    // and one where most first steps are free to move: no print step, and a given ceiling
    // two times in three (the generator draws a transient only beside some ceiling rule)
    let free = tran
        .minus(ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranPrintStep)))
        .minus(ir::FeatureSet::EMPTY.with(ir::Feature::Run(ir::RunExtra::TranCapSpan50)))
        .with(ir::Feature::Run(ir::RunExtra::TranCapSpan10));
    let (mut moved, mut kept, mut none) = (0, 0, 0);
    // the free set over four times the seeds: a move takes a first step (one in three) and a
    // given ceiling (one in three)
    let runs = [(tran, SEEDS), (tstep, SEEDS), (free, 0..4 * SEEDS.end)];
    for (set, seed) in runs
        .iter()
        .flat_map(|(s, r)| r.clone().map(move |k| (s, k)))
    {
        let mut design = generate(seed, set, super::SIZE);
        let leaf = ir::features_used(&design).contains(ir::Feature::Expr(ir::ExprTag::TStep));
        let before = transients(&design);
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        for ((item, old), (_, new)) in before.iter().zip(transients(&design)) {
            let notes: Vec<&ir::Diag> = diags.rows().iter().filter(|d| d.item == *item).collect();
            let (first, print) = (!old.initial_step.is_none(), !old.print_step.is_none());
            if !first && print {
                assert!(notes.is_empty(), "seed {seed}: {item:?} untouched");
                continue;
            }
            assert_eq!(notes.len(), 1, "seed {seed}: one note at {item:?}");
            let free = first && !print && !leaf && matches!(old.ceiling, StepCeiling::Given(_));
            match (free, first) {
                (true, _) => {
                    assert_eq!(new.print_step, old.initial_step, "seed {seed}: moved");
                    assert!(new.initial_step.is_none(), "seed {seed}: moved");
                    assert_eq!(notes[0].code.number, 419, "seed {seed}");
                    assert_eq!(notes[0].arg, old.initial_step.0, "seed {seed}: the value");
                    moved += 1;
                }
                (false, true) => {
                    assert_eq!(
                        (new.print_step, new.initial_step),
                        (old.print_step, old.initial_step)
                    );
                    assert_eq!(notes[0].code.number, 440, "seed {seed}");
                    kept += 1;
                }
                (false, false) => {
                    assert_eq!(notes[0].code.number, 441, "seed {seed}");
                    none += 1;
                }
            }
        }
    }
    eprintln!("tran_first_step_to_print: {moved} moved, {kept} kept, {none} with neither");
    assert!(moved >= 30, "{moved} generated first steps moved");
    assert!(kept >= 30, "{kept} generated first steps kept");
}

/// R73's bound on ngspice-45: the first step it takes is `min(print_step, stop / 100) / 100`,
/// never past the print step; the settle decks record the second time point.
#[test]
fn ngspice_takes_a_first_step_no_larger_than_its_print_step() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/passes/run/tran_first_step_to_print");
    for (deck, print, second) in [("settle_small", 1e-9, 1e-11), ("settle_large", 1e-6, 1e-9)] {
        let text = std::fs::read_to_string(dir.join(format!("{deck}.cir"))).expect(deck);
        let got = second_point(&text);
        assert_eq!(got, second, "{deck}: the second time point");
        assert!(got <= print, "{deck}: no larger than the print step");
    }
}

/// sim(ngspice): a VACASK-shaped transient (a first step, no print step, a given ceiling)
/// legalized for ngspice runs there, its first step taken no larger than the source's.
#[test]
fn a_moved_first_step_runs_on_ngspice_within_its_bound() {
    let text = "file \"rc.sp\" deck\n  title \"rc\"\n  element vsource v1 (a 0) dc=0 wave=sin(offset=0 amplitude=1 freq=100k delay=0 damping=0 phase_deg=0 pre=hold_delay_value)\n  element resistor r1 (a b) value=1k\n  element capacitor c1 (b 0) value=1n\n  analysis tran stop=10u initial_step=1n max_step=0.2u\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    passes::legalize(&mut design, &dialect::ngspice::FEATURES, &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let deck = dialect::write(&design, design.entry(), row, &mut diags);
    assert!(
        deck.contains(".tran 1n 10u"),
        "the first step is the print step:\n{deck}"
    );
    let got = second_point(&deck);
    assert!(
        got > 0.0 && got <= 1e-9,
        "{got}: the source's first step bounds it\n{deck}"
    );
}

/// The settle decks `tss_*`: the groups whose raw files are byte for byte one (time and every
/// vector), each with the point count its titles record, and the decks that differ from them.
#[test]
fn ngspice_steps_alike_for_every_print_step_at_or_past_the_stop() {
    let groups: [(&[&str], usize); 2] = [
        (&["tss_mid", "tss_stop", "tss_twice"], 136),
        (&["tss_stop_uic", "tss_twice_uic"], 135),
    ];
    for (decks, points) in groups {
        let first = raw(&deck(decks[0]));
        assert_eq!(first.real_data[0].len(), points, "{}: points", decks[0]);
        for d in &decks[1..] {
            assert_eq!(
                raw(&deck(d)).real_data,
                first.real_data,
                "{d} against {}",
                decks[0]
            );
        }
    }
    // the step binds below the stop/100 bound, and under uic below the stop
    for (d, like) in [
        ("tss_span100", "tss_mid"),
        ("tss_small", "tss_mid"),
        ("tss_mid_uic", "tss_stop_uic"),
    ] {
        assert_ne!(
            raw(&deck(d)).real_data,
            raw(&deck(like)).real_data,
            "{d} against {like}"
        );
    }
}

/// sim(ngspice): the settle deck read, its print step dropped (LTspice's shape: none, the ceiling
/// given), legalized and written for ngspice, runs the raw file of the deck with step 0.3m.
#[test]
fn an_ltspice_shaped_transient_runs_on_ngspice_as_with_no_step() {
    for (src, like) in [("tss_mid", "tss_mid"), ("tss_mid_uic", "tss_stop_uic")] {
        let ngspice = dialect::by_name("ngspice").expect("the ngspice row");
        let sources = dialect::Sources::one("deck.sp", &deck(src));
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        dialect::read(
            &sources,
            ngspice,
            dialect::Policy::Lenient,
            &mut design,
            &mut diags,
        );
        let (item, mut t) = transients(&design)[0];
        assert!(
            matches!(t.ceiling, StepCeiling::Given(_)),
            "{src}: the ceiling given"
        );
        t.print_step = ir::ExprId::NONE;
        let detail = design.row::<ir::Analysis>(item.index).detail;
        design.editor().set_detail(detail, t);
        let read = diags.len();
        dialect::legalize(&mut design, ngspice, &mut diags);
        let new = &diags.rows()[read..];
        assert!(
            new.iter().all(|d| !dialect::refuses(d.code)),
            "{src}: {new:?}"
        );
        assert!(
            new.iter().any(|d| d.code.number == 428),
            "{src}: the stop moved: {new:?}"
        );
        let written = dialect::write(&design, design.entry(), ngspice, &mut diags);
        assert!(
            written.contains(".tran 1m 1m"),
            "{src}: the stop as the step\n{written}"
        );
        assert_eq!(
            raw(&written).real_data,
            raw(&deck(like)).real_data,
            "{src}\n{written}"
        );
    }
}

/// A settle deck's text.
fn deck(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/passes_run/decks")
        .join(format!("{name}.sp"));
    std::fs::read_to_string(path).expect("the settle deck")
}

/// The second time point of `deck`'s transient, run by `ngspice -b -r` (an ASCII raw file).
fn second_point(deck: &str) -> f64 {
    let plot = raw(deck);
    let time = plot
        .variables
        .iter()
        .position(|v| v.name == "time")
        .expect("a time vector");
    plot.real_data[time][1]
}

/// `deck` run by `ngspice -b -r` (an ASCII raw file), its first plot.
fn raw(deck: &str) -> spicerack::result::RawData {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (cir, raw) = (dir.path().join("deck.cir"), dir.path().join("deck.raw"));
    std::fs::write(&cir, deck).expect("the deck is written");
    let out = std::process::Command::new("ngspice")
        .env("SPICE_ASCIIRAWFILE", "1")
        .args(["-b", "-r"])
        .arg(&raw)
        .arg(&cir)
        .output()
        .expect("ngspice runs (nix develop .#testing provides it; unsupported fails)");
    let bytes = std::fs::read(&raw).unwrap_or_else(|_| {
        panic!(
            "no raw file; ngspice said:\n{}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    parse_raw(&bytes).expect("a raw file")
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
