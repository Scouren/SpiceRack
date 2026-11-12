//! `check` on circuit rows. Oracle: hand-broken fixtures, one per invariant the circuit rows
//! state (IR-IMPL §2 "check"): each design is valid except for one field, and `check` must
//! report exactly that violation's code, once; the valid neighbour reports nothing. Rows are
//! pushed through the low tier (`Builder::push`, `Builder::detail`), which does not enforce the
//! circuit invariants, so a broken row can be built.
//!
//! Cites: `rewrite/IR-IMPL.md` §2; `rewrite/IR-KERNEL.md` §9; `rewrite/IR-DECISIONS.md` A5;
//! the field docs of `element.rs`, `passive.rs`, `source.rs`, `controlled.rs`, `semi.rs`,
//! `line.rs`, `wave.rs`, `model.rs`.
//!
//! Contents: tests only.

#[path = "circuit_common.rs"]
mod circuit_common;

#[cfg(test)]
mod tests {

    use super::circuit_common::{behav, codes, deck, line, model, n, pulse, semi, sin, source};
    use spicerack_ir::controlled::{
        Controlled, Delay, DelayLaw, Laplace, LaplaceForm, OutKind, Poly, Quantity, TableLaw,
        TableSource,
    };
    use spicerack_ir::diag::violation_code;
    use spicerack_ir::passive::{Coupling, IcMode, Passive};
    use spicerack_ir::semi::{InitialState, Switch};
    use spicerack_ir::wave::{Pwl, PwlJoin, PwlRepeat};
    use spicerack_ir::{
        Builder, Code, DefId, Design, Diags, ElemKind, Element, ExprId, List, ModelFamily, NO_ROW,
        NameId, Polarity, Ref, ScopeId, Span, Violation, WaveRef, check,
    };

