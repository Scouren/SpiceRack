//! `vsource` and `isource` instances → `VSource`/`ISource` rows with their `Source` row and
//! wave: `dc=`, `mag=`/`phase=`, `type=dc|sine|pulse|pwl` and the wave's fields.
//!
//! Rules (`rewrite/DIALECT-SPECTRE.md` §6, each with its evidence):
//! - the DC value is `dc=`, 0 when absent, beside any waveform: DC analyses read `dc`, the
//!   transient the wave (MAN only: "in DC analyses only dc, m, tc are active", §10);
//! - `mag=` the AC magnitude, `phase=` its phase in degrees, 0 when absent (MAN);
//! - `type=sine`: `sinedc ampl freq delay damp sinephase` (RUN-SP `p_measure_save`: `sinedc=0
//!   ampl=1 freq=1k` peaked at 1); before the delay the value at the delay (MAN only, §10);
//!   `sinedc`, `delay`, `damp`, `sinephase` default to 0 (MAN), `ampl` and `freq` must be
//!   given (their defaults are not settled: `Unread`);
//! - `type=pulse`: `val0 val1 delay rise fall width period` under SPICE's law, the fields as
//!   given (RUN-SP `t_m2_capacitors`); `delay` defaults to 0, every other field must be given;
//! - `type=pwl`: `wave=[t v …] delay= pwlperiod=` (RUN-SP `p_pwl_repeat`: 0.25 at 2.25 ms);
//!   a repeat joins its passes over `twidth`, by default `pwlperiod/1000` (MAN);
//! - any other key or type (`exp`, `m`, `tc1`, noise, PWL shaping) is not read yet: `Unread`.
//!
//! Contents: `FEATURES`, `source` (crate-private)

use super::codes;
use super::expr_read::{self, Context};
use super::read::Lower;
use super::read_elements::{Fields, value};
use crate::ir;
use crate::ir::CircuitExtra as X;
use crate::ir::Feature as F;
use crate::syntax::ast::{Node, Tag};

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(F::Elem(ir::ElemKind::VSource))
    .with(F::Elem(ir::ElemKind::ISource))
    .with(F::Wave(ir::WaveKind::Sin))
    .with(F::Wave(ir::WaveKind::Pulse))
    .with(F::Wave(ir::WaveKind::Pwl))
    .with(F::Circuit(X::DcGivenWithWave))
    .with(F::Circuit(X::GivenZeroAsGiven))
    .with(F::Circuit(X::SinPreHold))
    .with(F::Circuit(X::PulseSpice))
    .with(F::Circuit(X::PwlDelay))
    .with(F::Circuit(X::PwlRepeatPeriod))
    .with(F::Circuit(X::PwlJoinRampWidth));

/// The keys each type reads, beside `dc`, `mag`, `phase` and `type`.
const KEYS: &[(&str, &[&str])] = &[
    ("dc", &[]),
    (
        "sine",
        &["sinedc", "ampl", "freq", "delay", "damp", "sinephase"],
    ),
    (
        "pulse",
        &["val0", "val1", "delay", "rise", "fall", "width", "period"],
    ),
    ("pwl", &["wave", "delay", "pwlperiod", "twidth"]),
];

