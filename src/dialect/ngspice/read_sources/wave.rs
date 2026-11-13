//! A source card's waveform values → its wave row, positional as ngspice reads them
//! (`vsrcload.c`, `isrcload.c`), each field what the card holds and `NONE` when omitted
//! (A17, R35: the wave's `GivenZero::SpiceDefault` states ngspice's defaults and zero rule).
//!
//! Orders: PULSE `v1 v2 td tr tf pw per np`; SIN `vo va freq td theta phase`; EXP
//! `v1 v2 td1 tau1 td2 tau2`; SFFM `vo va fm mdi fc td phm phc` (an `I` card: `vo va fm mdi fc
//! phm phc`, no delay: `isrcload.c` SFFM reads its 6th value as the phase and never shifts the
//! time); AM `vo vmo vma fm fc td phm phc` (amplitude 1: ngspice's AM has none); PWL pairs with
//! `td` and `r`; TRNOISE `na nt nalpha namp rtsam rtscapt rtsemt`; TRRANDOM `type ts td p1 p2`.
//!
//! Contents: `pub(super) struct Wave`, `pub(super) const ARITY`,
//! `pub(super) fn read(…) -> Result<Wave, Fail>`, `pub(super) fn push(…) -> ir::WaveRef`

use super::card::Card;
use super::{EXTRA_VALUES, IGNORED, PWL_REPEAT, RANDOM_TYPE, SOURCE_INVALID, Val};
use super::{emit, emit_opt, int, invalid, unread, val};
use crate::dialect::ngspice::expr_read::Fail;
use crate::dialect::ngspice::number;
use crate::dialect::ngspice::read::{Lower, Stmt};
use crate::ir;
use crate::ir::WaveKind as K;
use crate::ir::wave as w;

/// A wave read and not yet pushed.
pub(super) struct Wave {
    kind: K,
    /// The values after the TRRANDOM type, as given (no more than the wave reads).
    vals: Vec<Val>,
    current: bool,
    td: Option<Val>,
    r: Option<Val>,
    random: w::TrRandomKind,
}

/// The values each wave reads (a V card; an `I` card's SFFM reads 7). PWL: any even count.
pub(super) const ARITY: [(K, usize); 8] = [
    (K::Pulse, 8),
    (K::Sin, 6),
    (K::Exp, 6),
    (K::Pwl, usize::MAX),
    (K::Sffm, 8),
    (K::Am, 8),
    (K::TrNoise, 7),
    (K::TrRandom, 5),
];

/// A wave row built and not yet pushed.
enum Row {
    Pulse(w::Pulse),
    Sin(w::Sin),
    Exp(w::Exp),
    Sffm(w::Sffm),
    Am(w::Am),
    Pwl(w::Pwl),
    TrNoise(w::TrNoise),
    TrRandom(w::TrRandom),
}

/// The wave of `kind` with value tokens `toks`, `c` for its `td` and `r`.
pub(super) fn read(
    lw: &Lower<'_, '_>,
    st: Stmt,
    kind: K,
    toks: &[&str],
    c: &Card<'_>,
    notes: &mut Vec<ir::Code>,
) -> Result<Wave, Fail> {
    let current = is_current(lw, st);
    let mut arity = ARITY.iter().find(|a| a.0 == kind).expect("every kind").1;
    if kind == K::Sffm && current {
        arity = 7;
    }
    let mut toks = toks;
    if toks.len() > arity {
        notes.push(EXTRA_VALUES);
        toks = &toks[..arity];
    }
    let mut random = w::TrRandomKind::Uniform;
    if kind == K::TrRandom {
        // ngspice: `rndtype = (int) coeffs[0]` (vsrcpar.c), types 1-4
        random = match number::card(toks[0]).map(|s| s.literal.value().trunc()) {
            Some(1.0) => w::TrRandomKind::Uniform,
            Some(2.0) => w::TrRandomKind::Gauss,
            Some(3.0) => w::TrRandomKind::Exponential,
            Some(4.0) => w::TrRandomKind::Poisson,
            _ => return Err(unread(RANDOM_TYPE)),
        };
        toks = &toks[1..];
    }
    let vals = toks
        .iter()
        .map(|t| val(lw, st, t))
        .collect::<Result<Vec<_>, _>>()?;
    let (mut td, mut r) = (None, None);
    if kind == K::Pwl {
        td =
            c.td.map(|t| t.map_or(Ok(int(0)), |t| val(lw, st, t)))
                .transpose()?;
        r = repeat(lw, st, c, &vals, notes)?;
    } else if c.td.is_some() || c.r.is_some() {
        notes.push(IGNORED);
    }
    Ok(Wave {
        kind,
        vals,
        current,
        td,
        r,
        random,
    })
}

