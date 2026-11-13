//! `dc_axes_to_loops` on the simulators: a VACASK deck whose DC axis sweeps a device parameter,
//! run on VACASK, against the loops of operating points the rule writes for ngspice, run on
//! ngspice-45. Each op plot, in loop order, holds the source's DC point of the same index.
//!
//! The source's sweep continues from point to point (`Dc::restart` false) and the rule
//! converts only a restarting one, so the test sets `restart` on the read design: the stand-in
//! for a source that restarts. The axis is a diode's area, a device parameter by the reader's
//! one kind per meaning (a resistor's `r` is an element value, which no row converts). The
//! circuit (a resistor into a diode) has one operating point, the same however a point starts,
//! so the two runs still compare. Tolerance: 2·(RELTOL·|v| + VNTOL) with RELTOL 1e-3, VNTOL
//! 1e-6 V (`common/cmp.rs`'s DC rule).
//!
//! And R72's reason for converting only a restarting DC: `decks/bistable_dc.sp` (a continued
//! `.dc`) and `decks/bistable_loop.sp` (the same values by `foreach`/`alter`/`op`) on ngspice-45
//! give other numbers for one bistable circuit (-2 V and 0 V at v(in) = 0).
//!
//! Contents: `pub fn dc_loops_give_the_points_vacask_gives()`,
//! `pub fn a_loop_of_ops_is_no_continued_sweep()`,
//! `pub fn dc_axes_to_loops_convert_generated_designs(Range<u64>, Size)`

use std::path::Path;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes;
use spicerack::passes::run::dc_axes_to_loops::AS_LOOPS;

use crate::sim::{self, Ran, Sim};

/// Runs the deck on both simulators and compares `out` point by point.
pub fn dc_loops_give_the_points_vacask_gives() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/passes_run/decks/dc_device_param.sim");
    let text = std::fs::read_to_string(&path).expect("the deck");
    let (vacask, ngspice) = (
        dialect::by_name("vacask").unwrap(),
        dialect::by_name("ngspice").unwrap(),
    );
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one("dc_device_param.sim", &text),
        vacask,
        Policy::Strict,
        &mut design,
        &mut diags,
    );
    assert!(
        diags.rows().iter().all(|d| !dialect::refuses(d.code)),
        "read: {:?}",
        diags.rows()
    );
    let dc: Vec<ir::ItemRef> = design
        .reachable()
        .into_iter()
        .map(|(i, _)| i)
        .filter(|i| {
            i.kind == ir::ItemKind::Analysis
                && design.row::<ir::Analysis>(i.index).kind == ir::AnalysisKind::Dc
        })
        .collect();
    assert_eq!(dc.len(), 1, "one DC analysis");
    let detail = design.row::<ir::Analysis>(dc[0].index).detail;
    let mut row = design.detail::<ir::dc_ac::Dc>(detail);
    let kinds: Vec<_> = design
        .dc_axes(row.axes)
        .iter()
        .map(|x| x.target.kind)
        .collect();
    assert_eq!(
        kinds,
        [ir::plan::TargetKind::DeviceParam],
        "the axis the rows name"
    );
    row.restart = true;
    design.editor().set_detail(detail, row);
    let read = diags.len();
    dialect::legalize(&mut design, ngspice, &mut diags);
    let new = &diags.rows()[read..];
    assert!(
        new.iter().all(|d| !dialect::refuses(d.code)),
        "legalize: {new:?}"
    );
    assert!(
        new.iter().any(|d| d.code == AS_LOOPS),
        "the rule ran: {new:?}"
    );
    let written = dialect::write(&design, design.entry(), ngspice, &mut diags);
    let a = plots(sim::run(Sim::Vacask, &path, &text, &[], 30));
    let b = plots(sim::run(
        Sim::Ngspice,
        &path.with_extension("sp"),
        &written,
        &[],
        30,
    ));
    let source = column(&a[0], "out");
    assert_eq!(source.len(), 3, "VACASK's DC plot has the three points");
    let target: Vec<f64> = b
        .iter()
        .filter(|p| p.plot_name.eq_ignore_ascii_case("operating point"))
        .map(|p| column(p, "out")[0])
        .collect();
    assert_eq!(target.len(), 3, "one op plot a pass:\n{written}");
    for (k, (x, y)) in source.iter().zip(&target).enumerate() {
        let tol = 2.0 * (1e-3 * x.abs() + 1e-6);
        assert!(
            (x - y).abs() <= tol,
            "point {k}: VACASK {x}, ngspice {y}\n{written}"
        );
    }
    assert!(
        source[0] > source[1] && source[1] > source[2],
        "the area moved the point: {source:?}"
    );
}

