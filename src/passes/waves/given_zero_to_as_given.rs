//! Pass `given_zero_to_as_given`: a design in, the same design out with every wave under
//! ngspice's zero rule (`GivenZero::SpiceDefault`) written under `AsGiven`, each default made
//! explicit as IR-CIRCUIT §3's table states it (`wave_eval`'s defaults, A30): an omitted
//! (`NONE`) field becomes its default; a *both* field also when it is the literal zero, and
//! when it is any other expression `x` it becomes `x == 0 ? default : x`. The defaults that
//! need the transient are the leaves `TStep` and `TStop` (`TSTOP`'s fractions `1/TStop`,
//! `5/TStop`, `500/TStop`), which `substitute_tstep` / `substitute_tstop` then replace; the
//! exponential's `td2` defaults to the new `td1` plus `TStep`. A both field that may hold a
//! draw (it would be read twice) leaves the wave under ngspice's rule with note `U0202`.
//!
//! Removes `Circuit(GivenZeroSpiceDefault)`; requires `Circuit(GivenZeroAsGiven)`,
//! `Expr(TStep)`, `Expr(TStop)`, `Expr(Ternary)`, `Binary(Eq)`, `Binary(Div)`, `Binary(Add)`,
//! `Expr(Binary)`, `Expr(Num)`, and `Eq`, `Div`, `Add` in both forms (a field of class `Temp` counts the dynamic one). One row: the zero rule has no form.
//!
//! Cites: `rewrite/AMEND-A12.md` A17 (R35), A30; `rewrite/IR-CIRCUIT.md` §3 (the zero rule's
//! table, `vsrcload.c` lines).
//!
//! Contents: `pub fn given_zero_to_as_given(&mut ir::Design, &mut ir::Diags)`

use super::GIVEN_ZERO_LEFT;
use super::nodes::{literal, may_draw, note_left, sources};
use crate::ir;
use crate::ir::wave::{Am, Exp, GivenZero, Pulse, Sffm, Sin, TrRandom};

/// A default of IR-CIRCUIT §3's table.
#[derive(Clone, Copy)]
enum Default {
    /// A whole number.
    Num(i64),
    /// TSTEP.
    TStep,
    /// TSTOP.
    TStop,
    /// `n / TSTOP`.
    Per(i64),
    /// The given expression plus TSTEP (the exponential's `td2`).
    AfterTStep(ir::ExprId),
}

/// Writes every `SpiceDefault` wave's defaults out and marks it `AsGiven`; notes a wave left.
pub fn given_zero_to_as_given(design: &mut ir::Design, diags: &mut ir::Diags) {
    let waves: Vec<ir::WaveRef> = sources(design)
        .into_iter()
        .map(|(_, s)| s.wave)
        .filter(|w| !w.is_none())
        .collect();
    for w in waves {
        match w.kind {
            ir::WaveKind::Pulse => convert::<Pulse>(design, w.index, pulse),
            ir::WaveKind::Sin => convert::<Sin>(design, w.index, sin),
            ir::WaveKind::Exp => convert::<Exp>(design, w.index, exp),
            ir::WaveKind::Sffm => convert::<Sffm>(design, w.index, sffm),
            ir::WaveKind::Am => convert::<Am>(design, w.index, am),
            ir::WaveKind::TrRandom => convert::<TrRandom>(design, w.index, trrandom),
            ir::WaveKind::Pwl | ir::WaveKind::TrNoise | ir::WaveKind::Pattern => {}
        }
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::GivenZeroSpiceDefault);
    note_left(design, diags, feature, GIVEN_ZERO_LEFT);
}

// ---- private ----

