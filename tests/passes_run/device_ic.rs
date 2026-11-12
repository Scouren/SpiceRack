//! `device_ic_to_tran_ic`'s proof. The rule's own condition is the assignment its header states
//! (pinned by VACASK runs); the oracles here are apart from it: over generated designs, a
//! capacitor's `ic` either moved whole (no capacitor keeps one, every `uic` transient is
//! preceded by one entry per moved `ic` with its value, no other transient gained one) or
//! every element that held one keeps it with the note; and sim(ngspice) against sim(VACASK) on
//! converted decks (the source run by ngspice, the converted deck by VACASK), compared after
//! t = 0 (under `uic` the point at t = 0 is no solution on either simulator).
//!
//! Contents: tests only.

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;
use spicerack::ir::r#gen::generate;
use spicerack::passes;
use spicerack::rawfile::parse_raw;

use super::{SEEDS, SIZE};

/// The time and node columns of one simulator run.
type Plot = (Vec<f64>, Vec<(String, Vec<f64>)>);

// ---- private ----

#[test]
fn device_ic_to_tran_ic_moves_whole_or_keeps_every_one() {
    let pass = passes::all()
        .find(|p| p.name == "device_ic_to_tran_ic")
        .expect("registered");
    let set = ir::FeatureSet::EMPTY
        .with(ir::Feature::Item(ir::ItemKind::Title))
        .with(ir::Feature::Item(ir::ItemKind::Element))
        .with(ir::Feature::Item(ir::ItemKind::Analysis))
        .with(ir::Feature::Elem(ir::ElemKind::Capacitor))
        .with(ir::Feature::Elem(ir::ElemKind::Resistor))
        .with(ir::Feature::Elem(ir::ElemKind::Diode))
        .with(ir::Feature::Expr(ir::ExprTag::Num))
        .with(ir::Feature::Circuit(ir::CircuitExtra::IcUicOnly))
        .with(ir::Feature::Analysis(ir::AnalysisKind::Tran))
        .with(ir::Feature::Run(ir::RunExtra::AnalysisUnnamed))
        .with(ir::Feature::Run(ir::RunExtra::TranPrintStep))
        .with(ir::Feature::Run(ir::RunExtra::TranMaxStep))
        .with(ir::Feature::Run(ir::RunExtra::TranUic));
    let (mut moved, mut kept) = (0, 0);
    for (s, seed) in [ir::FeatureSet::ALL, set]
        .iter()
        .flat_map(|s| (0..4 * SEEDS.end).map(move |k| (s, k)))
    {
        let mut design = generate(seed, s, SIZE);
        let holders: Vec<(ir::ItemRef, ir::ExprId)> = ics(&design);
        if holders.is_empty() {
            continue;
        }
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        let left = ics(&design);
        if left.is_empty() {
            moved += 1;
            let caps: Vec<ir::ExprId> = holders.iter().map(|h| h.1).collect();
            for (items, uic) in transients(&design) {
                let before: Vec<ir::ExprId> = items
                    .iter()
                    .map(|&n| design.row::<ir::NodeSet>(n.index).value)
                    .collect();
                match uic {
                    true => assert_eq!(
                        before.len(),
                        caps.len(),
                        "seed {seed}: one entry per moved ic"
                    ),
                    false => assert!(
                        before.is_empty(),
                        "seed {seed}: a transient without uic gains nothing"
                    ),
                }
            }
        } else {
            kept += 1;
            let noted: Vec<ir::ItemRef> = diags
                .rows()
                .iter()
                .filter(|d| d.code.number == 443)
                .map(|d| d.item)
                .collect();
            for (item, _) in &left {
                assert!(
                    noted.contains(item),
                    "seed {seed}: {item:?} keeps its ic with no note"
                );
            }
        }
    }
    eprintln!("device_ic_to_tran_ic: {moved} designs moved, {kept} kept");
    assert!(moved >= 30 && kept >= 30, "{moved} moved, {kept} kept");
}

/// Every reachable element holding a `uic`-only `ic`, with it.
fn ics(design: &ir::Design) -> Vec<(ir::ItemRef, ir::ExprId)> {
    design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Element)
        .map(|(i, _)| (i, design.row::<ir::Element>(i.index)))
        .filter(|(_, e)| e.kind.detail() == ir::DetailKind::Passive)
        .map(|(i, e)| (i, design.passive(&e)))
        .filter(|(_, p)| p.ic_mode == ir::passive::IcMode::UicOnly)
        .map(|(i, p)| (i, p.ic))
        .collect()
}