    /// Builds a design with `f` and returns `check`'s codes.
    fn checked(f: impl FnOnce(&mut Builder<'_>, ScopeId)) -> Vec<Code> {
        let mut d = Design::new();
        let top = deck(&mut d);
        f(&mut d.builder(), top);
        let mut diags = Diags::new();
        check(&d, &mut diags);
        codes(&diags)
    }

    /// An element header pushed as is.
    fn push(
        b: &mut Builder<'_>,
        top: ScopeId,
        kind: ElemKind,
        terms: &[&str],
        master: Option<&str>,
        detail: u32,
    ) {
        let name = b.name("e1");
        let terminals = b.names(terms);
        let model = Ref {
            name: master.map_or(NameId::NONE, |m| b.name(m)),
            def: DefId::NONE,
        };
        let row = Element {
            kind,
            name,
            terminals,
            model,
            params: List::EMPTY,
            mult: ExprId::NONE,
            detail,
        };
        b.push(top, row, Span::NONE);
    }

    fn only(v: Violation) -> Vec<Code> {
        vec![violation_code(v)]
    }

    #[test]
    fn a_valid_element_of_every_family_reports_nothing() {
        let got = checked(|b, top| {
            let one = n(b, 1);
            let r = b.detail(Passive {
                value: one,
                ..Passive::PLAIN
            });
            push(b, top, ElemKind::Resistor, &["a", "b"], None, r);
            let v = b.detail(source(one));
            push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            let s = b.detail(semi());
            push(
                b,
                top,
                ElemKind::Mosfet,
                &["d", "g", "s", "b"],
                Some("nch"),
                s,
            );
            let l = b.detail(spicerack_ir::line::Line {
                z0: one,
                td: one,
                ..line()
            });
            push(
                b,
                top,
                ElemKind::LosslessLine,
                &["a", "0", "b", "0"],
                None,
                l,
            );
            push(b, top, ElemKind::Instance, &["a"], Some("sub"), NO_ROW);
            push(b, top, ElemKind::Unresolved, &[], Some("kit_cell"), NO_ROW);
            let name = b.name("nch");
            b.push(
                top,
                model(name, ModelFamily::Bsim4, Polarity::N),
                Span::NONE,
            );
        });
        assert_eq!(got, Vec::<Code>::new());
    }

    #[test]
    fn terminal_count_outside_the_range() {
        let got = checked(|b, top| {
            let one = n(b, 1);
            let r = b.detail(Passive {
                value: one,
                ..Passive::PLAIN
            });
            push(b, top, ElemKind::Resistor, &["a"], None, r);
        });
        assert_eq!(got, only(Violation::TerminalCount));
    }

    #[test]
    fn detail_missing_present_or_out_of_range() {
        assert_eq!(
            checked(|b, top| push(b, top, ElemKind::Resistor, &["a", "b"], None, NO_ROW)),
            only(Violation::DetailRow)
        );
        assert_eq!(
            checked(|b, top| {
                let one = n(b, 1);
                b.detail(Passive {
                    value: one,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Instance, &["a"], Some("sub"), 0);
            }),
            only(Violation::DetailRow)
        );
        assert_eq!(
            checked(|b, top| push(b, top, ElemKind::Resistor, &["a", "b"], None, 7)),
            only(Violation::DetailRow)
        );
        assert_eq!(
            checked(|b, top| {
                let one = n(b, 1);
                let wave = WaveRef {
                    kind: spicerack_ir::WaveKind::Sin,
                    index: 3,
                };
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            only(Violation::DetailRow)
        );
    }

    /// One broken field per case; each must give exactly one `FieldDomain`.
    #[test]
    fn every_field_domain_rule_is_reported_alone() {
        type Case = fn(&mut Builder<'_>, ScopeId);
        let cases: [(&str, Case); 26] = [
            ("diode without a model", |b, top| {
                let s = b.detail(semi());
                push(b, top, ElemKind::Diode, &["a", "0"], None, s);
            }),
            ("source naming a model", |b, top| {
                let one = n(b, 1);
                let v = b.detail(source(one));
                push(b, top, ElemKind::VSource, &["a", "0"], Some("vm"), v);
            }),
            ("unresolved master bound", |b, top| {
                b.subckt(top, "sub", &[], &[], Span::NONE);
                let name = b.name("e1");
                let model = Ref {
                    name: b.name("sub"),
                    def: DefId(0),
                };
                let row = Element {
                    kind: ElemKind::Unresolved,
                    name,
                    terminals: List::EMPTY,
                    model,
                    params: List::EMPTY,
                    mult: ExprId::NONE,
                    detail: NO_ROW,
                };
                b.push(top, row, Span::NONE);
            }),
            ("ic without its mode", |b, top| {
                let one = n(b, 1);
                let c = b.detail(Passive {
                    value: one,
                    ic: one,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Capacitor, &["a", "0"], None, c);
            }),
            ("ic on a resistor", |b, top| {
                let one = n(b, 1);
                let r = b.detail(Passive {
                    value: one,
                    ic: one,
                    ic_mode: IcMode::UicOnly,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Resistor, &["a", "0"], None, r);
            }),
            ("source without dc", |b, top| {
                let v = b.detail(source(ExprId::NONE));
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("ac magnitude without phase", |b, top| {
                let one = n(b, 1);
                let v = b.detail(spicerack_ir::source::Source {
                    ac_mag: one,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("port without its port row", |b, top| {
                let one = n(b, 1);
                let v = b.detail(source(one));
                push(b, top, ElemKind::Port, &["a", "0"], None, v);
            }),
            ("cccs without a sense", |b, top| {
                let one = n(b, 1);
                let f = b.detail(Controlled {
                    gain: one,
                    sense: NameId::NONE,
                });
                push(b, top, ElemKind::Cccs, &["a", "0"], None, f);
            }),
            ("vcvs with a sense", |b, top| {
                let one = n(b, 1);
                let sense = b.name("v1");
                let e = b.detail(Controlled { gain: one, sense });
                push(b, top, ElemKind::Vcvs, &["a", "0", "c", "0"], None, e);
            }),
            ("poly with the wrong control count", |b, top| {
                let one = n(b, 1);
                let controls = b.names(&["a", "0", "b"]);
                let coeffs = b.exprs(&[one]);
                let p = b.detail(Poly {
                    controls,
                    coeffs,
                    dimension: 2,
                    output: Quantity::Voltage,
                    control: Quantity::Voltage,
                });
                push(b, top, ElemKind::Poly, &["o", "0"], None, p);
            }),
            ("table with an odd point list", |b, top| {
                let one = n(b, 1);
                let points = b.exprs(&[one, one, one]);
                let t = b.detail(TableSource {
                    input: one,
                    points,
                    delta: ExprId::NONE,
                    law: TableLaw::Exact,
                    output: Quantity::Voltage,
                });
                push(b, top, ElemKind::Table, &["o", "0"], None, t);
            }),
            ("charge capacitor with a voltage output", |b, top| {
                let one = n(b, 1);
                let q = b.detail(behav(one, OutKind::Voltage));
                push(b, top, ElemKind::ChargeCapacitor, &["a", "0"], None, q);
            }),
            (
                "four-terminal laplace with an input expression",
                |b, top| {
                    let one = n(b, 1);
                    let num = b.exprs(&[one]);
                    let den = b.exprs(&[one]);
                    let l = b.detail(Laplace {
                        num,
                        den,
                        input: one,
                        output: Quantity::Voltage,
                        form: LaplaceForm::Polynomial,
                    });
                    push(b, top, ElemKind::Laplace, &["o", "0", "i", "0"], None, l);
                },
            ),
            ("ideal delay with a sample count", |b, top| {
                let one = n(b, 1);
                let e = b.detail(Delay {
                    td: one,
                    gain: one,
                    samples: one,
                    law: DelayLaw::Ideal,
                });
                push(b, top, ElemKind::Delay, &["o", "0", "i", "0"], None, e);
            }),
            ("semi ic mode without ic", |b, top| {
                let s = b.detail(spicerack_ir::semi::Semi {
                    ic_mode: IcMode::InOp,
                    ..semi()
                });
                push(b, top, ElemKind::Diode, &["a", "0"], Some("dm"), s);
            }),
            ("current switch without a sense", |b, top| {
                let s = b.detail(Switch {
                    sense: NameId::NONE,
                    control: ExprId::NONE,
                    initial: InitialState::Unset,
                });
                push(b, top, ElemKind::ISwitch, &["a", "0"], Some("wm"), s);
            }),
            ("lossless line without z0", |b, top| {
                let one = n(b, 1);
                let l = b.detail(spicerack_ir::line::Line { td: one, ..line() });
                push(
                    b,
                    top,
                    ElemKind::LosslessLine,
                    &["a", "0", "b", "0"],
                    None,
                    l,
                );
            }),
            ("lossless line given two ways", |b, top| {
                let one = n(b, 1);
                let l = b.detail(spicerack_ir::line::Line {
                    z0: one,
                    td: one,
                    freq: one,
                    nl: one,
                    ..line()
                });
                push(
                    b,
                    top,
                    ElemKind::LosslessLine,
                    &["a", "0", "b", "0"],
                    None,
                    l,
                );
            }),
            ("coupling of one inductor", |b, top| {
                let one = n(b, 1);
                let inductors = b.names(&["l1"]);
                let k = b.detail(Coupling { inductors, k: one });
                push(b, top, ElemKind::Coupling, &[], None, k);
            }),
            ("pulse without a rise", |b, top| {
                let one = n(b, 1);
                let wave = b.wave(spicerack_ir::wave::Pulse {
                    rise: ExprId::NONE,
                    ..pulse(one)
                });
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("second sine frequency without its amplitude", |b, top| {
                let one = n(b, 1);
                let wave = b.wave(spicerack_ir::wave::Sin {
                    freq2: one,
                    ..sin(one)
                });
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("pwl that never repeats with a repeat time", |b, top| {
                let one = n(b, 1);
                let points = b.exprs(&[one, one]);
                let wave = b.wave(Pwl {
                    points,
                    delay: ExprId::NONE,
                    repeat_at: one,
                    repeat_count: ExprId::NONE,
                    join_width: ExprId::NONE,
                    file: NameId::NONE,
                    repeat: PwlRepeat::Never,
                    join: PwlJoin::Jump,
                });
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("code model without its name", |b, top| {
                let name = b.name("xf");
                b.push(
                    top,
                    model(name, ModelFamily::CodeModel, Polarity::None),
                    Span::NONE,
                );
            }),
            ("mosfet model without a polarity", |b, top| {
                let name = b.name("nch");
                b.push(
                    top,
                    model(name, ModelFamily::Bsim4, Polarity::None),
                    Span::NONE,
                );
            }),
            ("module model without its module", |b, top| {
                let name = b.name("m1");
                b.push(
                    top,
                    model(name, ModelFamily::Module, Polarity::None),
                    Span::NONE,
                );
            }),
        ];
        for (what, case) in cases {
            assert_eq!(checked(case), only(Violation::FieldDomain), "{what}");
        }
    }
}
