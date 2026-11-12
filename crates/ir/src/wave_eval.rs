//! The reference evaluator of independent sources (AMEND-A12 A30): a design, a source or a
//! waveform, an instant and `eval`'s bindings in; the source's value at that instant, or the
//! reason it has none, out. It is the oracle of every wave rule, as `eval` is of every
//! expression rule: each law's doc on [`wave_value`] is its specification, and the code adds
//! nothing to it. Writers and readers never call it, and nothing folds with it.
//!
//! A field is an expression evaluated by [`eval`] under the caller's bindings. A field the law
//! does not reach at the instant asked is not evaluated, so a default that needs `TSTEP` or
//! `TSTOP` (`GivenZero::SpiceDefault`) is asked of the bindings only where it is used. Times
//! are seconds, frequencies hertz, phases degrees (radians are `deg·π/180`), and
//! `τ = t − delay` throughout. Every comparison with a boundary is written out (`≤`, `<`), and
//! the value at a boundary is the one its law names. A result that is not finite is
//! [`WaveDomain::Undefined`], never a value.
//!
//! The laws are those of `wave.rs` and IR-CIRCUIT §3, checked against the areas' runs:
//! ngspice-45 `vsrcload.c` (PULSE `:96-162`, SINE `:164-189`, EXP `:191-219`, SFFM `:221-275`,
//! AM `:277-306`, PWL `:308-357`) with the runs of `dialect-ngspice`
//! `tests/fixtures/dialect/ngspice/read/sources/`, and VACASK 36be560 with the runs of
//! `vc-sources` `tests/fixtures/dialect/vacask/settle/src_*.sim` and `dcw.sim`
//! (`tests/wave_eval.rs` holds every value and its run). Laws no run pins yet are marked
//! *unmeasured: specified from the document only*: their slice's runs decide them.
//!
//! Cites: `rewrite/AMEND-A12.md` A30, A17, A22, C1–C4; `rewrite/RULINGS.md` R35, R36, R42;
//! `rewrite/IR-CIRCUIT.md` §3; `wave.rs`, `source.rs`; `eval.rs` (`Bindings`, `Domain`).
//!
//! Contents: `pub enum At`, `pub enum WaveDomain`,
//! `pub fn source_value(&Design, &Source, At, &dyn Bindings) -> Result<f64, WaveDomain>`,
//! `pub fn wave_value(&Design, WaveRef, f64, &dyn Bindings) -> Result<f64, WaveDomain>`

use std::f64::consts::TAU;

use crate::design::Design;
use crate::eval::{Bindings, Domain, eval};
use crate::id::{ExprId, NameId};
use crate::source::{DcRule, Source};
use crate::wave::{
    Am, AmModTime, Exp, GivenZero, Pattern, PreDelay, Pulse, PulseLaw, Pwl, PwlJoin, PwlRepeat,
    Sffm, SffmLaw, Sin, TdFrom, TrRandom, WaveKind, WaveRef,
};

/// The instant a source is read at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum At {
    /// The operating point (`.op`, a DC sweep point): `Source::dc_rule` says which value.
    OperatingPoint,
    /// Time `t` (s) of a transient.
    Time(f64),
}

/// Why a source or a waveform has no reference value at an instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveDomain {
    /// A field the law reaches has no value: `eval`'s answer for that field.
    Field(Domain),
    /// The law reaches a value the bindings do not bind: `TSTEP` or `TSTOP` for a default of
    /// `GivenZero::SpiceDefault`, or `temp`/`tnom` for the temperature factor of
    /// `Source::tc1`/`tc2`.
    Unbound,
    /// A random wave (`TrNoise`, `TrRandom`) after t = 0: a draw, not a law.
    Random,
    /// A PWL whose points are in a file (`Pwl::file`): the evaluator reads no file.
    File,
    /// A field outside the domain its law states, a time that is not finite, or a result
    /// that is not finite.
    Undefined,
}

