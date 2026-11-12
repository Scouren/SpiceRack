//! An `Analysis` row → `analysis name kind k=v …` (a `Dc`: its axes' `sweep` lines, outermost
//! first, then `analysis name op`), with the `ic=`/`nodeset=` lists of the `NodeSet` rows just
//! before it in its scope; a sweep's range and target spelled as VACASK's keys. The inverse of
//! `read_run::analysis` (VD cmd-analysis-*, cmd-sweep; a lin count is VACASK's interval count,
//! one less than the IR's values).
//!
//! Every analysis and axis has its name: `FEATURES` holds the named forms only, and the run
//! group's naming rows give one (R59, A35).
//!
//! Contents: `analysis`, `range`, `target` (crate-private)

use super::{name_of, quote};
use crate::dialect::vacask::expr_write::write_expr;
use crate::dialect::vacask::name::{self, Place};
use crate::dialect::vacask::number;
use crate::dialect::vacask::write::Writer;
use crate::ir;

/// Analysis `at`: its line (and a `Dc`'s sweep lines before it).
pub(super) fn analysis(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let row: ir::Analysis = d.row(at.index);
    assert!(!row.name.is_none(), "FEATURES lacks AnalysisUnnamed");
    let name = d.str(row.name).to_string();
    let scope = w.scopes.get(&at).copied();
    super::fold_alters(w, at);
    if row.kind == ir::AnalysisKind::Dc {
        let dc: ir::dc_ac::Dc = d.detail(row.detail);
        let axes: Vec<ir::dc_ac::DcAxis> = axes(d, dc.axes).to_vec();
        for axis in axes.iter().rev() {
            assert!(!axis.name.is_none(), "FEATURES lacks DcAxisUnnamed");
            w.line();
            w.out.push_str("sweep ");
            name::spell(d.str(axis.name), Place::Other, w.out);
            target(d, axis.target, w.out);
            range(d, axis.sweep, w.out);
            w.out.push('\n');
        }
        w.depth += 1;
    }
    w.line();
    w.out.push_str("analysis ");
    name::spell(&name, Place::Other, w.out);
    w.out.push(' ');
    let out = &mut *w.out;
    match row.kind {
        ir::AnalysisKind::Op | ir::AnalysisKind::Dc => out.push_str("op"),
        ir::AnalysisKind::DcInc => out.push_str("dcinc"),
        ir::AnalysisKind::DcXf => {
            let x: ir::dc_ac::DcXf = d.detail(row.detail);
            out.push_str("dcxf");
            output(d, x.output, out);
        }
        ir::AnalysisKind::Ac => {
            let x: ir::dc_ac::Ac = d.detail(row.detail);
            out.push_str("ac");
            range(d, x.sweep, out);
        }
        ir::AnalysisKind::Xf => {
            let x: ir::dc_ac::Xf = d.detail(row.detail);
            out.push_str("acxf");
            output(d, x.output, out);
            range(d, x.sweep, out);
        }
        ir::AnalysisKind::Stb => {
            let x: ir::periodic::Stb = d.detail(row.detail);
            out.push_str("acstb probe=");
            quote(d.str(x.probe), out);
            range(d, x.sweep, out);
        }
        ir::AnalysisKind::Sp => {
            let x: ir::dc_ac::Sp = d.detail(row.detail);
            out.push_str("acsp ports=[");
            for (k, &p) in d.names(x.ports).iter().enumerate() {
                if k > 0 {
                    out.push_str(", ");
                }
                quote(d.str(p), out);
            }
            out.push(']');
            range(d, x.sweep, out);
        }
        ir::AnalysisKind::Noise => {
            let x: ir::dc_ac::Noise = d.detail(row.detail);
            out.push_str("noise");
            output(d, x.output, out);
            out.push_str(" in=");
            quote(d.str(x.source), out);
            range(d, x.sweep, out);
        }
        ir::AnalysisKind::Tran => tran(d, d.detail(row.detail), out),
        k => unreachable!("FEATURES lacks Analysis({k:?}): legalize refused it"),
    }
    lists(d, scope, at, out);
    out.push('\n');
    if row.kind == ir::AnalysisKind::Dc {
        w.depth -= 1;
    }
}

/// ` from=… to=… step=…`, ` from=… to=… mode="lin|dec|oct" points=…` or ` values=[…]`.
pub(super) fn range(d: &ir::Design, s: ir::SweepSpec, out: &mut String) {
    if s.kind == ir::SweepKind::List {
        out.push_str(" values=[");
        for (k, &e) in d.exprs(s.values).iter().enumerate() {
            if k > 0 {
                out.push_str(", ");
            }
            write_expr(d, e, out);
        }
        out.push(']');
        return;
    }
    key(d, "from", s.start, out);
    key(d, "to", s.stop, out);
    let mode = match s.kind {
        ir::SweepKind::Lin if !s.step.is_none() => return key(d, "step", s.step, out),
        ir::SweepKind::Lin => "lin",
        ir::SweepKind::Dec => "dec",
        ir::SweepKind::Oct => "oct",
        k => unreachable!("FEATURES lacks the sweep kind {k:?}"),
    };
    out.push_str(" mode=");
    quote(mode, out);
    out.push_str(" points=");
    match mode {
        "lin" => minus_one(d, s.points, out),
        _ => write_expr(d, s.points, out),
    }
}

