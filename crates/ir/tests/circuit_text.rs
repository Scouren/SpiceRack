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

    use super::circuit_common::{behav, deck, k, line, model, n, port, semi, source};
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

    /// R90: the stated laws are one `laws` line before the entry file, in `Law::ALL` order; a
    /// word that names no law refuses the line and states nothing.
    #[test]
    fn laws_line() {
        use spicerack_ir::{Law, Laws};
        same(
            "laws bc_sat_areab_only diode_sidewall_own_f1\nfile \"t.sp\" deck\n",
            |b, _| b.set_laws(Laws::of(&[Law::DiodeSidewallOwnF1, Law::BcSatAreabOnly])),
        );
        let mut d = Design::new();
        let mut diags = Diags::new();
        parse(
            "laws areab_only\nfile \"t.sp\" deck\n",
            &mut d.builder(),
            &mut diags,
        );
        assert!(!diags.is_empty(), "an unknown law word is refused");
        assert!(d.laws().is_empty());
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
        // a versioned family states its version (round 7): without it `check` refuses the
        // bins, and so does the reader since it refuses what `check` refuses
        let text = "file \"t.sp\" deck\n  model nch.1 bsim4 polarity=n bin_of=nch version=\"4.8\"\n  \
                    model nch.2 bsim4 polarity=n bin_of=nch version=\"4.8\"\n  model mz mesa polarity=p\n  \
                    element mosfet m1 (d g s b) model=nch\n";
        same(text, |b, top| {
            for bin in ["nch.1", "nch.2"] {
                let name = b.name(bin);
                let bin_of = b.name("nch");
                let version = b.string("4.8");
                let row = spicerack_ir::Model {
                    bin_of,
                    version,
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

    /// A20: VACASK's ideal line as a library, a parameterless model and a line naming it.
    #[test]
    fn a_lossless_line_naming_the_ideal_line_model() {
        let text = "file \"t.sp\" deck\n  device_library tline_ideal\n  model tl tline_ideal\n  \
                    element lossless_line t1 (a 0 b 0) model=tl z0=50 td=1n\n";
        same(text, |b, top| {
            let lib = spicerack_ir::DeviceLibrary {
                path: spicerack_ir::NameId::NONE,
                family: ModelFamily::TlineIdeal,
            };
            b.device_library(top, lib, Span::NONE);
            let name = b.name("tl");
            b.model(
                top,
                model(name, ModelFamily::TlineIdeal, Polarity::None),
                Span::NONE,
            );
            let h = b.head("t1", &["a", "0", "b", "0"], Some("tl"), &[], ExprId::NONE);
            let z0 = n(b, 50);
            let td = k(b, 1, -9);
            b.line(
                top,
                ElemKind::LosslessLine,
                h,
                spicerack_ir::line::Line { z0, td, ..line() },
                Span::NONE,
            );
        });
    }

    /// R61/A36 at the reader (the trunk's mutation law, `ir` row, `worked.ir` seed 9): an
    /// instance whose terminals number differently from its subcircuit's ports, known only once
    /// `link` has bound it, is one `TEXT_SYNTAX` row at its line and is taken out of the design.
    /// Oracle: `check` of what was read reports nothing; the matching instance stays.
    #[test]
    fn an_instance_of_another_port_count_is_refused() {
        let text = concat!(
            "file \"rc.sp\" deck\n",
            "  subckt div (in gnd)\n",
            "    element resistor r1 (in gnd) value=1\n",
            "  element instance x1 (a b 0) model=div\n",
            "  element instance x2 (a 0) model=div\n",
        );
        let mut d = Design::new();
        let mut diags = Diags::new();
        parse(text, &mut d.builder(), &mut diags);
        let codes: Vec<_> = diags.rows().iter().map(|r| r.code).collect();
        assert_eq!(
            codes,
            vec![spicerack_ir::diag::TEXT_SYNTAX],
            "{:?}",
            diags.rows()
        );
        let at = diags.rows()[0].span;
        let x1 = text.find("element instance x1").unwrap() as u32;
        let x2 = text.find("element instance x2").unwrap() as u32;
        assert!(
            at.start >= x1 && at.start < x2,
            "the row names the instance's line: {at:?}"
        );
        let mut checked = Diags::new();
        spicerack_ir::check(&d, &mut checked);
        assert!(checked.is_empty(), "{:?}", checked.rows());
        let mut out = String::new();
        dump(&d, FileId(0), &mut out);
        assert!(!out.contains(" x1 ") && out.contains(" x2 "), "{out}");
    }

    /// The spine's mutation law (ir-spine 51cd4f21, `tests/dialect_mutants.rs`): an element or
    /// model line the builder would assert on is one `TEXT_SYNTAX` row and pushes no item, as the
    /// kernel rows' reader does (`item_text.rs`). Oracle: `check` of what was read reports
    /// nothing, and no element or model item exists.
    #[test]
    fn a_line_the_builder_would_refuse_pushes_nothing() {
        let cases = [
            // FieldDomain: a key of another kind leaves a resistor without its value
            "file \"rc.sp\" deck\n  element resistor r2 (out gnd) dc=(1 / 2)\n",
            // TerminalCount: a broken terminal list
            "file \"rc.sp\" deck\n  element resistor r1 out) value=1\n",
            // a code model without its word
            "file \"rc.sp\" deck\n  model m1 code_model\n",
            // a module model without its module
            "file \"rc.sp\" deck\n  model m2 module\n",
        ];
        // DuplicateElement: a second element of one whole name in one scope (the first stays)
        let text = "file \"rc.sp\" deck\n  subckt div (in out gnd) r=1\n    element resistor r1 (in out) value=r\n    element resistor r1 (in out) value=r\n";
        let mut d = Design::new();
        let mut diags = Diags::new();
        parse(text, &mut d.builder(), &mut diags);
        let codes: Vec<_> = diags.rows().iter().map(|r| r.code).collect();
        assert_eq!(codes, vec![spicerack_ir::diag::TEXT_SYNTAX], "{text}");
        let elements = d
            .reachable()
            .into_iter()
            .filter(|(i, _)| i.kind == spicerack_ir::ItemKind::Element)
            .count();
        assert_eq!(elements, 1, "{text}");
        let mut checked = Diags::new();
        spicerack_ir::check(&d, &mut checked);
        assert!(checked.is_empty(), "{text}: {:?}", checked.rows());
        for text in cases {
            let mut d = Design::new();
            let mut diags = Diags::new();
            parse(text, &mut d.builder(), &mut diags);
            assert!(!diags.is_empty(), "{text}");
            assert!(
                diags
                    .rows()
                    .iter()
                    .all(|r| r.code == spicerack_ir::diag::TEXT_SYNTAX),
                "{text}: {:?}",
                diags.rows()
            );
            let items: Vec<_> = d
                .reachable()
                .into_iter()
                .filter(|(i, _)| {
                    matches!(
                        i.kind,
                        spicerack_ir::ItemKind::Element | spicerack_ir::ItemKind::Model
                    )
                })
                .collect();
            assert!(items.is_empty(), "{text}: {items:?}");
            let mut checked = Diags::new();
            spicerack_ir::check(&d, &mut checked);
            assert!(checked.is_empty(), "{text}: {:?}", checked.rows());
        }
    }

    /// A32: the family word and the library rule are public. Oracle: the IR dump of a model of
    /// each family writes `model m <word>` (the reader's own word); the library rule's members by
    /// hand from A12 C5, A20 and R44.
    #[test]
    fn the_family_word_and_the_library_rule_are_public() {
        let mut words = std::collections::HashSet::new();
        for f in ModelFamily::ALL {
            let mut d = Design::new();
            let top = deck(&mut d);
            let mut b = d.builder();
            let name = b.name("m");
            b.push(top, model(name, f, Polarity::None), Span::NONE);
            let mut text = String::new();
            for file in d.files() {
                dump(&d, file, &mut text);
            }
            assert!(
                text.contains(&format!("model m {}", f.word())),
                "{f:?}: {text}"
            );
            assert!(
                words.insert(f.word()),
                "{f:?}: a second family with this word"
            );
        }
        for (f, free) in [
            (ModelFamily::SourceV, true),
            (ModelFamily::Mutual, true),
            (ModelFamily::Module, true),
            (ModelFamily::CodeModel, true),
            (ModelFamily::Other, true),
            (ModelFamily::TlineIdeal, false),
            (ModelFamily::Resistor, false),
            (ModelFamily::Bsim4, false),
        ] {
            assert_eq!(f.library_free(), free, "{f:?}");
        }
    }

    /// A32: the circuit rows' element names (`RowFacts::map_element_names`), in field order, each
    /// one use read in the element's scope: a coupling's inductors, a current-controlled
    /// source's sensing element, a current-controlled polynomial's controls, a current switch's
    /// sensing element; neighbours: a voltage-controlled polynomial's controls and every
    /// terminal are nets. Oracle: the names written in the text, and the text after a rename.
    #[test]
    fn the_circuit_rows_name_elements_by_name() {
        let src = concat!(
            "file \"t.sp\" deck\n",
            "  element coupling k1 () inductors=(l1 l2) k=1\n",
            "  element cccs f1 (o 0) gain=2 sense=v1\n",
            "  element poly f2 (o 0) controls=(v2 v3) coeffs=[0, 1, 1] dimension=2 output=current control=current\n",
            "  element poly e2 (o 0) controls=(a b) coeffs=[0, 1] dimension=1 output=voltage control=voltage\n",
            "  element iswitch w1 (a b) model=sw sense=v4\n",
        );
        let mut d = Design::new();
        let mut diags = spicerack_ir::Diags::new();
        parse(src, &mut d.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        let top = d.file(d.entry()).top;
        let names: Vec<String> = d
            .element_uses()
            .into_iter()
            .map(|(_, u)| {
                assert_eq!(u.scope, top);
                d.str(u.name).to_string()
            })
            .collect();
        assert_eq!(names, ["l1", "l2", "v1", "v2", "v3", "v4"]);
        // a rename stores back in every field
        let renames: Vec<(spicerack_ir::NameId, spicerack_ir::NameId)> =
            ["l1", "l2", "v1", "v2", "v3", "v4"]
                .iter()
                .map(|n| (d.find_name(n), d.builder().name(&format!("x{n}"))))
                .collect();
        let items: Vec<spicerack_ir::ItemRef> = d.items(top).to_vec();
        for item in items {
            d.editor().map_element_uses(top, item, &mut |u| {
                renames.iter().find(|r| r.0 == u.name).map(|r| r.1)
            });
        }
        let mut text = String::new();
        dump(&d, d.entry(), &mut text);
        for want in [
            "inductors=(xl1 xl2)",
            "sense=xv1",
            "controls=(xv2 xv3)",
            "controls=(a b)",
            "sense=xv4",
        ] {
            assert!(text.contains(want), "{want} in\n{text}");
        }
    }
}
