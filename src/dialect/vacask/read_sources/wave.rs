//! `vsource`/`isource` → one `VSource`/`ISource` element with its `Source` row and wave (VD
//! dev-builtin-src; runs src-dump, src-sine, src-pulse, src-exp, src-pwl, src-amfm, p1, p2):
//!
//! | `type` | IR |
//! |---|---|
//! | `dc` | `dc` given, no wave (`DcRule::Given`) |
//! | `sine` | `Sin{offset sinedc, amplitude ampl, freq, delay, damping theta, phase_deg tdphase, pre: HoldDelayValue}` |
//! | `pulse` | `Pulse{v1 val0, v2 val1, delay, rise, fall, width, period, law: Vacask}` |
//! | `exp` | `Exp{v1 val0, v2 val1, td1 delay, tau1, td2, tau2, td2_from: Td1}` |
//! | `pwl` | `Pwl{points wave (a first time above 0 gets the point (0, y1) VACASK holds), delay, period → Period + Extrapolate, file}` |
//! | `am` | `Am{offset sinedc, amplitude ampl, mod_offset 1, mod_index, mod_freq modfreq, carrier_freq freq, delay, phase_m modphase, phase_c tdphase, mod_time: Delay, pre: HoldDelayValue}` |
//! | `fm` | `Sffm{offset sinedc, amplitude ampl, carrier_freq freq, mod_index, signal_freq modfreq, delay, phase_c tdphase, phase_m modphase, law: Spice3, pre: HoldDelayValue}` |
//!
//! Every wave is `GivenZero::AsGiven` and `DcRule::WaveAtZero` (`dc` ignored, run src-sine).
//!
//! Cites: VD dev-builtin-src; `rewrite/DIALECT-VACASK.md` §3.2 sources … fm; A12 C1–C4, A17.
//!
//! Contents: `independent` (crate-private)

use super::common::{
    COMMON, Fail, Params, Val, at_default, build, check, head, invalid, mult, nonpositive, private,
    source_key, string, terminals, type_keys,
};
use super::{DORMANT, SHAPING, SPURS, UNSTATED, default_of};
use crate::dialect::vacask::read::Lower;
use crate::ir;
use crate::ir::wave as w;
use crate::syntax::ast::{Node, Tag};

/// The PWL parameters that shape it or set its breakpoints: `Private` off their defaults.
const PWL_SHAPING: &[&str] = &[
    "scale",
    "offset",
    "stretch",
    "tdphase",
    "break",
    "slopetol",
    "sloperel",
    "slopeglob",
];

