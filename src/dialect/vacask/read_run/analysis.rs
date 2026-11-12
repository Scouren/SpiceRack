//! `sweep …` lines and the `analysis name kind …` they bind → one `Dc` over the sweeps' axes
//! (the kind is `op`: one result over the grid, the last sweep the inner axis), or a `Loop` of
//! kind sweep per line around any other analysis (outermost first, `after: Restore`, run s1);
//! the analysis's own `ic=`/`nodeset=` lists are `NodeSet` rows just before it, for it only
//! (run e1). A form the IR cannot state makes the analysis and its sweeps `Private` under the
//! form's code (R43); what VACASK rejects makes them `InvalidInSource`. Every statement is
//! checked before the first row is pushed (A2).
//!
//! The kinds and their parameters are VACASK's (VD cmd-analysis-op, -dcinc, -dcxf, -ac, -acxf,
//! -acstb, -acsp, -noise, -tran, -trannoise): `tran`'s `step` is the initial step, `start` the
//! start of recording (a literal 0: none, R47), no `maxstep` (or a literal 0, run e7) the
//! span/50 rule (run tr1, `StepCeiling::Rule`);
//! `noisefmin` absent or 0 is `noisefmax/1e3`, stated as written; a lin `points` counts
//! intervals, so the IR's count is one more (VD cmd-analysis-ac, IR sweep.rs).
//!
//! Contents: `analysis` (crate-private)

use super::sweep::{
    BAD, Spec, Sweep, Why, check, check_arg, get, pairs, private, range, spec, sweep, target,
    value, vector,
};
use super::{ANALYSIS_FIELD, ANALYSIS_HB_SMALL, HB_FORM, OWN_PSS, PSS_PERIOD, bare_name, string};
use crate::dialect::vacask::number;
use crate::dialect::vacask::read::{self, Lower};
use crate::dialect::vacask::read_outputs::{self, NodeEntry};
use crate::ir;
use crate::syntax::ast::{NONE, Name, Node, Tag};

/// What an analysis computes, as nodes.
enum Kind {
    Op,
    DcInc,
    DcXf(u32),
    Ac(Spec),
    Xf(u32, Spec),
    Stb(String, Spec),
    Sp(Vec<String>, Spec),
    Noise(u32, String, Spec),
    Tran(Tran),
}

/// A transient's nodes (`NONE` when not given).
#[derive(Clone, Copy)]
struct Tran {
    stop: u32,
    step: u32,
    start: u32,
    maxstep: u32,
    uic: bool,
    fmax: u32,
    fmin: u32,
    seed: u32,
}

/// An analysis, checked.
struct Plan {
    name: String,
    kind: Kind,
    lists: Vec<NodeEntry>,
}

/// Keys every small-signal analysis that solves an operating point and sweeps frequency takes.
const AC_KEYS: &[&str] = &[
    "nodeset", "store", "opsolve", "from", "to", "step", "mode", "points", "values", "write",
    "writeop", "solver", "opsolver",
];

/// Keys with no IR field (R43, P2302).
const NO_FIELD: &[&str] = &[
    "store",
    "write",
    "writeop",
    "opsolve",
    "solver",
    "opsolver",
    "localgnd",
    "noisemode",
    "oversample",
    "noisescale",
    "rampfrac",
    "noisebreak",
];

