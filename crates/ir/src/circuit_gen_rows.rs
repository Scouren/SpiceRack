//! The row bodies of the circuit generator (`circuit_gen.rs`'s child module): a passive,
//! source, behavioural or line row, and every waveform row, built field by field in
//! declaration order (the order `dump` writes them), with optional fields set only when their
//! `CircuitExtra` is in the set.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator, round trip); `circuit_gen.rs`.
//!
//! Contents: `R::passive`, `R::source`, `R::behav`, `R::line` (`pub(super)`),
//! `pub(crate) fn generate_library` (re-exported by `circuit_gen`)

use super::{N, R};
use crate::build::Builder;
use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_features::dc_at_zero;
use crate::controlled::{Behav, OutKind};
use crate::element::{ElemKind, NO_ROW};
use crate::r#gen::Gen;
use crate::r#gen::Reads;
use crate::id::{ItemRef, ScopeId};
use crate::id::{List, NameId};
use crate::library::DeviceLibrary;
use crate::line::Line;
use crate::model::ModelFamily;
use crate::passive::{IcMode, Parasitics, Passive};
use crate::source::{DcRule, Distof, PortSpec, Source};
use crate::span::Span;
use crate::wave::WaveRef;

/// Pushes one random device library into `scope` (always possible).
pub(crate) fn generate_library(g: &mut Gen, scope: ScopeId, b: &mut Builder<'_>) -> ItemRef {
    let family = ModelFamily::ALL[g.below(ModelFamily::COUNT as u32) as usize];
    let path = if g.chance(2) {
        NameId::NONE
    } else {
        b.name(&format!("{}.osdi", g.fresh()))
    };
    b.device_library(scope, DeviceLibrary { path, family }, Span::NONE)
}

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
        // the wave kind and the DC rule decide whether `dc` exists, so both come first
        let wave_kind = self.wave_kind();
        let dc_rule = match wave_kind {
            Some(k)
                if self.has(dc_at_zero(k))
                    && (!self.has(X::DcGivenWithWave) || self.g.chance(2)) =>
            {
                DcRule::WaveAtZero
            }
            _ => DcRule::Given,
        };
        let dc = if dc_rule == DcRule::Given {
            self.ex()
        } else {
            N
        };
        let ac = self.g.chance(2);
        let ac_mag = if ac { self.ex() } else { N };
        let ac_phase_deg = if ac { self.ex() } else { N };
        let tc = self.maybe(X::SourceTc);
        let tc1 = if tc { self.ex() } else { N };
        let tc2 = if tc && self.g.chance(2) { self.ex() } else { N };
        let pac = self.maybe(X::SourcePac);
        let pac_mag = if pac { self.ex() } else { N };
        let pac_phase_deg = if pac { self.ex() } else { N };
        let wave = match wave_kind {
            Some(k) => self.wave_of(k),
            None => WaveRef::NONE,
        };
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
            dc_rule,
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
}
