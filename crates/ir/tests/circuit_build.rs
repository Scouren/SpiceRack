//! The circuit owner's builder pushes and typed accessors. Oracles: the per-kind facts table of
//! `ElemKind`'s docs, copied here by hand (spec example); every typed push read back through its
//! accessor (inverse); the builder's stated assertions (each `should_panic` case breaks one).
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (tables, pools, inverse); `rewrite/IR-CIRCUIT.md` §2, §5.
//!
//! Contents: tests only.

#[path = "circuit_common.rs"]
mod circuit_common;

#[cfg(test)]
mod tests {

    use super::circuit_common::{behav, deck, line, model, n, port, pulse, semi, sin, source};
    use spicerack_ir::controlled::{
        Controlled, Delay, DelayLaw, FreqLaw, FreqTable, FreqUnit, Laplace, LaplaceForm, OutKind,
        Poly, Quantity, TableLaw, TableSource,
    };
    use spicerack_ir::element::DetailKind;
    use spicerack_ir::passive::{Coupling, IcMode, Parasitics, Passive};
    use spicerack_ir::semi::{InitialState, Switch};
    use spicerack_ir::wave::{
        Am, AmModTime, Exp, Pattern, PreDelay, Pwl, PwlJoin, PwlRepeat, Sffm, SffmLaw, TrNoise,
        TrRandom, TrRandomKind,
    };
    use spicerack_ir::{
        DefKind, Design, ElemHead, ElemKind, Element, ExprId, ItemKind, List, ModelFamily, NO_ROW,
        NameId, Polarity, Ref, Span, Wave, WaveKind,
    };

    /// The facts of every kind, from `ElemKind`'s member docs: terminals, detail, model required,
    /// master kind.
    /// (kind, terminals, detail, model required, master kind).
    type Facts = (ElemKind, (u8, u8), DetailKind, bool, Option<DefKind>);

    const FACTS: [Facts; 37] = {
        use DetailKind as D;
        use ElemKind as E;
        let m = Some(DefKind::Model);
        [
            (E::Resistor, (2, 2), D::Passive, false, m),
            (E::Capacitor, (2, 2), D::Passive, false, m),
            (E::Inductor, (2, 2), D::Passive, false, m),
            (E::ChargeCapacitor, (2, 2), D::Behav, false, None),
            (E::FluxInductor, (2, 2), D::Behav, false, None),
            (E::Coupling, (0, 0), D::Coupling, false, m),
            (E::VSource, (2, 2), D::Source, false, m),
            (E::ISource, (2, 2), D::Source, false, m),
            (E::Port, (2, 2), D::Source, false, None),
            (E::Vcvs, (4, 4), D::Controlled, false, m),
            (E::Vccs, (4, 4), D::Controlled, false, m),
            (E::Cccs, (2, 2), D::Controlled, false, m),
            (E::Ccvs, (2, 2), D::Controlled, false, m),
            (E::Poly, (2, 2), D::Poly, false, None),
            (E::Table, (2, 2), D::Table, false, None),
            (E::Behav, (2, 2), D::Behav, false, None),
            (E::Laplace, (2, 4), D::Laplace, false, None),
            (E::FreqTable, (2, 4), D::FreqTable, false, None),
            (E::Delay, (4, 4), D::Delay, false, None),
            (E::Diode, (2, 3), D::Semi, true, m),
            (E::Bjt, (3, 5), D::Semi, true, m),
            (E::Mosfet, (3, 7), D::Semi, true, m),
            (E::Jfet, (3, 3), D::Semi, true, m),
            (E::Mesfet, (3, 3), D::Semi, true, m),
            (E::Igbt, (3, 3), D::Semi, true, m),
            (E::VSwitch, (2, 4), D::Switch, true, m),
            (E::ISwitch, (2, 2), D::Switch, true, m),
            (E::LosslessLine, (4, 4), D::Line, false, m),
            (E::LossyLine, (4, 4), D::Line, true, m),
            (E::RcLine, (3, 3), D::Line, true, m),
            (E::SingleLossyLine, (4, 4), D::Line, true, m),
            (E::CoupledLines, (4, 18), D::Line, true, m),
            (E::CodeModel, (0, 255), D::None, true, m),
            (E::ModuleModel, (0, 255), D::None, true, m),
            (E::Module, (0, 255), D::None, true, Some(DefKind::Module)),
            (E::Instance, (0, 255), D::None, true, Some(DefKind::Subckt)),
            (E::Unresolved, (0, 255), D::None, true, None),
        ]
    };

