//! The row bodies of the circuit generator (`circuit_gen.rs`'s child module): a passive,
//! source, behavioural or line row, and every waveform row, built field by field in
//! declaration order (the order `dump` writes them), with optional fields set only when their
//! `CircuitExtra` is in the set.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator, round trip); `circuit_gen.rs`.
//!
//! Contents: `R::passive`, `R::source`, `R::behav`, `R::line`, `R::wave` (`pub(super)`)

use super::{N, R};
use crate::circuit_extra::CircuitExtra as X;
use crate::controlled::{Behav, OutKind};
use crate::element::{ElemKind, NO_ROW};
use crate::feature::Feature;
use crate::r#gen::Reads;
use crate::id::{ExprId, List, NameId};
use crate::line::Line;
use crate::passive::{IcMode, Parasitics, Passive};
use crate::source::{Distof, PortSpec, Source};
use crate::wave::{
    Am, AmModTime, Exp, Pattern, PreDelay, Pulse, Pwl, PwlJoin, PwlRepeat, Sffm, SffmLaw, Sin,
    TrNoise, TrRandom, TrRandomKind, WaveKind, WaveRef,
};

impl R<'_, '_> {
    pub(super) fn passive(&mut self, kind: ElemKind) -> Passive {
        let reads = if self.has(X::PassiveDynamicValue) {
            Reads::Circuit
        } else {
            Reads::Constant
        };
        let value = self.g.expr(self.b, self.f, reads);
        let ic_mode = if kind == ElemKind::Resistor {
            IcMode::None
        } else {
            self.ic_mode()
        };
        let ic = if ic_mode == IcMode::None {
            N
        } else {
            self.ex()
        };
        let tc = self.maybe(X::PassiveTc);
        let tc1 = if tc { self.ex() } else { N };
        let tc2 = if tc && self.g.chance(2) { self.ex() } else { N };
        let tce = self.opt(X::PassiveTce);
        let temp_abs = self.opt(X::InstanceTemp);
        let dtemp = self.opt(X::InstanceDtemp);
        let ac = if kind == ElemKind::Resistor {
            self.opt(X::PassiveAc)
        } else {
            N
        };
        let scale = self.opt(X::PassiveScale);
        let parasitics = if kind != ElemKind::Resistor && self.maybe(X::ElementParasitics) {
            let cap = kind == ElemKind::Capacitor;
            let rser = self.ex();
            let lser = if cap && self.g.chance(2) {
                self.ex()
            } else {
                N
            };
            let rpar = if self.g.chance(2) { self.ex() } else { N };
            let cpar = if self.g.chance(2) { self.ex() } else { N };
            let rlshunt = if cap && self.g.chance(2) {
                self.ex()
            } else {
                N
            };
            self.b.detail(Parasitics {
                rser,
                lser,
                rpar,
                cpar,
                rlshunt,
            })
        } else {
            NO_ROW
        };
        let noisy = !self.maybe(X::PassiveNoiseless);
        Passive {
            value,
            ic,
            tc1,
            tc2,
            tce,
            temp_abs,
            dtemp,
            ac,
            scale,
            parasitics,
            ic_mode,
            noisy,
        }
    }

    pub(super) fn source(&mut self, kind: ElemKind) -> Source {
        let dc = self.ex();
        let ac = self.g.chance(2);
        let ac_mag = if ac { self.ex() } else { N };
        let ac_phase_deg = if ac { self.ex() } else { N };
        let tc = self.maybe(X::SourceTc);
        let tc1 = if tc { self.ex() } else { N };
        let tc2 = if tc && self.g.chance(2) { self.ex() } else { N };
        let pac = self.maybe(X::SourcePac);
        let pac_mag = if pac { self.ex() } else { N };
        let pac_phase_deg = if pac { self.ex() } else { N };
        let wave = self.wave();
        let noise_psd = if self.maybe(X::SourceNoisePsd) {
            let n = 2 + 2 * self.g.below(2);
            self.exprs(n)
        } else {
            List::EMPTY
        };
        let parasitics = if kind == ElemKind::VSource && self.maybe(X::SourceParasitics) {
            let rser = self.ex();
            let cpar = if self.g.chance(2) { self.ex() } else { N };
            self.b.detail(Parasitics {
                rser,
                lser: N,
                rpar: N,
                cpar,
                rlshunt: N,
            })
        } else {
            NO_ROW
        };
        let port = if kind == ElemKind::Port {
            let index = self.ex();
            let z0 = self.ex();
            let noise_temp = self.opt(X::PortNoiseTemp);
            let power_on = self.maybe(X::PortPower);
            let power = if power_on { self.ex() } else { N };
            let freq = if power_on { self.ex() } else { N };
            self.b.detail(PortSpec {
                index,
                z0,
                noise_temp,
                power,
                freq,
            })
        } else {
            NO_ROW
        };
        let distof = if self.maybe(X::SourceDistof) {
            let (f1_mag, f1_phase_deg) = (self.ex(), self.ex());
            self.b.detail(Distof {
                f1_mag,
                f1_phase_deg,
                f2_mag: N,
                f2_phase_deg: N,
            })
        } else {
            NO_ROW
        };
        Source {
            dc,
            ac_mag,
            ac_phase_deg,
            tc1,
            tc2,
            pac_mag,
            pac_phase_deg,
            wave,
            noise_psd,
            parasitics,
            port,
            distof,
        }
    }