/// The analysis `n` and the sweep lines `sweeps` that bind it.
pub(super) fn analysis(l: &mut Lower<'_, '_, '_>, sweeps: &[Node], n: Node) {
    let mut swept: Vec<Result<Sweep, Why>> = sweeps.iter().map(|&s| sweep(l, s)).collect();
    // Settle names_chain: one name twice among the sweeps of one analysis is "Sweep does not
    // have a unique name.".
    for k in 1..swept.len() {
        let again = match &swept[k] {
            Ok(s) => swept[..k]
                .iter()
                .any(|o| o.as_ref().is_ok_and(|o| o.name == s.name)),
            Err(_) => false,
        };
        if again {
            swept[k] = Err(BAD);
        }
    }
    let plan = plan(l, n);
    let failed = swept.iter().any(Result::is_err) || plan.is_err();
    if failed {
        let why = |w: Option<&Why>| w.copied();
        let whys: Vec<Option<Why>> = swept.iter().map(|w| why(w.as_ref().err())).collect();
        let own = why(plan.as_ref().err());
        let invalid = whys
            .iter()
            .chain(std::iter::once(&own))
            .flatten()
            .any(|w| w.class == ir::OpaqueClass::InvalidInSource);
        let class = match invalid {
            true => ir::OpaqueClass::InvalidInSource,
            false => ir::OpaqueClass::Private,
        };
        for (&s, w) in sweeps.iter().zip(&whys) {
            let saved = l.stmt;
            l.stmt = l.span(s);
            read::opaque(l, s, class, w.and_then(|w| w.code));
            l.stmt = saved;
        }
        return read::opaque(l, n, class, own.and_then(|w| w.code));
    }
    let swept: Vec<Sweep> = swept
        .into_iter()
        .map(|s| s.ok().expect("checked"))
        .collect();
    let plan = plan.ok().expect("checked");
    let span = l.stmt;
    let scope = l.scope;
    if matches!(plan.kind, Kind::Op) && !swept.is_empty() {
        let mut axes = Vec::with_capacity(swept.len());
        for s in swept.iter().rev() {
            let target = target(l, &s.aim);
            let sweep = spec(l, s.spec);
            let name = l.b.name(&s.name);
            axes.push(ir::dc_ac::DcAxis {
                target,
                sweep,
                name,
            });
        }
        let axes = l.b.dc_axes(&axes);
        read_outputs::node_sets(l, &plan.lists);
        let dc = ir::dc_ac::Dc {
            axes,
            restart: false,
        };
        l.b.analysis(scope, Some(&plan.name), dc, span);
        return;
    }
    for s in &swept {
        let target = target(l, &s.aim);
        let sweep = spec(l, s.spec);
        let row = ir::plan::SweepLoop { target, sweep };
        let (_, body) =
            l.b.loop_(l.scope, Some(&s.name), row, ir::LoopEnd::Restore, s.span);
        l.scope = body;
    }
    read_outputs::node_sets(l, &plan.lists);
    build(l, &plan, span);
    l.scope = scope;
}

// ---- private ----