/// What a wave's `Val`s are and which wave they make (read in field order).
enum Plan {
    /// No wave: `type="dc"`.
    Dc,
    /// One of the closed-form waves, its fields in declaration order.
    Wave(&'static str, Vec<Val>),
    /// A PWL: its points, `delay` and `period` (each optional), or a file.
    Pwl {
        points: Vec<Val>,
        delay: Option<Val>,
        period: Option<Val>,
        file: Option<String>,
    },
}

/// A `vsource` (`current` false) or `isource` statement, validated whole before the first push.
pub(super) fn independent(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    p: &Params,
    current: bool,
) -> Result<(), Fail> {
    if !p.keys().all(source_key) {
        return Err(invalid());
    }
    terminals(l, n, 2)?;
    let typ = match p.get("type") {
        Some(v) => string(l, v)?,
        None => "dc".to_string(),
    };
    let active = type_keys(&typ).ok_or_else(invalid)?;
    if ["spur", "smag", "sphase"]
        .iter()
        .any(|k| p.get(k).is_some())
    {
        return Err(private(SPURS));
    }
    for key in p.keys() {
        if COMMON.contains(&key) || active.contains(&key) {
            continue;
        }
        let v = p.get(key).expect("a given key");
        if !default_of(key).is_some_and(|d| at_default(l, v, d)) {
            return Err(private(DORMANT));
        }
    }
    let plan = plan(l, p, &typ)?;
    let ac = (p.get("mag").is_some() || p.get("phase").is_some())
        .then(|| (p.val("mag"), p.val("phase")));
    let dc = (typ == "dc").then(|| p.val("dc"));
    let mut vals: Vec<Val> = p.get("$mfactor").map(Val::Node).into_iter().collect();
    vals.extend(ac.iter().flat_map(|&(m, ph)| [m, ph]));
    vals.extend(dc);
    match &plan {
        Plan::Dc => {}
        Plan::Wave(_, f) => vals.extend(f.iter().copied()),
        Plan::Pwl {
            points,
            delay,
            period,
            ..
        } => vals.extend(points.iter().copied().chain(*delay).chain(*period)),
    }
    check(l, &vals)?;
    let mult = mult(l, p);
    let (ac_mag, ac_phase_deg) = match ac {
        Some((m, ph)) => (build(l, m), build(l, ph)),
        None => (ir::ExprId::NONE, ir::ExprId::NONE),
    };
    let dc = dc.map_or(ir::ExprId::NONE, |v| build(l, v));
    let wave = push_wave(l, plan);
    let dc_rule = match wave.is_none() {
        true => ir::source::DcRule::Given,
        false => ir::source::DcRule::WaveAtZero,
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
        dc_rule,
    };
    let kind = match current {
        true => ir::ElemKind::ISource,
        false => ir::ElemKind::VSource,
    };
    let head = head(l, n, mult);
    let span = l.span(n);
    l.b.source(l.scope, kind, head, row, span);
    Ok(())
}

// ---- private ----

/// The wave `typ` makes of `p`, with what VACASK rejects or the IR cannot hold refused.
fn plan(l: &Lower<'_, '_, '_>, p: &Params, typ: &str) -> Result<Plan, Fail> {
    let f = |keys: &[&str]| keys.iter().map(|k| p.val(k)).collect::<Vec<Val>>();
    let refuse_if = |key: &str| match p.get(key) {
        Some(v) if nonpositive(l, v) => Err(invalid()),
        _ => Ok(()),
    };
    Ok(match typ {
        "dc" => Plan::Dc,
        "sine" => {
            refuse_if("freq")?;
            Plan::Wave(
                "sine",
                f(&["sinedc", "ampl", "freq", "delay", "theta", "tdphase"]),
            )
        }
        "pulse" => {
            refuse_if("rise")?;
            Plan::Wave(
                "pulse",
                f(&["val0", "val1", "delay", "rise", "fall", "width", "period"]),
            )
        }
        "exp" => {
            p.get("td2").ok_or_else(invalid)?;
            refuse_if("td2")?;
            Plan::Wave("exp", f(&["val0", "val1", "delay", "tau1", "td2", "tau2"]))
        }
        "am" | "fm" => Plan::Wave(
            if typ == "am" { "am" } else { "fm" },
            f(&[
                "sinedc", "ampl", "modindex", "modfreq", "freq", "delay", "modphase", "tdphase",
            ]),
        ),
        _ => pwl(l, p)?,
    })
}

/// A PWL's plan (VD dev-builtin-src PWL; run src-pwl).
fn pwl(l: &Lower<'_, '_, '_>, p: &Params) -> Result<Plan, Fail> {
    for key in PWL_SHAPING {
        if let Some(v) = p.get(key)
            && !at_default(l, v, default_of(key).expect("a default"))
        {
            return Err(private(SHAPING));
        }
    }
    let delay = p.get("delay").map(Val::Node);
    let period = match p.get("period") {
        None => None,
        Some(v) if nonpositive(l, v) => None,
        Some(v) if l.cst.ast.tag(Node(v)) == Tag::Num => Some(Val::Node(v)),
        Some(_) => return Err(private(UNSTATED)),
    };
    if let Some(v) = p.get("file") {
        if p.get("wave").is_some() {
            return Err(private(DORMANT));
        }
        return Ok(Plan::Pwl {
            points: Vec::new(),
            delay,
            period,
            file: Some(string(l, v)?),
        });
    }
    let v = p.get("wave").ok_or_else(invalid)?;
    let ast = &l.cst.ast;
    if ast.tag(Node(v)) != Tag::Vector {
        return Err(private(UNSTATED));
    }
    let at = ast.rhs(Node(v));
    let items: Vec<u32> = (0..ast.extra(at + 1))
        .map(|k| ast.extra(at + 2 + k))
        .collect();
    if items.len() < 4 || !items.len().is_multiple_of(2) {
        return Err(invalid());
    }
    let mut points: Vec<Val> = Vec::with_capacity(items.len() + 2);
    if period.is_none() && !at_default(l, items[0], "0.") {
        // VACASK holds y1 before x1: the IR states it as the point (0, y1).
        if ast.tag(Node(items[0])) != Tag::Num {
            return Err(private(UNSTATED));
        }
        points.extend([Val::Lit("0."), Val::Node(items[1])]);
    }
    points.extend(items.iter().map(|&x| Val::Node(x)));
    Ok(Plan::Pwl {
        points,
        delay,
        period,
        file: None,
    })
}

/// Pushes the wave of a checked plan; `WaveRef::NONE` for `Dc`.
fn push_wave(l: &mut Lower<'_, '_, '_>, plan: Plan) -> ir::WaveRef {
    let (name, f) = match plan {
        Plan::Dc => return ir::WaveRef::NONE,
        Plan::Pwl {
            points,
            delay,
            period,
            file,
        } => return push_pwl(l, &points, delay, period, file),
        Plan::Wave(name, f) => (name, f),
    };
    let e: Vec<ir::ExprId> = f.into_iter().map(|v| build(l, v)).collect();
    let hold = w::PreDelay::HoldDelayValue;
    let as_given = w::GivenZero::AsGiven;
    match name {
        "sine" => l.b.wave(w::Sin {
            offset: e[0],
            amplitude: e[1],
            freq: e[2],
            delay: e[3],
            damping: e[4],
            phase_deg: e[5],
            cycles: ir::ExprId::NONE,
            amplitude2: ir::ExprId::NONE,
            freq2: ir::ExprId::NONE,
            pre: hold,
            given_zero: as_given,
        }),
        "pulse" => l.b.wave(w::Pulse {
            v1: e[0],
            v2: e[1],
            delay: e[2],
            rise: e[3],
            fall: e[4],
            width: e[5],
            period: e[6],
            cycles: ir::ExprId::NONE,
            law: w::PulseLaw::Vacask,
            given_zero: as_given,
        }),
        "exp" => l.b.wave(w::Exp {
            v1: e[0],
            v2: e[1],
            td1: e[2],
            tau1: e[3],
            td2: e[4],
            tau2: e[5],
            td2_from: w::TdFrom::Td1,
            given_zero: as_given,
        }),
        "am" => {
            let one = build(l, Val::Lit("1."));
            l.b.wave(w::Am {
                offset: e[0],
                amplitude: e[1],
                mod_offset: one,
                mod_index: e[2],
                mod_freq: e[3],
                carrier_freq: e[4],
                delay: e[5],
                phase_m_deg: e[6],
                phase_c_deg: e[7],
                mod_time: w::AmModTime::Delay,
                pre: hold,
            })
        }
        _ => l.b.wave(w::Sffm {
            offset: e[0],
            amplitude: e[1],
            carrier_freq: e[4],
            mod_index: e[2],
            signal_freq: e[3],
            delay: e[5],
            phase_c_deg: e[7],
            phase_m_deg: e[6],
            law: w::SffmLaw::Spice3,
            pre: hold,
            given_zero: as_given,
        }),
    }
}

/// Pushes a PWL wave.
fn push_pwl(
    l: &mut Lower<'_, '_, '_>,
    points: &[Val],
    delay: Option<Val>,
    period: Option<Val>,
    file: Option<String>,
) -> ir::WaveRef {
    let ids: Vec<ir::ExprId> = points.iter().map(|&v| build(l, v)).collect();
    let points = match ids.is_empty() {
        true => ir::List::EMPTY,
        false => l.b.exprs(&ids),
    };
    let delay = delay.map_or(ir::ExprId::NONE, |v| build(l, v));
    let (repeat, repeat_at, join) = match period {
        Some(v) => (w::PwlRepeat::Period, build(l, v), w::PwlJoin::Extrapolate),
        None => (w::PwlRepeat::Never, ir::ExprId::NONE, w::PwlJoin::Jump),
    };
    let file = file.map_or(ir::NameId::NONE, |f| l.b.name(&f));
    l.b.wave(w::Pwl {
        points,
        delay,
        repeat_at,
        repeat_count: ir::ExprId::NONE,
        join_width: ir::ExprId::NONE,
        file,
        repeat,
        join,
    })
}
