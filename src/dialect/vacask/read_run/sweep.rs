//! A `sweep name target range` line → its target and its range, checked, then built into a
//! `Target` and a `SweepSpec`; the range forms are shared with the frequency sweeps of the
//! small-signal analyses (`from`/`to` with `step`, or `mode="lin|dec|oct"` with `points`, or
//! `values=[…]`). A lin `points` counts intervals (VD cmd-sweep, cmd-analysis-ac), so the IR's
//! count is one more; targets `instance=`/`parameter=` (a device parameter, hierarchical names
//! split at `:`; `parameter="dc"` of an independent source of type dc is its DC value, R64, and
//! of a waveform source, whose `dc` VACASK ignores, Private: settle run/sweep_source), `model=`/`parameter=` (bound to the top level's model), `option="temp"` (the
//! temperature), `option=` a typed key or VACASK's own option by name. The principal parameter,
//! `continuation=0`, values by a variable's name and a sweep of a variable are Private (R43).
//!
//! Contents: `Why`, `Spec`, `Aim`, `Sweep`, `BAD`, `private`, `pairs`, `get`, `check`,
//! `vector`, `range`, `sweep`, `value`, `spec`, `target` (crate-private)

use super::{REJECTED, SWEEP_FORM, SWEEP_WAVE, VARIABLE, assign, bare_name, string};
use crate::dialect::vacask::expr_read::{self, Context};
use crate::dialect::vacask::number;
use crate::dialect::vacask::read::{self, Lower};
use crate::dialect::vacask::read_elements::Source;
use crate::ir;
use crate::syntax::ast::{NONE, Node, Tag};

/// Why a statement is no row: the class its opaque gets and its code.
#[derive(Clone, Copy)]
pub(super) struct Why {
    pub(super) class: ir::OpaqueClass,
    pub(super) code: Option<ir::Code>,
}

/// A frequency or sweep range, as nodes.
#[derive(Clone, Copy)]
pub(super) struct Spec {
    pub(super) kind: ir::SweepKind,
    pub(super) from: u32,
    pub(super) to: u32,
    pub(super) step: u32,
    pub(super) points: u32,
    pub(super) values: u32,
}

/// A sweep target, as names.
pub(super) enum Aim {
    Device(Vec<String>, String, String),
    Source(Vec<String>, String),
    Model(String, String),
    Temp,
    Option(ir::OptionKey),
    PrivateOption(String),
}

/// One `sweep` line, checked.
pub(super) struct Sweep {
    pub(super) name: String,
    pub(super) aim: Aim,
    pub(super) spec: Spec,
    pub(super) span: ir::Span,
}

/// A refusal: VACASK rejects the statement.
pub(super) const BAD: Why = Why {
    class: ir::OpaqueClass::InvalidInSource,
    code: Some(REJECTED),
};

/// A form the IR has no field for, under `code`.
pub(super) const fn private(code: ir::Code) -> Why {
    Why {
        class: ir::OpaqueClass::Private,
        code: Some(code),
    }
}

/// The `Assign` fields of `n` as `(key, value)`, or `BAD` for any other field.
pub(super) fn pairs(l: &Lower<'_, '_, '_>, n: Node) -> Result<Vec<(String, u32)>, Why> {
    let fields = l.cst.ast.fields(n).to_vec();
    fields.iter().map(|&f| assign(l, f).ok_or(BAD)).collect()
}

/// The value of `key` among `pairs`, or `NONE`.
pub(super) fn get(pairs: &[(String, u32)], key: &str) -> u32 {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map_or(NONE, |&(_, v)| v)
}

/// Checks each expression node of `nodes` that is given.
pub(super) fn check(l: &Lower<'_, '_, '_>, nodes: &[u32]) -> Result<(), Why> {
    for &node in nodes.iter().filter(|&&x| x != NONE) {
        expr_read::check_expr(l, node, Context::Param).map_err(|r| Why {
            class: r.class,
            code: Some(r.code),
        })?;
    }
    Ok(())
}

/// The elements of a `[…]` vector node, or `None` for any other node.
pub(super) fn vector(l: &Lower<'_, '_, '_>, node: u32) -> Option<Vec<u32>> {
    let ast = &l.cst.ast;
    let n = Node(node);
    (node != NONE && ast.tag(n) == Tag::Vector).then(|| ast.list(ast.rhs(n) + 1).to_vec())
}

