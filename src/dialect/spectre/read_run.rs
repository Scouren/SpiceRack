//! The run statements → IR rows: analyses (`name [(nodes)] master k=v …`), `options`, `save`,
//! and a `montecarlo` loop with its body.
//!
//! Rules (`rewrite/DIALECT-SPECTRE.md` §7, each with its evidence):
//! - `dc` with no parameter is the operating point (RUN-SP: every `op1 dc` deck); `dc dev=v1
//!   param=dc` sweeps a source's DC value, `dc param=p` a netlist parameter, over `start= stop=
//!   step=`, `lin=N` (N intervals: N + 1 points), `dec=N` or `values=[…]` (RUN-SP `p_dc_sweep`
//!   and the sweep-grid runs);
//! - `tran stop= [step=] [maxstep=] [skipdc=yes]`: `step` the output interval, the ceiling
//!   `maxstep` or by default span/50, `skipdc=yes` starts from the initial conditions (RUN-SP:
//!   `p_measure_save` undersampled at span/50; `p_charge_capacitor` with `skipdc`);
//! - `xf (p n)` the transfer function to the output `v(p, n)` over `values=[…]` or a range of
//!   frequencies (RUN-SP `p_tf`: a gain per source);
//! - `montecarlo numruns= [seed=] [variations=process|mismatch|all] { analyses }`; `donominal`
//!   is yes by default (MAN only) (RUN-SP: the `t_s*` decks);
//! - `options`: `reltol`, `vabstol`, `iabstol`, `gmin` as the portable keys, `temp`/`tnom` as the
//!   temperatures (one `Env` for both), every other key Spectre's own; each holds for the whole run (MAN: SR19
//!   options);
//! - `save a b …` saves those node voltages for every analysis (RUN-SP `p_measure_save`).
//!
//! Contents: `FEATURES`, `analysis`, `options`, `save` (crate-private)

use super::codes;
use super::expr_read::{self, Context};
use super::read::{self, Lower};
use super::read_elements::{Fields, fields, value};
use crate::ir;
use crate::ir::Feature as F;
use crate::ir::RunExtra as R;
use crate::syntax::ast::{NONE, Node, Tag};

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(F::Item(ir::ItemKind::Analysis))
    .with(F::Item(ir::ItemKind::Loop))
    .with(F::Item(ir::ItemKind::Option))
    .with(F::Item(ir::ItemKind::Env))
    .with(F::Item(ir::ItemKind::Save))
    .with(F::Analysis(ir::AnalysisKind::Op))
    .with(F::Analysis(ir::AnalysisKind::Dc))
    .with(F::Analysis(ir::AnalysisKind::Tran))
    .with(F::Analysis(ir::AnalysisKind::Xf))
    .with(F::Option(ir::OptionKey::RelTol))
    .with(F::Option(ir::OptionKey::VnTol))
    .with(F::Option(ir::OptionKey::AbsTol))
    .with(F::Option(ir::OptionKey::Gmin))
    .with(F::Option(ir::OptionKey::Private))
    .with(F::Run(R::SeveralAnalyses))
    .with(F::Run(R::AnalysisNamed))
    .with(F::Run(R::TranPrintStep))
    .with(F::Run(R::TranMaxStep))
    .with(F::Run(R::TranCapSpan50))
    .with(F::Run(R::TranUic))
    .with(F::Run(R::DcAxisParam))
    .with(F::Run(R::DcAxisUnnamed))
    .with(F::Run(R::AnalysisParamArgument))
    .with(F::Run(R::RunInterleaved))
    .with(F::Run(R::SweepDcLinStep))
    .with(F::Run(R::SweepDcLinPoints))
    .with(F::Run(R::SweepDcDec))
    .with(F::Run(R::SweepDcList))
    .with(F::Run(R::SweepFreqLinStep))
    .with(F::Run(R::SweepFreqLinPoints))
    .with(F::Run(R::SweepFreqDec))
    .with(F::Run(R::SweepFreqList))
    .with(F::Run(R::LoopMonteCarlo))
    .with(F::Run(R::LoopNamed))
    .with(F::Run(R::LoopRestore))
    .with(F::Run(R::LoopScoped))
    .with(F::Run(R::McSeed))
    .with(F::Run(R::McMismatch))
    .with(F::Run(R::McNominal))
    .with(F::Run(R::OptionWholeRun))
    .with(F::Run(R::EnvWholeRun))
    .with(F::Run(R::SaveWholeRun));

