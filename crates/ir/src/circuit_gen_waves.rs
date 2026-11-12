//! The waveform builders of the circuit generator (`circuit_gen.rs`'s child module): which
//! wave kind a source may carry under the set (and under some DC rule, A12 C4), and each wave
//! row built field by field in declaration order, laws and pre-delay rules only from the set.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator, round trip); `rewrite/AMEND-A12.md` C1–C4;
//! `circuit_gen.rs`.
//!
//! Contents: `R::wave_kind`, `R::wave_of` (`pub(super)`)

use super::{N, R};
use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_features::dc_at_zero;
use crate::expr::{Literal, NumForm};
use crate::feature::Feature;
use crate::id::{ExprId, NameId};
use crate::wave::{
    Am, AmModTime, Exp, GivenZero, Pattern, PreDelay, Pulse, PulseLaw, Pwl, PwlJoin, PwlRepeat,
    Sffm, SffmLaw, Sin, TdFrom, TrNoise, TrRandom, TrRandomKind, WaveKind, WaveRef,
};

impl R<'_, '_> {
    /// A waveform kind this set can build under some DC rule, or none.
    pub(super) fn wave_kind(&mut self) -> Option<WaveKind> {
        let kinds: Vec<WaveKind> = WaveKind::ALL
            .into_iter()
            .filter(|&k| {
                self.f.contains(Feature::Wave(k))
                    && match k {
                        WaveKind::Sin => {
                            self.has(X::SinPreOffset)
                                || self.has(X::SinPreHold)
                                || self.has(X::SinPreZero)
                        }
                        WaveKind::Pulse => self.has(X::PulseSpice) || self.has(X::PulseVacask),
                        WaveKind::Exp => self.has(X::ExpTd2FromZero) || self.has(X::ExpTd2FromTd1),
                        WaveKind::Sffm => {
                            (self.has(X::SffmSpice3) || self.has(X::SffmNgspice))
                                && (self.has(X::SffmPreOffset)
                                    || self.has(X::SffmPreHold)
                                    || self.has(X::SffmPreZero))
                        }
                        WaveKind::Am => {
                            self.has(X::AmPreOffset)
                                || self.has(X::AmPreHold)
                                || self.has(X::AmPreZero)
                        }
                        _ => true,
                    }
                    && (!matches!(
                        k,
                        WaveKind::Pulse
                            | WaveKind::Sin
                            | WaveKind::Exp
                            | WaveKind::Sffm
                            | WaveKind::Am
                            | WaveKind::TrRandom
                    ) || self.has(X::GivenZeroAsGiven)
                        || self.has(X::GivenZeroSpiceDefault))
                    && (self.has(X::DcGivenWithWave) || self.has(dc_at_zero(k)))
            })
            .collect();
        if kinds.is_empty() || self.g.chance(3) {
            return None;
        }
        Some(kinds[self.g.below(kinds.len() as u32) as usize])
    }