/// How a field's expression is read: the design, the bindings, the wave's zero rule.
struct Fields<'a> {
    design: &'a Design,
    env: &'a dyn Bindings,
    zero: GivenZero,
}

/// A pair of a PWL's evaluated points.
type Point = (f64, f64);

/// The value of `source` at `at` (V or A). Pure: the same design, source, instant and
/// bindings give the same result.
///
/// - The operating point under `DcRule::Given` is `dc`; under `DcRule::WaveAtZero` it is
///   [`wave_value`] at t = 0, the wave's own pre-delay rule included (ngspice `vsrcload.c:73-86`:
///   the DC modes run the waveform at time 0; VACASK: every wave's op is its value at 0).
/// - The transient at `t` is [`wave_value`] at `t`, or `dc` when the source has no wave. `dc`
///   is never added to a wave (`DcRule::Given` with a wave is SPICE's "a DC value and a
///   waveform": the op reads one, the transient the other, run `dcw`).
/// - Every slot is multiplied by `1 + tc1·ΔT + tc2·ΔT²`, ΔT = `temp − tnom`, when `tc1` or
///   `tc2` is set (`Source::tc1`); a `NONE` coefficient is 0.
///
/// # Errors
///
/// [`WaveDomain`]: `Undefined` for a `Given` source without `dc` or a non-finite `t`; the
/// wave's own answer otherwise.
pub fn source_value(
    design: &Design,
    source: &Source,
    at: At,
    env: &dyn Bindings,
) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, GivenZero::AsGiven);
    let value = match (at, source.dc_rule) {
        (At::OperatingPoint, DcRule::Given) => f.get(source.dc)?,
        (At::OperatingPoint, DcRule::WaveAtZero) => wave_value(design, source.wave, 0.0, env)?,
        (At::Time(_), _) if source.wave.is_none() => f.get(source.dc)?,
        (At::Time(t), _) => wave_value(design, source.wave, t, env)?,
    };
    let (tc1, tc2) = (f.opt(source.tc1)?, f.opt(source.tc2)?);
    let factor = if tc1.is_none() && tc2.is_none() {
        1.0
    } else {
        let dt = env.temp().ok_or(WaveDomain::Unbound)? - env.tnom().ok_or(WaveDomain::Unbound)?;
        1.0 + tc1.unwrap_or(0.0) * dt + tc2.unwrap_or(0.0) * dt * dt
    };
    finite(value * factor)
}