/// The analysis statement `n`, checked.
fn plan(l: &mut Lower<'_, '_, '_>, n: Node) -> Result<Plan, Why> {
    let ast = &l.cst.ast;
    let h = ast.header(n);
    if h.name == NONE || h.master == NONE || !bare_name(l, h.name) {
        return Err(BAD);
    }
    let name = read::field_name(l.cst, h.name);
    // Run dup: a second analysis of one name is "Analysis 'op1' redefinition.".
    if used(l, &name) {
        return Err(BAD);
    }
    let word = ast.name(Name(ast.rhs(Node(h.master)))).to_string();
    match word.as_str() {
        "pss" => return Err(private(PSS_PERIOD)),
        "pac" | "pnoise" => return Err(private(OWN_PSS)),
        "hb" => return Err(private(HB_FORM)),
        "hbac" | "hbnoise" => return Err(private(ANALYSIS_HB_SMALL)),
        _ => {}
    }
    let pairs = pairs(l, n)?;
    let mut keys: Vec<&str> = match word.as_str() {
        "op" => vec!["nodeset", "store", "write", "solver"],
        "dcinc" => vec!["nodeset", "store", "opsolve", "write", "writeop", "solver"],
        "dcxf" => vec![
            "nodeset", "store", "opsolve", "out", "write", "writeop", "solver",
        ],
        "ac" => vec![],
        "acxf" => vec!["out"],
        "acstb" => vec!["probe", "localgnd"],
        "acsp" => vec!["ports"],
        "noise" => vec!["out", "in"],
        "tran" => vec![
            "stop",
            "step",
            "start",
            "maxstep",
            "icmode",
            "ic",
            "nodeset",
            "store",
            "write",
            "opsolver",
            "noisefmax",
            "noisefmin",
            "noisemode",
            "oversample",
            "noiseseed",
            "noisescale",
            "rampfrac",
            "noisebreak",
        ],
        _ => return Err(BAD),
    };
    if matches!(word.as_str(), "ac" | "acxf" | "acstb" | "acsp" | "noise") {
        keys.extend_from_slice(AC_KEYS);
    }
    if pairs.iter().any(|(k, _)| !keys.contains(&k.as_str())) {
        return Err(BAD);
    }
    if pairs.iter().any(|(k, _)| NO_FIELD.contains(&k.as_str())) {
        return Err(private(ANALYSIS_FIELD));
    }
    let mut lists = Vec::new();
    for (k, kind) in [
        ("ic", ir::NodeSetKind::Ic),
        ("nodeset", ir::NodeSetKind::Nodeset),
    ] {
        let v = get(&pairs, k);
        if v != NONE {
            if l.cst.ast.tag(Node(v)) == Tag::Str {
                return Err(private(ANALYSIS_FIELD));
            }
            let entries = read_outputs::node_list(l, kind, v).ok_or(BAD)?;
            check(l, &entries.iter().map(|e| e.value).collect::<Vec<_>>())?;
            lists.push((pairs.iter().position(|(x, _)| x == k), entries));
        }
    }
    lists.sort_by_key(|(at, _)| *at);
    let lists = lists.into_iter().flat_map(|(_, e)| e).collect();
    let names = |v: u32| -> Result<Vec<String>, Why> {
        if let Some(s) = string(l, v) {
            return Ok(vec![s]);
        }
        let items = vector(l, v).ok_or(BAD)?;
        items.iter().map(|&i| string(l, i).ok_or(BAD)).collect()
    };
    let out = |v: u32| -> Result<u32, Why> {
        match v != NONE && matches!(names(v)?.len(), 1 | 2) {
            true => Ok(v),
            false => Err(BAD),
        }
    };
    let one = |key: &str| -> Result<String, Why> { string(l, get(&pairs, key)).ok_or(BAD) };
    let kind = match word.as_str() {
        "op" => Kind::Op,
        "dcinc" => Kind::DcInc,
        "dcxf" => Kind::DcXf(out(get(&pairs, "out"))?),
        "ac" => Kind::Ac(range(l, &pairs, ANALYSIS_FIELD)?),
        "acxf" => Kind::Xf(out(get(&pairs, "out"))?, range(l, &pairs, ANALYSIS_FIELD)?),
        "acstb" => Kind::Stb(one("probe")?, range(l, &pairs, ANALYSIS_FIELD)?),
        "acsp" => {
            let ports = names(get(&pairs, "ports"))?;
            Kind::Sp(ports, range(l, &pairs, ANALYSIS_FIELD)?)
        }
        "noise" => Kind::Noise(
            out(get(&pairs, "out"))?,
            one("in")?,
            range(l, &pairs, ANALYSIS_FIELD)?,
        ),
        _ => Kind::Tran(tran(l, &pairs)?),
    };
    Ok(Plan { name, kind, lists })
}

/// A transient's fields, checked.
fn tran(l: &Lower<'_, '_, '_>, pairs: &[(String, u32)]) -> Result<Tran, Why> {
    let [stop, step, start, maxstep, fmax, fmin, seed] = [
        "stop",
        "step",
        "start",
        "maxstep",
        "noisefmax",
        "noisefmin",
        "noiseseed",
    ]
    .map(|k| get(pairs, k));
    if stop == NONE || (fmax == NONE && (fmin != NONE || seed != NONE)) {
        return Err(BAD);
    }
    let uic = match get(pairs, "icmode") {
        NONE => false,
        m => match string(l, m).as_deref() {
            Some("op") => false,
            Some("uic") => true,
            _ => return Err(BAD),
        },
    };
    check_arg(l, &[stop, step, start, maxstep, fmax, fmin, seed])?;
    // A literal zero is VACASK's "none" for these (run e7; VD: noisefmin 0 is the default).
    let maxstep = if zero(l, maxstep) { NONE } else { maxstep };
    // `start=0` records from 0: no record start (R47; check refuses a literal-0 one).
    let start = if zero(l, start) { NONE } else { start };
    let fmin = if zero(l, fmin) { NONE } else { fmin };
    Ok(Tran {
        stop,
        step,
        start,
        maxstep,
        uic,
        fmax,
        fmin,
        seed,
    })
}

/// Whether `node` is a literal zero.
fn zero(l: &Lower<'_, '_, '_>, node: u32) -> bool {
    node != NONE
        && l.cst.ast.tag(Node(node)) == Tag::Num
        && number::lex(l.cst.tokens.text(l.cst.ast.main_token(Node(node))))
            .is_ok_and(|x| x.literal.mantissa == 0)
}