/// ` instance="x1:r1" parameter="r"`, ` instance="x1:v1" parameter="dc"` (a source's DC value,
/// R64), ` instance="__topinst__" parameter="p"` (a top-level parameter, R70),
/// ` model="m" parameter="p"`, ` option="name"`.
pub(super) fn target(d: &ir::Design, t: ir::Target, out: &mut String) {
    let element = |out: &mut String| {
        let mut path = name_of(d, t.path);
        if !path.is_empty() {
            path.push(':');
        }
        path.push_str(d.str(t.a.name));
        quote(&path, out);
    };
    match t.kind {
        ir::TargetKind::DeviceParam => {
            out.push_str(" instance=");
            element(out);
            out.push_str(" parameter=");
            quote(d.str(t.b), out);
        }
        // R70: the top-level instance is the design, its parameters the top level's.
        ir::TargetKind::Param => {
            out.push_str(" instance=\"__topinst__\" parameter=");
            quote(d.str(t.a.name), out);
        }
        ir::TargetKind::SourceDc => {
            out.push_str(" instance=");
            element(out);
            out.push_str(" parameter=\"dc\"");
        }
        ir::TargetKind::ModelParam => {
            out.push_str(" model=");
            quote(d.str(t.a.name), out);
            out.push_str(" parameter=");
            quote(d.str(t.b), out);
        }
        ir::TargetKind::Temp => out.push_str(" option=\"temp\""),
        ir::TargetKind::Option => {
            out.push_str(" option=");
            match t.option {
                ir::OptionKey::Private => quote(d.str(t.a.name), out),
                k => quote(super::plan::option_word(k), out),
            }
        }
        k => unreachable!("FEATURES lacks a target of kind {k:?}"),
    }
}

// ---- private ----

/// The axes of a `Dc`, innermost first.
fn axes(d: &ir::Design, list: ir::dc_ac::DcAxisList) -> &[ir::dc_ac::DcAxis] {
    d.dc_axes(list)
}

/// ` key=value` when `e` is given.
fn key(d: &ir::Design, key: &str, e: ir::ExprId, out: &mut String) {
    if !e.is_none() {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        write_expr(d, e, out);
    }
}

/// The IR's value count `e` as VACASK's interval count: an integer literal less one, `x + 1`
/// as `x`, any other `(e) - 1`.
fn minus_one(d: &ir::Design, e: ir::ExprId, out: &mut String) {
    match d.expr(e) {
        ir::Expr::Num(n) if d.num(n).form == ir::NumForm::Int => {
            let lit = d.num(n);
            number::spell(
                d,
                ir::Literal {
                    mantissa: lit.mantissa - 1,
                    ..lit
                },
                out,
            );
        }
        ir::Expr::Binary(ir::BinOp::Add, x, one) if is_int_one(d, one) => write_expr(d, x, out),
        _ => {
            out.push('(');
            write_expr(d, e, out);
            out.push_str(")-1");
        }
    }
}

/// Whether `e` is the integer literal 1.
fn is_int_one(d: &ir::Design, e: ir::ExprId) -> bool {
    matches!(d.expr(e), ir::Expr::Num(n) if d.num(n).form == ir::NumForm::Int && d.num(n).mantissa == 1)
}

/// ` out="o"` or ` out=["o", "p"]`.
fn output(d: &ir::Design, p: ir::Probe, out: &mut String) {
    out.push_str(" out=");
    let a = name_of(d, p.a);
    match p.kind {
        ir::ProbeKind::Vdiff => {
            out.push('[');
            quote(&a, out);
            out.push_str(", ");
            quote(&name_of(d, p.b), out);
            out.push(']');
        }
        _ => quote(&a, out),
    }
}

/// A transient's keys.
fn tran(d: &ir::Design, t: ir::tran::Tran, out: &mut String) {
    // R67, A41: VACASK has no output interval; `legalize` removed a given one first.
    debug_assert!(
        t.print_step.is_none(),
        "FEATURES lacks Run(TranPrintStep): legalize leaves no print step"
    );
    out.push_str("tran");
    key(d, "stop", t.stop, out);
    key(d, "step", t.initial_step, out);
    key(d, "start", t.record_start, out);
    if let ir::tran::StepCeiling::Given(e) = t.ceiling {
        key(d, "maxstep", e, out);
    }
    if t.uic {
        out.push_str(" icmode=\"uic\"");
    }
    key(d, "noisefmax", t.noise_fmax, out);
    key(d, "noisefmin", t.noise_fmin, out);
    key(d, "noiseseed", t.noise_seed, out);
}

/// The ` ic={…}` and ` nodeset={…}` lists of the `NodeSet` rows just before `at` in its scope,
/// each kind's list where its first row stood.
fn lists(d: &ir::Design, scope: Option<ir::ScopeId>, at: ir::ItemRef, out: &mut String) {
    let Some(scope) = scope else {
        return;
    };
    let items = d.items(scope);
    let k = items
        .iter()
        .position(|&i| i == at)
        .expect("its scope lists it");
    let first = items[..k]
        .iter()
        .rposition(|i| i.kind != ir::ItemKind::NodeSet)
        .map_or(0, |p| p + 1);
    let rows: Vec<ir::NodeSet> = items[first..k].iter().map(|i| d.row(i.index)).collect();
    let mut kinds: Vec<ir::NodeSetKind> = Vec::new();
    for r in &rows {
        if !kinds.contains(&r.kind) {
            kinds.push(r.kind);
        }
    }
    for kind in kinds {
        out.push_str(match kind {
            ir::NodeSetKind::Ic => " ic={",
            _ => " nodeset={",
        });
        for (j, r) in rows.iter().filter(|r| r.kind == kind).enumerate() {
            if j > 0 {
                out.push_str(", ");
            }
            quote(&name_of(d, r.node), out);
            out.push_str(", ");
            if r.reference.len > 0 {
                quote(&name_of(d, r.reference), out);
                out.push_str(", ");
            }
            write_expr(d, r.value, out);
        }
        out.push('}');
    }
}