/// The value of `wave` at time `t` (s), by its kind's law. Asserts `wave` is not
/// `WaveRef::NONE` and is in range.
///
/// **Defaults.** Under `GivenZero::AsGiven` every field means what it holds, and a required
/// field that is `NONE` is `Undefined`. Under `GivenZero::SpiceDefault` (IR-CIRCUIT §3's
/// table) a field the card omitted is `NONE` and means its default; a *both* field also means
/// its default when it evaluates to 0. Each law below lists its defaults.
///
/// **Pulse** (`Pulse`). Defaults: `delay` omitted 0; `rise`, `fall` both TSTEP; `width`,
/// `period` both TSTOP. τ = t − delay, and the value is v1 for τ ≤ 0. After that:
/// - `PulseLaw::Spice` (`vsrcload.c:133-160`): a `cycles` value c > 0 makes the value v1 for
///   τ > c·period. Else, for τ > period, τ becomes τ − period·⌊τ/period⌋. Then the value is v1
///   for τ ≤ 0 or τ ≥ rise + width + fall (tested first, so an ideal fall is v1 at
///   τ = rise + width); v2 for rise ≤ τ ≤ rise + width; v1 + (v2 − v1)·τ/rise for τ < rise; else
///   v2 + (v1 − v2)·(τ − rise − width)/fall. A period ≤ 0 is `Undefined`.
/// - `PulseLaw::Vacask` (VD dev-builtin-src; runs p1, src-pulse): x = τ, and for period > 0 x
///   becomes x mod period (`fmod`); `cycles` as above. Then v1 + (v2 − v1)·x/rise for x < rise;
///   v2 for fall ≤ 0 (no falling edge) or x < rise + width; v2 + (v1 − v2)·(x − rise − width)/fall
///   for x < rise + width + fall; else v1. Before the delay this is the value at x = 0, v1.
///
/// **Sine** (`Sin`). Defaults: `freq` both 1/TSTOP; `delay`, `damping`, `phase_deg` omitted 0.
/// τ = t − delay. For τ ≤ 0 the value is `pre`'s: `Offset` offset, `HoldDelayValue`
/// offset + amplitude·sin(phase) (`vsrcload.c:180-181`), `Zero` 0. For τ > 0, a `cycles` value n
/// makes τ = n/freq for τ > n/freq (the value holds; lt, *unmeasured: specified from the document only*), and the value is
/// offset + amplitude·sin(freq·τ·2π + phase)·e^(−τ·damping) + amplitude2·sin(2π·freq2·τ), the
/// last term only with a second tone.
///
/// **Exponential** (`Exp`). Defaults: `td1`, `tau1`, `tau2` both TSTEP; `td2` both: the second
/// edge TSTEP after the first. The second edge is at T2 = td2 (`TdFrom::Zero`) or
/// td1 + td2 (`TdFrom::Td1`) from t = 0. The value is v1 for t ≤ td1;
/// v1 + (v2 − v1)·(1 − e^(−(t − td1)/tau1)) for t ≤ T2; that plus
/// (v1 − v2)·(1 − e^(−(t − T2)/tau2)) after (`vsrcload.c:209-215`).
///
/// **PWL** (`Pwl`). The points (t₀, v₀) … (t_N, v_N), every field evaluated; times must not
/// decrease (`Undefined`); no points is `File` with a file, else `Undefined`. The plain wave
/// p(u) is v₀ for u ≤ t₀, v_N for u > t_N, and else, for the first point i ≥ 1 with t_i ≥ u,
/// v_{i−1} + (u − t_{i−1})/(t_i − t_{i−1})·(v_i − v_{i−1}) (`vsrcload.c:345-354`: at a listed
/// time the segment that ends there, so a time listed twice takes its first value). τ = t − delay.
/// - `PwlRepeat::Never`: p(τ).
/// - `PwlRepeat::From` (ng, hs, xy, lt): r = `repeat_at`, P = t_N − r, P ≤ 0 `Undefined`. For
///   τ ≤ t_N the value is p(τ). After, the pass m = ⌈(τ − t_N)/P⌉ (1 the first repeat); with a
///   `repeat_count` n and m > n the value is v_N; else u = r + ((τ − r) − P·⌊(τ − r)/P⌋)
///   (`vsrcload.c:335-340`) and the value is by `join`: `Jump` p(u); `Ramp` (lt, *unmeasured: specified from the document only*)
///   the line from (r, v_N) to (t₀, v₀) for u < t₀, else p(u). `RampWidth` and `Extrapolate`
///   are `Undefined` here (no simulator states them; `check` refuses the pair).
/// - `PwlRepeat::Period` (va, sp): P = `repeat_at`, P ≤ 0 `Undefined`; τ below 0 is 0 (VACASK
///   run src-pwl: the op and the value before the delay are the value at τ = 0); k = ⌊τ/P⌋, and
///   with a `repeat_count` n and k > n the value is v_N; else u = τ mod P (`fmod`) and the value
///   is by `join` (the evaluator takes `fmod`; x − P·⌊x/P⌋ agrees with every run to 1e-12, so the
///   runs do not tell the two apart): `Jump` p(u); `Extrapolate` (A22) the line through (t_N, v_N) and
///   (P + t₀, v₀) at u for u ≥ t_N or u < t₀, else p(u); `RampWidth` (sp, *unmeasured: specified from the document only*), w =
///   `join_width`: the line from (P − w, p(P − w)) to (P, p(0)) for u ≥ P − w, else p(u). `Ramp`
///   is `Undefined` here (no simulator states it; `check` refuses the pair).
///
/// **FM** (`Sffm`). Defaults: `signal_freq` omitted 5/TSTOP; `mod_index` omitted 90;
/// `carrier_freq` both 500/TSTOP; `delay`, `phase_c_deg`, `phase_m_deg` omitted 0. Under
/// `SffmLaw::Ngspice` an index above carrier/signal is carrier/signal, one below 0 is 0
/// (`vsrcload.c:256-266`); `Spice3` keeps it. τ = t − delay; for τ ≤ 0 the value is `pre`'s
/// (`Offset` offset, `HoldDelayValue` the formula at τ = 0, `Zero` 0); for τ > 0 it is
/// offset + amplitude·sin((2π·carrier·τ + phase_c) + index·sin(2π·signal·τ + phase_m)).
///
/// **AM** (`Am`). Defaults: `mod_index` omitted 1; `mod_freq` omitted 5/TSTOP; `carrier_freq`
/// omitted 500/TSTOP; `delay`, `phase_m_deg`, `phase_c_deg` omitted 0. τ = t − delay; for τ ≤ 0
/// the value is `pre`'s (as FM); for τ > 0, with τm = τ (`AmModTime::Delay`) or
/// τ + phase_c/(360·carrier) (`CarrierPhase`), it is offset + amplitude·(mod_offset +
/// mod_index·sin(2π·mod_freq·τm + phase_m))·sin(2π·carrier·τ + phase_c).
///
/// **Bit pattern** (`Pattern`, hs/xy PAT, *unmeasured: specified from the document only*). Each bit is a level: `1` high, `0`
/// low, `m` (high + low)/2. The sequence is the bits, then `repeats` passes (`NONE` forever) of
/// the bits from `repeat_start` (1-based); bit k holds [k·sample, (k + 1)·sample) of
/// τ = t − delay, starting with a line from the previous bit's level over `rise` (going up)
/// or `fall` (going down), then its own level. Before τ = 0 the first bit's level; after the
/// sequence its last level. No bits, another character, or a sample ≤ 0 is `Undefined`.
///
/// **Random waves** (`TrNoise`, `TrRandom`, A17). For t ≤ 0 a `TrNoise` is 0 (`vsrcload.c`:
/// `time == 0` is 0) and a `TrRandom` its offset `p2` (omitted under `SpiceDefault`: 0;
/// `1-f-code.c:239`); after, `Random`.
///
/// # Errors
///
/// [`WaveDomain`], as each law states.
pub fn wave_value(
    design: &Design,
    wave: WaveRef,
    t: f64,
    env: &dyn Bindings,
) -> Result<f64, WaveDomain> {
    assert!(!wave.is_none(), "a waveform");
    if !t.is_finite() {
        return Err(WaveDomain::Undefined);
    }
    let value = match wave.kind {
        WaveKind::Pulse => pulse(design, design.detail(wave.index), t, env),
        WaveKind::Sin => sine(design, design.detail(wave.index), t, env),
        WaveKind::Exp => exponential(design, design.detail(wave.index), t, env),
        WaveKind::Pwl => pwl(design, design.detail(wave.index), t, env),
        WaveKind::Sffm => fm(design, design.detail(wave.index), t, env),
        WaveKind::Am => am(design, design.detail(wave.index), t, env),
        WaveKind::TrNoise if t <= 0.0 => Ok(0.0),
        WaveKind::TrRandom if t <= 0.0 => {
            let r: TrRandom = design.detail(wave.index);
            Fields::new(design, env, r.given_zero).omit(r.p2, |_| Ok(0.0))
        }
        WaveKind::TrNoise | WaveKind::TrRandom => Err(WaveDomain::Random),
        WaveKind::Pattern => pattern(design, design.detail(wave.index), t, env),
    }?;
    finite(value)
}