/// Reads the row at `index`, builds its `AsGiven` twin with `rule`, and stores it; a row
/// `rule` declines (already `AsGiven`, or a both field that may draw) stays.
fn convert<D: ir::DetailRow + Copy>(
    design: &mut ir::Design,
    index: u32,
    rule: fn(&mut ir::Builder<'_>, D) -> Option<D>,
) {
    let row: D = design.detail(index);
    let mut editor = design.editor();
    let new = rule(&mut editor.builder(), row);
    if let Some(new) = new {
        editor.set_detail(index, new);
    }
}

fn pulse(b: &mut ir::Builder<'_>, p: Pulse) -> Option<Pulse> {
    (p.given_zero == GivenZero::SpiceDefault).then_some(())?;
    Some(Pulse {
        rise: both(b, p.rise, Default::TStep)?,
        fall: both(b, p.fall, Default::TStep)?,
        width: both(b, p.width, Default::TStop)?,
        period: both(b, p.period, Default::TStop)?,
        delay: omit(b, p.delay, Default::Num(0)),
        given_zero: GivenZero::AsGiven,
        ..p
    })
}

fn sin(b: &mut ir::Builder<'_>, s: Sin) -> Option<Sin> {
    (s.given_zero == GivenZero::SpiceDefault).then_some(())?;
    Some(Sin {
        freq: both(b, s.freq, Default::Per(1))?,
        delay: omit(b, s.delay, Default::Num(0)),
        damping: omit(b, s.damping, Default::Num(0)),
        phase_deg: omit(b, s.phase_deg, Default::Num(0)),
        given_zero: GivenZero::AsGiven,
        ..s
    })
}

fn exp(b: &mut ir::Builder<'_>, x: Exp) -> Option<Exp> {
    (x.given_zero == GivenZero::SpiceDefault).then_some(())?;
    let td1 = both(b, x.td1, Default::TStep)?;
    Some(Exp {
        td1,
        tau1: both(b, x.tau1, Default::TStep)?,
        td2: both(b, x.td2, Default::AfterTStep(td1))?,
        tau2: both(b, x.tau2, Default::TStep)?,
        given_zero: GivenZero::AsGiven,
        ..x
    })
}

fn sffm(b: &mut ir::Builder<'_>, f: Sffm) -> Option<Sffm> {
    (f.given_zero == GivenZero::SpiceDefault).then_some(())?;
    Some(Sffm {
        carrier_freq: both(b, f.carrier_freq, Default::Per(500))?,
        signal_freq: omit(b, f.signal_freq, Default::Per(5)),
        mod_index: omit(b, f.mod_index, Default::Num(90)),
        delay: omit(b, f.delay, Default::Num(0)),
        phase_c_deg: omit(b, f.phase_c_deg, Default::Num(0)),
        phase_m_deg: omit(b, f.phase_m_deg, Default::Num(0)),
        given_zero: GivenZero::AsGiven,
        ..f
    })
}

fn am(b: &mut ir::Builder<'_>, a: Am) -> Option<Am> {
    (a.given_zero == GivenZero::SpiceDefault).then_some(())?;
    Some(Am {
        mod_index: omit(b, a.mod_index, Default::Num(1)),
        mod_freq: omit(b, a.mod_freq, Default::Per(5)),
        carrier_freq: omit(b, a.carrier_freq, Default::Per(500)),
        delay: omit(b, a.delay, Default::Num(0)),
        phase_m_deg: omit(b, a.phase_m_deg, Default::Num(0)),
        phase_c_deg: omit(b, a.phase_c_deg, Default::Num(0)),
        given_zero: GivenZero::AsGiven,
        ..a
    })
}

fn trrandom(b: &mut ir::Builder<'_>, r: TrRandom) -> Option<TrRandom> {
    (r.given_zero == GivenZero::SpiceDefault).then_some(())?;
    Some(TrRandom {
        td: omit(b, r.td, Default::Num(0)),
        p1: omit(b, r.p1, Default::Num(1)),
        p2: omit(b, r.p2, Default::Num(0)),
        given_zero: GivenZero::AsGiven,
        ..r
    })
}

/// The default's expression.
fn build(b: &mut ir::Builder<'_>, d: Default) -> ir::ExprId {
    let int = |b: &mut ir::Builder<'_>, n: i64| {
        b.literal(ir::Literal {
            mantissa: n,
            frac: 0,
            exp10: 0,
            form: ir::NumForm::Plain,
        })
    };
    match d {
        Default::Num(n) => int(b, n),
        Default::TStep => b.tstep(),
        Default::TStop => b.tstop(),
        Default::Per(n) => {
            let (k, t) = (int(b, n), b.tstop());
            b.binary(ir::BinOp::Div, k, t)
        }
        Default::AfterTStep(e) => {
            let s = b.tstep();
            b.binary(ir::BinOp::Add, e, s)
        }
    }
}

/// An *omit* field: its default when `NONE`, else itself (a given 0 stays 0).
fn omit(b: &mut ir::Builder<'_>, x: ir::ExprId, d: Default) -> ir::ExprId {
    match x.is_none() {
        true => build(b, d),
        false => x,
    }
}

/// A *both* field: its default when `NONE` or the literal zero, itself when another literal,
/// else `x == 0 ? default : x`; `None` when `x` may hold a draw (read twice).
fn both(b: &mut ir::Builder<'_>, x: ir::ExprId, d: Default) -> Option<ir::ExprId> {
    if x.is_none() {
        return Some(build(b, d));
    }
    match literal(b.design(), x) {
        Some(0.0) => return Some(build(b, d)),
        Some(_) => return Some(x),
        None => {}
    }
    if may_draw(b.design(), x) {
        return None;
    }
    let zero = b.literal(ir::Literal::ZERO);
    let is_zero = b.binary(ir::BinOp::Eq, x, zero);
    let default = build(b, d);
    Some(b.ternary(is_zero, default, x))
}