    /// The SPICE letter of every kind, from `letter.rs`'s header (A13), at voltage output and
    /// control.
    const LETTERS: [(ElemKind, Option<u8>); 37] = {
        use ElemKind as E;
        [
            (E::Resistor, Some(b'R')),
            (E::Capacitor, Some(b'C')),
            (E::Inductor, Some(b'L')),
            (E::ChargeCapacitor, Some(b'C')),
            (E::FluxInductor, Some(b'L')),
            (E::Coupling, Some(b'K')),
            (E::VSource, Some(b'V')),
            (E::ISource, Some(b'I')),
            (E::Port, Some(b'P')),
            (E::Vcvs, Some(b'E')),
            (E::Vccs, Some(b'G')),
            (E::Cccs, Some(b'F')),
            (E::Ccvs, Some(b'H')),
            (E::Poly, Some(b'E')),
            (E::Table, Some(b'E')),
            (E::Behav, Some(b'B')),
            (E::Laplace, Some(b'E')),
            (E::FreqTable, Some(b'E')),
            (E::Delay, Some(b'E')),
            (E::Diode, Some(b'D')),
            (E::Bjt, Some(b'Q')),
            (E::Mosfet, Some(b'M')),
            (E::Jfet, Some(b'J')),
            (E::Mesfet, Some(b'Z')),
            (E::Igbt, Some(b'Z')),
            (E::VSwitch, Some(b'S')),
            (E::ISwitch, Some(b'W')),
            (E::LosslessLine, Some(b'T')),
            (E::LossyLine, Some(b'O')),
            (E::RcLine, Some(b'U')),
            (E::SingleLossyLine, Some(b'Y')),
            (E::CoupledLines, Some(b'P')),
            (E::CodeModel, Some(b'A')),
            (E::ModuleModel, Some(b'N')),
            (E::Module, Some(b'X')),
            (E::Instance, Some(b'X')),
            (E::Unresolved, None),
        ]
    };

    #[test]
    fn every_kind_has_its_spice_letter() {
        use spicerack_ir::controlled::Quantity::{Current as I, Voltage as V};
        use spicerack_ir::letter::letter_of;
        let listed: Vec<ElemKind> = LETTERS.iter().map(|l| l.0).collect();
        assert_eq!(listed, ElemKind::ALL.to_vec());
        for (kind, letter) in LETTERS {
            assert_eq!(letter_of(kind, V, V), letter, "{kind:?}");
        }
        // the polynomial's letter follows its output and control; tables, Laplace and
        // frequency tables are voltage-controlled, so only their output counts
        assert_eq!(letter_of(ElemKind::Poly, I, V), Some(b'G'));
        assert_eq!(letter_of(ElemKind::Poly, I, I), Some(b'F'));
        assert_eq!(letter_of(ElemKind::Poly, V, I), Some(b'H'));
        for kind in [ElemKind::Table, ElemKind::Laplace, ElemKind::FreqTable] {
            assert_eq!(letter_of(kind, I, V), Some(b'G'), "{kind:?}");
            assert_eq!(letter_of(kind, I, I), Some(b'G'), "{kind:?}");
        }
    }