// ---- private ----

impl<'a> Fields<'a> {
    fn new(design: &'a Design, env: &'a dyn Bindings, zero: GivenZero) -> Fields<'a> {
        Fields { design, env, zero }
    }

    /// A required field: `NONE` is `Undefined`.
    fn get(&self, e: ExprId) -> Result<f64, WaveDomain> {
        self.opt(e)?.ok_or(WaveDomain::Undefined)
    }

    /// A field that may be `NONE`.
    fn opt(&self, e: ExprId) -> Result<Option<f64>, WaveDomain> {
        if e == ExprId::NONE {
            return Ok(None);
        }
        eval(self.design, e, self.env)
            .map(Some)
            .map_err(WaveDomain::Field)
    }

    /// An *omit* field: under `SpiceDefault` `NONE` is `default`; under `AsGiven` required.
    fn omit(
        &self,
        e: ExprId,
        default: impl FnOnce(&Self) -> Result<f64, WaveDomain>,
    ) -> Result<f64, WaveDomain> {
        match (self.opt(e)?, self.zero) {
            (Some(v), _) => Ok(v),
            (None, GivenZero::SpiceDefault) => default(self),
            (None, GivenZero::AsGiven) => Err(WaveDomain::Undefined),
        }
    }

    /// A *both* field: under `SpiceDefault` `NONE` or 0 is `default`.
    fn both(
        &self,
        e: ExprId,
        default: impl FnOnce(&Self) -> Result<f64, WaveDomain>,
    ) -> Result<f64, WaveDomain> {
        match (self.opt(e)?, self.zero) {
            (Some(v), GivenZero::SpiceDefault) => {
                if v == 0.0 {
                    default(self)
                } else {
                    Ok(v)
                }
            }
            (Some(v), GivenZero::AsGiven) => Ok(v),
            (None, GivenZero::SpiceDefault) => default(self),
            (None, GivenZero::AsGiven) => Err(WaveDomain::Undefined),
        }
    }

    fn tstep(&self) -> Result<f64, WaveDomain> {
        self.env.tstep().ok_or(WaveDomain::Unbound)
    }

    fn tstop(&self) -> Result<f64, WaveDomain> {
        self.env.tstop().ok_or(WaveDomain::Unbound)
    }
}

