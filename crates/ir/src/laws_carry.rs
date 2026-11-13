//! The carrier predicates of the law table (R90, R92, R94, R94a): a law's row and one circuit
//! row (an element or a model) in, whether that row carries the law's point out. One predicate
//! per point, exhaustive over `Point` and `Carrier`; the three small-signal points share one.
//! Split from `laws.rs` at its size limit.
//!
//! The predicates read `Model.params` and `Element.params` by name: the family's own names,
//! which are ngspice-45's and the pinned VACASK module's alike for every name read here (both
//! builds' parameter tables, read 2026-11-12: `cjp`/`cjsw` and `tcvth`/`vtotc` are aliases in
//! both, so both spellings count). "Absent" is no parameter of any spelling; "given, not
//! literally v" is given with a value that is not the literal v (a parameter read is not
//! literal). An unresolved model is a carrier: its law is unknown.
//!
//! Cites: `rewrite/RULINGS.md` R86, R88, R91, R92, R93a, R94, R94a, R95, R95a, R96, R102 (on the `rulings` branch); the BSIM4 4.5 point, gf180's runs; the runs
//! each `Law` variant's doc names.
//!
//! Contents (crate-private): `pub(crate) fn carries(&Design, &Row, Carrier) -> bool`,
//! `pub(crate) const GEN_PARAMS` (gen)

use crate::circuit_text::get_row;
use crate::design::Design;
use crate::element::{ElemKind, Element};
use crate::expr::{BinOp, Expr, Func};
use crate::id::{ExprId, ParamList};
use crate::item::ItemKind;
use crate::laws::{Carrier, Point, Row};
use crate::model::{Model, ModelFamily, Polarity};
use crate::semi::Semi;

/// The parameters the predicates read, per family: the generator draws them (values at random)
/// so that a generated design reaches every carrier.
#[cfg(any(test, feature = "gen"))]
pub(crate) const GEN_PARAMS: &[(ModelFamily, &[&str])] = &[
    (ModelFamily::GummelPoon, &["iss", "ns", "irb"]),
    (ModelFamily::Diode, &["wm", "cjsw", "isr", "ttt1"]),
    (ModelFamily::DiodeLevel3, &["wm", "cjsw", "isr", "ttt1"]),
    (ModelFamily::Vdmos, &["tcvth", "cjo", "vj"]),
];

