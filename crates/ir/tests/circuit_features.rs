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
        Am, AmModTime, Exp, GivenZero, Pattern, PreDelay, PulseLaw, Pwl, PwlJoin, PwlRepeat, Sffm,
        SffmLaw, TdFrom, TrNoise, TrRandom, TrRandomKind,
    };
    use spicerack_ir::{
        Builder, CircuitExtra as X, Design, DeviceLibrary, ElemKind, ExprId, Feature, FeatureSet,
        Law, Laws, ModelFamily, NO_ROW, NameId, Polarity, Ref, ScopeId, Span, WaveKind,
        features_used,
    };
    use spicerack_ir::{DefKind, LoadKind};

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
            given_zero: spicerack_ir::wave::GivenZero::AsGiven,
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
            given_zero: spicerack_ir::wave::GivenZero::AsGiven,
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

    /// A source with a pulse whose zero fields follow `rule`.
    fn zero_rule(b: &mut Builder<'_>, top: ScopeId, rule: GivenZero) {
        let one = n(b, 1);
        let w = b.wave(spicerack_ir::wave::Pulse {
            given_zero: rule,
            ..pulse(one)
        });
        with_wave(b, top, w);
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
            given_zero: spicerack_ir::wave::GivenZero::AsGiven,
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
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
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
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
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
        // R58's evidence: ngspice holds `m` on a current output (parallel copies) and lacks it on
        // a voltage output (its writer's admit refuses `m` on E, H, a voltage POLY, table or B)
        (X::MultControlledCurrent, |b, t, on| {
            let m = opt(b, on);
            let g = n(b, 1);
            let h = b.head("g1", &["a", "0", "c", "0"], None, &[], m);
            b.controlled(
                t,
                ElemKind::Vccs,
                h,
                spicerack_ir::controlled::Controlled {
                    gain: g,
                    sense: NameId::NONE,
                },
                Span::NONE,
            );
        }),
        (X::MultControlledVoltage, |b, t, on| {
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
        (X::BehavUntypedParam, |b, t, on| {
            // A33: the bare read; its neighbour the read under `to_real`
            let one = n(b, 1);
            b.param(t, "p", one, Span::NONE);
            let p = b.param_ref("p");
            let read = if on {
                p
            } else {
                b.call(spicerack_ir::Func::ToReal, &[p])
            };
            let h = b.head("b1", &["a", "0"], None, &[], ExprId::NONE);
            b.behav(
                t,
                ElemKind::Behav,
                h,
                behav(read, OutKind::Voltage),
                Span::NONE,
            );
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
        (X::AmModOffset, |b, t, on| {
            // A31: an offset of 2; its neighbour the literal 1 (VACASK's only offset)
            let mut r = am(b, AmModTime::Delay, PreDelay::Zero);
            if on {
                r.mod_offset = n(b, 2);
            }
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
        (X::PwlPeriodJump, |b, t, on| {
            // A24: VACASK's periodic PWL only extrapolates (vc-sources W2202)
            let r = pwl(b);
            let at = n(b, 1);
            let repeat = if on {
                PwlRepeat::Period
            } else {
                PwlRepeat::From
            };
            let w = b.wave(Pwl {
                repeat,
                repeat_at: at,
                join: PwlJoin::Jump,
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
        // the A24 ruling: one feature per member; each member is the other's neighbour
        (X::SemiInitialOff, |b, t, on| {
            let initial = if on {
                InitialState::Off
            } else {
                InitialState::On
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
        (X::SemiInitialOn, |b, t, on| {
            let initial = if on {
                InitialState::On
            } else {
                InitialState::Off
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
        (X::GivenZeroAsGiven, |b, t, on| {
            zero_rule(
                b,
                t,
                if on {
                    GivenZero::AsGiven
                } else {
                    GivenZero::SpiceDefault
                },
            )
        }),
        (X::GivenZeroSpiceDefault, |b, t, on| {
            zero_rule(
                b,
                t,
                if on {
                    GivenZero::SpiceDefault
                } else {
                    GivenZero::AsGiven
                },
            )
        }),
        // R90: each law's member at a row carrying its point under the stated law; the
        // neighbour is the row where the two builds agree (`laws.rs`, each variant's doc).
        // R86: a BJT of area 2 against area 1
        (X::BjtBcSatAreaTimesAreab, |b, t, on| {
            bjt_law(b, t, on, Law::BcSatAreaTimesAreab)
        }),
        (X::BjtBcSatAreabOnly, |b, t, on| {
            bjt_law(b, t, on, Law::BcSatAreabOnly)
        }),
        // R88: a Gummel-Poon model giving `iss` with `ns = 2` against `ns = 1`
        (X::ModelSubSatLastFactor, |b, t, on| {
            substrate_law(b, t, on, Law::SubSatLastFactor)
        }),
        (X::ModelSubSatEmissionNs, |b, t, on| {
            substrate_law(b, t, on, Law::SubSatEmissionNs)
        }),
        // R91 F20: a level-3 diode from `w` and `l` with `m = 2` against `m = 1`
        (X::DiodeWlMultTwice, |b, t, on| {
            diode_mult(b, t, on, Law::DiodeWlMultTwice)
        }),
        (X::DiodeWlMultOnce, |b, t, on| {
            diode_mult(b, t, on, Law::DiodeWlMultOnce)
        }),
        // R91 F22: a diode of `pj = 1` against `pj = 0` on a model with `cjsw`
        (X::DiodeSidewallBottomF1, |b, t, on| {
            diode_sidewall(b, t, on, Law::DiodeSidewallBottomF1)
        }),
        (X::DiodeSidewallOwnF1, |b, t, on| {
            diode_sidewall(b, t, on, Law::DiodeSidewallOwnF1)
        }),
        // R92 F21: a diode model with `wm` and `xom` lacking `xoi` against one giving both
        (X::DiodeXomAbsent1e4, |b, t, on| {
            xom(b, t, on, Law::DiodeXomAbsent1e4)
        }),
        (X::DiodeXomAbsent1e14, |b, t, on| {
            xom(b, t, on, Law::DiodeXomAbsent1e14)
        }),
        // R92 F24: a VDMOS model lacking `kp` against one giving it
        (X::VdmosKpAbsent35, |b, t, on| {
            kp(b, t, on, Law::VdmosKpAbsent35)
        }),
        (X::VdmosKpAbsent20, |b, t, on| {
            kp(b, t, on, Law::VdmosKpAbsent20)
        }),
        // R92 F25: an n-channel VDMOS with `tcvth = 1` against `tcvth = 0`
        (X::VdmosTcvthLowers, |b, t, on| {
            tcvth(b, t, on, Law::VdmosTcvthLowers)
        }),
        (X::VdmosTcvthRaises, |b, t, on| {
            tcvth(b, t, on, Law::VdmosTcvthRaises)
        }),
        // R92 F26: a VDMOS with `cjo` and `vj = 6` (fc·vj 3) against `vj = 1` (0.5)
        (X::VdmosClampByN, |b, t, on| {
            clamp(b, t, on, Law::VdmosClampByN)
        }),
        (X::VdmosClampByFc, |b, t, on| {
            clamp(b, t, on, Law::VdmosClampByFc)
        }),
        // R94 F27, F28: a MOSFET of the family in a design with no analysis against one whose
        // design holds an operating point only
        (X::MosSmallSignalHand, |b, t, on| {
            mos_small(b, t, on, Law::MosSmallSignalHand, ModelFamily::Mos2)
        }),
        (X::MosSmallSignalExact, |b, t, on| {
            mos_small(b, t, on, Law::MosSmallSignalExact, ModelFamily::Mos3)
        }),
        (X::Mos6SmallSignalAbsent, |b, t, on| {
            mos_small(b, t, on, Law::Mos6SmallSignalAbsent, ModelFamily::Mos6)
        }),
        (X::Mos6SmallSignalExact, |b, t, on| {
            mos_small(b, t, on, Law::Mos6SmallSignalExact, ModelFamily::Mos6)
        }),
        // R94a F29: a BJT on a Gummel-Poon model with `irb = 1` against `irb = 0`
        (X::BjtIrbSmallSignalFrozen, |b, t, on| {
            irb(b, t, on, Law::BjtIrbSmallSignalFrozen)
        }),
        (X::BjtIrbSmallSignalExact, |b, t, on| {
            irb(b, t, on, Law::BjtIrbSmallSignalExact)
        }),
        // R95 F30: a VDMOS of m = 2 against m = 1, no analysis
        (X::VdmosMultChargesOff, |b, t, on| {
            vdmos_mult(b, t, on, Law::VdmosMultChargesOff)
        }),
        (X::VdmosMultChargesCopies, |b, t, on| {
            vdmos_mult(b, t, on, Law::VdmosMultChargesCopies)
        }),
        // R95 F31: a diode on a model of `isr = 1` against `isr = 0`, beside an operating point
        (X::DiodeIsrReverseNone, |b, t, on| {
            diode_isr(b, t, on, true, Law::DiodeIsrReverseNone)
        }),
        (X::DiodeIsrReversePresent, |b, t, on| {
            diode_isr(b, t, on, true, Law::DiodeIsrReversePresent)
        }),
        // R96: the same diode of `isr = 1`, no analysis, against one beside an operating point
        (X::DiodeIsrSmallSignalHand, |b, t, on| {
            diode_isr(b, t, true, !on, Law::DiodeIsrSmallSignalHand)
        }),
        (X::DiodeIsrSmallSignalExact, |b, t, on| {
            diode_isr(b, t, true, !on, Law::DiodeIsrSmallSignalExact)
        }),
        // R96: a diode model of `ttt1 = 1` against `ttt1 = 0`
        (X::DiodeTtFactorUnclamped, |b, t, on| {
            ttt(b, t, on, Law::DiodeTtFactorUnclamped)
        }),
        (X::DiodeTtFactorClamped, |b, t, on| {
            ttt(b, t, on, Law::DiodeTtFactorClamped)
        }),
    ];

    fn vdmos_mult(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let one = n(b, 1);
        law_model(b, t, law, ModelFamily::Vdmos, &[("kp", one)]);
        let name = b.design().find_name("m");
        let def = b.design().lookup(t, DefKind::Model, name);
        let mult = n(b, if on { 2 } else { 1 });
        let mut h = b.head("m1", &["d", "g", "s"], Some("m"), &[], mult);
        h.model = Ref { name, def };
        b.semi(t, ElemKind::Mosfet, h, semi(), Span::NONE);
    }

    fn diode_isr(b: &mut Builder<'_>, t: ScopeId, on: bool, op: bool, law: Law) {
        let value = n(b, if on { 1 } else { 0 });
        law_model(b, t, law, ModelFamily::Diode, &[("isr", value)]);
        let name = b.design().find_name("m");
        let def = b.design().lookup(t, DefKind::Model, name);
        let mut h = b.head("d1", &["a", "c"], Some("m"), &[], ExprId::NONE);
        h.model = Ref { name, def };
        b.semi(t, ElemKind::Diode, h, semi(), Span::NONE);
        if op {
            b.op(t, None, Span::NONE);
        }
    }

    fn ttt(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let value = n(b, if on { 1 } else { 0 });
        law_model(b, t, law, ModelFamily::Diode, &[("ttt1", value)]);
    }

    /// A model `m` of `family` (n polarity where the family has one) with `params`, under `law`.
    fn law_model(
        b: &mut Builder<'_>,
        t: ScopeId,
        law: Law,
        family: ModelFamily,
        params: &[(&str, ExprId)],
    ) {
        b.set_laws(Laws::of(&[law]));
        let name = b.name("m");
        let list = if params.is_empty() {
            spicerack_ir::List::EMPTY
        } else {
            b.params(params)
        };
        let polarity = match family {
            ModelFamily::Diode | ModelFamily::DiodeLevel3 => Polarity::None,
            _ => Polarity::N,
        };
        let row = spicerack_ir::Model {
            params: list,
            ..model(name, family, polarity)
        };
        b.model(t, row, Span::NONE);
    }

    fn xom(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let one = n(b, 1);
        // one of the two absent is enough
        let given: &[(&str, ExprId)] = if on {
            &[("wm", one), ("xom", one)]
        } else {
            &[("wm", one), ("xom", one), ("xoi", one)]
        };
        law_model(b, t, law, ModelFamily::Diode, given);
    }

    fn kp(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let one = n(b, 1);
        let given: &[(&str, ExprId)] = if on { &[] } else { &[("kp", one)] };
        law_model(b, t, law, ModelFamily::Vdmos, given);
    }

    fn tcvth(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let (one, tc) = (n(b, 1), n(b, if on { 1 } else { 0 }));
        law_model(b, t, law, ModelFamily::Vdmos, &[("kp", one), ("tcvth", tc)]);
    }

    fn clamp(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let (one, vj) = (n(b, 1), n(b, if on { 6 } else { 1 }));
        law_model(
            b,
            t,
            law,
            ModelFamily::Vdmos,
            &[("kp", one), ("cjo", one), ("vj", vj)],
        );
    }

    fn mos_small(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law, family: ModelFamily) {
        law_model(b, t, law, family, &[]);
        let name = b.design().find_name("m");
        let def = b.design().lookup(t, DefKind::Model, name);
        let mut h = b.head("m1", &["d", "g", "s", "b"], Some("m"), &[], ExprId::NONE);
        h.model = Ref { name, def };
        b.semi(t, ElemKind::Mosfet, h, semi(), Span::NONE);
        if !on {
            b.op(t, None, Span::NONE);
        }
    }

    fn irb(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        let value = n(b, if on { 1 } else { 0 });
        law_model(b, t, law, ModelFamily::GummelPoon, &[("irb", value)]);
        let name = b.design().find_name("m");
        let def = b.design().lookup(t, DefKind::Model, name);
        let mut h = b.head("q1", &["c", "b", "e"], Some("m"), &[], ExprId::NONE);
        h.model = Ref { name, def };
        b.semi(t, ElemKind::Bjt, h, semi(), Span::NONE);
    }

    fn substrate_law(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        b.set_laws(Laws::of(&[law]));
        let iss = n(b, 1);
        let ns = n(b, if on { 2 } else { 1 });
        let params = b.params(&[("iss", iss), ("ns", ns)]);
        let name = b.name("m");
        let row = spicerack_ir::Model {
            params,
            ..model(name, ModelFamily::GummelPoon, Polarity::N)
        };
        b.model(t, row, Span::NONE);
    }

    fn bjt_law(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        b.set_laws(Laws::of(&[law]));
        let name = b.name("m");
        b.model(
            t,
            model(name, ModelFamily::GummelPoon, Polarity::N),
            Span::NONE,
        );
        let def = b.design().lookup(t, DefKind::Model, name);
        let area = n(b, if on { 2 } else { 1 });
        let mut h = b.head("q1", &["c", "b", "e"], Some("m"), &[], ExprId::NONE);
        h.model = Ref { name, def };
        let row = spicerack_ir::semi::Semi { area, ..semi() };
        b.semi(t, ElemKind::Bjt, h, row, Span::NONE);
    }

    /// A diode `d1` naming model `m` of `family` (with `model_params`), bound.
    fn diode(
        b: &mut Builder<'_>,
        t: ScopeId,
        family: ModelFamily,
        model_params: &[(&str, ExprId)],
        params: &[(&str, ExprId)],
        mult: ExprId,
    ) {
        let name = b.name("m");
        let list = if model_params.is_empty() {
            spicerack_ir::List::EMPTY
        } else {
            b.params(model_params)
        };
        let row = spicerack_ir::Model {
            params: list,
            ..model(name, family, Polarity::None)
        };
        b.model(t, row, Span::NONE);
        let def = b.design().lookup(t, DefKind::Model, name);
        let mut h = b.head("d1", &["a", "c"], Some("m"), params, mult);
        h.model = Ref { name, def };
        b.semi(t, ElemKind::Diode, h, semi(), Span::NONE);
    }

    fn diode_mult(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        b.set_laws(Laws::of(&[law]));
        let (w, l) = (n(b, 1), n(b, 1));
        let m = n(b, if on { 2 } else { 1 });
        diode(
            b,
            t,
            ModelFamily::DiodeLevel3,
            &[],
            &[("w", w), ("l", l)],
            m,
        );
    }

    fn diode_sidewall(b: &mut Builder<'_>, t: ScopeId, on: bool, law: Law) {
        b.set_laws(Laws::of(&[law]));
        let cjsw = n(b, 1);
        let pj = n(b, if on { 1 } else { 0 });
        diode(
            b,
            t,
            ModelFamily::Diode,
            &[("cjsw", cjsw)],
            &[("pj", pj)],
            ExprId::NONE,
        );
    }

    /// R90: a carrier reports nothing while no law is stated (a built design states none), and
    /// an unresolved model carries the stated law.
    #[test]
    fn a_law_is_reported_only_where_stated() {
        let law = |x: Law| Feature::Circuit(x.member());
        let laws = CASES
            .iter()
            .filter(|c| Law::ALL.iter().any(|l| l.member() == c.0));
        for &(x, case) in laws {
            let mut d = Design::new();
            let top = deck(&mut d);
            case(&mut d.builder(), top, true);
            assert!(features_used(&d).contains(Feature::Circuit(x)));
            d.builder().set_laws(Laws::EMPTY);
            assert!(
                !features_used(&d).contains(Feature::Circuit(x)),
                "{x:?} reported unstated"
            );
        }
        let unresolved = used(
            |b, t, _| {
                b.set_laws(Laws::of(&[Law::BcSatAreabOnly, Law::DiodeSidewallOwnF1]));
                let (area, pj) = (n(b, 2), n(b, 1));
                let h = b.head("q1", &["c", "b", "e"], Some("nomodel"), &[], ExprId::NONE);
                let row = spicerack_ir::semi::Semi { area, ..semi() };
                b.semi(t, ElemKind::Bjt, h, row, Span::NONE);
                let h = b.head(
                    "d1",
                    &["a", "c"],
                    Some("nomodel"),
                    &[("pj", pj)],
                    ExprId::NONE,
                );
                b.semi(t, ElemKind::Diode, h, semi(), Span::NONE);
            },
            true,
        );
        let got: Vec<Law> = Law::ALL
            .into_iter()
            .filter(|&x| unresolved.contains(law(x)))
            .collect();
        assert_eq!(got, [Law::BcSatAreabOnly, Law::DiodeSidewallOwnF1]);
        // R94: a linearised analysis beside an operating point keeps the small-signal carrier
        let linearised = used(
            |b, t, _| {
                mos_small(b, t, false, Law::MosSmallSignalHand, ModelFamily::Mos9);
                b.dc_inc(t, None, Span::NONE);
            },
            true,
        );
        assert!(linearised.contains(law(Law::MosSmallSignalHand)));
        // R96: a diode whose model's `isr` is literally 0 carries no `isr` small-signal point
        let no_isr = used(
            |b, t, _| diode_isr(b, t, false, false, Law::DiodeIsrSmallSignalHand),
            true,
        );
        assert!(!no_isr.contains(law(Law::DiodeIsrSmallSignalHand)));
        // R95a: F30 is the charges at m != 1, carried beside a transient (which uses charges),
        // not beside operating points alone
        let tran = used(
            |b, t, _| {
                vdmos_mult(b, t, true, Law::VdmosMultChargesOff);
                let (stop, step) = (b.num(1e-6), b.num(1e-9));
                let row = spicerack_ir::tran::Tran {
                    stop,
                    record_start: ExprId::NONE,
                    print_step: step,
                    initial_step: ExprId::NONE,
                    noise_fmax: ExprId::NONE,
                    noise_fmin: ExprId::NONE,
                    noise_seed: ExprId::NONE,
                    segments: spicerack_ir::List::EMPTY,
                    ceiling: spicerack_ir::tran::StepCeiling::Rule(
                        spicerack_ir::tran::StepCap::None,
                    ),
                    uic: false,
                    startup: false,
                };
                b.analysis(t, None, row, Span::NONE);
            },
            true,
        );
        assert!(tran.contains(law(Law::VdmosMultChargesOff)));
        let op = used(
            |b, t, _| {
                vdmos_mult(b, t, true, Law::VdmosMultChargesOff);
                b.op(t, None, Span::NONE);
            },
            true,
        );
        assert!(!op.contains(law(Law::VdmosMultChargesOff)));
    }

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

    /// A17: the zero rule of an AM and a TRRANDOM, and a charge-defined capacitor's law, are
    /// features as on the other rows.
    #[test]
    fn am_trrandom_and_charge_laws_are_features() {
        let am = used(
            |b, t, _| {
                let r = am(b, AmModTime::Delay, PreDelay::Zero);
                let w = b.wave(Am {
                    given_zero: GivenZero::SpiceDefault,
                    ..r
                });
                with_wave(b, t, w);
            },
            true,
        );
        assert!(am.contains(Feature::Circuit(X::GivenZeroSpiceDefault)));
        let tr = used(
            |b, t, _| {
                let v = n(b, 1);
                let w = b.wave(TrRandom {
                    ts: v,
                    td: v,
                    p1: v,
                    p2: v,
                    kind: TrRandomKind::Gauss,
                    given_zero: GivenZero::AsGiven,
                });
                with_wave(b, t, w);
            },
            true,
        );
        assert!(tr.contains(Feature::Circuit(X::GivenZeroAsGiven)));
        let charge = |law: TcLaw| {
            let mut d = Design::new();
            let top = deck(&mut d);
            let mut b = d.builder();
            let e = n(&mut b, 1);
            let tc1 = n(&mut b, 1);
            let h = b.head("c1", &["a", "0"], None, &[], ExprId::NONE);
            let row = spicerack_ir::controlled::Behav {
                tc1,
                tc_law: law,
                ..behav(e, OutKind::Charge)
            };
            b.behav(top, ElemKind::ChargeCapacitor, h, row, Span::NONE);
            features_used(&d)
        };
        assert!(charge(TcLaw::Reciprocal).contains(Feature::Circuit(X::TcLawReciprocal)));
        assert!(charge(TcLaw::Direct).contains(Feature::Circuit(X::TcLawDirect)));
        assert!(!charge(TcLaw::Direct).contains(Feature::Circuit(X::TcLawReciprocal)));
    }

    /// A20: a lossless line naming a model is `BuiltinMaster` (ngspice's T card names none);
    /// the ideal line needs a library, unlike the other builtins.
    #[test]
    fn the_ideal_line_names_a_model_and_needs_a_library() {
        let line_with = |master: Option<&str>| {
            let mut d = Design::new();
            let top = deck(&mut d);
            let mut b = d.builder();
            if master.is_some() {
                let name = b.name("tl");
                b.model(
                    top,
                    model(name, ModelFamily::TlineIdeal, Polarity::None),
                    Span::NONE,
                );
            }
            let one = n(&mut b, 1);
            let h = b.head("t1", &["a", "0", "b", "0"], master, &[], ExprId::NONE);
            b.line(
                top,
                ElemKind::LosslessLine,
                h,
                spicerack_ir::line::Line {
                    z0: one,
                    td: one,
                    ..line()
                },
                Span::NONE,
            );
            features_used(&d)
        };
        let with = line_with(Some("tl"));
        assert!(with.contains(Feature::Circuit(X::BuiltinMaster)));
        assert!(with.contains(Feature::Circuit(X::ModelWithoutLibrary)));
        assert!(!line_with(None).contains(Feature::Circuit(X::BuiltinMaster)));
    }

    /// R44 (A24): a model of a user module takes its code from a `Load`, never from a family's
    /// library, so it needs none; a resistor model with no library still does.
    #[test]
    fn a_module_model_needs_no_library() {
        let with = |family: ModelFamily| {
            let mut d = Design::new();
            let top = deck(&mut d);
            let mut b = d.builder();
            let load = b.load(top, LoadKind::VerilogA, "x.va", Span::NONE);
            let module = b.name("x");
            let def = b.define(DefKind::Module, module, top, load);
            let name = b.name("m");
            let mut row = model(name, family, Polarity::None);
            match family {
                ModelFamily::Module => row.module = Ref { name: module, def },
                ModelFamily::CodeModel | ModelFamily::Other => row.word = b.name("xcode"),
                _ => {}
            }
            b.model(top, row, Span::NONE);
            let h = b.head("n1", &["a", "0"], Some("m"), &[], ExprId::NONE);
            b.instance(top, ElemKind::ModuleModel, h, Span::NONE);
            features_used(&d).contains(Feature::Circuit(X::ModelWithoutLibrary))
        };
        assert!(!with(ModelFamily::Module));
        assert!(!with(ModelFamily::CodeModel));
        assert!(!with(ModelFamily::Other));
        assert!(with(ModelFamily::Resistor));
    }

    /// A25: a library of family F states F, as a model of F does.
    #[test]
    fn a_device_library_uses_its_family() {
        let mut d = Design::new();
        let top = deck(&mut d);
        d.builder().device_library(
            top,
            DeviceLibrary {
                path: NameId::NONE,
                family: ModelFamily::Bsim4,
            },
            Span::NONE,
        );
        assert!(features_used(&d).contains(Feature::Family(ModelFamily::Bsim4)));
    }

    /// A behavioural source can name no model (`master_kind` is `None`), so naming none states
    /// nothing a target lacks (vc-devices: every VACASK `b` source was refused); a resistor
    /// with no model still states `ElementWithoutModel`.
    #[test]
    fn a_masterless_kind_needs_no_model() {
        let with = |resistor: bool| {
            let mut d = Design::new();
            let top = deck(&mut d);
            let mut b = d.builder();
            if resistor {
                let one = n(&mut b, 1);
                let h = b.head("r1", &["a", "0"], None, &[], ExprId::NONE);
                b.passive(
                    top,
                    ElemKind::Resistor,
                    h,
                    Passive {
                        value: one,
                        ..Passive::PLAIN
                    },
                    Span::NONE,
                );
            } else {
                behav_el(&mut b, top, OutKind::Voltage, false, false, false);
            }
            features_used(&d).contains(Feature::Circuit(X::ElementWithoutModel))
        };
        assert!(!with(false), "a behavioural source");
        assert!(with(true), "a resistor");
    }

    /// A33 (R56): a behavioural source whose expression reads a parameter outside `ToReal` and
    /// `ToInt` states `BehavUntypedParam`; neighbours: the read under `to_real`, and a resistor
    /// valued by the bare read (the feature is the behavioural source's own).
    #[test]
    fn an_untyped_parameter_read_in_a_behavioural_source() {
        let used = |case: fn(&mut Builder<'_>, ScopeId)| {
            let mut d = Design::new();
            let top = deck(&mut d);
            case(&mut d.builder(), top);
            features_used(&d).contains(Feature::Circuit(X::BehavUntypedParam))
        };
        fn behav_reading(b: &mut Builder<'_>, top: ScopeId, typed: bool) {
            let one = n(b, 1);
            b.param(top, "p", one, Span::NONE);
            let p = b.param_ref("p");
            let read = if typed {
                b.call(spicerack_ir::Func::ToReal, &[p])
            } else {
                p
            };
            let v = b.v("a", None);
            let e = b.binary(spicerack_ir::BinOp::Mul, read, v);
            let h = b.head("b1", &["a", "0"], None, &[], ExprId::NONE);
            b.behav(
                top,
                ElemKind::Behav,
                h,
                behav(e, OutKind::Voltage),
                Span::NONE,
            );
        }
        assert!(used(|b, t| behav_reading(b, t, false)));
        assert!(!used(|b, t| behav_reading(b, t, true)));
        assert!(!used(|b, top| {
            let one = n(b, 1);
            b.param(top, "p", one, Span::NONE);
            let p = b.param_ref("p");
            let h = b.head("r1", &["a", "0"], None, &[], ExprId::NONE);
            b.passive(
                top,
                ElemKind::Resistor,
                h,
                Passive {
                    value: p,
                    ..Passive::PLAIN
                },
                Span::NONE,
            );
        }));
    }

    /// A31 as ruled after vc-sources' gap: `AmModOffset` fires for every modulating offset but
    /// the real literal 1 (VACASK's reader fills that one and its writer spells none): `1`
    /// (Plain) no, `1i` (Int) yes, `1.5` yes.
    #[test]
    fn the_am_offset_is_free_only_as_the_real_literal_one() {
        use spicerack_ir::{Literal, NumForm};
        let with = |offset: Literal| {
            let mut d = Design::new();
            let top = deck(&mut d);
            let mut b = d.builder();
            let mut r = am(&mut b, AmModTime::Delay, PreDelay::Zero);
            r.mod_offset = b.literal(offset);
            let w = b.wave(r);
            with_wave(&mut b, top, w);
            features_used(&d).contains(Feature::Circuit(X::AmModOffset))
        };
        let lit = |mantissa: i64, frac: u8, form: NumForm| Literal {
            mantissa,
            frac,
            exp10: 0,
            form,
        };
        assert!(!with(lit(1, 0, NumForm::Plain)), "1");
        assert!(with(lit(1, 0, NumForm::Int)), "1i");
        assert!(with(lit(15, 1, NumForm::Plain)), "1.5");
    }

    /// The split follows the output quantity, not the kind: a behavioural source's `mult` is
    /// the voltage member on a voltage output and the current member on any other (ngspice's
    /// admit refuses `m` on a voltage B only). Oracle: the ruling's evidence, case by case.
    #[test]
    fn a_behavioural_multiplier_follows_its_output() {
        let cases = [
            (OutKind::Voltage, X::MultControlledVoltage),
            (OutKind::Current, X::MultControlledCurrent),
            (OutKind::Resistance, X::MultControlledCurrent),
        ];
        for (out, want) in cases {
            let mut d = Design::new();
            let t = deck(&mut d);
            let mut b = d.builder();
            let (e, m) = (n(&mut b, 1), n(&mut b, 2));
            let h = b.head("b1", &["a", "0"], None, &[], m);
            b.behav(t, ElemKind::Behav, h, behav(e, out), Span::NONE);
            let used = features_used(&d);
            let other = if want == X::MultControlledVoltage {
                X::MultControlledCurrent
            } else {
                X::MultControlledVoltage
            };
            assert!(used.contains(Feature::Circuit(want)), "{out:?}");
            assert!(!used.contains(Feature::Circuit(other)), "{out:?}");
        }
    }
}