/// `vsource`/`isource` instance `n` with `fields`: a source row, or why not.
///
/// # Errors
/// The class and code of the `Opaque` the statement becomes.
pub(super) fn source(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    master: &str,
    fields: &Fields,
) -> Result<(), (ir::OpaqueClass, ir::Code)> {
    let unread = (ir::OpaqueClass::Unread, codes::UNREAD);
    let kind = match master {
        "vsource" => ir::ElemKind::VSource,
        _ => ir::ElemKind::ISource,
    };
    let h = l.cst.ast.header(n);
    if h.nodes.len() != 2 {
        return Err((ir::OpaqueClass::InvalidInSource, codes::TERMINALS));
    }
    let get = |k: &str| fields.iter().find(|(key, _)| key == k).map(|&(_, v)| v);
    let ty = get("type").map_or("dc".to_string(), |v| l.field(v));
    let Some((_, keys)) = KEYS.iter().find(|(t, _)| *t == ty) else {
        return Err(unread);
    };
    let known = |k: &str| matches!(k, "dc" | "mag" | "phase" | "type") || keys.contains(&k);
    if fields.iter().any(|(k, _)| !known(k)) {
        return Err(unread);
    }
    let required: &[&str] = match ty.as_str() {
        "sine" => &["ampl", "freq"],
        "pulse" => &["val0", "val1", "rise", "fall", "width", "period"],
        "pwl" => &["wave"],
        _ => &[],
    };
    if required.iter().any(|k| get(k).is_none()) {
        return Err((ir::OpaqueClass::Unread, codes::SOURCE_DEFAULT));
    }
    let points = match get("wave") {
        Some(w) => Some(vector(l, w).ok_or(unread)?),
        None => None,
    };
    // (time, value) pairs, at least one (MAN: `wave=[t1 v1 t2 v2 …]`)
    if points
        .as_ref()
        .is_some_and(|p| p.is_empty() || p.len() % 2 == 1)
    {
        return Err((ir::OpaqueClass::InvalidInSource, codes::MALFORMED));
    }
    for (k, v) in fields {
        if !matches!(k.as_str(), "type" | "wave") {
            expr_read::check_expr(l, *v, Context::Param).map_err(|r| (r.class, r.code))?;
        }
    }
    for &p in points.iter().flatten() {
        expr_read::check_expr(l, p, Context::Param).map_err(|r| (r.class, r.code))?;
    }
    let opt = |l: &mut Lower<'_, '_, '_>, k: &str| get(k).map_or(ir::ExprId::NONE, |v| value(l, v));
    let zero = |l: &mut Lower<'_, '_, '_>, e: ir::ExprId| match e.is_none() {
        true => l.b.literal(ir::Literal::ZERO),
        false => e,
    };
    let dc = opt(l, "dc");
    let dc = zero(l, dc);
    let ac_mag = opt(l, "mag");
    let ac_phase_deg = match (ac_mag.is_none(), opt(l, "phase")) {
        (true, p) if p.is_none() => p,
        (true, _) => return Err(unread),
        (false, p) => zero(l, p),
    };
    let wave = match ty.as_str() {
        "sine" => {
            let offset = opt(l, "sinedc");
            let offset = zero(l, offset);
            let amplitude = opt(l, "ampl");
            let freq = opt(l, "freq");
            let delay = opt(l, "delay");
            let delay = zero(l, delay);
            let damping = opt(l, "damp");
            let damping = zero(l, damping);
            let phase_deg = opt(l, "sinephase");
            let phase_deg = zero(l, phase_deg);
            l.b.wave(ir::wave::Sin {
                offset,
                amplitude,
                freq,
                delay,
                damping,
                phase_deg,
                cycles: ir::ExprId::NONE,
                amplitude2: ir::ExprId::NONE,
                freq2: ir::ExprId::NONE,
                pre: ir::wave::PreDelay::HoldDelayValue,
                given_zero: ir::wave::GivenZero::AsGiven,
            })
        }
        "pulse" => {
            let v1 = opt(l, "val0");
            let v2 = opt(l, "val1");
            let delay = opt(l, "delay");
            let delay = zero(l, delay);
            let rise = opt(l, "rise");
            let fall = opt(l, "fall");
            let width = opt(l, "width");
            let period = opt(l, "period");
            l.b.wave(ir::wave::Pulse {
                v1,
                v2,
                delay,
                rise,
                fall,
                width,
                period,
                cycles: ir::ExprId::NONE,
                law: ir::wave::PulseLaw::Spice,
                given_zero: ir::wave::GivenZero::AsGiven,
            })
        }
        "pwl" => {
            let values: Vec<ir::ExprId> = points
                .expect("required")
                .into_iter()
                .map(|p| value(l, p))
                .collect();
            let points = l.b.exprs(&values);
            let delay = opt(l, "delay");
            let repeat_at = opt(l, "pwlperiod");
            let (repeat, join, join_width) = match (repeat_at.is_none(), get("twidth")) {
                (true, None) => (
                    ir::wave::PwlRepeat::Never,
                    ir::wave::PwlJoin::Jump,
                    ir::ExprId::NONE,
                ),
                (true, Some(_)) => return Err(unread),
                (false, Some(w)) => (
                    ir::wave::PwlRepeat::Period,
                    ir::wave::PwlJoin::RampWidth,
                    value(l, w),
                ),
                (false, None) => {
                    let period = value(l, get("pwlperiod").expect("given"));
                    let thousand = l.b.literal(ir::Literal {
                        mantissa: 1000,
                        frac: 0,
                        exp10: 0,
                        form: ir::NumForm::Plain,
                    });
                    let width = l.b.binary(ir::BinOp::Div, period, thousand);
                    (
                        ir::wave::PwlRepeat::Period,
                        ir::wave::PwlJoin::RampWidth,
                        width,
                    )
                }
            };
            l.b.wave(ir::wave::Pwl {
                points,
                delay,
                repeat_at,
                repeat_count: ir::ExprId::NONE,
                join_width,
                file: ir::NameId::NONE,
                repeat,
                join,
            })
        }
        _ => ir::WaveRef::NONE,
    };
    let row = ir::source::Source {
        dc,
        ac_mag,
        ac_phase_deg,
        tc1: ir::ExprId::NONE,
        tc2: ir::ExprId::NONE,
        pac_mag: ir::ExprId::NONE,
        pac_phase_deg: ir::ExprId::NONE,
        wave,
        noise_psd: ir::List::EMPTY,
        parasitics: ir::NO_ROW,
        port: ir::NO_ROW,
        distof: ir::NO_ROW,
        dc_rule: ir::source::DcRule::Given,
    };
    let name = l.field(h.name);
    let nodes: Vec<String> = h.nodes.iter().map(|&x| l.field(x)).collect();
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let head = l.b.head(&name, &nodes, None, &[], ir::ExprId::NONE);
    let span = l.span(n);
    l.b.source(l.scope, kind, head, row, span);
    Ok(())
}

// ---- private ----

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
