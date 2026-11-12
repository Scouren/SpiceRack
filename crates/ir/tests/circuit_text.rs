//! IR text of circuit rows. Oracle: spec examples. Each case is hand-written text in the forms
//! the circuit files' headers document; `parse` must build exactly the rows the test builds by
//! hand (same calls in text order, then `link`, as every builder does), with no diagnostic; and
//! `dump` of the hand-built design must be that text, byte for byte (canonical form).
//!
//! Cites: `rewrite/IR-IMPL.md` §2 ("IR text"); `rewrite/IR-KERNEL.md` §9; the IR-text forms in
//! the headers of `element.rs` and `model.rs`.
//!
//! Contents: tests only.

#[path = "circuit_common.rs"]
mod circuit_common;

#[cfg(test)]
mod tests {

    use super::circuit_common::{behav, deck, k, model, n, port, semi, source};
    use spicerack_ir::controlled::{Controlled, OutKind};
    use spicerack_ir::passive::{IcMode, Parasitics, Passive};
    use spicerack_ir::semi::InitialState;
    use spicerack_ir::{
        BinOp, Builder, Design, Diags, ElemHead, ElemKind, ExprId, FileId, List, Literal,
        ModelFamily, NumForm, Polarity, Ref, ScopeId, Span, dump, link, parse,
    };