    #[test]
    fn spice_letter_reads_the_detail() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        let one = n(&mut b, 1);
        let controls = b.names(&["v1"]);
        let coeffs = b.exprs(&[one]);
        let poly = Poly {
            controls,
            coeffs,
            dimension: 1,
            output: Quantity::Current,
            control: Quantity::Current,
        };
        let h = b.head("f1", &["o", "0"], None, &[], ExprId::NONE);
        let f1 = b.poly(top, h, poly, Span::NONE);
        let points = b.exprs(&[one, one]);
        let table = TableSource {
            input: one,
            points,
            delta: ExprId::NONE,
            law: TableLaw::Exact,
            output: Quantity::Current,
        };
        let h = b.head("g1", &["o", "0"], None, &[], ExprId::NONE);
        let g1 = b.table_source(top, h, table, Span::NONE);
        let design = b.design();
        let letter = |i: spicerack_ir::ItemRef| {
            spicerack_ir::spice_letter(design, &design.row::<Element>(i.index))
        };
        assert_eq!(letter(f1), Some(b'F'));
        assert_eq!(letter(g1), Some(b'G'));
    }

    #[test]
    fn kind_facts_match_the_docs() {
        let listed: Vec<ElemKind> = FACTS.iter().map(|f| f.0).collect();
        assert_eq!(listed, ElemKind::ALL.to_vec());
        for (kind, terminals, detail, needs, master) in FACTS {
            assert_eq!(kind.terminals(), terminals, "{kind:?}");
            assert_eq!(kind.detail(), detail, "{kind:?}");
            assert_eq!(kind.has_detail(), detail != DetailKind::None, "{kind:?}");
            assert_eq!(kind.needs_model(), needs, "{kind:?}");
            assert_eq!(kind.master_kind(), master, "{kind:?}");
        }
    }

    #[test]
    fn head_interns_in_order_and_leaves_the_master_unresolved() {
        let mut d = Design::new();
        deck(&mut d);
        let mut b = d.builder();
        let h = b.head("r1", &["a", "b"], Some("rmod"), &[], ExprId::NONE);
        let design = b.design();
        assert_eq!(design.str(h.name), "r1");
        let terms: Vec<&str> = design
            .names(h.terminals)
            .iter()
            .map(|&t| design.str(t))
            .collect();
        assert_eq!(terms, ["a", "b"]);
        assert_eq!(design.str(h.model.name), "rmod");
        assert!(!h.model.is_resolved());
        assert!(h.params.is_empty());
        let none = d.builder().head("v1", &[], None, &[], ExprId::NONE);
        assert_eq!(none.model.name, NameId::NONE);
    }

    #[test]
    fn every_typed_push_reads_back() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        let one = n(&mut b, 1);
        let two = n(&mut b, 2);

        let par = b.detail(Parasitics {
            rser: one,
            lser: ExprId::NONE,
            rpar: ExprId::NONE,
            cpar: two,
            rlshunt: ExprId::NONE,
        });
        let passive = Passive {
            value: one,
            ic: two,
            ic_mode: IcMode::InOp,
            parasitics: par,
            ..Passive::PLAIN
        };
        let h = b.head("c1", &["a", "0"], None, &[], ExprId::NONE);
        let c1 = b.passive(top, ElemKind::Capacitor, h, passive, Span::NONE);

        let inductors = b.names(&["l1", "l2", "l3"]);
        let h = b.head("k1", &[], None, &[], ExprId::NONE);
        let k1 = b.coupling(top, h, Coupling { inductors, k: one }, Span::NONE);

        let wave = b.wave(pulse(one));
        let p = b.detail(port(one, two));
        let src = spicerack_ir::source::Source {
            wave,
            port: p,
            ..source(one)
        };
        let h = b.head("p1", &["a", "0"], None, &[], ExprId::NONE);
        let p1 = b.source(top, ElemKind::Port, h, src, Span::NONE);

        let sense = b.name("v1");
        let h = b.head("f1", &["a", "0"], None, &[], ExprId::NONE);
        let f1 = b.controlled(
            top,
            ElemKind::Cccs,
            h,
            Controlled { gain: two, sense },
            Span::NONE,
        );

        let controls = b.names(&["a", "0"]);
        let coeffs = b.exprs(&[one, two]);
        let poly = Poly {
            controls,
            coeffs,
            dimension: 1,
            output: Quantity::Voltage,
            control: Quantity::Voltage,
        };
        let h = b.head("e1", &["o", "0"], None, &[], ExprId::NONE);
        let e1 = b.poly(top, h, poly, Span::NONE);

        let points = b.exprs(&[one, two]);
        let table = TableSource {
            input: one,
            points,
            delta: ExprId::NONE,
            law: TableLaw::Exact,
            output: Quantity::Current,
        };
        let h = b.head("g1", &["o", "0"], None, &[], ExprId::NONE);
        let g1 = b.table_source(top, h, table, Span::NONE);

        let h = b.head("q1", &["a", "0"], None, &[], ExprId::NONE);
        let q1 = b.behav(
            top,
            ElemKind::ChargeCapacitor,
            h,
            behav(one, OutKind::Charge),
            Span::NONE,
        );

        let num = b.exprs(&[one]);
        let den = b.exprs(&[one, two]);
        let lap = Laplace {
            num,
            den,
            input: ExprId::NONE,
            output: Quantity::Voltage,
            form: LaplaceForm::Polynomial,
        };
        let h = b.head("e2", &["o", "0", "i", "0"], None, &[], ExprId::NONE);
        let e2 = b.laplace(top, h, lap, Span::NONE);

        let fpoints = b.exprs(&[one, two, one]);
        let ft = FreqTable {
            input: ExprId::NONE,
            points: fpoints,
            law: FreqLaw::DbLogFreq,
            unit: FreqUnit::Db,
            output: Quantity::Voltage,
        };
        let h = b.head("e3", &["o", "0", "i", "0"], None, &[], ExprId::NONE);
        let e3 = b.freq_table(top, h, ft, Span::NONE);

        let delay = Delay {
            td: one,
            gain: two,
            samples: ExprId::NONE,
            law: DelayLaw::Ideal,
        };
        let h = b.head("e4", &["o", "0", "i", "0"], None, &[], ExprId::NONE);
        let e4 = b.delay(top, h, delay, Span::NONE);

        let srow = spicerack_ir::semi::Semi {
            area: two,
            initial: InitialState::Off,
            ..semi()
        };
        let h = b.head("d1", &["a", "0"], Some("dmod"), &[], ExprId::NONE);
        let d1 = b.semi(top, ElemKind::Diode, h, srow, Span::NONE);

        let sw = Switch {
            sense: NameId::NONE,
            control: ExprId::NONE,
            initial: InitialState::On,
        };
        let h = b.head("s1", &["a", "0", "c", "0"], Some("smod"), &[], ExprId::NONE);
        let s1 = b.switch(top, ElemKind::VSwitch, h, sw, Span::NONE);

        let lrow = spicerack_ir::line::Line {
            z0: two,
            td: one,
            ..line()
        };
        let h = b.head("t1", &["a", "0", "b", "0"], None, &[], ExprId::NONE);
        let t1 = b.line(top, ElemKind::LosslessLine, h, lrow, Span::NONE);

        let h = b.head("x1", &["a", "b"], Some("div"), &[], two);
        let x1 = b.instance(top, ElemKind::Instance, h, Span::NONE);

        let design = b.design();
        let el = |i: spicerack_ir::ItemRef| -> Element {
            assert_eq!(i.kind, ItemKind::Element);
            design.row::<Element>(i.index)
        };
        assert_eq!(design.passive(&el(c1)), passive);
        assert_eq!(design.detail::<Parasitics>(par).cpar, two);
        assert_eq!(design.coupling(&el(k1)), Coupling { inductors, k: one });
        assert_eq!(design.source(&el(p1)), src);
        assert_eq!(design.wave(wave), Wave::Pulse(pulse(one)));
        assert_eq!(design.controlled(&el(f1)), Controlled { gain: two, sense });
        assert_eq!(design.poly(&el(e1)), poly);
        assert_eq!(design.table_source(&el(g1)), table);
        assert_eq!(design.behav(&el(q1)), behav(one, OutKind::Charge));
        assert_eq!(design.laplace(&el(e2)), lap);
        assert_eq!(design.freq_table(&el(e3)), ft);
        assert_eq!(design.delay(&el(e4)), delay);
        assert_eq!(design.semi(&el(d1)), srow);
        assert_eq!(design.switch(&el(s1)), sw);
        assert_eq!(design.line(&el(t1)), lrow);
        let x = el(x1);
        assert_eq!(
            (x.kind, x.detail, x.mult),
            (ElemKind::Instance, NO_ROW, two)
        );
        assert_eq!(design.items(top).len(), 14);
    }

    #[test]
    fn every_wave_kind_reads_back() {
        let mut d = Design::new();
        deck(&mut d);
        let mut b = d.builder();
        let v = n(&mut b, 1);
        let points = b.exprs(&[v, v]);
        let bits = b.name("10m1");
        let waves = [
            Wave::Pulse(pulse(v)),
            Wave::Sin(spicerack_ir::wave::Sin {
                pre: PreDelay::HoldDelayValue,
                ..sin(v)
            }),
            Wave::Exp(Exp {
                v1: v,
                v2: v,
                td1: v,
                tau1: v,
                td2: v,
                tau2: v,
                td2_from: spicerack_ir::wave::TdFrom::Td1,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            }),
            Wave::Pwl(Pwl {
                points,
                delay: ExprId::NONE,
                repeat_at: v,
                repeat_count: ExprId::NONE,
                join_width: ExprId::NONE,
                file: NameId::NONE,
                repeat: PwlRepeat::From,
                join: PwlJoin::Ramp,
            }),
            Wave::Sffm(Sffm {
                offset: v,
                amplitude: v,
                carrier_freq: v,
                mod_index: v,
                signal_freq: v,
                delay: v,
                phase_c_deg: v,
                phase_m_deg: v,
                law: SffmLaw::Ngspice,
                pre: PreDelay::Zero,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            }),
            Wave::Am(Am {
                offset: v,
                amplitude: v,
                mod_offset: v,
                mod_index: v,
                mod_freq: v,
                carrier_freq: v,
                delay: v,
                phase_m_deg: v,
                phase_c_deg: v,
                mod_time: AmModTime::CarrierPhase,
                pre: PreDelay::Zero,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            }),
            Wave::TrNoise(TrNoise {
                na: v,
                nt: v,
                nalpha: ExprId::NONE,
                namp: ExprId::NONE,
                rts_amp: ExprId::NONE,
                rts_capture: ExprId::NONE,
                rts_emission: ExprId::NONE,
            }),
            Wave::TrRandom(TrRandom {
                ts: v,
                td: v,
                p1: v,
                p2: v,
                kind: TrRandomKind::Poisson,
                given_zero: spicerack_ir::wave::GivenZero::AsGiven,
            }),
            Wave::Pattern(Pattern {
                high: v,
                low: v,
                delay: v,
                rise: v,
                fall: v,
                sample: v,
                bits,
                repeats: ExprId::NONE,
                repeat_start: v,
            }),
        ];
        let refs: Vec<_> = waves
            .iter()
            .map(|w| match *w {
                Wave::Pulse(r) => b.wave(r),
                Wave::Sin(r) => b.wave(r),
                Wave::Exp(r) => b.wave(r),
                Wave::Pwl(r) => b.wave(r),
                Wave::Sffm(r) => b.wave(r),
                Wave::Am(r) => b.wave(r),
                Wave::TrNoise(r) => b.wave(r),
                Wave::TrRandom(r) => b.wave(r),
                Wave::Pattern(r) => b.wave(r),
            })
            .collect();
        let kinds: Vec<WaveKind> = refs.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, WaveKind::ALL.to_vec());
        for (r, w) in refs.iter().zip(waves) {
            assert_eq!(r.index, 0);
            assert_eq!(b.design().wave(*r), w);
        }
    }

    #[test]
    fn model_push_defines_the_model() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        let name = b.name("nch");
        let item = b.model(
            top,
            model(name, ModelFamily::Bsim4, Polarity::N),
            Span::NONE,
        );
        let design = b.design();
        assert_eq!(item.kind, ItemKind::Model);
        let defs = design.definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(
            (defs[0].kind, defs[0].name, defs[0].item),
            (DefKind::Model, name, item)
        );
        assert_eq!(
            design.row::<spicerack_ir::Model>(item.index).family,
            ModelFamily::Bsim4
        );
    }

    fn pushed(f: impl FnOnce(&mut spicerack_ir::Builder<'_>, spicerack_ir::ScopeId)) {
        let mut d = Design::new();
        let top = deck(&mut d);
        f(&mut d.builder(), top);
    }

    #[test]
    #[should_panic(expected = "terminal count")]
    fn element_refuses_a_terminal_count_outside_the_range() {
        pushed(|b, top| {
            let one = n(b, 1);
            let h = b.head("r1", &["a"], None, &[], ExprId::NONE);
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
        });
    }

    #[test]
    #[should_panic(expected = "not a passive kind")]
    fn passive_refuses_a_kind_of_another_family() {
        pushed(|b, top| {
            let one = n(b, 1);
            let h = b.head("v1", &["a", "0"], None, &[], ExprId::NONE);
            b.passive(
                top,
                ElemKind::VSource,
                h,
                Passive {
                    value: one,
                    ..Passive::PLAIN
                },
                Span::NONE,
            );
        });
    }

    #[test]
    #[should_panic(expected = "detail row")]
    fn element_refuses_a_missing_detail() {
        pushed(|b, top| {
            let h = b.head("r1", &["a", "b"], None, &[], ExprId::NONE);
            b.element(top, ElemKind::Resistor, h, NO_ROW, Span::NONE);
        });
    }

    #[test]
    #[should_panic(expected = "detail row")]
    fn element_refuses_a_detail_on_a_kind_without_one() {
        pushed(|b, top| {
            let h = b.head("x1", &[], Some("div"), &[], ExprId::NONE);
            b.element(top, ElemKind::Instance, h, 0, Span::NONE);
        });
    }

    #[test]
    #[should_panic(expected = "master")]
    fn element_refuses_a_master_on_a_kind_without_one() {
        pushed(|b, top| {
            let one = n(b, 1);
            let h = b.head("b1", &["a", "0"], Some("bmod"), &[], ExprId::NONE);
            b.behav(
                top,
                ElemKind::Behav,
                h,
                behav(one, OutKind::Voltage),
                Span::NONE,
            );
        });
    }

    #[test]
    #[should_panic(expected = "master")]
    fn element_refuses_a_missing_required_master() {
        pushed(|b, top| {
            let h = b.head("d1", &["a", "0"], None, &[], ExprId::NONE);
            b.semi(top, ElemKind::Diode, h, semi(), Span::NONE);
        });
    }

    #[test]
    #[should_panic(expected = "output")]
    fn behav_refuses_a_charge_on_a_plain_behavioural_source() {
        pushed(|b, top| {
            let one = n(b, 1);
            let h = b.head("b1", &["a", "0"], None, &[], ExprId::NONE);
            b.behav(
                top,
                ElemKind::Behav,
                h,
                behav(one, OutKind::Charge),
                Span::NONE,
            );
        });
    }

    #[test]
    #[should_panic(expected = "port row")]
    fn source_refuses_a_port_row_on_a_voltage_source() {
        pushed(|b, top| {
            let one = n(b, 1);
            let p = b.detail(port(one, one));
            let h = b.head("v1", &["a", "0"], None, &[], ExprId::NONE);
            b.source(
                top,
                ElemKind::VSource,
                h,
                spicerack_ir::source::Source {
                    port: p,
                    ..source(one)
                },
                Span::NONE,
            );
        });
    }

    #[test]
    #[should_panic(expected = "word")]
    fn model_refuses_a_code_model_without_its_name() {
        pushed(|b, top| {
            let name = b.name("xfer");
            b.model(
                top,
                model(name, ModelFamily::CodeModel, Polarity::None),
                Span::NONE,
            );
        });
    }

    #[test]
    fn head_built_by_a_reader_keeps_its_resolved_master() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        let (_, _body) = b.subckt(top, "div", &["a", "b"], &[], Span::NONE);
        let def = b.design().definitions()[0];
        let terminals = b.names(&["x", "y"]);
        let name = b.name("x1");
        let head = ElemHead {
            name,
            terminals,
            model: Ref {
                name: def.name,
                def: spicerack_ir::DefId(0),
            },
            params: List::EMPTY,
            mult: ExprId::NONE,
            mult_law: spicerack_ir::MultLaw::Parallel,
        };
        let item = b.instance(top, ElemKind::Instance, head, Span::NONE);
        assert!(b.design().row::<Element>(item.index).model.is_resolved());
    }

    /// A15 C4 (R33): the first bin of a group also defines the group's name, bound to that bin;
    /// a later bin defines only itself; an instance naming the group binds to it.
    #[test]
    fn a_bin_group_is_defined_by_its_first_bin() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        let push_bin = |b: &mut spicerack_ir::Builder<'_>, name: &str| {
            let name = b.name(name);
            let bin_of = b.name("nch");
            // a versioned family states its version
            let version = b.string("4.8.3");
            let row = spicerack_ir::Model {
                bin_of,
                version,
                ..model(name, ModelFamily::Bsim4, Polarity::N)
            };
            b.model(top, row, Span::NONE)
        };
        let first = push_bin(&mut b, "nch.1");
        push_bin(&mut b, "nch.2");
        let h = b.head("m1", &["d", "g", "s", "b"], Some("nch"), &[], ExprId::NONE);
        let m1 = b.semi(top, ElemKind::Mosfet, h, semi(), Span::NONE);
        let design = b.design();
        let names: Vec<&str> = design
            .definitions()
            .iter()
            .map(|d| design.str(d.name))
            .collect();
        assert_eq!(names, ["nch.1", "nch", "nch.2"]);
        let group = design.definitions()[1];
        assert_eq!((group.kind, group.item), (DefKind::Model, first));
        let mut diags = spicerack_ir::Diags::new();
        spicerack_ir::link(&mut d, &mut diags);
        let element = d.row::<Element>(m1.index);
        assert_eq!(
            element.model.def,
            spicerack_ir::DefId(1),
            "the instance binds to the group"
        );
        let mut diags = spicerack_ir::Diags::new();
        spicerack_ir::check(&d, &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
    }

    #[test]
    fn spice_letters_of_a_behavioural_source_and_a_port() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        let one = n(&mut b, 1);
        let h = b.head("e4", &["o", "0"], None, &[], ExprId::NONE);
        let v = b.behav(
            top,
            ElemKind::Behav,
            h,
            behav(one, OutKind::Voltage),
            Span::NONE,
        );
        let h = b.head("g3", &["o", "0"], None, &[], ExprId::NONE);
        let i = b.behav(
            top,
            ElemKind::Behav,
            h,
            behav(one, OutKind::Current),
            Span::NONE,
        );
        let h = b.head("b2", &["o", "0"], None, &[], ExprId::NONE);
        let r = b.behav(
            top,
            ElemKind::Behav,
            h,
            behav(one, OutKind::Resistance),
            Span::NONE,
        );
        let design = b.design();
        let letters = |x: spicerack_ir::ItemRef| {
            spicerack_ir::spice_letters(design, &design.row::<Element>(x.index))
        };
        assert_eq!(letters(v), b"BE");
        assert_eq!(letters(i), b"BG");
        assert_eq!(letters(r), b"B");
        assert_eq!(
            spicerack_ir::spice_letter(design, &design.row::<Element>(v.index)),
            Some(b'B')
        );
    }

    /// `n` resistors, each valued by a parameter of its own pushed just before it: every element
    /// push follows a definition, as a reader of a PDK library does.
    #[cfg(feature = "gen")]
    fn resistor_deck(n: usize) -> Design {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        for i in 0..n {
            let one = super::circuit_common::n(&mut b, 1);
            let name = format!("p{i}");
            b.param(top, &name, one, Span::NONE);
            let value = b.param_ref(&name);
            let h = b.head(&format!("r{i}"), &["a", "0"], None, &[], ExprId::NONE);
            b.passive(
                top,
                ElemKind::Resistor,
                h,
                Passive {
                    value,
                    ..Passive::PLAIN
                },
                Span::NONE,
            );
        }
        d
    }

    /// Building and checking the circuit rows computes the definition classes a fixed number of
    /// times, whatever the deck's size (the round-4 regression: the builder's element assert
    /// recomputed them after every push, quadratic, and only a debug build showed it). Oracle: a
    /// count, not a clock (`Design::class_computes`), so load cannot move it: none while the
    /// deck is built (the builder's assert assumes, `Classes::Assume`), one for `check`, at N and
    /// at 4N alike.
    #[cfg(feature = "gen")]
    #[test]
    fn building_and_checking_compute_the_classes_once() {
        const N: usize = 2_000;
        for n in [N, 4 * N] {
            let d = resistor_deck(n);
            assert_eq!(
                d.class_computes(),
                0,
                "built at {n}: the classes never computed"
            );
            spicerack_ir::check(&d, &mut spicerack_ir::Diags::new());
            assert_eq!(
                d.class_computes(),
                1,
                "checked at {n}: the classes computed once"
            );
        }
    }
}