/// The range of `pairs` (`from`, `to`, `step`, `mode`, `points`, `values`), with `code` for a
/// form the IR cannot state.
pub(super) fn range(
    l: &Lower<'_, '_, '_>,
    pairs: &[(String, u32)],
    code: ir::Code,
) -> Result<Spec, Why> {
    let [from, to, step, mode, points, values] =
        ["from", "to", "step", "mode", "points", "values"].map(|k| get(pairs, k));
    let none = Spec {
        kind: ir::SweepKind::List,
        from: NONE,
        to: NONE,
        step: NONE,
        points: NONE,
        values: NONE,
    };
    if values != NONE {
        let alone = [from, to, step, mode, points].iter().all(|&x| x == NONE);
        let items = vector(l, values).ok_or(private(code))?;
        if !alone || items.is_empty() {
            return Err(private(code));
        }
        check(l, &items)?;
        return Ok(Spec { values, ..none });
    }
    if from == NONE || to == NONE {
        return Err(private(code));
    }
    let kind = match (step != NONE, mode) {
        (true, NONE) => ir::SweepKind::Lin,
        (false, m) if m != NONE => match string(l, m).as_deref() {
            Some("lin") => ir::SweepKind::Lin,
            Some("dec") => ir::SweepKind::Dec,
            Some("oct") => ir::SweepKind::Oct,
            Some(_) => return Err(BAD),
            None => return Err(private(code)),
        },
        _ => return Err(private(code)),
    };
    if step == NONE && points == NONE {
        return Err(private(code));
    }
    check(l, &[from, to, step, points])?;
    Ok(Spec {
        kind,
        from,
        to,
        step,
        points,
        values: NONE,
    })
}

/// A `sweep name target range` line, checked.
pub(super) fn sweep(l: &mut Lower<'_, '_, '_>, s: Node) -> Result<Sweep, Why> {
    let fields = l.cst.ast.fields(s).to_vec();
    let Some((&name, rest)) = fields.split_first() else {
        return Err(BAD);
    };
    if l.cst.ast.tag(Node(name)) != Tag::Word {
        return Err(BAD);
    }
    if !bare_name(l, name) {
        return Err(BAD);
    }
    let name = read::field_name(l.cst, name);
    let mut pairs = Vec::with_capacity(rest.len());
    for &f in rest {
        pairs.push(assign(l, f).ok_or(BAD)?);
    }
    let known = [
        "instance",
        "model",
        "option",
        "variable",
        "parameter",
        "from",
        "to",
        "step",
        "mode",
        "points",
        "values",
        "continuation",
    ];
    if pairs.iter().any(|(k, _)| !known.contains(&k.as_str())) {
        return Err(BAD);
    }
    let text = |k: &str| match get(&pairs, k) {
        NONE => Ok(None),
        v => string(l, v).map(Some).ok_or(private(SWEEP_FORM)),
    };
    let (instance, model, option, variable, parameter) = (
        text("instance")?,
        text("model")?,
        text("option")?,
        text("variable")?,
        text("parameter")?,
    );
    let given = [&instance, &model, &option, &variable]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if given != 1 {
        return Err(BAD);
    }
    if get(&pairs, "continuation") != NONE {
        return Err(private(SWEEP_FORM));
    }
    let aim = match (instance, model, option, parameter) {
        (Some(i), _, _, Some(p)) => {
            let mut path: Vec<String> = i.split(':').map(str::to_string).collect();
            let element = path.pop().expect("split gives one part");
            match (p.as_str(), l.declared.source(&path, &element)) {
                ("dc", Some(Source::Dc)) => Aim::Source(path, element),
                ("dc", Some(Source::Wave)) => return Err(private(SWEEP_WAVE)),
                _ => Aim::Device(path, element, p),
            }
        }
        (Some(_), _, _, None) => return Err(private(SWEEP_FORM)),
        (_, Some(m), _, Some(p)) => Aim::Model(m, p),
        (_, Some(_), _, None) => return Err(BAD),
        (_, _, Some(o), None) => option_aim(&o).ok_or(private(SWEEP_FORM))?,
        (_, _, Some(_), Some(_)) => return Err(BAD),
        _ => return Err(private(VARIABLE)),
    };
    let spec = range(l, &pairs, SWEEP_FORM)?;
    Ok(Sweep {
        name,
        aim,
        spec,
        span: l.span(s),
    })
}

/// The expression at `node`, or `NONE` (checked before).
pub(super) fn value(l: &mut Lower<'_, '_, '_>, node: u32) -> ir::ExprId {
    match node {
        NONE => ir::ExprId::NONE,
        n => expr_read::expr(l, n, Context::Param).expect("checked"),
    }
}

/// The IR sweep of `s`: a lin count of intervals becomes one more value (an integer literal
/// plus one, any other expression `+ 1`).
pub(super) fn spec(l: &mut Lower<'_, '_, '_>, s: Spec) -> ir::SweepSpec {
    if s.values != NONE {
        let items = vector(l, s.values).expect("checked");
        let values: Vec<ir::ExprId> = items.iter().map(|&i| value(l, i)).collect();
        return ir::SweepSpec::list(l.b.exprs(&values));
    }
    let (from, to) = (value(l, s.from), value(l, s.to));
    match s.kind {
        ir::SweepKind::Lin if s.step != NONE => ir::SweepSpec::lin_step(from, to, value(l, s.step)),
        ir::SweepKind::Lin => ir::SweepSpec::lin_points(from, to, plus_one(l, s.points)),
        ir::SweepKind::Dec => ir::SweepSpec::dec(from, to, value(l, s.points)),
        _ => ir::SweepSpec::oct(from, to, value(l, s.points)),
    }
}