/// The rows over the laws' own set (`gen_sets.rs`): some generated design converts.
pub fn dc_axes_to_loops_convert_generated_designs(seeds: std::ops::Range<u64>, size: Size) {
    // the laws' own set (`pass_laws.rs` GEN_SETS): restarting DC analyses with source and list
    // axes beside the row's; the laws mean something for the rows only if some design converts
    for (name, axis) in [
        (
            "dc_axis_device_param_to_loop",
            ir::RunExtra::DcAxisDeviceParam,
        ),
        (
            "dc_axis_model_param_to_loop",
            ir::RunExtra::DcAxisModelParam,
        ),
    ] {
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let set = crate::gen_sets::dc_loops(axis);
        let mut converted = 0;
        for seed in seeds.clone() {
            let mut design = generate(seed, &set, size);
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            let as_loops = passes::run::dc_axes_to_loops::AS_LOOPS;
            converted += diags.rows().iter().filter(|d| d.code == as_loops).count();
        }
        eprintln!("{name}: {converted} analyses converted");
        assert!(converted > 0, "{name}: no generated design converted");
    }
}

/// R72's run: a continued `.dc` and a loop of operating points disagree on a bistable circuit.
pub fn a_loop_of_ops_is_no_continued_sweep() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/passes_run/decks");
    let run = |name: &str| {
        let path = dir.join(name);
        let text = std::fs::read_to_string(&path).expect("the deck");
        plots(sim::run(Sim::Ngspice, &path, &text, &[], 30))
    };
    let dc = run("bistable_dc.sp");
    let swept = column(&dc[0], "out");
    let ops: Vec<f64> = run("bistable_loop.sp")
        .iter()
        .filter(|p| p.plot_name.eq_ignore_ascii_case("operating point"))
        .map(|p| column(p, "out")[0])
        .collect();
    assert_eq!((swept.len(), ops.len()), (7, 7), "seven points each");
    // v(in) = 0 V, the fifth point: the continued sweep holds the lower branch, the op from
    // the initial guess lands on the middle root
    assert!(
        (swept[4] + 2.0).abs() < 1e-3,
        "the .dc holds -2 V: {swept:?}"
    );
    assert!(ops[4].abs() < 1e-3, "the op finds 0 V: {ops:?}");
}

// ---- private ----

fn plots(r: Ran) -> Vec<spicerack::result::RawData> {
    match r {
        Ran::Plots(p) => p,
        Ran::Timeout => panic!("past the CPU budget"),
        Ran::Failed(why) => panic!("run failed: {why}"),
    }
}

/// The column of node `name` (`out` or `v(out)`), case ignored.
fn column(p: &spicerack::result::RawData, name: &str) -> Vec<f64> {
    let wrapped = format!("v({name})");
    let i = p
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case(name) || v.name.eq_ignore_ascii_case(&wrapped));
    let i = i.unwrap_or_else(|| {
        panic!(
            "no {name} in {:?}",
            p.variables.iter().map(|v| &v.name).collect::<Vec<_>>()
        )
    });
    p.real_data[i].clone()
}
