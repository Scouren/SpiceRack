//! `check`'s fixtures: a valid design that reports nothing, and for every kernel-owned
//! `Violation` a design broken in exactly that invariant, reported with that code and nothing
//! else. Crate-internal, because the public doors refuse to build a broken design: each fixture
//! corrupts one table directly.
//!
//! Oracle: one hand-broken fixture per variant; the covered variants are compared by name with
//! `Violation::ALL` minus the two the circuit owner's rows own (`TerminalCount`, `DetailRow`).
//!
//! Contents: none (tests only).

use crate::check::{Violation, check};
use crate::design::Design;
use crate::diag::{Diags, violation_code};
use crate::expr::{Func, Literal, NumForm, UnOp};
use crate::id::{DefId, ExprId, ItemRef, NameId};
use crate::item::{DefKind, Global, LoadKind, OpaqueClass, Param, Role};
use crate::link::link;
use crate::scope::Def;
use crate::span::Span;

/// A small valid deck: a param, a subcircuit with a body param, an if, a func, a load with
/// its module, an include, a global, a comment and a title.
struct Base {
    d: Design,
    param: ItemRef,
    sub_body: crate::id::ScopeId,
    one: ExprId,
}

// ---- private ----

fn base() -> Base {
    let mut d = Design::new();
    let (param, sub_body, one) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        b.title(top, "fixture", Span::NONE);
        b.comment(top, "a note", Span::NONE);
        let one = b.num(1.0);
        let param = b.param(top, "k", one, Span::NONE);
        let w = b.param_ref("k");
        let (_, sub_body) = b.subckt(top, "s", &["a", "b"], &[("w", w)], Span::NONE);
        let inner = b.param_ref("w");
        b.param(sub_body, "j", inner, Span::NONE);
        let cond = b.param_ref("k");
        b.if_(sub_body, &[cond, ExprId::NONE], Span::NONE);
        let x = b.param_ref("x");
        let body = b.call(Func::Exp, &[x]);
        b.func(top, "f", &["x"], body, Span::NONE);
        let load = b.load(top, LoadKind::Osdi, "m.osdi", Span::NONE);
        let module = b.name("m");
        b.define(DefKind::Module, module, top, load);
        b.include(top, "lib.sp", Some("tt"), Span::NONE);
        b.global(top, &["vdd"], Span::NONE);
        b.opaque(
            top,
            "ngspice",
            OpaqueClass::Inert,
            ".width out=80",
            Span::NONE,
        );
        (param, sub_body, one)
    };
    link(&mut d, &mut Diags::new());
    Base {
        d,
        param,
        sub_body,
        one,
    }
}

fn violations(d: &Design) -> Vec<Violation> {
    let mut diags = Diags::new();
    check(d, &mut diags);
    let by_code = |c| Violation::ALL.into_iter().find(|v| violation_code(*v) == c);
    diags
        .rows()
        .iter()
        .map(|r| by_code(r.code).expect("an X code"))
        .collect()
}

fn set_param(d: &mut Design, item: ItemRef, row: Param) {
    d.editor().set(item.index, row);
}

#[test]
fn a_valid_design_reports_nothing() {
    assert_eq!(violations(&base().d), vec![]);
    assert_eq!(violations(&Design::new()), vec![]);
}