/// `name [(nodes)] master k=v …`: one analysis, a `montecarlo` loop, or an `Opaque`.
pub(super) fn analysis(l: &mut Lower<'_, '_, '_>, n: Node) {
    let h = l.cst.ast.header(n);
    if h.name == NONE || h.master == NONE {
        return read::unread(l, n);
    }
    let (name, master) = (l.field(h.name), l.field(h.master));
    let Some(fields) = fields(l, n) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    if used(l, &name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
    }
    let body = l.cst.ast.body(n).to_vec();
    if !body.is_empty() && master != "montecarlo" {
        return read::unread(l, n);
    }
    let checked = fields.iter().all(|(k, v)| {
        matches!(
            k.as_str(),
            "dev" | "param" | "values" | "variations" | "skipdc" | "donominal"
        ) || expr_read::check_expr(l, *v, Context::Param).is_ok()
    });
    if !checked {
        return read::unread(l, n);
    }
    let span = l.span(n);
    let scope = l.scope;
    let done = match master.as_str() {
        "dc" if fields.is_empty() && h.nodes.is_empty() => {
            l.b.op(scope, Some(&name), span);
            true
        }
        "dc" if h.nodes.is_empty() => dc(l, &name, &fields, span),
        "tran" if h.nodes.is_empty() => tran(l, &name, &fields, span),
        "xf" => xf(l, n, &name, &fields, span),
        "montecarlo" if h.nodes.is_empty() => montecarlo(l, n, &name, &fields, &body, span),
        _ => false,
    };
    if !done {
        read::unread(l, n);
    }
}

/// `name options k=v …`: one `Option` or `Env` row per key, each for the whole run.
pub(super) fn options(l: &mut Lower<'_, '_, '_>, n: Node) {
    let Some(fields) = fields(l, n) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    if !l.cst.ast.body(n).is_empty() || l.cst.ast.header(n).name == NONE {
        return read::unread(l, n);
    }
    for (_, v) in &fields {
        if let Err(r) = expr_read::check_expr(l, *v, Context::Param) {
            return read::opaque(l, n, r.class, r.code);
        }
    }
    let span = l.span(n);
    let (scope, holds) = (l.scope, ir::Applies::WholeRun);
    let temp = get(&fields, "temp");
    let tnom = get(&fields, "tnom");
    let mut env = false;
    for (k, v) in &fields {
        if matches!(k.as_str(), "temp" | "tnom") {
            if !env {
                env = true;
                let temp = temp.map_or(ir::ExprId::NONE, |t| value(l, t));
                let tnom = tnom.map_or(ir::ExprId::NONE, |t| value(l, t));
                l.b.push(scope, ir::Env { temp, tnom, holds }, span);
            }
            continue;
        }
        let e = value(l, *v);
        let (key, private, dialect) = match key(k) {
            Some(key) => (key, ir::NameId::NONE, ir::NameId::NONE),
            None => (ir::OptionKey::Private, l.b.name(k), l.b.name("spectre")),
        };
        let row = ir::OptionRow {
            key,
            private,
            dialect,
            value: e,
            holds,
        };
        l.b.push(scope, row, span);
    }
}

/// `save a b …`: one `Save` of those node voltages for every analysis and the whole run.
pub(super) fn save(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    let fields = ast.fields(n).to_vec();
    let words = fields.iter().all(|&f| ast.tag(Node(f)) == Tag::Word);
    if fields.is_empty() || !words {
        return read::unread(l, n);
    }
    let names: Vec<String> = fields.iter().map(|&f| l.field(f)).collect();
    if names.iter().any(|s| s.contains(':')) {
        return read::unread(l, n);
    }
    let probes: Vec<ir::Probe> = names.iter().map(|s| l.b.probe_v(&[s.as_str()])).collect();
    let span = l.span(n);
    l.b.save(
        l.scope,
        ir::AnalysisKinds::ALL,
        ir::Applies::WholeRun,
        &probes,
        span,
    );
}

// ---- private ----

/// The portable option a Spectre key states.
fn key(k: &str) -> Option<ir::OptionKey> {
    Some(match k {
        "reltol" => ir::OptionKey::RelTol,
        "vabstol" => ir::OptionKey::VnTol,
        "iabstol" => ir::OptionKey::AbsTol,
        "gmin" => ir::OptionKey::Gmin,
        _ => return None,
    })
}

