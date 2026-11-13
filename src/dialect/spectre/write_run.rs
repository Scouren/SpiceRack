//! The run rows → Spectre text, the reader's image (`read_run.rs`) read back: `name dc` (the
//! operating point), `name dc dev=v1 param=dc …` / `name dc param=p …`, `name tran stop=
//! [step=] [maxstep=] [skipdc=yes]`, `name (p n) xf …`, the sweep as `start= stop= step=`,
//! `lin=` (points − 1), `dec=` or `values=[…]`; `name montecarlo numruns= [seed=] variations=
//! donominal= {`; `name options k=v` for an option or the temperatures (a made-up name);
//! `save a b`.
//!
//! Contents: `analysis`, `loop_head`, `option`, `env`, `save` (crate-private)

use super::expr_write::{write_expr, write_item};
use super::write::Writer;
use crate::ir;

/// One analysis.
pub(super) fn analysis(w: &mut Writer<'_, '_>, row: ir::Analysis) {
    let d = w.design;
    assert!(!row.name.is_none(), "FEATURES lacks AnalysisUnnamed");
    let name = d.str(row.name);
    w.line();
    match row.kind {
        ir::AnalysisKind::Op => w.out.push_str(&format!("{name} dc")),
        ir::AnalysisKind::Dc => {
            let dc: ir::dc_ac::Dc = d.detail(row.detail);
            let [axis] = d.dc_axes(dc.axes) else {
                unreachable!("FEATURES lacks DcNested")
            };
            w.out.push_str(&format!("{name} dc"));
            let a = d.str(axis.target.a.name);
            match axis.target.kind {
                ir::TargetKind::SourceDc => w.out.push_str(&format!(" dev={a} param=dc")),
                ir::TargetKind::Param => w.out.push_str(&format!(" param={a}")),
                other => unreachable!("FEATURES lacks the {other:?} axis"),
            }
            sweep(w, axis.sweep);
        }
        ir::AnalysisKind::Tran => {
            let t: ir::tran::Tran = d.detail(row.detail);
            w.out.push_str(&format!("{name} tran"));
            w.value("stop", t.stop);
            if !t.print_step.is_none() {
                w.value("step", t.print_step);
            }
            match t.ceiling {
                ir::tran::StepCeiling::Given(m) => w.value("maxstep", m),
                ir::tran::StepCeiling::Rule(ir::tran::StepCap::SpanOver50) => {}
                ir::tran::StepCeiling::Rule(other) => unreachable!("FEATURES lacks {other:?}"),
            }
            if t.uic {
                w.out.push_str(" skipdc=yes");
            }
        }
        ir::AnalysisKind::Xf => {
            let x: ir::dc_ac::Xf = d.detail(row.detail);
            let (p, n) = match x.output.kind {
                ir::ProbeKind::V => (path(d, x.output.a), "0".to_string()),
                ir::ProbeKind::Vdiff => (path(d, x.output.a), path(d, x.output.b)),
                other => unreachable!("FEATURES lacks the {other:?} output"),
            };
            w.out.push_str(&format!("{name} ({p} {n}) xf"));
            sweep(w, x.sweep);
        }
        other => unreachable!("FEATURES lacks {other:?}"),
    }
    w.out.push('\n');
}

/// A loop's head line, `{` included.
pub(super) fn loop_head(w: &mut Writer<'_, '_>, row: ir::Loop) {
    let d = w.design;
    assert!(
        row.kind == ir::LoopKind::MonteCarlo,
        "FEATURES lacks the {:?} loop",
        row.kind
    );
    assert!(!row.name.is_none(), "FEATURES lacks LoopUnnamed");
    let mc: ir::plan::MonteCarlo = d.detail(row.detail);
    w.line();
    w.out.push_str(&format!("{} montecarlo", d.str(row.name)));
    w.value("numruns", mc.runs);
    if !mc.seed.is_none() {
        w.value("seed", mc.seed);
    }
    let variations = match mc.variations {
        ir::plan::Variations::Process => "process",
        ir::plan::Variations::Mismatch => "mismatch",
        ir::plan::Variations::All => "all",
    };
    let nominal = if mc.nominal { "yes" } else { "no" };
    w.out.push_str(&format!(
        " variations={variations} donominal={nominal} {{\n"
    ));
}

