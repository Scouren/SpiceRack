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
                parasitics=(rser=1m cpar=1) ic_mode=in_op noisy=false\n";
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
                noisy: false,
                ..Passive::PLAIN
            };
            b.passive(top, ElemKind::Capacitor, h, row, Span::NONE);
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
            };
            b.instance(top, ElemKind::Instance, h, Span::NONE);
        });
    }

    #[test]
    fn unresolved_master_stays_unresolved() {
        same(
            "file \"t.sp\" deck\n  element unresolved u1 (a) model=cell?\n",
            |b, top| {
                let h = b.head("u1", &["a"], Some("cell"), &[], ExprId::NONE);
                b.instance(top, ElemKind::Unresolved, h, Span::NONE);
            },
        );
    }
}
