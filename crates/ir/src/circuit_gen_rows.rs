//! The row bodies of the circuit generator (`circuit_gen.rs`'s child module): a passive,
//! source, behavioural or line row, and every waveform row, built field by field in
//! declaration order (the order `dump` writes them), with optional fields set only when their
//! `CircuitExtra` is in the set.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator, round trip); `circuit_gen.rs`.
//!
//! Contents: `R::passive`, `R::source`, `R::behav_output`, `R::behav`, `R::line` (`pub(super)`),
//! `pub(crate) fn generate_library` (re-exported by `circuit_gen`)

use super::{N, R};
use crate::build::Builder;
use crate::circuit_extra::CircuitExtra as X;
use crate::circuit_features::dc_at_zero;
use crate::class::ExprClass;
use crate::controlled::{Behav, OutKind};
use crate::element::{ElemKind, NO_ROW};
use crate::feature::{Feature, FeatureSet};
use crate::r#gen::Gen;
use crate::r#gen::Reads;
use crate::id::{ItemRef, ScopeId};
use crate::id::{List, NameId};
use crate::library::DeviceLibrary;
use crate::line::Line;
use crate::model::ModelFamily;
use crate::passive::{IcMode, Parasitics, Passive, TcBase, TcLaw};
use crate::source::{DcRule, Distof, PortSpec, Source};
use crate::span::Span;
use crate::wave::WaveRef;

/// Pushes one random device library of a family `features` holds into `scope` (A25), or
/// returns `ItemRef::NONE` when it holds none.
pub(crate) fn generate_library(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    features: &FeatureSet,
) -> ItemRef {
    let families: Vec<ModelFamily> = ModelFamily::ALL
        .into_iter()
        .filter(|&f| features.contains(Feature::Family(f)))
        .collect();
    if families.is_empty() {
        return ItemRef::NONE;
    }
    let family = families[g.below(families.len() as u32) as usize];
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
        // the class the generator tracked while drawing it, exact (ir-kernel 6e35638d; later draws
        // overwrite it): `expr_class` here would recompute the design's table after every push
        let dynamic = self.g.class == ExprClass::Circuit;
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
        // a circuit-valued element's coefficient needs a base (and a capacitor's a law) the set
        // allows (A15 C2)
        let cap = kind == ElemKind::Capacitor;
        let bases = [
            (TcBase::Tnom, X::TcBaseTnom),
            (TcBase::Fixed27, X::TcBaseFixed27),
        ];
        let laws = [
            (TcLaw::Direct, X::TcLawDirect),
            (TcLaw::Reciprocal, X::TcLawReciprocal),
        ];
        let base_ok = bases.iter().any(|b| self.has(b.1));
        let law_ok = laws.iter().any(|l| self.has(l.1));
        let tc = self.maybe(X::PassiveTc) && (!dynamic || base_ok && (!cap || law_ok));
        let (tc_base, tc_law) = if tc && dynamic {
            let base = self.pick(&bases.map(|(m, x)| (m, Some(x))));
            let law = if cap {
                self.pick(&laws.map(|(m, x)| (m, Some(x))))
            } else {
                TcLaw::Direct
            };
            (base, law)
        } else {
            (TcBase::Tnom, TcLaw::Direct)
        };
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
        // A19: noiseless only where there is thermal noise to switch off
        let noisy = !((kind == ElemKind::Resistor || dynamic) && self.maybe(X::PassiveNoiseless));
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
            tc_base,
            tc_law,
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

    /// A behavioural source's output, as the set allows.
    pub(super) fn behav_output(&mut self) -> OutKind {
        self.pick(&[
            (OutKind::Voltage, None),
            (OutKind::Current, None),
            (OutKind::Resistance, Some(X::BehavResistance)),
            (OutKind::Conductance, Some(X::BehavConductance)),
            (OutKind::Capacitance, Some(X::BehavCapacitance)),
            (OutKind::CapacitanceCharge, Some(X::BehavCapacitanceCharge)),
            (OutKind::Inductance, Some(X::BehavInductance)),
            (OutKind::Power, Some(X::BehavPower)),
        ])
    }

    pub(super) fn behav(&mut self, kind: ElemKind) -> Behav {
        // A33: without `BehavUntypedParam` a behavioural source's parameter reads are typed
        // (`ToReal`/`ToInt` built at the leaf, or none when the set has neither)
        self.g.typed_params = kind == ElemKind::Behav && !self.has(X::BehavUntypedParam);
        // R66: the behavioural evaluator's expression counts dynamic whatever its class
        self.g.dyn_form = true;
        let expr = self.dy();
        (self.g.typed_params, self.g.dyn_form) = (false, false);
        // a charge-defined capacitor's coefficient also needs a law the set has (A17)
        let charge = kind == ElemKind::ChargeCapacitor;
        let law_ok = self.has(X::TcLawDirect) || self.has(X::TcLawReciprocal);
        let tc = self.maybe(X::BehavTc)
            && (self.has(X::TcBaseTnom) || self.has(X::TcBaseFixed27))
            && (!charge || law_ok);
        let tc_law = if tc && charge {
            self.pick(&[
                (TcLaw::Direct, Some(X::TcLawDirect)),
                (TcLaw::Reciprocal, Some(X::TcLawReciprocal)),
            ])
        } else {
            TcLaw::Direct
        };
        let tc_base = if tc {
            self.pick(&[
                (TcBase::Tnom, Some(X::TcBaseTnom)),
                (TcBase::Fixed27, Some(X::TcBaseFixed27)),
            ])
        } else {
            TcBase::Tnom
        };
        let tc1 = if tc { self.ex() } else { N };
        let tc2 = if tc && self.g.chance(2) { self.ex() } else { N };
        let temp_abs = self.opt(X::InstanceTemp);
        let dtemp = self.opt(X::InstanceDtemp);
        let rpar = self.opt(X::BehavRpar);
        let output = match kind {
            ElemKind::ChargeCapacitor => OutKind::Charge,
            ElemKind::FluxInductor => OutKind::Flux,
            // drawn with the header: it fixes the multiplier's member (R58)
            _ => self
                .behav_out
                .expect("a behavioural source's output is drawn first"),
        };
        Behav {
            expr,
            tc1,
            tc2,
            temp_abs,
            dtemp,
            rpar,
            output,
            tc_base,
            tc_law,
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