    /// Parses `text`, builds the expected design with `build` (then `link`), and compares both
    /// ways.
    fn same(text: &str, build: impl FnOnce(&mut Builder<'_>, ScopeId)) {
        let mut parsed = Design::new();
        let mut diags = Diags::new();
        parse(text, &mut parsed.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());

        let mut want = Design::new();
        let top = deck(&mut want);
        build(&mut want.builder(), top);
        let mut link_diags = Diags::new();
        link(&mut want, &mut link_diags);

        assert!(parsed.eq_ignoring_spans(&want), "parse of:\n{text}");
        let mut out = String::new();
        dump(&want, FileId(0), &mut out);
        assert_eq!(out, text);
    }

    fn lit(b: &mut Builder<'_>, mantissa: i64, frac: u8, exp10: i16, form: NumForm) -> ExprId {
        b.literal(Literal {
            mantissa,
            frac,
            exp10,
            form,
        })
    }

    fn head(b: &mut Builder<'_>, name: &str, terms: &[&str]) -> ElemHead {
        b.head(name, terms, None, &[], ExprId::NONE)
    }

    #[test]
    fn resistor() {
        same(
            "file \"t.sp\" deck\n  element resistor r1 (a b) value=1k\n",
            |b, top| {
                let h = head(b, "r1", &["a", "b"]);
                let v = k(b, 1, 3);
                b.passive(
                    top,
                    ElemKind::Resistor,
                    h,
                    Passive {
                        value: v,
                        ..Passive::PLAIN
                    },
                    Span::NONE,
                );
            },
        );
    }

    #[test]
    fn capacitor_with_ic_parasitics_and_noise() {
        let text = "file \"t.sp\" deck\n  element capacitor c1 (a 0) value=1p ic=0.5 \
                parasitics=(rser=1m cpar=1) ic_mode=in_op\n  element resistor r1 (a b) value=1k noisy=false\n";
        same(text, |b, top| {
            let h = head(b, "c1", &["a", "0"]);
            let value = k(b, 1, -12);
            let ic = lit(b, 5, 1, 0, NumForm::Plain);
            let rser = k(b, 1, -3);
            let cpar = n(b, 1);
            let par = b.detail(Parasitics {
                rser,
                lser: ExprId::NONE,
                rpar: ExprId::NONE,
                cpar,
                rlshunt: ExprId::NONE,
            });
            let row = Passive {
                value,
                ic,
                parasitics: par,
                ic_mode: IcMode::InOp,
                ..Passive::PLAIN
            };
            b.passive(top, ElemKind::Capacitor, h, row, Span::NONE);
            // A19: a noiseless element is a resistor (or circuit-valued)
            let h = head(b, "r1", &["a", "b"]);
            let value = k(b, 1, 3);
            let row = Passive {
                value,
                noisy: false,
                ..Passive::PLAIN
            };
            b.passive(top, ElemKind::Resistor, h, row, Span::NONE);
        });
    }

    #[test]
    fn voltage_source_with_ac_and_a_counted_pulse() {
        let text = "file \"t.sp\" deck\n  element vsource v1 (in 0) dc=0 ac_mag=1 ac_phase_deg=0 \
                wave=pulse(v1=0 v2=1.8 delay=0 rise=$tstep fall=1n width=5u period=10u cycles=3)\n";
        same(text, |b, top| {
            let h = head(b, "v1", &["in", "0"]);
            let dc = n(b, 0);
            let ac_mag = n(b, 1);
            let ac_phase_deg = n(b, 0);
            let v1 = n(b, 0);
            let v2 = lit(b, 18, 1, 0, NumForm::Plain);
            let delay = n(b, 0);
            let rise = b.tstep();
            let fall = k(b, 1, -9);
            let width = k(b, 5, -6);
            let period = k(b, 10, -6);
            let cycles = n(b, 3);
            let wave = b.wave(spicerack_ir::wave::Pulse {
                v1,
                v2,
                delay,
                rise,
                fall,
                width,
                period,
                cycles,
                law: spicerack_ir::wave::PulseLaw::Spice,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            });
            let row = spicerack_ir::source::Source {
                ac_mag,
                ac_phase_deg,
                wave,
                ..source(dc)
            };
            b.source(top, ElemKind::VSource, h, row, Span::NONE);
        });
    }

    #[test]
    fn port_holds_its_port_row() {
        same(
            "file \"t.sp\" deck\n  element port p1 (a 0) dc=0 port=(index=1 z0=50)\n",
            |b, top| {
                let h = head(b, "p1", &["a", "0"]);
                let dc = n(b, 0);
                let index = n(b, 1);
                let z0 = n(b, 50);
                let p = b.detail(port(index, z0));
                b.source(
                    top,
                    ElemKind::Port,
                    h,
                    spicerack_ir::source::Source {
                        port: p,
                        ..source(dc)
                    },
                    Span::NONE,
                );
            },
        );
    }

    #[test]
    fn current_controlled_source_names_its_sense() {
        same(
            "file \"t.sp\" deck\n  element cccs f1 (o 0) gain=2 sense=v1\n",
            |b, top| {
                let h = head(b, "f1", &["o", "0"]);
                let gain = n(b, 2);
                let sense = b.name("v1");
                b.controlled(
                    top,
                    ElemKind::Cccs,
                    h,
                    Controlled { gain, sense },
                    Span::NONE,
                );
            },
        );
    }

    #[test]
    fn behavioural_current_reads_the_circuit() {
        same(
            "file \"t.sp\" deck\n  element behav b1 (o 0) expr=(v(a) * 2) output=current\n",
            |b, top| {
                let h = head(b, "b1", &["o", "0"]);
                let va = b.v("a", None);
                let two = n(b, 2);
                let e = b.binary(BinOp::Mul, va, two);
                b.behav(
                    top,
                    ElemKind::Behav,
                    h,
                    behav(e, OutKind::Current),
                    Span::NONE,
                );
            },
        );
    }

    #[test]
    fn model_then_diode_bound_to_it() {
        let text = "file \"t.sp\" deck\n  model dmod diode params=(is=1e-14 n=1.05)\n  \
                element diode d1 (a 0) model=dmod area=2 initial=off\n";
        same(text, |b, top| {
            let name = b.name("dmod");
            b.name("is");
            let is = lit(b, 1, 0, -14, NumForm::Exponent);
            b.name("n");
            let nv = lit(b, 105, 2, 0, NumForm::Plain);
            let params = b.params(&[("is", is), ("n", nv)]);
            b.model(
                top,
                spicerack_ir::Model {
                    params,
                    ..model(name, ModelFamily::Diode, Polarity::None)
                },
                Span::NONE,
            );
            let h = b.head("d1", &["a", "0"], Some("dmod"), &[], ExprId::NONE);
            let area = n(b, 2);
            b.semi(
                top,
                ElemKind::Diode,
                h,
                spicerack_ir::semi::Semi {
                    area,
                    initial: InitialState::Off,
                    ..semi()
                },
                Span::NONE,
            );
        });
    }

    #[test]
    fn model_with_polarity_and_version() {
        same(
            "file \"t.sp\" deck\n  model nch bsim4 polarity=n version=\"4.8\"\n",
            |b, top| {
                let name = b.name("nch");
                let version = b.string("4.8");
                b.model(
                    top,
                    spicerack_ir::Model {
                        version,
                        ..model(name, ModelFamily::Bsim4, Polarity::N)
                    },
                    Span::NONE,
                );
            },
        );
    }

    #[test]
    fn subcircuit_instance_with_multiplicity() {
        let text = "file \"t.sp\" deck\n  subckt div (a b)\n  element instance x1 (in out) model=div mult=2\n";
        same(text, |b, top| {
            b.subckt(top, "div", &["a", "b"], &[], Span::NONE);
            let name = b.name("x1");
            let terminals = b.names(&["in", "out"]);
            let div = b.name("div");
            let mult = n(b, 2);
            let model = Ref::unresolved(div);
            let h = ElemHead {
                name,
                terminals,
                model,
                params: List::EMPTY,
                mult,
                mult_law: spicerack_ir::MultLaw::Parallel,
            };
            b.instance(top, ElemKind::Instance, h, Span::NONE);
        });
    }

    #[test]
    fn unresolved_master_stays_unresolved() {
        let build = |b: &mut Builder<'_>, top: ScopeId| {
            let h = b.head("u1", &["a"], Some("cell"), &[], ExprId::NONE);
            b.instance(top, ElemKind::Unresolved, h, Span::NONE);
        };
        // canonical: an unresolved reference that lookup would not bind is written bare
        same(
            "file \"t.sp\" deck\n  element unresolved u1 (a) model=cell\n",
            build,
        );
        // the hand-written `name?` form reads the same rows
        let mut parsed = Design::new();
        let mut diags = Diags::new();
        let text = "file \"t.sp\" deck\n  element unresolved u1 (a) model=cell?\n";
        parse(text, &mut parsed.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        let mut want = Design::new();
        let top = deck(&mut want);
        build(&mut want.builder(), top);
        assert!(parsed.eq_ignoring_spans(&want));
    }

    #[test]
    fn vacask_pulse_whose_dc_is_the_wave_at_zero() {
        let text = "file \"t.sp\" deck\n  element vsource v1 (in 0) wave=pulse(v1=0 v2=1 delay=0 \
                    rise=1n fall=0 width=5u period=0 law=vacask) dc_rule=wave_at_zero\n";
        same(text, |b, top| {
            let h = head(b, "v1", &["in", "0"]);
            let v1 = n(b, 0);
            let v2 = n(b, 1);
            let delay = n(b, 0);
            let rise = k(b, 1, -9);
            let fall = n(b, 0);
            let width = k(b, 5, -6);
            let period = n(b, 0);
            let wave = b.wave(spicerack_ir::wave::Pulse {
                v1,
                v2,
                delay,
                rise,
                fall,
                width,
                period,
                cycles: ExprId::NONE,
                law: spicerack_ir::wave::PulseLaw::Vacask,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            });
            let row = spicerack_ir::source::Source {
                wave,
                dc_rule: spicerack_ir::source::DcRule::WaveAtZero,
                ..source(ExprId::NONE)
            };
            b.source(top, ElemKind::VSource, h, row, Span::NONE);
        });
    }

    #[test]
    fn exp_counted_from_td1_and_sffm_zero_before_its_delay() {
        let text = "file \"t.sp\" deck\n  element isource i1 (a 0) dc=0 wave=exp(v1=0 v2=1 td1=1 \
                    tau1=1 td2=2 tau2=1 td2_from=td1)\n  element vsource v2 (b 0) dc=0 \
                    wave=sffm(offset=0 amplitude=1 carrier_freq=1 mod_index=1 signal_freq=1 \
                    delay=1 phase_c_deg=0 phase_m_deg=0 law=ngspice pre=zero)\n";
        same(text, |b, top| {
            let h = head(b, "i1", &["a", "0"]);
            let dc = n(b, 0);
            let [v1, v2, td1, tau1, td2, tau2] = [0, 1, 1, 1, 2, 1].map(|x| n(b, x));
            let td2_from = spicerack_ir::wave::TdFrom::Td1;
            let wave = b.wave(spicerack_ir::wave::Exp {
                v1,
                v2,
                td1,
                tau1,
                td2,
                tau2,
                td2_from,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            });
            let row = spicerack_ir::source::Source { wave, ..source(dc) };
            b.source(top, ElemKind::ISource, h, row, Span::NONE);
            let h = head(b, "v2", &["b", "0"]);
            let dc = n(b, 0);
            let [
                offset,
                amplitude,
                carrier_freq,
                mod_index,
                signal_freq,
                delay,
                phase_c_deg,
                phase_m_deg,
            ] = [0, 1, 1, 1, 1, 1, 0, 0].map(|x| n(b, x));
            let wave = b.wave(spicerack_ir::wave::Sffm {
                offset,
                amplitude,
                carrier_freq,
                mod_index,
                signal_freq,
                delay,
                phase_c_deg,
                phase_m_deg,
                law: spicerack_ir::wave::SffmLaw::Ngspice,
                pre: spicerack_ir::wave::PreDelay::Zero,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            });
            let row = spicerack_ir::source::Source { wave, ..source(dc) };
            b.source(top, ElemKind::VSource, h, row, Span::NONE);
        });
    }

    #[test]
    fn voltage_source_through_its_builtin_model() {
        let text =
            "file \"t.sp\" deck\n  model v source_v\n  element vsource v1 (a 0) model=v dc=1\n";
        same(text, |b, top| {
            let name = b.name("v");
            b.model(
                top,
                model(name, ModelFamily::SourceV, Polarity::None),
                Span::NONE,
            );
            let h = b.head("v1", &["a", "0"], Some("v"), &[], ExprId::NONE);
            let dc = n(b, 1);
            b.source(top, ElemKind::VSource, h, source(dc), Span::NONE);
        });
    }

    #[test]
    fn device_libraries_with_and_without_a_path() {
        let text = "file \"t.sp\" deck\n  device_library resistor\n  \
                    device_library bsim4 path=\"spice/sn/bsim4.osdi\"\n";
        same(text, |b, top| {
            let lib = spicerack_ir::DeviceLibrary {
                path: spicerack_ir::NameId::NONE,
                family: ModelFamily::Resistor,
            };
            b.device_library(top, lib, Span::NONE);
            let path = b.name("spice/sn/bsim4.osdi");
            let lib = spicerack_ir::DeviceLibrary {
                path,
                family: ModelFamily::Bsim4,
            };
            b.device_library(top, lib, Span::NONE);
        });
    }

    #[test]
    fn ngspice_multiplier_and_behavioural_temperature_laws() {
        let text = "file \"t.sp\" deck\n  subckt div (a b)\n  element instance x1 (in out) model=div \
                    mult=2 mult_law=ngspice_append\n  element capacitor c1 (a 0) value=(v(a)) tc1=1m \
                    tc_base=fixed27 tc_law=reciprocal\n";
        same(text, |b, top| {
            b.subckt(top, "div", &["a", "b"], &[], Span::NONE);
            let name = b.name("x1");
            let terminals = b.names(&["in", "out"]);
            let div = b.name("div");
            let mult = n(b, 2);
            let h = ElemHead {
                name,
                terminals,
                model: Ref::unresolved(div),
                params: List::EMPTY,
                mult,
                mult_law: spicerack_ir::MultLaw::NgspiceAppend,
            };
            b.instance(top, ElemKind::Instance, h, Span::NONE);
            let h = head(b, "c1", &["a", "0"]);
            let value = b.v("a", None);
            let tc1 = k(b, 1, -3);
            let row = Passive {
                value,
                tc1,
                tc_base: spicerack_ir::passive::TcBase::Fixed27,
                tc_law: spicerack_ir::passive::TcLaw::Reciprocal,
                ..Passive::PLAIN
            };
            b.passive(top, ElemKind::Capacitor, h, row, Span::NONE);
        });
    }

    #[test]
    fn bins_of_a_group_and_an_instance_of_the_group() {
        let text = "file \"t.sp\" deck\n  model nch.1 bsim4 polarity=n bin_of=nch\n  \
                    model nch.2 bsim4 polarity=n bin_of=nch\n  model mz mesa polarity=p\n  \
                    element mosfet m1 (d g s b) model=nch\n";
        same(text, |b, top| {
            for bin in ["nch.1", "nch.2"] {
                let name = b.name(bin);
                let bin_of = b.name("nch");
                let row = spicerack_ir::Model {
                    bin_of,
                    ..model(name, ModelFamily::Bsim4, Polarity::N)
                };
                b.model(top, row, Span::NONE);
            }
            let name = b.name("mz");
            b.model(top, model(name, ModelFamily::Mesa, Polarity::P), Span::NONE);
            let h = b.head("m1", &["d", "g", "s", "b"], Some("nch"), &[], ExprId::NONE);
            b.semi(top, ElemKind::Mosfet, h, semi(), Span::NONE);
        });
    }

    /// A17 (R35): under ngspice's zero rule a field holds what the card holds, and an omitted
    /// one is absent.
    #[test]
    fn a_pulse_under_the_spice_zero_rule() {
        let text = "file \"t.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=0 v2=1 delay=0 \
                    rise=0 given_zero=spice_default)\n";
        same(text, |b, top| {
            let h = head(b, "v1", &["a", "0"]);
            let dc = n(b, 0);
            let [v1, v2, delay, rise] = [0, 1, 0, 0].map(|x| n(b, x));
            let wave = b.wave(spicerack_ir::wave::Pulse {
                v1,
                v2,
                delay,
                rise,
                fall: ExprId::NONE,
                width: ExprId::NONE,
                period: ExprId::NONE,
                cycles: ExprId::NONE,
                law: spicerack_ir::wave::PulseLaw::Spice,
                given_zero: spicerack_ir::wave::GivenZero::SpiceDefault,
            });
            let row = spicerack_ir::source::Source { wave, ..source(dc) };
            b.source(top, ElemKind::VSource, h, row, Span::NONE);
        });
    }

    /// A17: an AM with ngspice's omitted fields, and a charge-defined capacitor whose
    /// coefficient divides the charge.
    #[test]
    fn an_am_with_omitted_fields_and_a_reciprocal_charge() {
        let text = "file \"t.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=am(offset=0 amplitude=1 \
                    mod_offset=1 pre=zero given_zero=spice_default)\n  element charge_capacitor c1 (b 0) \
                    expr=(v(b) * 1p) tc1=1m output=charge tc_law=reciprocal\n";
        same(text, |b, top| {
            let h = head(b, "v1", &["a", "0"]);
            let dc = n(b, 0);
            let [offset, amplitude, mod_offset] = [0, 1, 1].map(|x| n(b, x));
            let none = ExprId::NONE;
            let wave = b.wave(spicerack_ir::wave::Am {
                offset,
                amplitude,
                mod_offset,
                mod_index: none,
                mod_freq: none,
                carrier_freq: none,
                delay: none,
                phase_m_deg: none,
                phase_c_deg: none,
                mod_time: spicerack_ir::wave::AmModTime::Delay,
                pre: spicerack_ir::wave::PreDelay::Zero,
                given_zero: spicerack_ir::wave::GivenZero::SpiceDefault,
            });
            b.source(
                top,
                ElemKind::VSource,
                h,
                spicerack_ir::source::Source { wave, ..source(dc) },
                Span::NONE,
            );
            let h = head(b, "c1", &["b", "0"]);
            let vb = b.v("b", None);
            let p = k(b, 1, -12);
            let q = b.binary(BinOp::Mul, vb, p);
            let tc1 = k(b, 1, -3);
            let row = spicerack_ir::controlled::Behav {
                tc1,
                tc_law: spicerack_ir::passive::TcLaw::Reciprocal,
                ..behav(q, OutKind::Charge)
            };
            b.behav(top, ElemKind::ChargeCapacitor, h, row, Span::NONE);
        });
    }
}