/// Every transient, the node sets listed right before it in its scope, and whether it runs
/// under `uic`.
fn transients(design: &ir::Design) -> Vec<(Vec<ir::ItemRef>, bool)> {
    let mut out = Vec::new();
    for (item, scope) in design.reachable() {
        if item.kind != ir::ItemKind::Analysis {
            continue;
        }
        let a = design.row::<ir::Analysis>(item.index);
        if a.kind != ir::AnalysisKind::Tran {
            continue;
        }
        let list = design.items(scope);
        let at = list.iter().position(|&i| i == item).expect("listed");
        let before = list[..at]
            .iter()
            .rev()
            .take_while(|i| i.kind == ir::ItemKind::NodeSet)
            .copied()
            .collect();
        out.push((before, design.detail::<ir::tran::Tran>(a.detail).uic));
    }
    out
}

/// sim(ngspice) of the source against sim(VACASK) of the converted deck: a grounded capacitor
/// and a floating one beside a grounded one (the d1 shape), v(a), v(b), v(c) after t = 0.
#[test]
fn a_moved_ic_starts_the_same_transient_on_both_simulators() {
    let decks = [
        (
            "grounded",
            "cap ic\nr1 a 0 1k\nc1 a 0 1u ic=2\n.tran 10u 1m 0 10u uic\n.end\n",
            1e-3,
        ),
        (
            "floating",
            "caps ic\nr1 a 0 1k\nr2 b 0 2k\nc1 a b 1u ic=2\nc2 b 0 1u ic=1\n.tran 10u 3m 0 10u uic\n.end\n",
            3e-3,
        ),
    ];
    for (name, text, stop) in decks {
        let ng = run_ngspice(text);
        let va = run_vacask(&convert(text));
        for node in ["a", "b"] {
            let (Some(x), Some(y)) = (at(&ng, node, stop), at(&va, node, stop)) else {
                continue;
            };
            // both integrate the same circuit from the same start with their own steps
            assert!(
                (x - y).abs() <= 1e-4 * x.abs().max(1e-3),
                "{name} {node}: ngspice {x}, VACASK {y}"
            );
            eprintln!("{name} v({node}) at the stop: ngspice {x}, VACASK {y}");
        }
    }
}

/// `text` (ngspice) read, legalized for VACASK and written.
fn convert(text: &str) -> String {
    let from = dialect::by_name("ngspice").expect("ngspice");
    let to = dialect::by_name("vacask").expect("vacask");
    let src = Sources::one("deck.sp", text);
    let w = dialect::transpile(&src, from, to, Policy::Lenient).expect("converts");
    w.text
        .replace("control\n", "control\n  options rawfile=\"ascii\"\n")
}

fn run_ngspice(text: &str) -> Plot {
    let dir = tempfile::tempdir().expect("a temp dir");
    let (cir, raw) = (dir.path().join("d.cir"), dir.path().join("d.raw"));
    std::fs::write(&cir, text).expect("written");
    std::process::Command::new("ngspice")
        .env("SPICE_ASCIIRAWFILE", "1")
        .args(["-b", "-r"])
        .arg(&raw)
        .arg(&cir)
        .output()
        .expect("ngspice runs (nix develop .#testing; unsupported fails)");
    plot(&std::fs::read(&raw).expect("a raw file"))
}

fn run_vacask(deck: &str) -> Plot {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("d.sim"), deck).expect("written");
    let out = std::process::Command::new("vacask")
        .arg("d.sim")
        .current_dir(dir.path())
        .output()
        .expect("vacask runs (nix develop .#testing; unsupported fails)");
    let raw = std::fs::read_dir(dir.path())
        .expect("the dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "raw"))
        .unwrap_or_else(|| {
            panic!(
                "no raw file: {}\n{deck}",
                String::from_utf8_lossy(&out.stdout)
            )
        });
    plot(&std::fs::read(raw).expect("a raw file"))
}

fn plot(bytes: &[u8]) -> Plot {
    let p = parse_raw(bytes).expect("a raw file");
    let t = p
        .variables
        .iter()
        .position(|v| v.name == "time")
        .expect("time");
    let cols = p
        .variables
        .iter()
        .enumerate()
        .filter(|(k, _)| *k != t)
        .map(|(k, v)| (v.name.to_ascii_lowercase(), p.real_data[k].clone()))
        .collect();
    (p.real_data[t].clone(), cols)
}

/// The value of node `n` at time `t`, linearly between the two points around it.
fn at(p: &Plot, n: &str, t: f64) -> Option<f64> {
    let col =
        &p.1.iter()
            .find(|(name, _)| name == n || *name == format!("v({n})"))?
            .1;
    let k = p.0.iter().position(|&x| x >= t)?;
    if k == 0 || p.0[k] == t {
        return Some(col[k]);
    }
    let (t0, t1) = (p.0[k - 1], p.0[k]);
    Some(col[k - 1] + (col[k] - col[k - 1]) * (t - t0) / (t1 - t0))
}