/// The voltage `out` names: a node, or the first of two minus the second.
fn output(l: &mut Lower<'_, '_, '_>, v: u32) -> ir::Probe {
    let names: Vec<String> = match string(l, v) {
        Some(s) => vec![s],
        None => vector(l, v)
            .expect("checked")
            .iter()
            .map(|&i| string(l, i).expect("checked"))
            .collect(),
    };
    let path = |s: &String| -> Vec<String> { s.split(':').map(str::to_string).collect() };
    let a = path(&names[0]);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    match names.get(1) {
        None => l.b.probe_v(&a),
        Some(b) => {
            let b = path(b);
            let b: Vec<&str> = b.iter().map(String::as_str).collect();
            l.b.probe_vdiff(&a, &b)
        }
    }
}

/// The analysis row of `plan` into the current scope.
fn build(l: &mut Lower<'_, '_, '_>, plan: &Plan, span: ir::Span) {
    use ir::dc_ac::{Ac, DcXf, Noise, NoiseRef, Sp, Xf};
    use ir::periodic::Stb;
    let (scope, name) = (l.scope, Some(plan.name.as_str()));
    match &plan.kind {
        Kind::Op => {
            l.b.op(scope, name, span);
        }
        Kind::DcInc => {
            l.b.dc_inc(scope, name, span);
        }
        Kind::DcXf(out) => {
            let output = output(l, *out);
            l.b.analysis(scope, name, DcXf { output }, span);
        }
        Kind::Ac(s) => {
            let sweep = spec(l, *s);
            l.b.analysis(scope, name, Ac { sweep }, span);
        }
        Kind::Xf(out, s) => {
            let output = output(l, *out);
            let sweep = spec(l, *s);
            l.b.analysis(scope, name, Xf { output, sweep }, span);
        }
        Kind::Stb(probe, s) => {
            let probe = l.b.name(probe);
            let sweep = spec(l, *s);
            l.b.analysis(scope, name, Stb { probe, sweep }, span);
        }
        Kind::Sp(ports, s) => {
            let sweep = spec(l, *s);
            let ports: Vec<&str> = ports.iter().map(String::as_str).collect();
            let ports = l.b.names(&ports);
            let row = Sp {
                sweep,
                ports,
                noise: false,
            };
            l.b.analysis(scope, name, row, span);
        }
        Kind::Noise(out, input, s) => {
            let output = output(l, *out);
            let source = l.b.name(input);
            let sweep = spec(l, *s);
            let row = Noise {
                output,
                source,
                sweep,
                points_per_summary: ir::ExprId::NONE,
                ac_sweep: false,
                input_ref: NoiseRef::Unit,
            };
            l.b.analysis(scope, name, row, span);
        }
        Kind::Tran(t) => {
            let row = tran_row(l, *t);
            l.b.analysis(scope, name, row, span);
        }
    }
}

/// The `Tran` detail of `t`.
fn tran_row(l: &mut Lower<'_, '_, '_>, t: Tran) -> ir::tran::Tran {
    let stop = value(l, t.stop);
    let record_start = value(l, t.start);
    let ceiling = match t.maxstep {
        NONE => ir::tran::StepCeiling::Rule(ir::tran::StepCap::SpanOver50),
        m => ir::tran::StepCeiling::Given(value(l, m)),
    };
    let initial_step = value(l, t.step);
    let noise_fmax = value(l, t.fmax);
    let noise_fmin = match (t.fmax, t.fmin) {
        (NONE, _) => ir::ExprId::NONE,
        (fmax, NONE) => {
            let again = value(l, fmax);
            let thousand = l.b.literal(number::lex("1e3").expect("a number").literal);
            l.b.binary(ir::BinOp::DivTyped, again, thousand)
        }
        (_, fmin) => value(l, fmin),
    };
    let noise_seed = value(l, t.seed);
    ir::tran::Tran {
        stop,
        record_start,
        print_step: ir::ExprId::NONE,
        initial_step,
        noise_fmax,
        noise_fmin,
        noise_seed,
        segments: ir::List::EMPTY,
        ceiling,
        uic: t.uic,
        startup: false,
    }
}

/// Whether an analysis of the design already has `name`.
fn used(l: &Lower<'_, '_, '_>, name: &str) -> bool {
    let design = l.b.design();
    let id = design.find_name(name);
    !id.is_none()
        && design.reachable().iter().any(|(at, _)| {
            at.kind == ir::ItemKind::Analysis && design.row::<ir::Analysis>(at.index).name == id
        })
}