/// `x`, or `Undefined` when it is not finite.
fn finite(x: f64) -> Result<f64, WaveDomain> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(WaveDomain::Undefined)
    }
}

/// Degrees in radians, as the simulators write it (`deg·π/180`).
fn rad(deg: f64) -> f64 {
    deg * std::f64::consts::PI / 180.0
}

fn pulse(design: &Design, p: Pulse, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, p.given_zero);
    let v1 = f.get(p.v1)?;
    let tau = t - f.omit(p.delay, |_| Ok(0.0))?;
    if tau <= 0.0 {
        return Ok(v1);
    }
    let v2 = f.get(p.v2)?;
    let rise = f.both(p.rise, Fields::tstep)?;
    let fall = f.both(p.fall, Fields::tstep)?;
    let width = f.both(p.width, Fields::tstop)?;
    let period = f.both(p.period, Fields::tstop)?;
    if let Some(c) = f.opt(p.cycles)?
        && c > 0.0
        && tau > c * period
    {
        return Ok(v1);
    }
    match p.law {
        PulseLaw::Spice => {
            if period <= 0.0 {
                return Err(WaveDomain::Undefined);
            }
            let x = if tau > period {
                tau - period * (tau / period).floor()
            } else {
                tau
            };
            Ok(if x <= 0.0 || x >= rise + width + fall {
                v1
            } else if x >= rise && x <= rise + width {
                v2
            } else if x < rise {
                v1 + (v2 - v1) * x / rise
            } else {
                v2 + (v1 - v2) * (x - (rise + width)) / fall
            })
        }
        PulseLaw::Vacask => {
            let x = if period > 0.0 { tau % period } else { tau };
            Ok(if x < rise {
                v1 + (v2 - v1) * x / rise
            } else if fall <= 0.0 || x < rise + width {
                v2
            } else if x < rise + width + fall {
                v2 + (v1 - v2) * (x - rise - width) / fall
            } else {
                v1
            })
        }
    }
}

