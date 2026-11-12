//! Features of circuit rows. Oracle: spec example with a legal neighbour (IR-IMPL §2
//! "Features"): for every `CircuitExtra`, one design whose single element or model sets exactly
//! that field (`features_used` contains it) and its neighbour that differs only in that field
//! (`features_used` does not). The covered list is compared with `CircuitExtra::ALL` by name.
//! Also the kind features: `Feature::Elem` and `Feature::Wave` of what is pushed.
//!
//! Cites: `rewrite/IR-IMPL.md` §2; `circuit_extra.rs` member docs.
//!
//! Contents: tests only.

#[path = "circuit_common.rs"]
mod circuit_common;

#[cfg(test)]
mod tests {

    use super::circuit_common::{behav, deck, line, model, n, port, pulse, semi, sin, source};
    use spicerack_ir::controlled::{
        Delay, DelayLaw, FreqLaw, FreqTable, FreqUnit, Laplace, LaplaceForm, OutKind, Quantity,
        TableLaw, TableSource,
    };
    use spicerack_ir::passive::{Coupling, IcMode, Parasitics, Passive, TcBase, TcLaw};
    use spicerack_ir::semi::{InitialState, Switch};
    use spicerack_ir::source::{DcRule, Distof, Source};
    use spicerack_ir::wave::{
        Am, AmModTime, Exp, Pattern, PreDelay, PulseLaw, Pwl, PwlJoin, PwlRepeat, Sffm, SffmLaw,
        TdFrom, TrNoise, TrRandom, TrRandomKind,
    };
    use spicerack_ir::{
        Builder, CircuitExtra as X, Design, DeviceLibrary, ElemKind, ExprId, Feature, FeatureSet,
        ModelFamily, NO_ROW, NameId, Polarity, Ref, ScopeId, Span, WaveKind, features_used,
    };