/// The value of `key` in `fields`.
fn get(fields: &Fields, key: &str) -> Option<u32> {
    fields.iter().find(|(k, _)| k == key).map(|&(_, v)| v)
}

/// Whether `fields` holds only keys of `allowed`.
fn only(fields: &Fields, allowed: &[&str]) -> bool {
    fields.iter().all(|(k, _)| allowed.contains(&k.as_str()))
}

/// A sweep of `start= stop= step=|lin=|dec=` or `values=[…]`; `None` for any other form.
fn sweep(l: &mut Lower<'_, '_, '_>, fields: &Fields) -> Option<ir::SweepSpec> {
    if let Some(v) = get(fields, "values") {
        let items = vector(l, v)?;
        if items
            .iter()
            .any(|&i| expr_read::check_expr(l, i, Context::Param).is_err())
        {
            return None;
        }
        let values: Vec<ir::ExprId> = items.iter().map(|&i| value(l, i)).collect();
        return Some(ir::SweepSpec::list(l.b.exprs(&values)));
    }
    let (start, stop) = (get(fields, "start")?, get(fields, "stop")?);
    let step = get(fields, "step");
    let lin = get(fields, "lin");
    let dec = get(fields, "dec");
    let (from, to) = (value(l, start), value(l, stop));
    match (step, lin, dec) {
        (Some(s), None, None) => Some(ir::SweepSpec::lin_step(from, to, value(l, s))),
        (None, Some(k), None) => {
            let points = plus_one(l, k);
            Some(ir::SweepSpec::lin_points(from, to, points))
        }
        (None, None, Some(k)) => Some(ir::SweepSpec::dec(from, to, value(l, k))),
        _ => None,
    }
}

/// The point count of `lin=N` (N intervals): a plain whole literal's successor, `x` for the
/// writer's `x - 1`, else `N + 1`.
fn plus_one(l: &mut Lower<'_, '_, '_>, node: u32) -> ir::ExprId {
    if let Some(x) = expr_read::literal_of(l, node)
        && x.form == ir::NumForm::Plain
        && x.frac == 0
        && x.mantissa < i64::MAX
    {
        return l.b.literal(ir::Literal {
            mantissa: x.mantissa + 1,
            ..x
        });
    }
    let e = value(l, node);
    let one = ir::Literal {
        mantissa: 1,
        frac: 0,
        exp10: 0,
        form: ir::NumForm::Plain,
    };
    let d = l.b.design();
    if let ir::Expr::Binary(ir::BinOp::Sub, x, k) = d.expr(e)
        && let ir::Expr::Num(n) = d.expr(k)
        && d.num(n) == one
    {
        return x;
    }
    let one = l.b.literal(one);
    l.b.binary(ir::BinOp::Add, e, one)
}

/// `dc dev=v1 param=dc …` or `dc param=p …`: a one-axis `Dc`.
fn dc(l: &mut Lower<'_, '_, '_>, name: &str, fields: &Fields, span: ir::Span) -> bool {
    let ranges = ["start", "stop", "step", "lin", "dec", "values"];
    let keys: Vec<&str> = ["dev", "param"].iter().chain(&ranges).copied().collect();
    if !only(fields, &keys) {
        return false;
    }
    let param = get(fields, "param").map(|p| l.field(p));
    let target = match (get(fields, "dev").map(|d| l.field(d)), param.as_deref()) {
        (Some(dev), Some("dc")) => l.b.target(ir::TargetKind::SourceDc, &[], &dev, None),
        (None, Some(p)) => l.b.target(ir::TargetKind::Param, &[], p, None),
        _ => return false,
    };
    let Some(sweep) = sweep(l, fields) else {
        return false;
    };
    let axis = ir::dc_ac::DcAxis {
        target,
        sweep,
        name: ir::NameId::NONE,
        afresh: false,
    };
    let axes = l.b.dc_axes(&[axis]);
    let item =
        l.b.analysis(l.scope, Some(name), ir::dc_ac::Dc { axes }, span);
    if let (None, Some(p)) = (get(fields, "dev"), param) {
        l.refer(item, 0, ir::DefKind::Param, &p);
    }
    true
}

