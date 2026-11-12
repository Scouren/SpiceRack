//! The reference evaluator of independent sources (AMEND-A12 A30): a design, a source or a
//! waveform, an instant and `eval`'s bindings in; the source's value at that instant, or the
//! reason it has none, out. It is the oracle of every wave rule, as `eval` is of every
//! expression rule: each law's doc below is its specification, and the code adds nothing to it.
//! Writers and readers never call it, and nothing folds with it.
//!
//! A field is an expression evaluated by [`eval`] under the caller's bindings. A field that the
//! law does not reach at the instant asked is not evaluated, so a default that needs `TSTEP` or
//! `TSTOP` (`GivenZero::SpiceDefault`) is asked of the bindings only where it is used. Times
//! are seconds, frequencies hertz, phases degrees (turned into radians as `deg·π/180`), and
//! `τ = t − delay` throughout. Every comparison with a boundary is written out (`≤`, `<`), and
//! the value at a boundary is the one its law names. A result that is not finite is
//! [`WaveDomain::Undefined`], never a value.
//!
//! The laws are those of `wave.rs` and IR-CIRCUIT §3, checked against the areas' runs:
//! ngspice-45 `vsrcload.c` (PULSE `:96-162`, SINE `:164-189`, EXP `:191-219`, SFFM `:221-275`,
//! AM `:277-306`, PWL `:308-357`) with the runs of `dialect-ngspice`
//! `tests/fixtures/dialect/ngspice/read/sources/`, and VACASK 36be560 with the runs of
//! `vc-sources` `tests/fixtures/dialect/vacask/settle/src_*.sim` and `dcw.sim`
//! (`tests/wave_eval.rs` holds every value and its run).
//!
//! Cites: `rewrite/AMEND-A12.md` A30, A17, A22, C1–C4; `rewrite/RULINGS.md` R35, R36, R42;
//! `rewrite/IR-CIRCUIT.md` §3; `wave.rs`, `source.rs`; `eval.rs` (`Bindings`, `Domain`).
//!
//! Contents: `pub enum At`, `pub enum WaveDomain`,
//! `pub fn source_value(&Design, &Source, At, &dyn Bindings) -> Result<f64, WaveDomain>`,
//! `pub fn wave_value(&Design, WaveRef, f64, &dyn Bindings) -> Result<f64, WaveDomain>`

use crate::design::Design;
use crate::eval::{Bindings, Domain, eval};
use crate::source::Source;
use crate::wave::WaveRef;

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

/// The value of `source` at `at` (V or A). Pure: the same design, source, instant and
/// bindings give the same result.
///
/// - The operating point under `DcRule::Given` is `dc`; under `DcRule::WaveAtZero` it is
///   [`wave_value`] at t = 0, the wave's own pre-delay rule included (ngspice `vsrcload.c:73-86`:
///   the DC modes run the waveform at time 0; VACASK: every wave's op is its value at 0).
/// - The transient at `t` is [`wave_value`] at `t`, or `dc` when the source has no wave. `dc`
///   is never added to a deterministic wave (`DcRule::Given` with a wave is SPICE's "a DC
///   value and a waveform": the op reads one, the transient the other, run `dcw`).
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
    let _ = (design, source, at, env, eval);
    Err(WaveDomain::Undefined)
}

/// The value of `wave` at time `t` (s), by its kind's law below. Asserts `wave` is not
/// `WaveRef::NONE` and is in range.
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
    let _ = (design, wave, t, env);
    Err(WaveDomain::Undefined)
}