/// Whether `row` carries `law`'s point.
pub(crate) fn carries(design: &Design, law: &Row, row: Carrier<'_>) -> bool {
    let f = law.families;
    match (law.point, row) {
        (Point::BcSatArea, Carrier::Element(e)) => bc_sat_area(design, e, f),
        (Point::DiodeWlMult, Carrier::Element(e)) => diode_wl_mult(design, e, f),
        (Point::DiodeSidewallKnee, Carrier::Element(e)) => diode_sidewall(design, e, f),
        (Point::MosSmallSignal | Point::Mos6SmallSignal, Carrier::Element(e)) => small_signal(
            design,
            e,
            ElemKind::Mosfet,
            f,
            |_, _| true,
            |_, _| true,
            crate::analysis::small_signal_design,
        ),
        (Point::BjtIrbSmallSignal, Carrier::Element(e)) => {
            let irb = |d: &Design, m: &Model| given_not(d, param(d, m.params, &["irb"]), 0.0);
            small_signal(
                design,
                e,
                ElemKind::Bjt,
                f,
                irb,
                |_, _| true,
                crate::analysis::small_signal_design,
            )
        }
        (Point::VdmosMultCharges, Carrier::Element(e)) => {
            let mult =
                |d: &Design, e: &Element| !e.mult.is_none() && !crate::run_words::is_one(d, e.mult);
            let charges = crate::analysis::charge_design;
            small_signal(design, e, ElemKind::Mosfet, f, |_, _| true, mult, charges)
        }
        (Point::DiodeIsrSmallSignal, Carrier::Element(e)) => small_signal(
            design,
            e,
            ElemKind::Diode,
            f,
            isr,
            |_, _| true,
            crate::analysis::small_signal_design,
        ),
        (Point::DiodeIsrReverse, Carrier::Element(e)) => {
            e.kind == ElemKind::Diode
                && match model_of(design, e) {
                    Ok(None) => true,
                    Ok(Some(m)) => f.contains(&m.family) && isr(design, &m),
                    Err(()) => false,
                }
        }
        (Point::Bsim4v45SmallSignal, Carrier::Element(e)) => small_signal(
            design,
            e,
            ElemKind::Mosfet,
            f,
            bsim4_v45,
            |_, _| true,
            crate::analysis::small_signal_design,
        ),
        (point, Carrier::Node(e)) => node(point, e),
        (point, Carrier::Design) => whole(design, point),
        (Point::DiodeTtFactor, Carrier::Model(m)) => {
            let ttt = |k: &str| given_not(design, param(design, m.params, &[k]), 0.0);
            f.contains(&m.family) && (ttt("ttt1") || ttt("ttt2"))
        }
        (Point::SubSatTemp, Carrier::Model(m)) => f.contains(&m.family) && sub_sat_temp(design, m),
        (Point::DiodeXom, Carrier::Model(m)) => f.contains(&m.family) && diode_xom(design, m),
        (Point::VdmosKp, Carrier::Model(m)) => {
            f.contains(&m.family) && param(design, m.params, &["kp"]).is_none()
        }
        (Point::VdmosTcvth, Carrier::Model(m)) => {
            let tcvth = param(design, m.params, &["tcvth", "vtotc"]);
            f.contains(&m.family) && m.polarity != Polarity::P && given_not(design, tcvth, 0.0)
        }
        (Point::VdmosClamp, Carrier::Model(m)) => f.contains(&m.family) && vdmos_clamp(design, m),
        (
            Point::BcSatArea
            | Point::DiodeWlMult
            | Point::DiodeSidewallKnee
            | Point::MosSmallSignal
            | Point::Mos6SmallSignal
            | Point::BjtIrbSmallSignal
            | Point::VdmosMultCharges
            | Point::DiodeIsrReverse
            | Point::DiodeIsrSmallSignal
            | Point::TrigReduce
            | Point::ExpClamp
            | Point::LogZero
            | Point::DivGuard
            | Point::AsinEdge
            | Point::DefaultTemp
            | Point::DefaultTnom
            | Point::Bsim4v45SmallSignal,
            Carrier::Model(_),
        )
        | (
            Point::SubSatTemp
            | Point::DiodeXom
            | Point::VdmosKp
            | Point::VdmosTcvth
            | Point::VdmosClamp
            | Point::DiodeTtFactor
            | Point::TrigReduce
            | Point::ExpClamp
            | Point::LogZero
            | Point::DivGuard
            | Point::AsinEdge
            | Point::DefaultTemp
            | Point::DefaultTnom,
            Carrier::Element(_),
        ) => false,
    }
}

// ---- private ----

/// R93a's function points: whether an expression node in a root of dynamic form carries
/// `point` (the function or operator whose ngspice-45 B-source law differs).
fn node(point: Point, e: Expr) -> bool {
    let call = |fs: &[Func]| matches!(e, Expr::Call(f, _) if fs.contains(&f));
    match point {
        Point::TrigReduce => call(&[Func::Sin, Func::Cos, Func::Tan]),
        Point::ExpClamp => call(&[Func::Exp]),
        Point::LogZero => call(&[Func::Ln, Func::Log10]),
        Point::DivGuard => matches!(e, Expr::Binary(BinOp::Div | BinOp::DivTyped, ..)),
        Point::AsinEdge => call(&[Func::Asin, Func::Acos]),
        Point::BcSatArea
        | Point::SubSatTemp
        | Point::DiodeWlMult
        | Point::DiodeSidewallKnee
        | Point::DiodeXom
        | Point::VdmosKp
        | Point::VdmosTcvth
        | Point::VdmosClamp
        | Point::MosSmallSignal
        | Point::Mos6SmallSignal
        | Point::BjtIrbSmallSignal
        | Point::VdmosMultCharges
        | Point::DiodeIsrReverse
        | Point::DiodeIsrSmallSignal
        | Point::DiodeTtFactor
        | Point::DefaultTemp
        | Point::DefaultTnom
        | Point::Bsim4v45SmallSignal => false,
    }
}