#[test]
fn each_kernel_invariant_has_its_fixture() {
    type Break = fn(&mut Base);
    let cases: [(Violation, Break); 13] = [
        (Violation::HandleRange, |b| {
            let row = Param {
                name: NameId(10_000),
                value: b.one,
            };
            set_param(&mut b.d, b.param, row);
        }),
        (Violation::ScopeTree, |b| {
            // the param item in a second list
            let item = b.param;
            b.d.editor().items_mut(b.sub_body).push(item);
        }),
        (Violation::ScopeOwner, |b| {
            b.d.scopes[b.sub_body.index()].owner = b.param;
        }),
        (Violation::ExprOrder, |b| {
            let neg = b.d.builder().unary(UnOp::Neg, b.one);
            let later = b.d.builder().unary(UnOp::Neg, neg);
            // `neg` now reads a row above itself
            b.d.exprs.a[neg.index()] = later.0;
        }),
        (Violation::ExprArity, |b| {
            let sin = b.d.builder().call(Func::Sin, &[b.one]);
            b.d.builder().exprs(&[b.one, b.one]);
            b.d.exprs.b[sin.index()] = 2; // two arguments for a one-argument function
        }),
        (Violation::RefTarget, |b| {
            let read = b.d.builder().param_ref("s");
            let sub = b.d.lookup(b.sub_body, DefKind::Subckt, b.d.find_name("s"));
            b.d.exprs.b[read.index()] = sub.0; // a parameter read bound to a subckt
            let row = Param {
                name: b.d.find_name("k"),
                value: read,
            };
            set_param(&mut b.d, b.param, row);
        }),
        (Violation::NonFinite, |b| {
            let huge = Literal {
                mantissa: 1,
                frac: 0,
                exp10: 400,
                form: NumForm::Exponent,
            };
            b.d.exprs.nums.push(huge);
            b.d.exprs.a[b.one.index()] = (b.d.exprs.nums.len() - 1) as u32;
        }),
        (Violation::LiteralShape, |b| {
            let bad = Literal {
                mantissa: 0,
                frac: 1,
                exp10: 0,
                form: NumForm::Plain,
            };
            b.d.exprs.nums.push(bad);
            b.d.exprs.a[b.one.index()] = (b.d.exprs.nums.len() - 1) as u32;
        }),
        (Violation::EmptyName, |b| {
            let empty = b.d.names.intern_any("");
            set_param(
                &mut b.d,
                b.param,
                Param {
                    name: empty,
                    value: b.one,
                },
            );
        }),
        (Violation::ProbeContext, |b| {
            let probe = b.d.builder().v("out", None);
            let row = Param {
                name: b.d.find_name("k"),
                value: probe,
            };
            set_param(&mut b.d, b.param, row);
        }),
        (Violation::DuplicateDef, |b| {
            let first = b.d.defs[0];
            b.d.defs.push(Def { ..first });
        }),
        (Violation::OpaqueEmpty, |b| {
            let top = b.d.file(crate::id::FileId(0)).top;
            b.d.builder()
                .opaque(top, "ngspice", OpaqueClass::Inert, "", Span::NONE);
        }),
        (Violation::FieldDomain, |b| {
            let top = b.d.file(crate::id::FileId(0)).top;
            let global =
                b.d.items(top)
                    .iter()
                    .copied()
                    .find(|i| i.kind == crate::ItemKind::Global);
            let nets = b.d.builder().name_ids(&[NameId::GROUND]);
            b.d.editor().set(
                global.expect("the base has a global").index,
                Global { nets },
            );
        }),
    ];
    for (want, break_it) in cases {
        let mut b = base();
        break_it(&mut b);
        assert_eq!(violations(&b.d), vec![want], "the {want:?} fixture");
    }
    let covered: Vec<Violation> = cases.iter().map(|c| c.0).collect();
    let owners = [Violation::TerminalCount, Violation::DetailRow];
    let kernel: Vec<Violation> = Violation::ALL
        .into_iter()
        .filter(|v| !owners.contains(v))
        .collect();
    assert_eq!(
        covered, kernel,
        "every kernel-owned violation has its fixture"
    );
}

#[test]
fn a_measure_definition_read_outside_a_measure_is_a_ref_target() {
    let mut b = base();
    let top = b.d.file(crate::id::FileId(0)).top;
    let m = b.d.builder().name("m1");
    let def = b.d.builder().define(DefKind::Measure, m, top, b.param);
    let read = b.d.builder().param_ref("m1");
    b.d.exprs.b[read.index()] = def.0;
    let row = Param {
        name: b.d.find_name("k"),
        value: read,
    };
    set_param(&mut b.d, b.param, row);
    assert_eq!(violations(&b.d), vec![Violation::RefTarget]);
    let _ = DefId::NONE;
}

#[test]
fn a_non_canonical_empty_list_is_a_handle_range() {
    let mut b = base();
    let top = b.d.file(crate::id::FileId(0)).top;
    let sub =
        b.d.items(top)
            .iter()
            .copied()
            .find(|i| i.kind == crate::ItemKind::Subckt)
            .unwrap();
    let mut row: crate::item::Subckt = b.d.row(sub.index);
    row.ports = crate::id::List::range(1, 0); // empty, but not `List::EMPTY`
    b.d.editor().set(sub.index, row);
    assert_eq!(violations(&b.d), vec![Violation::HandleRange]);
}