    type Case = fn(&mut Builder<'_>, ScopeId, bool);

    fn used(case: Case, on: bool) -> FeatureSet {
        let mut d = Design::new();
        let top = deck(&mut d);
        case(&mut d.builder(), top, on);
        features_used(&d)
    }

    fn opt(b: &mut Builder<'_>, on: bool) -> ExprId {
        if on { n(b, 1) } else { ExprId::NONE }
    }

    fn passive(b: &mut Builder<'_>, top: ScopeId, kind: ElemKind, mult: ExprId, row: Passive) {
        let terms: &[&str] = &["a", "b"];
        let h = b.head("e1", terms, None, &[], mult);
        b.passive(top, kind, h, row, Span::NONE);
    }

    fn plain(b: &mut Builder<'_>) -> Passive {
        let value = n(b, 1);
        Passive {
            value,
            ..Passive::PLAIN
        }
    }

    fn vsrc(b: &mut Builder<'_>, top: ScopeId, kind: ElemKind, mult: ExprId, row: Source) {
        let h = b.head("e1", &["a", "0"], None, &[], mult);
        b.source(top, kind, h, row, Span::NONE);
    }

    fn with_wave(b: &mut Builder<'_>, top: ScopeId, wave: spicerack_ir::WaveRef) {
        let dc = n(b, 0);
        vsrc(
            b,
            top,
            ElemKind::VSource,
            ExprId::NONE,
            Source { wave, ..source(dc) },
        );
    }

    fn pwl(b: &mut Builder<'_>) -> Pwl {
        let one = n(b, 1);
        let points = b.exprs(&[one, one]);
        Pwl {
            points,
            delay: ExprId::NONE,
            repeat_at: ExprId::NONE,
            repeat_count: ExprId::NONE,
            join_width: ExprId::NONE,
            file: NameId::NONE,
            repeat: PwlRepeat::Never,
            join: PwlJoin::Jump,
        }
    }

    fn sffm(b: &mut Builder<'_>, law: SffmLaw) -> Sffm {
        let v = n(b, 1);
        Sffm {
            offset: v,
            amplitude: v,
            carrier_freq: v,
            mod_index: v,
            signal_freq: v,
            delay: v,
            phase_c_deg: v,
            phase_m_deg: v,
            law,
            pre: PreDelay::Zero,
        }
    }

    fn am(b: &mut Builder<'_>, mod_time: AmModTime, pre: PreDelay) -> Am {
        let v = n(b, 1);
        Am {
            offset: v,
            amplitude: v,
            mod_offset: v,
            mod_index: v,
            mod_freq: v,
            carrier_freq: v,
            delay: v,
            phase_m_deg: v,
            phase_c_deg: v,
            mod_time,
            pre,
        }
    }

    fn behav_el(
        b: &mut Builder<'_>,
        top: ScopeId,
        out: OutKind,
        on_tc: bool,
        on_rpar: bool,
        on_temp: bool,
    ) {
        let e = n(b, 1);
        let tc1 = opt(b, on_tc);
        let temp_abs = opt(b, on_temp);
        let rpar = opt(b, on_rpar);
        let h = b.head("e1", &["a", "0"], None, &[], ExprId::NONE);
        b.behav(
            top,
            ElemKind::Behav,
            h,
            spicerack_ir::controlled::Behav {
                tc1,
                temp_abs,
                rpar,
                ..behav(e, out)
            },
            Span::NONE,
        );
    }

    fn table(b: &mut Builder<'_>, top: ScopeId, law: TableLaw) {
        let one = n(b, 1);
        let points = b.exprs(&[one, one]);
        let h = b.head("e1", &["a", "0"], None, &[], ExprId::NONE);
        b.table_source(
            top,
            h,
            TableSource {
                input: one,
                points,
                delta: ExprId::NONE,
                law,
                output: Quantity::Voltage,
            },
            Span::NONE,
        );
    }

    fn laplace(b: &mut Builder<'_>, top: ScopeId, form: LaplaceForm, expr_input: bool) {
        let one = n(b, 1);
        let num = b.exprs(&[one]);
        let den = b.exprs(&[one]);
        let input = opt(b, expr_input);
        let terms: &[&str] = if expr_input {
            &["o", "0"]
        } else {
            &["o", "0", "i", "0"]
        };
        let h = b.head("e1", terms, None, &[], ExprId::NONE);
        b.laplace(
            top,
            h,
            Laplace {
                num,
                den,
                input,
                output: Quantity::Voltage,
                form,
            },
            Span::NONE,
        );
    }

    fn freq(b: &mut Builder<'_>, top: ScopeId, law: FreqLaw) {
        let one = n(b, 1);
        let points = b.exprs(&[one, one, one]);
        let h = b.head("e1", &["o", "0", "i", "0"], None, &[], ExprId::NONE);
        b.freq_table(
            top,
            h,
            FreqTable {
                input: ExprId::NONE,
                points,
                law,
                unit: FreqUnit::Db,
                output: Quantity::Voltage,
            },
            Span::NONE,
        );
    }

    fn semi_el(
        b: &mut Builder<'_>,
        top: ScopeId,
        kind: ElemKind,
        terms: &[&str],
        mult: ExprId,
        row: spicerack_ir::semi::Semi,
    ) {
        let h = b.head("e1", terms, Some("m"), &[], mult);
        b.semi(top, kind, h, row, Span::NONE);
    }

    fn switch(b: &mut Builder<'_>, top: ScopeId, initial: InitialState, control: bool) {
        let c = opt(b, control);
        let terms: &[&str] = if control {
            &["a", "0"]
        } else {
            &["a", "0", "c", "0"]
        };
        let h = b.head("e1", terms, Some("sw"), &[], ExprId::NONE);
        b.switch(
            top,
            ElemKind::VSwitch,
            h,
            Switch {
                sense: NameId::NONE,
                control: c,
                initial,
            },
            Span::NONE,
        );
    }

    fn lossless(b: &mut Builder<'_>, top: ScopeId, form: u8) {
        let one = n(b, 1);
        let mut row = spicerack_ir::line::Line { z0: one, ..line() };
        match form {
            0 => row.td = n(b, 1),
            1 => (row.freq, row.nl) = (n(b, 1), n(b, 1)),
            _ => (row.length, row.vel) = (n(b, 1), n(b, 1)),
        }
        let h = b.head("e1", &["a", "0", "b", "0"], None, &[], ExprId::NONE);
        b.line(top, ElemKind::LosslessLine, h, row, Span::NONE);
    }

    /// A behavioural source with a `tc1` (when `tc`) measured from `base`.
    fn behav_tc(b: &mut Builder<'_>, top: ScopeId, tc: bool, base: TcBase) {
        let e = n(b, 1);
        let tc1 = opt(b, tc);
        let h = b.head("b1", &["a", "0"], None, &[], ExprId::NONE);
        let row = spicerack_ir::controlled::Behav {
            tc1,
            tc_base: base,
            ..behav(e, OutKind::Voltage)
        };
        b.behav(top, ElemKind::Behav, h, row, Span::NONE);
    }

    /// A capacitor whose value reads the circuit, with a `tc1` applied by `law`.
    fn dynamic_cap(b: &mut Builder<'_>, top: ScopeId, law: TcLaw) {
        let value = b.v("a", None);
        let tc1 = n(b, 1);
        let h = b.head("c1", &["a", "0"], None, &[], ExprId::NONE);
        let row = Passive {
            value,
            tc1,
            tc_law: law,
            ..Passive::PLAIN
        };
        b.passive(top, ElemKind::Capacitor, h, row, Span::NONE);
    }

    fn pulse_law(b: &mut Builder<'_>, top: ScopeId, law: PulseLaw) {
        let one = n(b, 1);
        let w = b.wave(spicerack_ir::wave::Pulse { law, ..pulse(one) });
        with_wave(b, top, w);
    }

    fn exp_from(b: &mut Builder<'_>, top: ScopeId, td2_from: TdFrom) {
        let v = n(b, 1);
        let w = b.wave(Exp {
            v1: v,
            v2: v,
            td1: v,
            tau1: v,
            td2: v,
            tau2: v,
            td2_from,
        });
        with_wave(b, top, w);
    }

    fn sffm_pre(b: &mut Builder<'_>, top: ScopeId, pre: PreDelay) {
        let r = sffm(b, SffmLaw::Spice3);
        let w = b.wave(Sffm { pre, ..r });
        with_wave(b, top, w);
    }

    /// A valid waveform of `kind`.
    fn wave_of(b: &mut Builder<'_>, kind: WaveKind) -> spicerack_ir::WaveRef {
        let v = n(b, 1);
        match kind {
            WaveKind::Pulse => b.wave(pulse(v)),
            WaveKind::Sin => b.wave(sin(v)),
            WaveKind::Exp => b.wave(Exp {
                v1: v,
                v2: v,
                td1: v,
                tau1: v,
                td2: v,
                tau2: v,
                td2_from: TdFrom::Zero,
            }),
            WaveKind::Pwl => {
                let r = pwl(b);
                b.wave(r)
            }
            WaveKind::Sffm => {
                let r = sffm(b, SffmLaw::Spice3);
                b.wave(r)
            }
            WaveKind::Am => {
                let r = am(b, AmModTime::Delay, PreDelay::Offset);
                b.wave(r)
            }
            WaveKind::TrNoise => b.wave(TrNoise {
                na: v,
                nt: v,
                nalpha: ExprId::NONE,
                namp: ExprId::NONE,
                rts_amp: ExprId::NONE,
                rts_capture: ExprId::NONE,
                rts_emission: ExprId::NONE,
            }),
            WaveKind::TrRandom => b.wave(TrRandom {
                ts: v,
                td: v,
                p1: v,
                p2: v,
                kind: TrRandomKind::Gauss,
            }),
            WaveKind::Pattern => {
                let bits = b.name("10");
                b.wave(Pattern {
                    high: v,
                    low: v,
                    delay: v,
                    rise: v,
                    fall: v,
                    sample: v,
                    bits,
                    repeats: ExprId::NONE,
                    repeat_start: v,
                })
            }
        }
    }

    /// A source of wave `kind` whose DC is the wave at t = 0 (`on`) or given (neighbour).
    fn at_zero(b: &mut Builder<'_>, top: ScopeId, kind: WaveKind, on: bool) {
        let (dc, dc_rule) = if on {
            (ExprId::NONE, DcRule::WaveAtZero)
        } else {
            (n(b, 0), DcRule::Given)
        };
        let wave = wave_of(b, kind);
        vsrc(
            b,
            top,
            ElemKind::VSource,
            ExprId::NONE,
            Source {
                wave,
                dc_rule,
                ..source(dc)
            },
        );
    }

    fn model_el(b: &mut Builder<'_>, top: ScopeId, bin: bool, ako: bool) {
        let name = b.name("m1");
        let bin_of = if bin { b.name("m") } else { NameId::NONE };
        let ako_ref = if ako {
            Ref::unresolved(b.name("base"))
        } else {
            Ref::unresolved(NameId::NONE)
        };
        b.model(
            top,
            spicerack_ir::Model {
                bin_of,
                ako: ako_ref,
                ..model(name, ModelFamily::Resistor, Polarity::None)
            },
            Span::NONE,
        );
    }

    const CASES: &[(X, Case)] = &[
        (X::MultResistor, |b, t, on| {
            let m = opt(b, on);
            let r = plain(b);
            passive(b, t, ElemKind::Resistor, m, r)
        }),
        (X::MultCapacitor, |b, t, on| {
            let m = opt(b, on);
            let r = plain(b);
            passive(b, t, ElemKind::Capacitor, m, r)
        }),
        (X::MultInductor, |b, t, on| {
            let m = opt(b, on);
            let r = plain(b);
            passive(b, t, ElemKind::Inductor, m, r)
        }),
        (X::MultVSource, |b, t, on| {
            let m = opt(b, on);
            let dc = n(b, 0);
            vsrc(b, t, ElemKind::VSource, m, source(dc))
        }),
        (X::MultISource, |b, t, on| {
            let m = opt(b, on);
            let dc = n(b, 0);
            vsrc(b, t, ElemKind::ISource, m, source(dc))
        }),
        (X::MultControlled, |b, t, on| {
            let m = opt(b, on);
            let g = n(b, 1);
            let h = b.head("e1", &["a", "0", "c", "0"], None, &[], m);
            b.controlled(
                t,
                ElemKind::Vcvs,
                h,
                spicerack_ir::controlled::Controlled {
                    gain: g,
                    sense: NameId::NONE,
                },
                Span::NONE,
            );
        }),
        (X::MultSemi, |b, t, on| {
            let m = opt(b, on);
            semi_el(b, t, ElemKind::Diode, &["a", "0"], m, semi())
        }),
        (X::MultInstanceParallel, |b, t, on| {
            let m = opt(b, on);
            let h = b.head("e1", &["a"], Some("sub"), &[], m);
            b.instance(t, ElemKind::Instance, h, Span::NONE);
        }),
        (X::MultInstanceNgspiceAppend, |b, t, on| {
            let m = n(b, 2);
            let law = if on {
                spicerack_ir::MultLaw::NgspiceAppend
            } else {
                spicerack_ir::MultLaw::Parallel
            };
            let h = spicerack_ir::ElemHead {
                mult_law: law,
                ..b.head("e1", &["a"], Some("sub"), &[], m)
            };
            b.instance(t, ElemKind::Instance, h, Span::NONE);
        }),
        (X::MultModule, |b, t, on| {
            let m = opt(b, on);
            let h = b.head("e1", &["a"], Some("va"), &[], m);
            b.instance(t, ElemKind::ModuleModel, h, Span::NONE);
        }),
        (X::MultOther, |b, t, on| {
            let m = opt(b, on);
            let one = n(b, 1);
            let p = b.detail(port(one, one));
            vsrc(
                b,
                t,
                ElemKind::Port,
                m,
                Source {
                    port: p,
                    ..source(one)
                },
            );
        }),
        (X::IcUicOnly, |b, t, on| {
            let mut r = plain(b);
            if on {
                r.ic = n(b, 1);
                r.ic_mode = IcMode::UicOnly;
            }
            passive(b, t, ElemKind::Capacitor, ExprId::NONE, r);
        }),
        (X::IcInOp, |b, t, on| {
            let mut r = plain(b);
            if on {
                r.ic = n(b, 1);
                r.ic_mode = IcMode::InOp;
            }
            passive(b, t, ElemKind::Capacitor, ExprId::NONE, r);
        }),
        (X::PassiveDynamicValue, |b, t, on| {
            let value = if on { b.v("a", None) } else { n(b, 1) };
            passive(
                b,
                t,
                ElemKind::Resistor,
                ExprId::NONE,
                Passive {
                    value,
                    ..Passive::PLAIN
                },
            );
        }),
        (X::PassiveNoiseless, |b, t, on| {
            let r = Passive {
                noisy: !on,
                ..plain(b)
            };
            passive(b, t, ElemKind::Resistor, ExprId::NONE, r)
        }),
        (X::PassiveAc, |b, t, on| {
            let r = plain(b);
            let ac = opt(b, on);
            passive(b, t, ElemKind::Resistor, ExprId::NONE, Passive { ac, ..r })
        }),
        (X::PassiveScale, |b, t, on| {
            let r = plain(b);
            let scale = opt(b, on);
            passive(
                b,
                t,
                ElemKind::Resistor,
                ExprId::NONE,
                Passive { scale, ..r },
            )
        }),
        (X::PassiveTc, |b, t, on| {
            let r = plain(b);
            let tc2 = opt(b, on);
            passive(b, t, ElemKind::Resistor, ExprId::NONE, Passive { tc2, ..r })
        }),
        (X::PassiveTce, |b, t, on| {
            let r = plain(b);
            let tce = opt(b, on);
            passive(b, t, ElemKind::Resistor, ExprId::NONE, Passive { tce, ..r })
        }),
        (X::InstanceTemp, |b, t, on| {
            let temp_abs = opt(b, on);
            semi_el(
                b,
                t,
                ElemKind::Diode,
                &["a", "0"],
                ExprId::NONE,
                spicerack_ir::semi::Semi { temp_abs, ..semi() },
            )
        }),
        (X::InstanceDtemp, |b, t, on| {
            let r = plain(b);
            let dtemp = opt(b, on);
            passive(
                b,
                t,
                ElemKind::Resistor,
                ExprId::NONE,
                Passive { dtemp, ..r },
            )
        }),
        (X::ElementParasitics, |b, t, on| {
            let r = plain(b);
            let parasitics = if on {
                let one = n(b, 1);
                b.detail(Parasitics {
                    rser: one,
                    lser: ExprId::NONE,
                    rpar: ExprId::NONE,
                    cpar: ExprId::NONE,
                    rlshunt: ExprId::NONE,
                })
            } else {
                NO_ROW
            };
            passive(
                b,
                t,
                ElemKind::Inductor,
                ExprId::NONE,
                Passive { parasitics, ..r },
            );
        }),
        (X::SourceParasitics, |b, t, on| {
            let dc = n(b, 0);
            let parasitics = if on {
                let one = n(b, 1);
                b.detail(Parasitics {
                    rser: one,
                    lser: ExprId::NONE,
                    rpar: ExprId::NONE,
                    cpar: ExprId::NONE,
                    rlshunt: ExprId::NONE,
                })
            } else {
                NO_ROW
            };
            vsrc(
                b,
                t,
                ElemKind::VSource,
                ExprId::NONE,
                Source {
                    parasitics,
                    ..source(dc)
                },
            );
        }),
        (X::BehavRpar, |b, t, on| {
            behav_el(b, t, OutKind::Voltage, false, on, false)
        }),
        (X::CouplingMany, |b, t, on| {
            let one = n(b, 1);
            let inductors = if on {
                b.names(&["l1", "l2", "l3"])
            } else {
                b.names(&["l1", "l2"])
            };
            let h = b.head("k1", &[], None, &[], ExprId::NONE);
            b.coupling(t, h, Coupling { inductors, k: one }, Span::NONE);
        }),
        (X::CouplingCore, |b, t, on| {
            let one = n(b, 1);
            let inductors = b.names(&["l1", "l2"]);
            let h = b.head(
                "k1",
                &[],
                if on { Some("core") } else { None },
                &[],
                ExprId::NONE,
            );
            b.coupling(t, h, Coupling { inductors, k: one }, Span::NONE);
        }),
        (X::SourceTc, |b, t, on| {
            let dc = n(b, 0);
            let tc1 = opt(b, on);
            vsrc(
                b,
                t,
                ElemKind::VSource,
                ExprId::NONE,
                Source { tc1, ..source(dc) },
            )
        }),
        (X::SourcePac, |b, t, on| {
            let dc = n(b, 0);
            let (pac_mag, pac_phase_deg) = (opt(b, on), opt(b, on));
            vsrc(
                b,
                t,
                ElemKind::VSource,
                ExprId::NONE,
                Source {
                    pac_mag,
                    pac_phase_deg,
                    ..source(dc)
                },
            );
        }),
        (X::SourceNoisePsd, |b, t, on| {
            let dc = n(b, 0);
            let noise_psd = if on {
                let one = n(b, 1);
                b.exprs(&[one, one])
            } else {
                spicerack_ir::List::EMPTY
            };
            vsrc(
                b,
                t,
                ElemKind::VSource,
                ExprId::NONE,
                Source {
                    noise_psd,
                    ..source(dc)
                },
            );
        }),
        (X::SourceDistof, |b, t, on| {
            let dc = n(b, 0);
            let distof = if on {
                let one = n(b, 1);
                b.detail(Distof {
                    f1_mag: one,
                    f1_phase_deg: one,
                    f2_mag: ExprId::NONE,
                    f2_phase_deg: ExprId::NONE,
                })
            } else {
                NO_ROW
            };
            vsrc(
                b,
                t,
                ElemKind::VSource,
                ExprId::NONE,
                Source {
                    distof,
                    ..source(dc)
                },
            );
        }),
        (X::PortNoiseTemp, |b, t, on| {
            let one = n(b, 1);
            let noise_temp = opt(b, on);
            let p = b.detail(spicerack_ir::source::PortSpec {
                noise_temp,
                ..port(one, one)
            });
            vsrc(
                b,
                t,
                ElemKind::Port,
                ExprId::NONE,
                Source {
                    port: p,
                    ..source(one)
                },
            );
        }),
        (X::PortPower, |b, t, on| {
            let one = n(b, 1);
            let (power, freq) = (opt(b, on), opt(b, on));
            let p = b.detail(spicerack_ir::source::PortSpec {
                power,
                freq,
                ..port(one, one)
            });
            vsrc(
                b,
                t,
                ElemKind::Port,
                ExprId::NONE,
                Source {
                    port: p,
                    ..source(one)
                },
            );
        }),
        (X::PulseCycles, |b, t, on| {
            let one = n(b, 1);
            let cycles = opt(b, on);
            let w = b.wave(spicerack_ir::wave::Pulse {
                cycles,
                ..pulse(one)
            });
            with_wave(b, t, w)
        }),
        (X::SinCycles, |b, t, on| {
            let one = n(b, 1);
            let cycles = opt(b, on);
            let w = b.wave(spicerack_ir::wave::Sin { cycles, ..sin(one) });
            with_wave(b, t, w)
        }),
        (X::SinPreOffset, |b, t, on| {
            let one = n(b, 1);
            let pre = if on { PreDelay::Offset } else { PreDelay::Zero };
            let w = b.wave(spicerack_ir::wave::Sin { pre, ..sin(one) });
            with_wave(b, t, w)
        }),
        (X::SinPreHold, |b, t, on| {
            let one = n(b, 1);
            let pre = if on {
                PreDelay::HoldDelayValue
            } else {
                PreDelay::Offset
            };
            let w = b.wave(spicerack_ir::wave::Sin { pre, ..sin(one) });
            with_wave(b, t, w)
        }),
        (X::SinPreZero, |b, t, on| {
            let one = n(b, 1);
            let pre = if on { PreDelay::Zero } else { PreDelay::Offset };
            let w = b.wave(spicerack_ir::wave::Sin { pre, ..sin(one) });
            with_wave(b, t, w)
        }),
        (X::SinSecondTone, |b, t, on| {
            let one = n(b, 1);
            let (amplitude2, freq2) = (opt(b, on), opt(b, on));
            let w = b.wave(spicerack_ir::wave::Sin {
                amplitude2,
                freq2,
                ..sin(one)
            });
            with_wave(b, t, w)
        }),
        (X::SffmSpice3, |b, t, on| {
            let r = sffm(
                b,
                if on {
                    SffmLaw::Spice3
                } else {
                    SffmLaw::Ngspice
                },
            );
            let w = b.wave(r);
            with_wave(b, t, w)
        }),
        (X::SffmNgspice, |b, t, on| {
            let r = sffm(
                b,
                if on {
                    SffmLaw::Ngspice
                } else {
                    SffmLaw::Spice3
                },
            );
            let w = b.wave(r);
            with_wave(b, t, w)
        }),
        (X::AmPreOffset, |b, t, on| {
            let r = am(
                b,
                AmModTime::Delay,
                if on { PreDelay::Offset } else { PreDelay::Zero },
            );
            let w = b.wave(r);
            with_wave(b, t, w)
        }),
        (X::AmPreHold, |b, t, on| {
            let r = am(
                b,
                AmModTime::Delay,
                if on {
                    PreDelay::HoldDelayValue
                } else {
                    PreDelay::Zero
                },
            );
            let w = b.wave(r);
            with_wave(b, t, w)
        }),
        (X::AmPreZero, |b, t, on| {
            let r = am(
                b,
                AmModTime::Delay,
                if on { PreDelay::Zero } else { PreDelay::Offset },
            );
            let w = b.wave(r);
            with_wave(b, t, w)
        }),
        (X::AmModCarrierPhase, |b, t, on| {
            let r = am(
                b,
                if on {
                    AmModTime::CarrierPhase
                } else {
                    AmModTime::Delay
                },
                PreDelay::Zero,
            );
            let w = b.wave(r);
            with_wave(b, t, w)
        }),
        (X::PwlDelay, |b, t, on| {
            let r = pwl(b);
            let delay = opt(b, on);
            let w = b.wave(Pwl { delay, ..r });
            with_wave(b, t, w)
        }),
        (X::PwlRepeatFrom, |b, t, on| {
            let r = pwl(b);
            let r = if on {
                let at = n(b, 1);
                Pwl {
                    repeat: PwlRepeat::From,
                    repeat_at: at,
                    ..r
                }
            } else {
                r
            };
            let w = b.wave(r);
            with_wave(b, t, w);
        }),
        (X::PwlRepeatPeriod, |b, t, on| {
            let r = pwl(b);
            let r = if on {
                let at = n(b, 1);
                Pwl {
                    repeat: PwlRepeat::Period,
                    repeat_at: at,
                    ..r
                }
            } else {
                r
            };
            let w = b.wave(r);
            with_wave(b, t, w);
        }),
        (X::PwlRepeatCount, |b, t, on| {
            let r = pwl(b);
            let at = n(b, 1);
            let repeat_count = opt(b, on);
            let w = b.wave(Pwl {
                repeat: PwlRepeat::From,
                repeat_at: at,
                repeat_count,
                ..r
            });
            with_wave(b, t, w);
        }),
        (X::PwlJoinRamp, |b, t, on| {
            let r = pwl(b);
            let at = n(b, 1);
            let join = if on { PwlJoin::Ramp } else { PwlJoin::Jump };
            let w = b.wave(Pwl {
                repeat: PwlRepeat::From,
                repeat_at: at,
                join,
                ..r
            });
            with_wave(b, t, w);
        }),
        (X::PwlJoinRampWidth, |b, t, on| {
            let r = pwl(b);
            let at = n(b, 1);
            let (join, join_width) = if on {
                (PwlJoin::RampWidth, n(b, 1))
            } else {
                (PwlJoin::Jump, ExprId::NONE)
            };
            let w = b.wave(Pwl {
                repeat: PwlRepeat::Period,
                repeat_at: at,
                join,
                join_width,
                ..r
            });
            with_wave(b, t, w);
        }),
        (X::PwlJoinExtrapolate, |b, t, on| {
            let r = pwl(b);
            let at = n(b, 1);
            let join = if on {
                PwlJoin::Extrapolate
            } else {
                PwlJoin::Jump
            };
            let w = b.wave(Pwl {
                repeat: PwlRepeat::Period,
                repeat_at: at,
                join,
                ..r
            });
            with_wave(b, t, w);
        }),
        (X::PwlFile, |b, t, on| {
            let r = pwl(b);
            let r = if on {
                let file = b.name("wave.csv");
                Pwl { file, ..r }
            } else {
                r
            };
            let w = b.wave(r);
            with_wave(b, t, w);
        }),
        (X::TableExact, |b, t, on| {
            table(
                b,
                t,
                if on {
                    TableLaw::Exact
                } else {
                    TableLaw::NgspiceRounded
                },
            )
        }),
        (X::TableNgspiceRounded, |b, t, on| {
            table(
                b,
                t,
                if on {
                    TableLaw::NgspiceRounded
                } else {
                    TableLaw::Exact
                },
            )
        }),
        (X::TableSmoothed, |b, t, on| {
            table(
                b,
                t,
                if on {
                    TableLaw::Smoothed
                } else {
                    TableLaw::Exact
                },
            )
        }),
        (X::BehavTc, |b, t, on| {
            behav_el(b, t, OutKind::Voltage, on, false, false)
        }),
        (X::BehavResistance, |b, t, on| {
            behav_el(
                b,
                t,
                if on {
                    OutKind::Resistance
                } else {
                    OutKind::Voltage
                },
                false,
                false,
                false,
            )
        }),
        (X::BehavConductance, |b, t, on| {
            behav_el(
                b,
                t,
                if on {
                    OutKind::Conductance
                } else {
                    OutKind::Current
                },
                false,
                false,
                false,
            )
        }),
        (X::BehavCapacitance, |b, t, on| {
            behav_el(
                b,
                t,
                if on {
                    OutKind::Capacitance
                } else {
                    OutKind::Current
                },
                false,
                false,
                false,
            )
        }),
        (X::BehavCapacitanceCharge, |b, t, on| {
            behav_el(
                b,
                t,
                if on {
                    OutKind::CapacitanceCharge
                } else {
                    OutKind::Current
                },
                false,
                false,
                false,
            )
        }),
        (X::BehavInductance, |b, t, on| {
            behav_el(
                b,
                t,
                if on {
                    OutKind::Inductance
                } else {
                    OutKind::Voltage
                },
                false,
                false,
                false,
            )
        }),
        (X::BehavPower, |b, t, on| {
            behav_el(
                b,
                t,
                if on { OutKind::Power } else { OutKind::Current },
                false,
                false,
                false,
            )
        }),
        (X::LaplacePoleZero, |b, t, on| {
            laplace(
                b,
                t,
                if on {
                    LaplaceForm::PoleZero
                } else {
                    LaplaceForm::Polynomial
                },
                false,
            )
        }),
        (X::LaplaceExprInput, |b, t, on| {
            laplace(b, t, LaplaceForm::Polynomial, on)
        }),
        (X::FreqComplexLinear, |b, t, on| {
            freq(
                b,
                t,
                if on {
                    FreqLaw::ComplexLinear
                } else {
                    FreqLaw::DbLogFreq
                },
            )
        }),
        (X::FreqDbLogFreq, |b, t, on| {
            freq(
                b,
                t,
                if on {
                    FreqLaw::DbLogFreq
                } else {
                    FreqLaw::ComplexLinear
                },
            )
        }),
        (X::DelaySampled, |b, t, on| {
            let one = n(b, 1);
            let samples = opt(b, on);
            let law = if on {
                DelayLaw::Sampled
            } else {
                DelayLaw::Ideal
            };
            let h = b.head("e1", &["o", "0", "i", "0"], None, &[], ExprId::NONE);
            b.delay(
                t,
                h,
                Delay {
                    td: one,
                    gain: one,
                    samples,
                    law,
                },
                Span::NONE,
            );
        }),
        (X::SemiArea, |b, t, on| {
            let area = opt(b, on);
            semi_el(
                b,
                t,
                ElemKind::Bjt,
                &["c", "b", "e"],
                ExprId::NONE,
                spicerack_ir::semi::Semi { area, ..semi() },
            )
        }),
        (X::SemiInitial, |b, t, on| {
            let initial = if on {
                InitialState::On
            } else {
                InitialState::Unset
            };
            semi_el(
                b,
                t,
                ElemKind::Mosfet,
                &["d", "g", "s"],
                ExprId::NONE,
                spicerack_ir::semi::Semi { initial, ..semi() },
            )
        }),
        (X::DiodeThermal, |b, t, on| {
            let terms: &[&str] = if on { &["a", "k", "tj"] } else { &["a", "k"] };
            semi_el(b, t, ElemKind::Diode, terms, ExprId::NONE, semi())
        }),
        (X::SwitchInitial, |b, t, on| {
            switch(
                b,
                t,
                if on {
                    InitialState::Off
                } else {
                    InitialState::Unset
                },
                false,
            )
        }),
        (X::SwitchControlExpr, |b, t, on| {
            switch(b, t, InitialState::Unset, on)
        }),
        (X::LineFreqNl, |b, t, on| {
            lossless(b, t, if on { 1 } else { 0 })
        }),
        (X::LineLengthVel, |b, t, on| {
            lossless(b, t, if on { 2 } else { 0 })
        }),
        (X::LineLumps, |b, t, on| {
            let length = n(b, 1);
            let lumps = opt(b, on);
            let h = b.head("e1", &["a", "b", "c"], Some("urc"), &[], ExprId::NONE);
            b.line(
                t,
                ElemKind::RcLine,
                h,
                spicerack_ir::line::Line {
                    length,
                    lumps,
                    ..line()
                },
                Span::NONE,
            );
        }),
        (X::ModelBin, |b, t, on| model_el(b, t, on, false)),
        (X::ModelAko, |b, t, on| model_el(b, t, false, on)),
        (X::PulseSpice, |b, t, on| {
            pulse_law(
                b,
                t,
                if on {
                    PulseLaw::Spice
                } else {
                    PulseLaw::Vacask
                },
            )
        }),
        (X::PulseVacask, |b, t, on| {
            pulse_law(
                b,
                t,
                if on {
                    PulseLaw::Vacask
                } else {
                    PulseLaw::Spice
                },
            )
        }),
        (X::ExpTd2FromZero, |b, t, on| {
            exp_from(b, t, if on { TdFrom::Zero } else { TdFrom::Td1 })
        }),
        (X::ExpTd2FromTd1, |b, t, on| {
            exp_from(b, t, if on { TdFrom::Td1 } else { TdFrom::Zero })
        }),
        (X::SffmPreOffset, |b, t, on| {
            sffm_pre(b, t, if on { PreDelay::Offset } else { PreDelay::Zero })
        }),
        (X::SffmPreHold, |b, t, on| {
            sffm_pre(
                b,
                t,
                if on {
                    PreDelay::HoldDelayValue
                } else {
                    PreDelay::Zero
                },
            )
        }),
        (X::SffmPreZero, |b, t, on| {
            sffm_pre(b, t, if on { PreDelay::Zero } else { PreDelay::Offset })
        }),
        (X::DcGivenWithWave, |b, t, on| {
            let dc = n(b, 0);
            let wave = if on {
                wave_of(b, WaveKind::Pulse)
            } else {
                spicerack_ir::WaveRef::NONE
            };
            vsrc(
                b,
                t,
                ElemKind::VSource,
                ExprId::NONE,
                Source { wave, ..source(dc) },
            );
        }),
        (X::DcAtZeroPulse, |b, t, on| {
            at_zero(b, t, WaveKind::Pulse, on)
        }),
        (X::DcAtZeroSin, |b, t, on| at_zero(b, t, WaveKind::Sin, on)),
        (X::DcAtZeroExp, |b, t, on| at_zero(b, t, WaveKind::Exp, on)),
        (X::DcAtZeroPwl, |b, t, on| at_zero(b, t, WaveKind::Pwl, on)),
        (X::DcAtZeroSffm, |b, t, on| {
            at_zero(b, t, WaveKind::Sffm, on)
        }),
        (X::DcAtZeroAm, |b, t, on| at_zero(b, t, WaveKind::Am, on)),
        (X::DcAtZeroTrNoise, |b, t, on| {
            at_zero(b, t, WaveKind::TrNoise, on)
        }),
        (X::DcAtZeroTrRandom, |b, t, on| {
            at_zero(b, t, WaveKind::TrRandom, on)
        }),
        (X::DcAtZeroPattern, |b, t, on| {
            at_zero(b, t, WaveKind::Pattern, on)
        }),
        (X::ElementWithoutModel, |b, t, on| {
            let one = n(b, 1);
            let h = b.head(
                "r1",
                &["a", "b"],
                if on { None } else { Some("rm") },
                &[],
                ExprId::NONE,
            );
            b.passive(
                t,
                ElemKind::Resistor,
                h,
                Passive {
                    value: one,
                    ..Passive::PLAIN
                },
                Span::NONE,
            );
        }),
        (X::ModelWithoutLibrary, |b, t, on| {
            if !on {
                let lib = DeviceLibrary {
                    path: NameId::NONE,
                    family: ModelFamily::Resistor,
                };
                b.device_library(t, lib, Span::NONE);
            }
            let name = b.name("rm");
            b.model(
                t,
                model(name, ModelFamily::Resistor, Polarity::None),
                Span::NONE,
            );
        }),
        (X::BuiltinMaster, |b, t, on| {
            let dc = n(b, 0);
            let h = b.head(
                "v1",
                &["a", "0"],
                if on { Some("v") } else { None },
                &[],
                ExprId::NONE,
            );
            b.source(t, ElemKind::VSource, h, source(dc), Span::NONE);
        }),
        (X::NameWithoutLetter, |b, t, on| {
            let one = n(b, 1);
            let h = b.head(
                if on { "load" } else { "R1" },
                &["a", "b"],
                None,
                &[],
                ExprId::NONE,
            );
            b.passive(
                t,
                ElemKind::Resistor,
                h,
                Passive {
                    value: one,
                    ..Passive::PLAIN
                },
                Span::NONE,
            );
        }),
        (X::TcBaseTnom, |b, t, on| behav_tc(b, t, on, TcBase::Tnom)),
        (X::TcBaseFixed27, |b, t, on| {
            behav_tc(b, t, true, if on { TcBase::Fixed27 } else { TcBase::Tnom })
        }),
        (X::TcLawDirect, |b, t, on| {
            dynamic_cap(b, t, if on { TcLaw::Direct } else { TcLaw::Reciprocal })
        }),
        (X::TcLawReciprocal, |b, t, on| {
            dynamic_cap(b, t, if on { TcLaw::Reciprocal } else { TcLaw::Direct })
        }),
    ];

    #[test]
    fn every_circuit_extra_is_set_by_its_field_and_not_by_its_neighbour() {
        let covered: Vec<X> = CASES.iter().map(|c| c.0).collect();
        assert_eq!(
            covered,
            X::ALL.to_vec(),
            "one case per member, in declaration order"
        );
        for &(extra, case) in CASES {
            let f = Feature::Circuit(extra);
            assert!(
                used(case, true).contains(f),
                "{extra:?} not set by its field"
            );
            assert!(
                !used(case, false).contains(f),
                "{extra:?} set by its neighbour"
            );
        }
    }

    #[test]
    fn a_builtin_model_needs_no_library() {
        let set = used(
            |b, t, _| {
                let name = b.name("v");
                b.model(
                    t,
                    model(name, ModelFamily::SourceV, Polarity::None),
                    Span::NONE,
                );
            },
            true,
        );
        assert!(!set.contains(Feature::Circuit(X::ModelWithoutLibrary)));
    }

    #[test]
    fn kinds_are_features() {
        let set = used(
            |b, t, _| {
                let one = n(b, 1);
                let w = b.wave(sin(one));
                with_wave(b, t, w);
                let h = b.head("u1", &[], Some("cell"), &[], ExprId::NONE);
                b.instance(t, ElemKind::Unresolved, h, Span::NONE);
            },
            true,
        );
        assert!(set.contains(Feature::Elem(ElemKind::VSource)));
        assert!(set.contains(Feature::Elem(ElemKind::Unresolved)));
        assert!(set.contains(Feature::Wave(WaveKind::Sin)));
        assert!(!set.contains(Feature::Wave(WaveKind::Pulse)));
        assert!(!set.contains(Feature::Elem(ElemKind::Resistor)));
    }

    /// A15 K5 (R32): a model sets the feature of its family, and only that one.
    #[test]
    fn a_model_sets_its_family() {
        let with = used(
            |b, t, _| {
                let name = b.name("nch");
                b.model(t, model(name, ModelFamily::Bsim4, Polarity::N), Span::NONE);
            },
            true,
        );
        assert!(with.contains(Feature::Family(ModelFamily::Bsim4)));
        assert!(!with.contains(Feature::Family(ModelFamily::Mos1)));
        let without = used(|b, t, on| at_zero(b, t, WaveKind::Pulse, on), false);
        assert!(!without.contains(Feature::Family(ModelFamily::Bsim4)));
    }

    /// ng-elements' finding (A13): a voltage `Behav` may be an E card and a current one a G
    /// card (ngspice reads `E … vol=`, `G … cur=` as B sources), and a port a V card; such a
    /// name is no `NameWithoutLetter`, the other controlled letter is.
    #[test]
    fn a_behavioural_source_takes_b_or_its_controlled_letter() {
        let named = |name: &str, out: OutKind| {
            let mut d = Design::new();
            let t = deck(&mut d);
            let mut b = d.builder();
            let e = n(&mut b, 1);
            let h = b.head(name, &["o", "0"], None, &[], ExprId::NONE);
            b.behav(t, ElemKind::Behav, h, behav(e, out), Span::NONE);
            features_used(&d).contains(Feature::Circuit(X::NameWithoutLetter))
        };
        assert!(!named("B1", OutKind::Voltage));
        assert!(!named("E4", OutKind::Voltage));
        assert!(!named("G3", OutKind::Current));
        assert!(named("G3", OutKind::Voltage));
        assert!(named("E4", OutKind::Current));
        let port = used(
            |b, t, _| {
                let one = n(b, 1);
                let p = b.detail(port(one, one));
                let h = b.head("V1", &["a", "0"], None, &[], ExprId::NONE);
                b.source(
                    t,
                    ElemKind::Port,
                    h,
                    Source {
                        port: p,
                        ..source(one)
                    },
                    Span::NONE,
                );
            },
            true,
        );
        assert!(!port.contains(Feature::Circuit(X::NameWithoutLetter)));
    }
}