/// R102: whether the design carries `point`, a default of the whole run: no `Env` item states
/// the temperature (resp. the nominal temperature).
fn whole(design: &Design, point: Point) -> bool {
    let stated = |field: fn(&crate::option::Env) -> crate::id::ExprId| {
        design.reachable().into_iter().any(|(item, _)| {
            item.kind == ItemKind::Env
                && !field(&design.row::<crate::option::Env>(item.index)).is_none()
        })
    };
    match point {
        Point::DefaultTemp => !stated(|e| e.temp),
        Point::DefaultTnom => !stated(|e| e.tnom),
        Point::BcSatArea
        | Point::SubSatTemp
        | Point::DiodeWlMult
        | Point::DiodeSidewallKnee
        | Point::DiodeXom
        | Point::VdmosKp
        | Point::VdmosTcvth
        | Point::VdmosClamp
        | Point::MosSmallSignal
        | Point::Mos6SmallSignal
        | Point::BjtIrbSmallSignal
        | Point::VdmosMultCharges
        | Point::DiodeIsrReverse
        | Point::DiodeIsrSmallSignal
        | Point::DiodeTtFactor
        | Point::TrigReduce
        | Point::ExpClamp
        | Point::LogZero
        | Point::DivGuard
        | Point::AsinEdge
        | Point::Bsim4v45SmallSignal => false,
    }
}

/// The model an element names: `Ok(None)` when unresolved (its law unknown: a carrier),
/// `Ok(Some(m))` for a model row, `Err(())` when it names another kind of definition.
fn model_of(design: &Design, e: &Element) -> Result<Option<Model>, ()> {
    let Some(d) = e
        .model
        .def
        .index_checked(design.defs.len())
        .map(|k| design.defs[k])
    else {
        return Ok(None);
    };
    if d.item.kind != ItemKind::Model {
        return Err(());
    }
    Ok(Some(design.row(d.item.index)))
}

/// A parameter's value under any of its spellings, `None` when none is given.
fn param(design: &Design, list: ParamList, names: &[&str]) -> Option<ExprId> {
    design
        .params(list)
        .iter()
        .find(|p| names.contains(&design.str(p.0)))
        .map(|p| p.1)
}

/// Whether `e` is given and not literally `v` (a parameter read is not literal).
fn given_not(design: &Design, e: Option<ExprId>, v: f64) -> bool {
    e.is_some_and(|e| literal(design, e) != Some(v))
}

/// R86's carrier: a BJT with an area not literally 1 on a model of `families` that does not
/// give both `ibe` and `ibc`, or on an unresolved model.
fn bc_sat_area(design: &Design, e: &Element, families: &[ModelFamily]) -> bool {
    if e.kind != ElemKind::Bjt {
        return false;
    }
    let Some(s) = get_row::<Semi>(design, e.detail) else {
        return false;
    };
    if s.area.is_none() || crate::run_words::is_one(design, s.area) {
        return false;
    }
    match model_of(design, e) {
        Ok(None) => true,
        Ok(Some(m)) => {
            let given = |k: &str| param(design, m.params, &[k]).is_some();
            families.contains(&m.family) && !(given("ibe") && given("ibc"))
        }
        Err(()) => false,
    }
}

/// R91 F20's carrier: a diode giving `w` and `l` with `m` given and not literally 1, on a model
/// of `families` or an unresolved one.
fn diode_wl_mult(design: &Design, e: &Element, families: &[ModelFamily]) -> bool {
    let wl = param(design, e.params, &["w"]).is_some() && param(design, e.params, &["l"]).is_some();
    if e.kind != ElemKind::Diode || !wl {
        return false;
    }
    if e.mult.is_none() || crate::run_words::is_one(design, e.mult) {
        return false;
    }
    match model_of(design, e) {
        Ok(None) => true,
        Ok(Some(m)) => families.contains(&m.family),
        Err(()) => false,
    }
}

/// R91 F22's carrier: a diode with a perimeter on a model of `families` whose `cjsw` (`cjp`)
/// is given and not literally 0, or on an unresolved model.
fn diode_sidewall(design: &Design, e: &Element, families: &[ModelFamily]) -> bool {
    if e.kind != ElemKind::Diode {
        return false;
    }
    let pj = given_not(design, param(design, e.params, &["pj", "perim"]), 0.0);
    let wl = param(design, e.params, &["w"]).is_some() && param(design, e.params, &["l"]).is_some();
    match model_of(design, e) {
        Ok(None) => pj || wl,
        Ok(Some(m)) => {
            let perimeter = pj || wl && m.family == ModelFamily::DiodeLevel3;
            let cjsw = param(design, m.params, &["cjsw", "cjp"]);
            perimeter && families.contains(&m.family) && given_not(design, cjsw, 0.0)
        }
        Err(()) => false,
    }
}