/// `tran stop= [step=] [maxstep=] [skipdc=yes|no]`.
fn tran(l: &mut Lower<'_, '_, '_>, name: &str, fields: &Fields, span: ir::Span) -> bool {
    if !only(fields, &["stop", "step", "maxstep", "skipdc"]) {
        return false;
    }
    let uic = match get(fields, "skipdc").map(|v| l.field(v)) {
        None => false,
        Some(s) if s == "no" => false,
        Some(s) if s == "yes" => true,
        Some(_) => return false,
    };
    let Some(stop) = get(fields, "stop") else {
        return false;
    };
    let stop = value(l, stop);
    let print_step = get(fields, "step").map_or(ir::ExprId::NONE, |s| value(l, s));
    let ceiling = match get(fields, "maxstep") {
        None => ir::tran::StepCeiling::Rule(ir::tran::StepCap::SpanOver50),
        Some(m) => ir::tran::StepCeiling::Given(value(l, m)),
    };
    let row = ir::tran::Tran {
        stop,
        record_start: ir::ExprId::NONE,
        print_step,
        initial_step: ir::ExprId::NONE,
        noise_fmax: ir::ExprId::NONE,
        noise_fmin: ir::ExprId::NONE,
        noise_seed: ir::ExprId::NONE,
        segments: ir::List::EMPTY,
        ceiling,
        uic,
        startup: false,
    };
    l.b.analysis(l.scope, Some(name), row, span);
    true
}

/// `xf (p n) values=[…]|start= stop= …`: the transfer function to `v(p, n)`.
fn xf(l: &mut Lower<'_, '_, '_>, n: Node, name: &str, fields: &Fields, span: ir::Span) -> bool {
    let nodes: Vec<String> = l
        .cst
        .ast
        .header(n)
        .nodes
        .iter()
        .map(|&x| l.field(x))
        .collect();
    let ranges = ["start", "stop", "step", "lin", "dec", "values"];
    let [p, m] = nodes.as_slice() else {
        return false;
    };
    if !only(fields, &ranges) {
        return false;
    }
    let Some(sweep) = sweep(l, fields) else {
        return false;
    };
    let output = match m.as_str() {
        "0" => l.b.probe_v(&[p.as_str()]),
        m => l.b.probe_vdiff(&[p.as_str()], &[m]),
    };
    let row = ir::dc_ac::Xf { output, sweep };
    l.b.analysis(l.scope, Some(name), row, span);
    true
}

/// `montecarlo numruns= [seed=] [variations=] [donominal=] { … }`: a `MonteCarlo` loop.
fn montecarlo(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    name: &str,
    fields: &Fields,
    body: &[u32],
    span: ir::Span,
) -> bool {
    if !only(fields, &["numruns", "seed", "variations", "donominal"]) {
        return false;
    }
    let variations = match get(fields, "variations").map(|v| l.field(v)).as_deref() {
        None | Some("process") => ir::plan::Variations::Process,
        Some("mismatch") => ir::plan::Variations::Mismatch,
        Some("all") => ir::plan::Variations::All,
        Some(_) => return false,
    };
    let nominal = match get(fields, "donominal").map(|v| l.field(v)).as_deref() {
        None | Some("yes") => true,
        Some("no") => false,
        Some(_) => return false,
    };
    let Some(runs) = get(fields, "numruns") else {
        return false;
    };
    let runs = value(l, runs);
    let seed = get(fields, "seed").map_or(ir::ExprId::NONE, |s| value(l, s));
    let row = ir::plan::MonteCarlo {
        runs,
        seed,
        samples: ir::List::EMPTY,
        variations,
        sampler: ir::plan::Sampler::Random,
        nominal,
    };
    let (_, scope) =
        l.b.loop_(l.scope, Some(name), row, ir::LoopEnd::Restore, span);
    let until = l.cst.ast.closer(n).map(|c| {
        let t = l.cst.tokens.start(l.cst.ast.main_token(c));
        t.saturating_sub(l.cst.shift)
    });
    l.body(scope, body, until);
    true
}

/// The items of a `[a b …]` vector node, or `None` for any other node.
fn vector(l: &Lower<'_, '_, '_>, node: u32) -> Option<Vec<u32>> {
    let ast = &l.cst.ast;
    if ast.tag(Node(node)) != Tag::Vector {
        return None;
    }
    let at = ast.rhs(Node(node));
    let count = ast.extra(at + 1);
    Some((0..count).map(|k| ast.extra(at + 2 + k)).collect())
}

/// Whether an analysis or loop of the design already has `name`.
fn used(l: &Lower<'_, '_, '_>, name: &str) -> bool {
    let design = l.b.design();
    let id = design.find_name(name);
    let rows = <ir::Analysis as ir::ItemRow>::table(design).rows();
    !id.is_none() && rows.iter().any(|a| a.name == id)
}
