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
    use spicerack_ir::passive::{Coupling, IcMode, Passive, TcBase, TcLaw};
    use spicerack_ir::semi::{InitialState, Switch};
    use spicerack_ir::source::DcRule;
    use spicerack_ir::wave::{PulseLaw, Pwl, PwlJoin, PwlRepeat};
    use spicerack_ir::{
        Builder, Code, DefId, Design, Diags, ElemKind, Element, ExprId, List, ModelFamily, MultLaw,
        NO_ROW, NameId, Polarity, Ref, ScopeId, Span, Violation, WaveRef, check,
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
        let name = unique(b, top);
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
            mult_law: MultLaw::Parallel,
        };
        b.push(top, row, Span::NONE);
    }

    /// Pushes a model of `family` named `name` and returns a reference bound to it.
    fn bound(b: &mut Builder<'_>, top: ScopeId, name: &str, family: ModelFamily) -> Ref {
        let n = b.name(name);
        let polarity = if matches!(family, ModelFamily::Bsim4 | ModelFamily::Mesa) {
            Polarity::N
        } else {
            Polarity::None
        };
        // a versioned family states its version
        let version = if family == ModelFamily::Bsim4 {
            b.string("4.8.3")
        } else {
            ExprId::NONE
        };
        let row = spicerack_ir::Model {
            version,
            ..model(n, family, polarity)
        };
        b.model(top, row, Span::NONE);
        let def = DefId(b.design().definitions().len() as u32 - 1);
        Ref { name: n, def }
    }

    /// An element of `kind` whose master is `master`, with a valid detail row.
    fn with_master(b: &mut Builder<'_>, top: ScopeId, kind: ElemKind, master: Ref) {
        let one = n(b, 1);
        let (terms, detail): (&[&str], u32) = match kind {
            ElemKind::VSource => (&["a", "0"], b.detail(source(one))),
            _ => (
                &["a", "b"],
                b.detail(Passive {
                    value: one,
                    ..Passive::PLAIN
                }),
            ),
        };
        let name = unique(b, top);
        let terminals = b.names(terms);
        let row = Element {
            kind,
            name,
            terminals,
            model: master,
            params: List::EMPTY,
            mult: ExprId::NONE,
            detail,
            mult_law: MultLaw::Parallel,
        };
        b.push(top, row, Span::NONE);
    }

    /// A name no element of `top` has yet: `e` and the scope's item count (A13: names are unique
    /// in a scope).
    fn unique(b: &mut Builder<'_>, top: ScopeId) -> spicerack_ir::NameId {
        let k = b.design().items(top).len();
        b.name(&format!("e{k}"))
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
            let wave = b.wave(pulse(one));
            let w = b.detail(spicerack_ir::source::Source {
                wave,
                dc_rule: DcRule::WaveAtZero,
                ..source(ExprId::NONE)
            });
            push(b, top, ElemKind::VSource, &["a", "0"], None, w);
            // under ngspice's zero rule an omitted rise, fall, width and period are legal (A17)
            let omitted = b.wave(spicerack_ir::wave::Pulse {
                rise: ExprId::NONE,
                fall: ExprId::NONE,
                width: ExprId::NONE,
                period: ExprId::NONE,
                given_zero: spicerack_ir::wave::GivenZero::SpiceDefault,
                ..pulse(one)
            });
            let o = b.detail(spicerack_ir::source::Source {
                wave: omitted,
                ..source(one)
            });
            push(b, top, ElemKind::VSource, &["a", "0"], None, o);
            let name = b.name("nch");
            let version = b.string("4.8.3");
            b.push(
                top,
                spicerack_ir::Model {
                    version,
                    ..model(name, ModelFamily::Bsim4, Polarity::N)
                },
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
        let cases: [(&str, Case); 39] = [
            ("diode without a model", |b, top| {
                let s = b.detail(semi());
                push(b, top, ElemKind::Diode, &["a", "0"], None, s);
            }),
            ("behavioural source naming a model", |b, top| {
                let one = n(b, 1);
                let v = b.detail(behav(one, spicerack_ir::controlled::OutKind::Voltage));
                push(b, top, ElemKind::Behav, &["a", "0"], Some("bm"), v);
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
                    mult_law: MultLaw::Parallel,
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
            ("wave-at-zero rule with a dc", |b, top| {
                let one = n(b, 1);
                let wave = b.wave(pulse(one));
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    dc_rule: DcRule::WaveAtZero,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("wave-at-zero rule without a wave", |b, top| {
                let v = b.detail(spicerack_ir::source::Source {
                    dc_rule: DcRule::WaveAtZero,
                    ..source(ExprId::NONE)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("vacask pulse with a zero rise", |b, top| {
                let one = n(b, 1);
                let zero = n(b, 0);
                let wave = b.wave(spicerack_ir::wave::Pulse {
                    rise: zero,
                    law: PulseLaw::Vacask,
                    ..pulse(one)
                });
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            }),
            ("voltage source bound to a transistor model", |b, top| {
                let m = bound(b, top, "nch", ModelFamily::Bsim4);
                with_master(b, top, ElemKind::VSource, m);
            }),
            ("resistor bound to a builtin source model", |b, top| {
                let m = bound(b, top, "v", ModelFamily::SourceV);
                with_master(b, top, ElemKind::Resistor, m);
            }),
            ("builtin model with parameters", |b, top| {
                let name = b.name("v");
                let one = n(b, 1);
                let params = b.params(&[("dc", one)]);
                b.push(
                    top,
                    spicerack_ir::Model {
                        params,
                        ..model(name, ModelFamily::SourceV, Polarity::None)
                    },
                    Span::NONE,
                );
            }),
            ("builtin model with a polarity", |b, top| {
                let name = b.name("v");
                b.push(
                    top,
                    model(name, ModelFamily::SourceV, Polarity::N),
                    Span::NONE,
                );
            }),
            ("a constant resistor measured from 27 degrees", |b, top| {
                let one = n(b, 1);
                let r = b.detail(Passive {
                    value: one,
                    tc1: one,
                    tc_base: TcBase::Fixed27,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Resistor, &["a", "b"], None, r);
            }),
            ("a reciprocal temperature law on a resistor", |b, top| {
                let value = b.v("a", None);
                let one = n(b, 1);
                let r = b.detail(Passive {
                    value,
                    tc1: one,
                    tc_law: TcLaw::Reciprocal,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Resistor, &["a", "b"], None, r);
            }),
            ("a behavioural base with no coefficient", |b, top| {
                let one = n(b, 1);
                let row = spicerack_ir::controlled::Behav {
                    tc_base: TcBase::Fixed27,
                    ..behav(one, spicerack_ir::controlled::OutKind::Voltage)
                };
                let v = b.detail(row);
                push(b, top, ElemKind::Behav, &["a", "0"], None, v);
            }),
            ("ngspice's multiplier law on a resistor", |b, top| {
                let one = n(b, 1);
                let detail = b.detail(Passive {
                    value: one,
                    ..Passive::PLAIN
                });
                let name = unique(b, top);
                let terminals = b.names(&["a", "b"]);
                let row = Element {
                    kind: ElemKind::Resistor,
                    name,
                    terminals,
                    model: Ref {
                        name: NameId::NONE,
                        def: DefId::NONE,
                    },
                    params: List::EMPTY,
                    mult: one,
                    detail,
                    mult_law: MultLaw::NgspiceAppend,
                };
                b.push(top, row, Span::NONE);
            }),
            ("ngspice's multiplier law with no multiplier", |b, top| {
                let name = unique(b, top);
                let model = Ref {
                    name: b.name("sub"),
                    def: DefId::NONE,
                };
                let row = Element {
                    kind: ElemKind::Instance,
                    name,
                    terminals: List::EMPTY,
                    model,
                    params: List::EMPTY,
                    mult: ExprId::NONE,
                    detail: NO_ROW,
                    mult_law: MultLaw::NgspiceAppend,
                };
                b.push(top, row, Span::NONE);
            }),
            (
                "an omitted exponential delay under the as-given rule",
                |b, top| {
                    let one = n(b, 1);
                    let wave = b.wave(spicerack_ir::wave::Exp {
                        v1: one,
                        v2: one,
                        td1: ExprId::NONE,
                        tau1: one,
                        td2: one,
                        tau2: one,
                        td2_from: spicerack_ir::wave::TdFrom::Zero,
                        given_zero: spicerack_ir::wave::GivenZero::AsGiven,
                    });
                    let v = b.detail(spicerack_ir::source::Source {
                        wave,
                        ..source(one)
                    });
                    push(b, top, ElemKind::VSource, &["a", "0"], None, v);
                },
            ),
        ];
        for (what, case) in cases {
            assert_eq!(checked(case), only(Violation::FieldDomain), "{what}");
        }
    }

    /// An element of `kind` named `name` in `scope`, with a valid detail row.
    fn named(b: &mut Builder<'_>, scope: ScopeId, kind: ElemKind, name: &str) {
        let one = n(b, 1);
        let detail = match kind {
            ElemKind::VSource => b.detail(source(one)),
            _ => b.detail(Passive {
                value: one,
                ..Passive::PLAIN
            }),
        };
        let name = b.name(name);
        let terminals = b.names(&["a", "0"]);
        let row = Element {
            kind,
            name,
            terminals,
            model: Ref {
                name: NameId::NONE,
                def: DefId::NONE,
            },
            params: List::EMPTY,
            mult: ExprId::NONE,
            detail,
            mult_law: MultLaw::Parallel,
        };
        b.push(scope, row, Span::NONE);
    }

    #[test]
    fn two_elements_of_one_name_in_one_scope_across_kinds() {
        let mut d = Design::new();
        let top = deck(&mut d);
        let mut b = d.builder();
        named(&mut b, top, ElemKind::Resistor, "x1");
        named(&mut b, top, ElemKind::VSource, "x1");
        let mut diags = Diags::new();
        check(&d, &mut diags);
        assert_eq!(codes(&diags), only(Violation::DuplicateElement));
        assert_eq!(diags.rows()[0].arg, 1, "the second element's row");
    }

    #[test]
    fn one_name_in_two_scopes_is_legal() {
        let got = checked(|b, top| {
            named(b, top, ElemKind::Resistor, "x1");
            let (_, body) = b.subckt(top, "sub", &[], &[], Span::NONE);
            named(b, body, ElemKind::VSource, "x1");
        });
        assert_eq!(got, Vec::<Code>::new());
    }

    /// A17 (ng-sources' table): under `SpiceDefault` the omit-only fields of SIN, SFFM, AM and
    /// TRRANDOM may be absent; under `AsGiven` they may not.
    #[test]
    fn omitted_fields_under_the_spice_rule() {
        use spicerack_ir::wave::{
            Am, AmModTime, GivenZero, PreDelay, Sffm, SffmLaw, TrRandom, TrRandomKind,
        };
        let source_of = |b: &mut Builder<'_>, top: ScopeId, wave: spicerack_ir::WaveRef| {
            let one = n(b, 1);
            let v = b.detail(spicerack_ir::source::Source {
                wave,
                ..source(one)
            });
            push(b, top, ElemKind::VSource, &["a", "0"], None, v);
        };
        let am = |b: &mut Builder<'_>, rule: GivenZero| {
            let one = n(b, 1);
            let none = ExprId::NONE;
            b.wave(Am {
                offset: one,
                amplitude: one,
                mod_offset: one,
                mod_index: none,
                mod_freq: none,
                carrier_freq: none,
                delay: none,
                phase_m_deg: none,
                phase_c_deg: none,
                mod_time: AmModTime::Delay,
                pre: PreDelay::Zero,
                given_zero: rule,
            })
        };
        let valid = checked(|b, top| {
            let one = n(b, 1);
            let none = ExprId::NONE;
            let w = b.wave(spicerack_ir::wave::Sin {
                delay: none,
                damping: none,
                phase_deg: none,
                given_zero: GivenZero::SpiceDefault,
                ..sin(one)
            });
            source_of(b, top, w);
            let w = b.wave(Sffm {
                offset: one,
                amplitude: one,
                carrier_freq: none,
                mod_index: none,
                signal_freq: none,
                delay: none,
                phase_c_deg: none,
                phase_m_deg: none,
                law: SffmLaw::Ngspice,
                pre: PreDelay::Zero,
                given_zero: GivenZero::SpiceDefault,
            });
            source_of(b, top, w);
            let w = am(b, GivenZero::SpiceDefault);
            source_of(b, top, w);
            let w = b.wave(TrRandom {
                ts: one,
                td: none,
                p1: none,
                p2: none,
                kind: TrRandomKind::Gauss,
                given_zero: GivenZero::SpiceDefault,
            });
            source_of(b, top, w);
        });
        assert_eq!(valid, Vec::<Code>::new());
        let as_given = checked(|b, top| {
            let w = am(b, GivenZero::AsGiven);
            source_of(b, top, w);
        });
        assert_eq!(as_given, only(Violation::FieldDomain));
    }

    /// A17: a charge-defined capacitor's coefficient may divide the charge (`Reciprocal`);
    /// any other behavioural row may not, nor one with no coefficient.
    #[test]
    fn the_reciprocal_law_on_a_behavioural_row() {
        use spicerack_ir::controlled::{Behav, OutKind};
        let row = |b: &mut Builder<'_>, output: OutKind, tc: bool| {
            let one = n(b, 1);
            let tc1 = if tc { n(b, 1) } else { ExprId::NONE };
            Behav {
                tc1,
                tc_law: TcLaw::Reciprocal,
                ..behav(one, output)
            }
        };
        let ok = checked(|b, top| {
            let r = row(b, OutKind::Charge, true);
            let q = b.detail(r);
            push(b, top, ElemKind::ChargeCapacitor, &["a", "0"], None, q);
        });
        assert_eq!(ok, Vec::<Code>::new());
        let plain = checked(|b, top| {
            let r = row(b, OutKind::Voltage, true);
            let v = b.detail(r);
            push(b, top, ElemKind::Behav, &["a", "0"], None, v);
        });
        assert_eq!(plain, only(Violation::FieldDomain));
        let no_tc = checked(|b, top| {
            let r = row(b, OutKind::Charge, false);
            let q = b.detail(r);
            push(b, top, ElemKind::ChargeCapacitor, &["a", "0"], None, q);
        });
        assert_eq!(no_tc, only(Violation::FieldDomain));
    }

    /// A19 (R38): invariants every simulator holds, each broken alone; the neighbours are legal.
    #[test]
    fn a19_invariants_broken_alone() {
        type Case = fn(&mut Builder<'_>, ScopeId);
        fn semi_with(b: &mut Builder<'_>, top: ScopeId, key: &str) {
            let one = n(b, 1);
            let s = b.detail(semi());
            let name = unique(b, top);
            let terminals = b.names(&["a", "0"]);
            let params = b.params(&[(key, one)]);
            let model = Ref {
                name: b.name("dm"),
                def: DefId::NONE,
            };
            let row = Element {
                kind: ElemKind::Diode,
                name,
                terminals,
                model,
                params,
                mult: ExprId::NONE,
                detail: s,
                mult_law: MultLaw::Parallel,
            };
            b.push(top, row, Span::NONE);
        }
        fn model_with(b: &mut Builder<'_>, top: ScopeId, family: ModelFamily, key: &str) {
            let name = unique(b, top);
            let one = n(b, 1);
            let params = b.params(&[(key, one)]);
            let polarity = if matches!(family, ModelFamily::Bsim4 | ModelFamily::Mesa) {
                Polarity::N
            } else {
                Polarity::None
            };
            b.push(
                top,
                spicerack_ir::Model {
                    params,
                    ..model(name, family, polarity)
                },
                Span::NONE,
            );
        }
        let cases: [(&str, Case); 11] = [
            ("a resistor with no value and no model", |b, top| {
                let r = b.detail(Passive::PLAIN);
                push(b, top, ElemKind::Resistor, &["a", "b"], None, r);
            }),
            ("an AC resistance on a capacitor", |b, top| {
                let one = n(b, 1);
                let c = b.detail(Passive {
                    value: one,
                    ac: one,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Capacitor, &["a", "0"], None, c);
            }),
            // R74: tce is a resistor's on every dialect we run (ngspice res.c; VACASK
            // sp_capacitor and sp_inductor list no tce; Xyce 7.10 "Unrecognized parameter TCE
            // for device C1" and L1)
            ("an exponential coefficient on a capacitor", |b, top| {
                let one = n(b, 1);
                let c = b.detail(Passive {
                    value: one,
                    tce: one,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Capacitor, &["a", "0"], None, c);
            }),
            ("an exponential coefficient on an inductor", |b, top| {
                let one = n(b, 1);
                let l = b.detail(Passive {
                    value: one,
                    tce: one,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Inductor, &["a", "0"], None, l);
            }),
            ("a noiseless constant inductor", |b, top| {
                let one = n(b, 1);
                let l = b.detail(Passive {
                    value: one,
                    noisy: false,
                    ..Passive::PLAIN
                });
                push(b, top, ElemKind::Inductor, &["a", "0"], None, l);
            }),
            ("a diode parameter named as its area field", |b, top| {
                semi_with(b, top, "area")
            }),
            ("a diode parameter m (its multiplicity)", |b, top| {
                semi_with(b, top, "m")
            }),
            ("a diode parameter temp (its temperature)", |b, top| {
                semi_with(b, top, "temp")
            }),
            ("a level parameter on a family that fixes it", |b, top| {
                model_with(b, top, ModelFamily::Bsim4, "level")
            }),
            ("a lossless line with neither form", |b, top| {
                let one = n(b, 1);
                let l = b.detail(spicerack_ir::line::Line { z0: one, ..line() });
                push(
                    b,
                    top,
                    ElemKind::LosslessLine,
                    &["a", "0", "b", "0"],
                    None,
                    l,
                );
            }),
            ("a version on a family with one version", |b, top| {
                let name = unique(b, top);
                let version = b.string("2");
                b.push(
                    top,
                    spicerack_ir::Model {
                        version,
                        ..model(name, ModelFamily::Mos1, Polarity::N)
                    },
                    Span::NONE,
                );
            }),
        ];
        for (what, case) in cases {
            assert_eq!(checked(case), only(Violation::FieldDomain), "{what}");
        }
        // a model value that reads the circuit: the kernel's `ProbeContext` (a model value is a
        // field constant in time), not a second report of ours
        let probe = checked(|b, top| {
            let name = unique(b, top);
            let va = b.v("a", None);
            let params = b.params(&[("is", va)]);
            b.push(
                top,
                spicerack_ir::Model {
                    params,
                    ..model(name, ModelFamily::Diode, Polarity::None)
                },
                Span::NONE,
            );
        });
        assert_eq!(probe, only(Violation::ProbeContext));
        let legal = checked(|b, top| {
            let one = n(b, 1);
            // a value given by the model alone, a noiseless resistor, a noiseless circuit-valued
            // capacitor, a parameter m on a subcircuit instance (a declared m, A15), a level on
            // MESA and on an unknown family, a version on BSIM4, a lossless line by frequency
            let r = b.detail(Passive::PLAIN);
            push(b, top, ElemKind::Resistor, &["a", "b"], Some("rm"), r);
            let r = b.detail(Passive {
                value: one,
                noisy: false,
                ..Passive::PLAIN
            });
            push(b, top, ElemKind::Resistor, &["a", "b"], None, r);
            let va = b.v("a", None);
            let c = b.detail(Passive {
                value: va,
                noisy: false,
                ..Passive::PLAIN
            });
            push(b, top, ElemKind::Capacitor, &["a", "0"], None, c);
            let name = unique(b, top);
            let terminals = b.names(&["a"]);
            let params = b.params(&[("m", one)]);
            let model_ref = Ref {
                name: b.name("sub"),
                def: DefId::NONE,
            };
            let x = Element {
                kind: ElemKind::Instance,
                name,
                terminals,
                model: model_ref,
                params,
                mult: ExprId::NONE,
                detail: NO_ROW,
                mult_law: MultLaw::Parallel,
            };
            b.push(top, x, Span::NONE);
            model_with(b, top, ModelFamily::Mesa, "level");
            let name = unique(b, top);
            let word = b.name("hvmos");
            let params = b.params(&[("level", one)]);
            b.push(
                top,
                spicerack_ir::Model {
                    params,
                    word,
                    ..model(name, ModelFamily::Other, Polarity::None)
                },
                Span::NONE,
            );
            let name = unique(b, top);
            let version = b.string("4.8");
            b.push(
                top,
                spicerack_ir::Model {
                    version,
                    ..model(name, ModelFamily::Bsim4, Polarity::N)
                },
                Span::NONE,
            );
            let l = b.detail(spicerack_ir::line::Line {
                z0: one,
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
        });
        assert_eq!(legal, Vec::<Code>::new());
    }

    /// A20 (R39): a lossless line may name the simulator's own ideal line, and only that; the
    /// model has no parameters and no polarity.
    #[test]
    fn a_lossless_line_through_its_ideal_line_model() {
        fn tline(b: &mut Builder<'_>, top: ScopeId, master: Ref) {
            let one = n(b, 1);
            let detail = b.detail(spicerack_ir::line::Line {
                z0: one,
                td: one,
                ..line()
            });
            let name = unique(b, top);
            let terminals = b.names(&["a", "0", "b", "0"]);
            let row = Element {
                kind: ElemKind::LosslessLine,
                name,
                terminals,
                model: master,
                params: List::EMPTY,
                mult: ExprId::NONE,
                detail,
                mult_law: MultLaw::Parallel,
            };
            b.push(top, row, Span::NONE);
        }
        let ok = checked(|b, top| {
            let m = bound(b, top, "tl", ModelFamily::TlineIdeal);
            tline(b, top, m);
        });
        assert_eq!(ok, Vec::<Code>::new());
        let wrong = checked(|b, top| {
            let m = bound(b, top, "nch", ModelFamily::Bsim4);
            tline(b, top, m);
        });
        assert_eq!(wrong, only(Violation::FieldDomain));
        let params = checked(|b, top| {
            let name = b.name("tl");
            let one = n(b, 1);
            let params = b.params(&[("z0", one)]);
            b.push(
                top,
                spicerack_ir::Model {
                    params,
                    ..model(name, ModelFamily::TlineIdeal, Polarity::None)
                },
                Span::NONE,
            );
        });
        assert_eq!(params, only(Violation::FieldDomain));
    }

    /// A model of a versioned family states its version (ng-models' L5 finding: "no version"
    /// means each simulator's own default); its neighbour, a family of one version with none
    /// stated, stays legal.
    #[test]
    fn a_versioned_family_states_its_version() {
        let versionless = checked(|b, top| {
            let name = b.name("nch");
            b.push(
                top,
                model(name, ModelFamily::Bsim4, Polarity::N),
                Span::NONE,
            );
        });
        assert_eq!(versionless, only(Violation::FieldDomain));
        let given = checked(|b, top| {
            let name = b.name("nch");
            let version = b.string("4.8.3");
            b.push(
                top,
                spicerack_ir::Model {
                    version,
                    ..model(name, ModelFamily::Bsim4, Polarity::N)
                },
                Span::NONE,
            );
        });
        assert_eq!(given, Vec::<Code>::new());
        let single = checked(|b, top| {
            let name = b.name("m1");
            b.push(top, model(name, ModelFamily::Mos1, Polarity::N), Span::NONE);
        });
        assert_eq!(single, Vec::<Code>::new());
    }

    /// A30's finding 4: the PWL repeat and join pairs no simulator states (`Ramp` under
    /// `Period`, `RampWidth` or `Extrapolate` under `From`) are refused; the six others are legal.
    #[test]
    fn the_pwl_repeat_and_join_pairs() {
        for (repeat, join, legal) in [
            (PwlRepeat::From, PwlJoin::Jump, true),
            (PwlRepeat::From, PwlJoin::Ramp, true),
            (PwlRepeat::From, PwlJoin::RampWidth, false),
            (PwlRepeat::From, PwlJoin::Extrapolate, false),
            (PwlRepeat::Period, PwlJoin::Jump, true),
            (PwlRepeat::Period, PwlJoin::Ramp, false),
            (PwlRepeat::Period, PwlJoin::RampWidth, true),
            (PwlRepeat::Period, PwlJoin::Extrapolate, true),
        ] {
            let got = checked(|b, top| {
                let one = n(b, 1);
                let points = b.exprs(&[one, one]);
                let wave = b.wave(Pwl {
                    points,
                    delay: ExprId::NONE,
                    repeat_at: one,
                    repeat_count: ExprId::NONE,
                    join_width: if join == PwlJoin::RampWidth {
                        one
                    } else {
                        ExprId::NONE
                    },
                    file: NameId::NONE,
                    repeat,
                    join,
                });
                let v = b.detail(spicerack_ir::source::Source {
                    wave,
                    ..source(one)
                });
                push(b, top, ElemKind::VSource, &["a", "0"], None, v);
            });
            let want = if legal {
                Vec::new()
            } else {
                only(Violation::FieldDomain)
            };
            assert_eq!(got, want, "{repeat:?} {join:?}");
        }
    }

    /// R61/A36: a subcircuit instance has as many terminals as its bound master has ports
    /// (ngspice refuses the deck, VACASK fills missing trailing terminals with internal nodes:
    /// the IR states neither). A short and a long instance each report `InstancePortCount`
    /// alone; neighbours: the matching count, and an instance of a master no file defines.
    #[test]
    fn an_instance_has_its_masters_port_count() {
        let codes_of = |terminals: &str, master: &str| {
            let text = format!(
                "file \"t.sp\" deck\n  subckt div (a b)\n    element resistor r1 (a b) value=1\n  element instance x1 ({terminals}) model={master}\n"
            );
            let mut d = Design::new();
            let mut diags = Diags::new();
            spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
            assert!(diags.is_empty(), "{:?}", diags.rows());
            spicerack_ir::link(&mut d, &mut diags);
            let mut out = Diags::new();
            check(&d, &mut out);
            codes(&out)
        };
        assert_eq!(
            codes_of("n1", "div"),
            only(Violation::InstancePortCount),
            "short"
        );
        assert_eq!(
            codes_of("n1 n2 n3", "div"),
            only(Violation::InstancePortCount),
            "long"
        );
        assert_eq!(codes_of("n1 n2", "div"), Vec::<Code>::new(), "matching");
        assert_eq!(codes_of("n1", "elsewhere"), Vec::<Code>::new(), "unbound");
    }

    /// A subcircuit that instantiates itself, directly or through another, has no finite
    /// elaboration: ngspice-45 stops at "unknown subckt: x0.x1.x1…" and VACASK says
    /// "Hierarchical recursion detected." (`tests/fixtures/hierarchy/{ngspice,vacask}.out`). Each instance on
    /// the cycle reports `InstanceCycle`, used or not; neighbours: a chain without a cycle, and
    /// a top-level instance of a subcircuit (no enclosing subcircuit, no edge).
    #[test]
    fn a_subcircuit_never_reaches_itself() {
        let codes_of = |body: &str| {
            let text = format!("file \"t.sp\" deck\n{body}");
            let mut d = Design::new();
            let mut diags = Diags::new();
            spicerack_ir::parse_unchecked(&text, &mut d.builder(), &mut diags);
            assert!(diags.is_empty(), "{:?}", diags.rows());
            spicerack_ir::link(&mut d, &mut diags);
            let mut out = Diags::new();
            check(&d, &mut out);
            codes(&out)
        };
        let sub = |name: &str, inst: &str| {
            format!(
                "  subckt {name} (a)\n    element resistor r1 (a 0) value=1\n    element instance x1 (a) model={inst}\n"
            )
        };
        let leaf = "  subckt q (a)\n    element resistor r1 (a 0) value=1\n";
        let top = "  element instance x0 (n) model=p\n";
        assert_eq!(
            codes_of(&sub("p", "p")),
            only(Violation::InstanceCycle),
            "self"
        );
        let two = codes_of(&format!("{}{}{top}", sub("p", "q"), sub("q", "p")));
        let cycle = violation_code(Violation::InstanceCycle);
        assert_eq!(two, vec![cycle, cycle], "a two-cycle: both instances");
        assert_eq!(
            codes_of(&format!("{}{leaf}{top}", sub("p", "q"))),
            Vec::<Code>::new(),
            "a chain"
        );
        assert_eq!(
            codes_of(&format!("{leaf}  element instance x0 (n) model=q\n")),
            Vec::<Code>::new(),
            "top level"
        );
    }
}