/// The small-signal and charge points' one carrier (R94, R94a, R95, R95a, R96): an element of
/// `kind` meeting `instance_ok` on a model of `families` meeting `model_ok` (or an unresolved
/// model, whose conditions are unknown) in a design `analyses` admits (a linearised analysis or
/// none; for F30's charges, an analysis using charges or none).
fn small_signal(
    design: &Design,
    e: &Element,
    kind: ElemKind,
    families: &[ModelFamily],
    model_ok: fn(&Design, &Model) -> bool,
    instance_ok: fn(&Design, &Element) -> bool,
    analyses: fn(&Design) -> bool,
) -> bool {
    if e.kind != kind || !instance_ok(design, e) {
        return false;
    }
    let model = match model_of(design, e) {
        Ok(None) => true,
        Ok(Some(m)) => families.contains(&m.family) && model_ok(design, &m),
        Err(()) => false,
    };
    model && analyses(design)
}

/// Whether a model's version is 4.5 (BSIM4's 4.5 code on ngspice-45): the literal 4.5 or the
/// string "4.5"; a read is not literal.
fn bsim4_v45(design: &Design, m: &Model) -> bool {
    if m.version.is_none() {
        return false;
    }
    match design.expr(m.version) {
        Expr::Num(_) => literal(design, m.version) == Some(4.5),
        Expr::Str(s) => design.str(s) == "4.5",
        _ => false,
    }
}

/// Whether a diode model's `isr` is given and not literally 0.
fn isr(design: &Design, m: &Model) -> bool {
    given_not(design, param(design, m.params, &["isr"]), 0.0)
}

/// R88's carrier: a model giving `iss`, `tlev` absent or literally 0 or 1, and `ns` not
/// literally the last-set factor's exponent.
fn sub_sat_temp(design: &Design, m: &Model) -> bool {
    let get = |key: &str| param(design, m.params, &[key]);
    let tlev_ok =
        get("tlev").is_none_or(|e| literal(design, e).is_some_and(|v| v == 0.0 || v == 1.0));
    if get("iss").is_none() || !tlev_ok {
        return false;
    }
    // the exponent the last-set factor uses: nr with both currents, else 1; absent means 1
    let both = get("ibe").is_some() && get("ibc").is_some();
    let other = if both {
        get("nr").map_or(Some(1.0), |e| literal(design, e))
    } else {
        Some(1.0)
    };
    let ns = get("ns").map_or(Some(1.0), |e| literal(design, e));
    !matches!((ns, other), (Some(a), Some(b)) if a == b)
}

/// R92 F21's carrier: a model giving `wm`, `lm`, `wp` or `lp` and lacking `xom` or `xoi`.
fn diode_xom(design: &Design, m: &Model) -> bool {
    let metal = param(design, m.params, &["wm", "lm", "wp", "lp"]).is_some();
    let absent = |k: &str| param(design, m.params, &[k]).is_none();
    metal && (absent("xom") || absent("xoi"))
}

/// R92 F26's carrier: a model giving `cjo` whose fc·vj is not provably at most 2.5 (absent:
/// fc 0.5, vj 0.8, in both builds).
fn vdmos_clamp(design: &Design, m: &Model) -> bool {
    if param(design, m.params, &["cjo"]).is_none() {
        return false;
    }
    let value = |k: &str, absent: f64| {
        param(design, m.params, &[k]).map_or(Some(absent), |e| literal(design, e))
    };
    match (value("fc", 0.5), value("vj", 0.8)) {
        (Some(fc), Some(vj)) => fc * vj > 2.5,
        _ => true,
    }
}

/// The value of a literal number; `None` for a parameter read or any other expression.
fn literal(design: &Design, e: ExprId) -> Option<f64> {
    match design.expr(e) {
        crate::expr::Expr::Num(k) => Some(design.literal_value(design.num(k))),
        _ => None,
    }
}