/// Appends the wave's rows.
pub(super) fn push(lw: &mut Lower<'_, '_>, st: Stmt, wave: &Wave) -> ir::WaveRef {
    // each field emitted in its row's declaration order (the IR text's), from its position
    let none = ir::ExprId::NONE;
    let mut at = |k: usize| emit_opt(lw, st, wave.vals.get(k));
    // ngspice's rule for every wave that has one (A17, R35)
    let gz = w::GivenZero::SpiceDefault;
    let row = match wave.kind {
        K::Pulse => Row::Pulse(w::Pulse {
            v1: at(0),
            v2: at(1),
            delay: at(2),
            rise: at(3),
            fall: at(4),
            width: at(5),
            period: at(6),
            cycles: at(7),
            law: w::PulseLaw::Spice,
            given_zero: gz,
        }),
        K::Sin => Row::Sin(w::Sin {
            offset: at(0),
            amplitude: at(1),
            freq: at(2),
            delay: at(3),
            damping: at(4),
            phase_deg: at(5),
            cycles: none,
            amplitude2: none,
            freq2: none,
            pre: w::PreDelay::HoldDelayValue,
            given_zero: gz,
        }),
        K::Exp => Row::Exp(w::Exp {
            v1: at(0),
            v2: at(1),
            td1: at(2),
            tau1: at(3),
            td2: at(4),
            tau2: at(5),
            td2_from: w::TdFrom::Zero,
            given_zero: gz,
        }),
        K::Sffm => {
            // an `I` card's SFFM: no delay, the 6th value the modulation phase (isrcload.c)
            let (delay, phc, phm) = if wave.current {
                (None, 6, 5)
            } else {
                (Some(5), 7, 6)
            };
            Row::Sffm(w::Sffm {
                offset: at(0),
                amplitude: at(1),
                carrier_freq: at(4),
                mod_index: at(3),
                signal_freq: at(2),
                delay: delay.map_or(none, &mut at),
                phase_c_deg: at(phc),
                phase_m_deg: at(phm),
                law: w::SffmLaw::Ngspice,
                given_zero: gz,
                pre: if wave.current {
                    w::PreDelay::HoldDelayValue
                } else {
                    w::PreDelay::Zero
                },
            })
        }
        K::Am => {
            let offset = at(0);
            let amplitude = emit(lw, st, &int(1));
            let mut at = |k: usize| emit_opt(lw, st, wave.vals.get(k));
            Row::Am(w::Am {
                offset,
                amplitude,
                mod_offset: at(1),
                mod_index: at(2),
                mod_freq: at(3),
                carrier_freq: at(4),
                delay: at(5),
                phase_m_deg: at(6),
                phase_c_deg: at(7),
                mod_time: w::AmModTime::Delay,
                given_zero: gz,
                pre: w::PreDelay::Zero,
            })
        }
        K::Pwl => {
            let v: Vec<ir::ExprId> = (0..wave.vals.len()).map(&mut at).collect();
            let points = lw.b.exprs(&v);
            let delay = emit_opt(lw, st, wave.td.as_ref());
            let repeat_at = emit_opt(lw, st, wave.r.as_ref());
            Row::Pwl(w::Pwl {
                points,
                delay,
                repeat_at,
                repeat_count: none,
                join_width: none,
                file: ir::NameId::NONE,
                repeat: if wave.r.is_some() {
                    w::PwlRepeat::From
                } else {
                    w::PwlRepeat::Never
                },
                join: w::PwlJoin::Jump,
            })
        }
        K::TrNoise => Row::TrNoise(w::TrNoise {
            na: at(0),
            nt: at(1),
            nalpha: at(2),
            namp: at(3),
            rts_amp: at(4),
            rts_capture: at(5),
            rts_emission: at(6),
        }),
        K::TrRandom => Row::TrRandom(w::TrRandom {
            ts: at(0),
            td: at(1),
            p1: at(2),
            p2: at(3),
            kind: wave.random,
            given_zero: gz,
        }),
        K::Pattern => unreachable!("ngspice has no pattern source"),
    };
    match row {
        Row::Pulse(x) => lw.b.wave(x),
        Row::Sin(x) => lw.b.wave(x),
        Row::Exp(x) => lw.b.wave(x),
        Row::Sffm(x) => lw.b.wave(x),
        Row::Am(x) => lw.b.wave(x),
        Row::Pwl(x) => lw.b.wave(x),
        Row::TrNoise(x) => lw.b.wave(x),
        Row::TrRandom(x) => lw.b.wave(x),
    }
}

// ---- private ----

/// Whether the statement is an `I` card.
fn is_current(lw: &Lower<'_, '_>, st: Stmt) -> bool {
    let head = lw.toks.text(lw.ast.main_token(st.node));
    head.as_bytes()[0].eq_ignore_ascii_case(&b'i')
}

/// A PWL's `r` (`vsrcpar.c` VSRC_R): given after the PWL, a literal below −0.5 is no repeat,
/// any other literal must be a listed time before the last (else ngspice's error), an
/// expression is `Unread` (it may mean no repeat).
fn repeat(
    lw: &Lower<'_, '_>,
    st: Stmt,
    c: &Card<'_>,
    points: &[Val],
    notes: &mut Vec<ir::Code>,
) -> Result<Option<Val>, Fail> {
    let Some((tok, after)) = c.r else {
        return Ok(None);
    };
    if !after {
        return Err(unread(PWL_REPEAT));
    }
    let r = tok.map_or(Ok(int(0)), |t| val(lw, st, t))?;
    let Val::Lit(lit, _) = r else {
        return Err(unread(PWL_REPEAT));
    };
    if lit.value() < -0.5 {
        notes.push(IGNORED);
        return Ok(None);
    }
    let times: Option<Vec<ir::Literal>> = points
        .iter()
        .step_by(2)
        .map(|p| match p {
            Val::Lit(l, _) => Some(*l),
            Val::Tree(_) | Val::Str(_) => None,
        })
        .collect();
    if let Some(times) = times {
        let last = times.last().expect("a PWL has a point");
        let listed = times.iter().any(|t| t.eq_value(lit));
        if !listed || lit.value() >= last.value() {
            return Err(invalid(SOURCE_INVALID));
        }
    }
    Ok(Some(r))
}