    pub(super) fn behav(&mut self, kind: ElemKind) -> Behav {
        let expr = self.dy();
        let tc = self.maybe(X::BehavTc);
        let tc1 = if tc { self.ex() } else { N };
        let tc2 = if tc && self.g.chance(2) { self.ex() } else { N };
        let temp_abs = self.opt(X::InstanceTemp);
        let dtemp = self.opt(X::InstanceDtemp);
        let rpar = self.opt(X::BehavRpar);
        let output = match kind {
            ElemKind::ChargeCapacitor => OutKind::Charge,
            ElemKind::FluxInductor => OutKind::Flux,
            _ => self.pick(&[
                (OutKind::Voltage, None),
                (OutKind::Current, None),
                (OutKind::Resistance, Some(X::BehavResistance)),
                (OutKind::Conductance, Some(X::BehavConductance)),
                (OutKind::Capacitance, Some(X::BehavCapacitance)),
                (OutKind::CapacitanceCharge, Some(X::BehavCapacitanceCharge)),
                (OutKind::Inductance, Some(X::BehavInductance)),
                (OutKind::Power, Some(X::BehavPower)),
            ]),
        };
        Behav {
            expr,
            tc1,
            tc2,
            temp_abs,
            dtemp,
            rpar,
            output,
        }
    }

    pub(super) fn line(&mut self, kind: ElemKind) -> Line {
        let mut l = Line {
            z0: N,
            td: N,
            freq: N,
            nl: N,
            length: N,
            vel: N,
            lumps: N,
        };
        match kind {
            ElemKind::LosslessLine => {
                let form = self.pick(&[
                    (0u8, None),
                    (1, Some(X::LineFreqNl)),
                    (2, Some(X::LineLengthVel)),
                ]);
                l.z0 = self.ex();
                match form {
                    0 => l.td = self.ex(),
                    1 => {
                        l.freq = self.ex();
                        l.nl = self.ex();
                    }
                    _ => {
                        l.length = self.ex();
                        l.vel = self.ex();
                    }
                }
            }
            _ => {
                if self.g.chance(2) {
                    l.length = self.ex();
                }
                if kind == ElemKind::RcLine {
                    l.lumps = self.opt(X::LineLumps);
                }
            }
        }
        l
    }

    /// A waveform of a kind in the set, or `WaveRef::NONE`.
    pub(super) fn wave(&mut self) -> WaveRef {
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
                        WaveKind::Sffm => self.has(X::SffmSpice3) || self.has(X::SffmNgspice),
                        WaveKind::Am => {
                            self.has(X::AmPreOffset)
                                || self.has(X::AmPreHold)
                                || self.has(X::AmPreZero)
                        }
                        _ => true,
                    }
            })
            .collect();
        if kinds.is_empty() || self.g.chance(3) {
            return WaveRef::NONE;
        }
        match kinds[self.g.below(kinds.len() as u32) as usize] {
            WaveKind::Pulse => {
                let v: [ExprId; 7] = std::array::from_fn(|_| self.ex());
                let cycles = self.opt(X::PulseCycles);
                let [v1, v2, delay, rise, fall, width, period] = v;
                self.b.wave(Pulse {
                    v1,
                    v2,
                    delay,
                    rise,
                    fall,
                    width,
                    period,
                    cycles,
                })
            }
            WaveKind::Sin => {
                let pre = self.pre(X::SinPreOffset, X::SinPreHold, X::SinPreZero);
                let [offset, amplitude, freq, delay, damping, phase_deg]: [ExprId; 6] =
                    std::array::from_fn(|_| self.ex());
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
                })
            }
            WaveKind::Exp => {
                let [v1, v2, td1, tau1, td2, tau2]: [ExprId; 6] =
                    std::array::from_fn(|_| self.ex());
                self.b.wave(Exp {
                    v1,
                    v2,
                    td1,
                    tau1,
                    td2,
                    tau2,
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
                let [
                    offset,
                    amplitude,
                    carrier_freq,
                    mod_index,
                    signal_freq,
                    delay,
                    phase_c_deg,
                    phase_m_deg,
                ]: [ExprId; 8] = std::array::from_fn(|_| self.ex());
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
                })
            }
            WaveKind::Am => {
                let pre = self.pre(X::AmPreOffset, X::AmPreHold, X::AmPreZero);
                let mod_time = if self.maybe(X::AmModCarrierPhase) {
                    AmModTime::CarrierPhase
                } else {
                    AmModTime::Delay
                };
                let [
                    offset,
                    amplitude,
                    mod_offset,
                    mod_index,
                    mod_freq,
                    carrier_freq,
                    delay,
                    phase_m_deg,
                    phase_c_deg,
                ]: [ExprId; 9] = std::array::from_fn(|_| self.ex());
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
                let [ts, td, p1, p2]: [ExprId; 4] = std::array::from_fn(|_| self.ex());
                let kind = TrRandomKind::ALL[self.g.below(4) as usize];
                self.b.wave(TrRandom {
                    ts,
                    td,
                    p1,
                    p2,
                    kind,
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
}