fn sine(design: &Design, s: Sin, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, s.given_zero);
    let offset = f.get(s.offset)?;
    let amplitude = f.get(s.amplitude)?;
    let mut tau = t - f.omit(s.delay, |_| Ok(0.0))?;
    let phase = rad(f.omit(s.phase_deg, |_| Ok(0.0))?);
    if tau <= 0.0 {
        return Ok(match s.pre {
            PreDelay::Offset => offset,
            PreDelay::HoldDelayValue => offset + amplitude * phase.sin(),
            PreDelay::Zero => 0.0,
        });
    }
    let freq = f.both(s.freq, |f| Ok(1.0 / f.tstop()?))?;
    let damping = f.omit(s.damping, |_| Ok(0.0))?;
    if let Some(n) = f.opt(s.cycles)?
        && tau > n / freq
    {
        tau = n / freq;
    }
    let mut value = offset
        + amplitude
            * (freq * tau * 2.0 * std::f64::consts::PI + phase).sin()
            * (-tau * damping).exp();
    if let Some(a2) = f.opt(s.amplitude2)? {
        value += a2 * (TAU * f.get(s.freq2)? * tau).sin();
    }
    Ok(value)
}

fn exponential(design: &Design, e: Exp, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, e.given_zero);
    let v1 = f.get(e.v1)?;
    let td1 = f.both(e.td1, Fields::tstep)?;
    if t <= td1 {
        return Ok(v1);
    }
    let v2 = f.get(e.v2)?;
    let tau1 = f.both(e.tau1, Fields::tstep)?;
    let given = match (f.opt(e.td2)?, e.given_zero) {
        (Some(x), GivenZero::AsGiven) => Some(x),
        (Some(x), GivenZero::SpiceDefault) if x != 0.0 => Some(x),
        (None, GivenZero::AsGiven) => return Err(WaveDomain::Undefined),
        (Some(_) | None, GivenZero::SpiceDefault) => None,
    };
    let edge2 = match (given, e.td2_from) {
        (None, _) => td1 + f.tstep()?,
        (Some(x), TdFrom::Zero) => x,
        (Some(x), TdFrom::Td1) => td1 + x,
    };
    let rise = v1 + (v2 - v1) * (1.0 - (-(t - td1) / tau1).exp());
    if t <= edge2 {
        return Ok(rise);
    }
    let tau2 = f.both(e.tau2, Fields::tstep)?;
    Ok(rise + (v1 - v2) * (1.0 - (-(t - edge2) / tau2).exp()))
}