    /// A waveform row of `kind` (one `wave_kind` admits).
    pub(super) fn wave_of(&mut self, kind: WaveKind) -> WaveRef {
        match kind {
            WaveKind::Pulse => {
                let law = self.pick(&[
                    (PulseLaw::Spice, Some(X::PulseSpice)),
                    (PulseLaw::Vacask, Some(X::PulseVacask)),
                ]);
                let given_zero = self.zero_rule();
                let [v1, v2]: [ExprId; 2] = std::array::from_fn(|_| self.ex());
                let delay = self.defaulted(given_zero);
                // VACASK requires `rise > 0`: a positive literal
                let rise = if law == PulseLaw::Vacask {
                    let mantissa = 1 + i64::from(self.g.below(999));
                    self.b.literal(Literal {
                        mantissa,
                        frac: 0,
                        exp10: 0,
                        form: NumForm::Plain,
                    })
                } else {
                    self.defaulted(given_zero)
                };
                let [fall, width, period]: [ExprId; 3] =
                    std::array::from_fn(|_| self.defaulted(given_zero));
                let cycles = self.opt(X::PulseCycles);
                self.b.wave(Pulse {
                    v1,
                    v2,
                    delay,
                    rise,
                    fall,
                    width,
                    period,
                    cycles,
                    law,
                    given_zero,
                })
            }
            WaveKind::Sin => {
                let pre = self.pre(X::SinPreOffset, X::SinPreHold, X::SinPreZero);
                let given_zero = self.zero_rule();
                let [offset, amplitude]: [ExprId; 2] = std::array::from_fn(|_| self.ex());
                let freq = self.defaulted(given_zero);
                let [delay, damping, phase_deg]: [ExprId; 3] =
                    std::array::from_fn(|_| self.defaulted(given_zero));
                let cycles = self.opt(X::SinCycles);
                let tone = self.maybe(X::SinSecondTone);
                let amplitude2 = if tone { self.ex() } else { N };
                let freq2 = if tone { self.ex() } else { N };
                self.b.wave(Sin {
                    offset,
                    amplitude,
                    freq,
                    delay,
                    damping,
                    phase_deg,
                    cycles,
                    amplitude2,
                    freq2,
                    pre,
                    given_zero,
                })
            }
            WaveKind::Exp => {
                let td2_from = self.pick(&[
                    (TdFrom::Zero, Some(X::ExpTd2FromZero)),
                    (TdFrom::Td1, Some(X::ExpTd2FromTd1)),
                ]);
                let given_zero = self.zero_rule();
                let [v1, v2]: [ExprId; 2] = std::array::from_fn(|_| self.ex());
                let [td1, tau1, td2, tau2]: [ExprId; 4] =
                    std::array::from_fn(|_| self.defaulted(given_zero));
                self.b.wave(Exp {
                    v1,
                    v2,
                    td1,
                    tau1,
                    td2,
                    tau2,
                    td2_from,
                    given_zero,
                })
            }
            WaveKind::Pwl => {
                let row = self.pwl();
                self.b.wave(row)
            }
            WaveKind::Sffm => {
                let law = self.pick(&[
                    (SffmLaw::Spice3, Some(X::SffmSpice3)),
                    (SffmLaw::Ngspice, Some(X::SffmNgspice)),
                ]);
                let pre = self.pre(X::SffmPreOffset, X::SffmPreHold, X::SffmPreZero);
                let given_zero = self.zero_rule();
                let [offset, amplitude]: [ExprId; 2] = std::array::from_fn(|_| self.ex());
                let carrier_freq = self.defaulted(given_zero);
                let [mod_index, signal_freq, delay, phase_c_deg, phase_m_deg]: [ExprId; 5] =
                    std::array::from_fn(|_| self.defaulted(given_zero));
                self.b.wave(Sffm {
                    offset,
                    amplitude,
                    carrier_freq,
                    mod_index,
                    signal_freq,
                    delay,
                    phase_c_deg,
                    phase_m_deg,
                    law,
                    pre,
                    given_zero,
                })
            }
            WaveKind::Am => {
                let pre = self.pre(X::AmPreOffset, X::AmPreHold, X::AmPreZero);
                let mod_time = if self.maybe(X::AmModCarrierPhase) {
                    AmModTime::CarrierPhase
                } else {
                    AmModTime::Delay
                };
                let given_zero = self.zero_rule();
                let [offset, amplitude, mod_offset]: [ExprId; 3] =
                    std::array::from_fn(|_| self.ex());
                let [
                    mod_index,
                    mod_freq,
                    carrier_freq,
                    delay,
                    phase_m_deg,
                    phase_c_deg,
                ]: [ExprId; 6] = std::array::from_fn(|_| self.defaulted(given_zero));
                self.b.wave(Am {
                    offset,
                    amplitude,
                    mod_offset,
                    mod_index,
                    mod_freq,
                    carrier_freq,
                    delay,
                    phase_m_deg,
                    phase_c_deg,
                    mod_time,
                    pre,
                    given_zero,
                })
            }
            WaveKind::TrNoise => {
                let [na, nt]: [ExprId; 2] = std::array::from_fn(|_| self.ex());
                let [nalpha, namp, rts_amp, rts_capture, rts_emission]: [ExprId; 5] =
                    std::array::from_fn(|_| if self.g.chance(2) { self.ex() } else { N });
                self.b.wave(TrNoise {
                    na,
                    nt,
                    nalpha,
                    namp,
                    rts_amp,
                    rts_capture,
                    rts_emission,
                })
            }
            WaveKind::TrRandom => {
                let given_zero = self.zero_rule();
                let ts = self.ex();
                let [td, p1, p2]: [ExprId; 3] = std::array::from_fn(|_| self.defaulted(given_zero));
                let kind = TrRandomKind::ALL[self.g.below(4) as usize];
                self.b.wave(TrRandom {
                    ts,
                    td,
                    p1,
                    p2,
                    kind,
                    given_zero,
                })
            }
            WaveKind::Pattern => {
                let [high, low, delay, rise, fall, sample]: [ExprId; 6] =
                    std::array::from_fn(|_| self.ex());
                let text: String = (0..1 + self.g.below(6))
                    .map(|_| ['0', '1', 'm'][self.g.below(3) as usize])
                    .collect();
                let bits = self.b.name(&text);
                let repeats = if self.g.chance(2) { self.ex() } else { N };
                let repeat_start = self.ex();
                self.b.wave(Pattern {
                    high,
                    low,
                    delay,
                    rise,
                    fall,
                    sample,
                    bits,
                    repeats,
                    repeat_start,
                })
            }
        }
    }