/// The IR target of `aim`, bound by VACASK's rule (a model at the top level).
pub(super) fn target(l: &mut Lower<'_, '_, '_>, aim: &Aim) -> ir::Target {
    match aim {
        Aim::Device(path, element, param) => {
            let path: Vec<&str> = path.iter().map(String::as_str).collect();
            l.b.target(ir::TargetKind::DeviceParam, &path, element, Some(param))
        }
        Aim::Source(path, element) => {
            let path: Vec<&str> = path.iter().map(String::as_str).collect();
            l.b.target(ir::TargetKind::SourceDc, &path, element, None)
        }
        Aim::Model(model, param) => {
            let t =
                l.b.target(ir::TargetKind::ModelParam, &[], model, Some(param));
            bind_model(l, t)
        }
        Aim::Temp => ir::Target::TEMP,
        Aim::Option(k) => l.b.target_option(*k, None),
        Aim::PrivateOption(name) => {
            l.b.target_option(ir::OptionKey::Private, Some(("vacask", name)))
        }
    }
}

// ---- private ----

/// The target of `option="name"`: temperature, a typed key, or VACASK's own option kept by
/// name (a solver knob or a VACASK-only key); `None` for an output-only key.
fn option_aim(name: &str) -> Option<Aim> {
    const KEYS: &[(&str, ir::OptionKey)] = &[
        ("reltol", ir::OptionKey::RelTol),
        ("abstol", ir::OptionKey::AbsTol),
        ("vntol", ir::OptionKey::VnTol),
        ("chgtol", ir::OptionKey::ChgTol),
        ("gmin", ir::OptionKey::Gmin),
        ("gshunt", ir::OptionKey::Gshunt),
        ("scale", ir::OptionKey::Scale),
        ("tran_maxord", ir::OptionKey::MaxOrd),
        ("op_itl", ir::OptionKey::Itl1),
        ("tran_itl", ir::OptionKey::Itl4),
        ("homotopy_gminsteps", ir::OptionKey::GminSteps),
        ("homotopy_srcsteps", ir::OptionKey::SrcSteps),
    ];
    if name == "temp" {
        return Some(Aim::Temp);
    }
    if let Some(&(_, k)) = KEYS.iter().find(|(n, _)| *n == name) {
        return Some(Aim::Option(k));
    }
    let own = name.starts_with("tran_")
        || name.starts_with("nr_")
        || name.starts_with("homotopy_")
        || name.starts_with("op_")
        || name.starts_with("hb_")
        || name.starts_with("pss_");
    (own && !name.ends_with("_debug")).then(|| Aim::PrivateOption(name.to_string()))
}

/// `node` plus one: an integer literal's successor, `x` for `x - 1`, else `node + 1`.
fn plus_one(l: &mut Lower<'_, '_, '_>, node: u32) -> ir::ExprId {
    let ast = &l.cst.ast;
    if ast.tag(Node(node)) == Tag::Num
        && let Ok(x) = number::lex(l.cst.tokens.text(ast.main_token(Node(node))))
        && x.int
        && x.literal.mantissa < i64::MAX
    {
        let next = ir::Literal {
            mantissa: x.literal.mantissa + 1,
            ..x.literal
        };
        return l.b.literal(next);
    }
    let e = value(l, node);
    // `x - 1` is the writer's spelling of the count `x` (`write_run::analyses::minus_one`).
    if let ir::Expr::Binary(ir::BinOp::Sub, x, one) = l.b.design().expr(e)
        && let ir::Expr::Num(n) = l.b.design().expr(one)
        && l.b.design().num(n).form == ir::NumForm::Int
        && l.b.design().num(n) == int_one()
    {
        return x;
    }
    let one = l.b.literal(int_one());
    l.b.binary(ir::BinOp::Add, e, one)
}

/// The integer literal 1.
fn int_one() -> ir::Literal {
    number::lex("1").expect("a number").literal
}

/// `t` with its model reference bound to the top level's model of that name, if one exists.
fn bind_model(l: &Lower<'_, '_, '_>, mut t: ir::Target) -> ir::Target {
    let design = l.b.design();
    let top = design.file(l.cst.file).top;
    let def = design.lookup(top, ir::DefKind::Model, t.a.name);
    if !def.is_none() {
        t.a.def = def;
    }
    t
}