fn pwl(design: &Design, w: Pwl, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, GivenZero::AsGiven);
    let exprs = design.exprs(w.points);
    if exprs.is_empty() {
        return Err(if w.file == NameId::NONE {
            WaveDomain::Undefined
        } else {
            WaveDomain::File
        });
    }
    if !exprs.len().is_multiple_of(2) {
        return Err(WaveDomain::Undefined);
    }
    let mut points: Vec<Point> = Vec::with_capacity(exprs.len() / 2);
    for pair in exprs.chunks_exact(2) {
        points.push((f.get(pair[0])?, f.get(pair[1])?));
    }
    if points.windows(2).any(|p| p[1].0 < p[0].0) {
        return Err(WaveDomain::Undefined);
    }
    let (first, last) = (points[0], points[points.len() - 1]);
    let tau = t - f.opt(w.delay)?.unwrap_or(0.0);
    let count = f.opt(w.repeat_count)?;
    match (w.repeat, w.join) {
        (PwlRepeat::Never, _) => Ok(plain(&points, tau)),
        (PwlRepeat::From, PwlJoin::Jump | PwlJoin::Ramp) => {
            let r = f.get(w.repeat_at)?;
            let period = last.0 - r;
            if period <= 0.0 {
                return Err(WaveDomain::Undefined);
            }
            if tau <= last.0 {
                return Ok(plain(&points, tau));
            }
            if count.is_some_and(|n| ((tau - last.0) / period).ceil() > n) {
                return Ok(last.1);
            }
            let u = r + ((tau - r) - period * ((tau - r) / period).floor());
            Ok(if w.join == PwlJoin::Ramp && u < first.0 {
                line((r, last.1), first, u)
            } else {
                plain(&points, u)
            })
        }
        (PwlRepeat::Period, PwlJoin::Jump | PwlJoin::Extrapolate | PwlJoin::RampWidth) => {
            let period = f.get(w.repeat_at)?;
            if period <= 0.0 {
                return Err(WaveDomain::Undefined);
            }
            let tau = tau.max(0.0);
            if count.is_some_and(|n| (tau / period).floor() > n) {
                return Ok(last.1);
            }
            let u = tau % period;
            Ok(match w.join {
                PwlJoin::Extrapolate if u >= last.0 || u < first.0 => {
                    line(last, (period + first.0, first.1), u)
                }
                PwlJoin::RampWidth => {
                    let start = period - f.get(w.join_width)?;
                    if u >= start {
                        line(
                            (start, plain(&points, start)),
                            (period, plain(&points, 0.0)),
                            u,
                        )
                    } else {
                        plain(&points, u)
                    }
                }
                PwlJoin::Jump | PwlJoin::Extrapolate | PwlJoin::Ramp => plain(&points, u),
            })
        }
        (PwlRepeat::From, PwlJoin::RampWidth | PwlJoin::Extrapolate)
        | (PwlRepeat::Period, PwlJoin::Ramp) => Err(WaveDomain::Undefined),
    }
}

/// The plain PWL `p(u)` of the law (non-empty, non-decreasing points).
fn plain(points: &[Point], u: f64) -> f64 {
    let (first, last) = (points[0], points[points.len() - 1]);
    if u <= first.0 {
        return first.1;
    }
    if u > last.0 {
        return last.1;
    }
    // bounded: one pass over the points; `u <= last.0` makes the search succeed
    let i = (1..points.len())
        .find(|&i| points[i].0 >= u)
        .expect("a point at or after u");
    let (a, b) = (points[i - 1], points[i]);
    a.1 + (u - a.0) / (b.0 - a.0) * (b.1 - a.1)
}

/// The line through `a` and `b` at `u`.
fn line(a: Point, b: Point, u: f64) -> f64 {
    a.1 + (b.1 - a.1) * (u - a.0) / (b.0 - a.0)
}

fn fm(design: &Design, s: Sffm, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, s.given_zero);
    let offset = f.get(s.offset)?;
    let tau = t - f.omit(s.delay, |_| Ok(0.0))?;
    if tau <= 0.0 && s.pre != PreDelay::HoldDelayValue {
        return Ok(match s.pre {
            PreDelay::Offset => offset,
            PreDelay::HoldDelayValue | PreDelay::Zero => 0.0,
        });
    }
    let tau = tau.max(0.0);
    let amplitude = f.get(s.amplitude)?;
    let carrier = f.both(s.carrier_freq, |f| Ok(500.0 / f.tstop()?))?;
    let signal = f.omit(s.signal_freq, |f| Ok(5.0 / f.tstop()?))?;
    let mut index = f.omit(s.mod_index, |_| Ok(90.0))?;
    if s.law == SffmLaw::Ngspice {
        if index > carrier / signal {
            index = carrier / signal;
        } else if index < 0.0 {
            index = 0.0;
        }
    }
    let phase_c = rad(f.omit(s.phase_c_deg, |_| Ok(0.0))?);
    let phase_m = rad(f.omit(s.phase_m_deg, |_| Ok(0.0))?);
    Ok(offset
        + amplitude
            * ((2.0 * std::f64::consts::PI * carrier * tau + phase_c)
                + index * (2.0 * std::f64::consts::PI * signal * tau + phase_m).sin())
            .sin())
}