    fn pre(&mut self, offset: X, hold: X, zero: X) -> PreDelay {
        self.pick(&[
            (PreDelay::Offset, Some(offset)),
            (PreDelay::HoldDelayValue, Some(hold)),
            (PreDelay::Zero, Some(zero)),
        ])
    }

    fn pwl(&mut self) -> Pwl {
        let repeat = self.pick(&[
            (PwlRepeat::Never, None),
            (PwlRepeat::From, Some(X::PwlRepeatFrom)),
            (PwlRepeat::Period, Some(X::PwlRepeatPeriod)),
        ]);
        let join = if repeat == PwlRepeat::Never {
            PwlJoin::Jump
        } else {
            self.pick(&[
                (PwlJoin::Jump, None),
                (PwlJoin::Ramp, Some(X::PwlJoinRamp)),
                (PwlJoin::RampWidth, Some(X::PwlJoinRampWidth)),
                (PwlJoin::Extrapolate, Some(X::PwlJoinExtrapolate)),
            ])
        };
        let n = 2 + 2 * self.g.below(2);
        let points = self.exprs(n);
        let delay = self.opt(X::PwlDelay);
        let repeat_at = if repeat == PwlRepeat::Never {
            N
        } else {
            self.ex()
        };
        let repeat_count = if repeat == PwlRepeat::Never {
            N
        } else {
            self.opt(X::PwlRepeatCount)
        };
        let join_width = if join == PwlJoin::RampWidth {
            self.ex()
        } else {
            N
        };
        let file = if self.maybe(X::PwlFile) {
            self.g.fresh_name(self.b)
        } else {
            NameId::NONE
        };
        Pwl {
            points,
            delay,
            repeat_at,
            repeat_count,
            join_width,
            file,
            repeat,
            join,
        }
    }

    /// The zero rule of a pulse, sine, exponential or SFFM, from the set (A17; `wave_kind`
    /// admitted the wave only when one member is in it).
    fn zero_rule(&mut self) -> GivenZero {
        self.pick(&[
            (GivenZero::AsGiven, Some(X::GivenZeroAsGiven)),
            (GivenZero::SpiceDefault, Some(X::GivenZeroSpiceDefault)),
        ])
    }

    /// A field with a default under `rule`: under `SpiceDefault` sometimes omitted, else an
    /// expression.
    fn defaulted(&mut self, rule: GivenZero) -> ExprId {
        if rule == GivenZero::SpiceDefault && self.g.chance(3) {
            N
        } else {
            self.ex()
        }
    }
}