/// An option for the whole run: `name options key=value`.
pub(super) fn option(w: &mut Writer<'_, '_>, row: ir::OptionRow) {
    let d = w.design;
    let key = match row.key {
        ir::OptionKey::RelTol => "reltol",
        ir::OptionKey::VnTol => "vabstol",
        ir::OptionKey::AbsTol => "iabstol",
        ir::OptionKey::Gmin => "gmin",
        ir::OptionKey::Private => d.str(row.private),
        other => unreachable!("FEATURES lacks Option({other:?})"),
    };
    let name = w.opt_name("opts");
    w.line();
    w.out.push_str(&format!("{name} options"));
    w.value(key, row.value);
    w.out.push('\n');
}

/// The temperatures for the whole run: `name options temp= tnom=`.
pub(super) fn env(w: &mut Writer<'_, '_>, row: ir::Env) {
    let name = w.opt_name("opts");
    w.line();
    w.out.push_str(&format!("{name} options"));
    if !row.temp.is_none() {
        w.value("temp", row.temp);
    }
    if !row.tnom.is_none() {
        w.value("tnom", row.tnom);
    }
    w.out.push('\n');
}

/// `save a b …`.
pub(super) fn save(w: &mut Writer<'_, '_>, row: ir::Save) {
    let d = w.design;
    w.line();
    w.out.push_str("save");
    for p in d.probes(row.probes) {
        assert!(
            p.kind == ir::ProbeKind::V,
            "FEATURES lacks the {:?} probe",
            p.kind
        );
        w.out.push(' ');
        w.out.push_str(&path(d, p.a));
    }
    w.out.push('\n');
}

// ---- private ----

/// A node path as one name (a top-level node: the path holds one name).
fn path(d: &ir::Design, list: ir::NameList) -> String {
    let names: Vec<&str> = d.names(list).iter().map(|&n| d.str(n)).collect();
    assert_eq!(names.len(), 1, "FEATURES lacks ProbeHierarchical");
    names[0].to_string()
}

/// ` start= stop= step=|lin=|dec=` or ` values=[…]`.
fn sweep(w: &mut Writer<'_, '_>, s: ir::SweepSpec) {
    let d = w.design;
    assert!(s.end == ir::SweepEnd::Grid, "FEATURES lacks SweepEndOnStop");
    assert!(
        s.bounds == ir::SweepBounds::StartStop,
        "FEATURES lacks SweepCenterSpan"
    );
    if s.kind == ir::SweepKind::List {
        w.out.push_str(" values=[");
        for (k, &v) in d.exprs(s.values).iter().enumerate() {
            if k > 0 {
                w.out.push(' ');
            }
            write_item(d, v, w.out);
        }
        w.out.push(']');
        return;
    }
    w.value("start", s.start);
    w.value("stop", s.stop);
    match s.kind {
        ir::SweepKind::Lin if !s.step.is_none() => w.value("step", s.step),
        ir::SweepKind::Lin => {
            w.out.push_str(" lin=");
            minus_one(d, s.points, w.out);
        }
        ir::SweepKind::Dec => w.value("dec", s.points),
        other => unreachable!("FEATURES lacks the {other:?} sweep"),
    }
}

/// The interval count of `points` points: a plain whole literal's predecessor, else `(e)-1`
/// (the reader's `plus_one` takes either back).
fn minus_one(d: &ir::Design, points: ir::ExprId, out: &mut String) {
    if let ir::Expr::Num(n) = d.expr(points) {
        let x = d.num(n);
        if x.form == ir::NumForm::Plain && x.frac == 0 && x.mantissa > 0 {
            let less = ir::Literal {
                mantissa: x.mantissa - 1,
                ..x
            };
            let less = match less.mantissa == 0 {
                true => ir::Literal::ZERO,
                false => less,
            };
            super::number::spell(less, out);
            return;
        }
    }
    out.push('(');
    write_expr(d, points, out);
    out.push_str(")-1");
}