fn am(design: &Design, a: Am, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, a.given_zero);
    let offset = f.get(a.offset)?;
    let tau = t - f.omit(a.delay, |_| Ok(0.0))?;
    if tau <= 0.0 && a.pre != PreDelay::HoldDelayValue {
        return Ok(match a.pre {
            PreDelay::Offset => offset,
            PreDelay::HoldDelayValue | PreDelay::Zero => 0.0,
        });
    }
    let tau = tau.max(0.0);
    let amplitude = f.get(a.amplitude)?;
    let mod_offset = f.get(a.mod_offset)?;
    let index = f.omit(a.mod_index, |_| Ok(1.0))?;
    let mod_freq = f.omit(a.mod_freq, |f| Ok(5.0 / f.tstop()?))?;
    let carrier = f.omit(a.carrier_freq, |f| Ok(500.0 / f.tstop()?))?;
    let phase_m = rad(f.omit(a.phase_m_deg, |_| Ok(0.0))?);
    let phase_c_deg = f.omit(a.phase_c_deg, |_| Ok(0.0))?;
    let tau_m = match a.mod_time {
        AmModTime::Delay => tau,
        AmModTime::CarrierPhase => tau + phase_c_deg / (360.0 * carrier),
    };
    let pi2 = 2.0 * std::f64::consts::PI;
    Ok(offset
        + amplitude
            * (mod_offset + index * (pi2 * mod_freq * tau_m + phase_m).sin())
            * (pi2 * carrier * tau + rad(phase_c_deg)).sin())
}

fn pattern(design: &Design, p: Pattern, t: f64, env: &dyn Bindings) -> Result<f64, WaveDomain> {
    let f = Fields::new(design, env, GivenZero::AsGiven);
    let (high, low) = (f.get(p.high)?, f.get(p.low)?);
    let level = |c: u8| match c {
        b'1' => Ok(high),
        b'0' => Ok(low),
        b'm' | b'M' => Ok((high + low) / 2.0),
        _ => Err(WaveDomain::Undefined),
    };
    let bits = if p.bits == NameId::NONE {
        &[][..]
    } else {
        design.str(p.bits).as_bytes()
    };
    if bits.is_empty() {
        return Err(WaveDomain::Undefined);
    }
    let tau = t - f.get(p.delay)?;
    if tau < 0.0 {
        return level(bits[0]);
    }
    let sample = f.get(p.sample)?;
    if sample <= 0.0 {
        return Err(WaveDomain::Undefined);
    }
    let start = (f.opt(p.repeat_start)?.unwrap_or(1.0) as usize).clamp(1, bits.len()) - 1;
    let tail = bits.len() - start;
    // the k-th bit of the sequence: the bits, then `repeats` passes of bits[start..]
    let total = f
        .opt(p.repeats)?
        .map(|r| bits.len() as f64 + r.max(0.0).floor() * tail as f64);
    let bit_at = |k: f64| -> u8 {
        let k = match total {
            Some(n) if k >= n => n - 1.0,
            _ => k,
        };
        if k < bits.len() as f64 {
            bits[k as usize]
        } else {
            bits[start + ((k - bits.len() as f64) % tail as f64) as usize]
        }
    };
    let k = (tau / sample).floor();
    let into = tau - k * sample;
    let here = level(bit_at(k))?;
    let before = if k == 0.0 {
        here
    } else {
        level(bit_at(k - 1.0))?
    };
    let edge = if here > before {
        f.get(p.rise)?
    } else {
        f.get(p.fall)?
    };
    let after_end = total.is_some_and(|n| k >= n);
    Ok(if !after_end && into < edge && here != before {
        before + (here - before) * into / edge
    } else {
        here
    })
}
