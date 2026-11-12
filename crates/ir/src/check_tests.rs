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
    let cases: [(Violation, Break); 18] = [
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
        (Violation::DefinitionCycle, |b| {
            // k = k
            let top = b.d.file(crate::id::FileId(0)).top;
            let k = b.d.lookup(top, DefKind::Param, b.d.find_name("k"));
            let read = b.d.builder().param_ref("k");
            b.d.builder().resolve(read, k);
            let row = Param {
                name: b.d.find_name("k"),
                value: read,
            };
            set_param(&mut b.d, b.param, row);
        }),
        (Violation::StaleDefinition, |b| {
            // the parameter dropped through the raw list: its definition stays in the index
            let top = b.d.file(crate::id::FileId(0)).top;
            let param = b.param;
            b.d.editor().items_mut(top).retain(|&i| i != param);
        }),
        (Violation::CallArity, |b| {
            // f(x) called with two arguments
            let top = b.d.file(crate::id::FileId(0)).top;
            let f = b.d.lookup(top, DefKind::Func, b.d.find_name("f"));
            let mut bl = b.d.builder();
            let (one, two) = (bl.num(1.0), bl.num(2.0));
            let call = bl.user_call("f", &[one, two]);
            bl.resolve(call, f);
            bl.param(top, "c", call, Span::NONE);
        }),
        (Violation::StringOrListBody, |b| {
            // g() = "fast"; a string as a parameter's value beside it is legal
            let top = b.d.file(crate::id::FileId(0)).top;
            let mut bl = b.d.builder();
            let s = bl.string("fast");
            bl.func(top, "g", &[], s, Span::NONE);
            let t = bl.string("slow");
            bl.param(top, "corner", t, Span::NONE);
        }),
        (Violation::StringOrListField, |b| {
            // an if whose condition is a list; a list as a formal's default beside it is legal
            let top = b.d.file(crate::id::FileId(0)).top;
            let mut bl = b.d.builder();
            let one = bl.num(1.0);
            let l = bl.list(&[one]);
            bl.if_(top, &[l], Span::NONE);
            let m = bl.list(&[one]);
            bl.subckt(top, "t", &[], &[("w", m)], Span::NONE);
        }),
    ];
    for (want, break_it) in cases {
        let mut b = base();
        break_it(&mut b);
        assert_eq!(violations(&b.d), vec![want], "the {want:?} fixture");
    }
    let covered: Vec<Violation> = cases.iter().map(|c| c.0).collect();
    // circuit-owned: element rows, and A13's names (circuit_check.rs has their fixtures)
    let owners = [
        Violation::TerminalCount,
        Violation::DetailRow,
        Violation::DuplicateElement,
        Violation::InstancePortCount,
    ];
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

#[test]
fn a_title_or_comment_with_a_line_break_is_a_field_domain() {
    // A15 K3: a statement's text is one line; each break beside its one-line neighbour
    for (text, want) in [
        ("a\nb", vec![Violation::FieldDomain]),
        ("a\rb", vec![Violation::FieldDomain]),
        ("a b", vec![]),
    ] {
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let (_, top) = b.file("t.sp", Role::Deck);
            b.title(top, text, Span::NONE);
            b.comment(top, text, Span::NONE);
        }
        let got = violations(&d);
        let want: Vec<Violation> = want.iter().flat_map(|v| [*v, *v]).collect();
        assert_eq!(got, want, "{text:?}: the title and the comment");
    }
}

#[test]
fn probe_context_follows_the_class() {
    // A15 K3: a constant-in-time field reading the circuit through a function is a
    // ProbeContext, as one holding v(out) itself is; reading a constant function is not
    let build = |probe: bool| {
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let (_, top) = b.file("t.sp", Role::Deck);
            let x = b.param_ref("x");
            let body = if probe {
                let v = b.v("out", None);
                b.binary(crate::expr::BinOp::Add, v, x)
            } else {
                b.call(Func::Exp, &[x])
            };
            b.func(top, "h", &["x"], body, Span::NONE);
            let one = b.num(1.0);
            let call = b.user_call("h", &[one]);
            b.param(top, "k", call, Span::NONE);
        }
        link(&mut d, &mut Diags::new());
        violations(&d)
    };
    assert_eq!(
        build(true),
        vec![Violation::ProbeContext],
        "k = h(1), h reads v(out)"
    );
    assert_eq!(build(false), vec![], "k = h(1), h = exp(x)");
}

#[test]
fn a_title_outside_a_decks_top_is_a_field_domain() {
    // `ItemKind::Title`: only in a `Deck` file's top scope. The builder asserts it and IR text
    // refuses it; a pass that moves the item is what `check` catches
    let moved = |to_body: bool| {
        let mut d = Design::new();
        let (top, body) = {
            let mut b = d.builder();
            let (_, top) = b.file("t.sp", Role::Deck);
            b.title(top, "t", Span::NONE);
            let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
            (top, body)
        };
        if to_body {
            let mut e = d.editor();
            let title = e.items_mut(top).remove(0);
            e.items_mut(body).push(title);
        }
        violations(&d)
    };
    assert_eq!(moved(false), vec![], "in the deck's top");
    assert_eq!(
        moved(true),
        vec![Violation::FieldDomain],
        "moved into a subckt body"
    );
}

#[test]
fn a_row_names_the_item_it_is_about() {
    // A25 (R48): `Diag.item` names the refused item, so a design with no spans (built, or
    // generated) still says which element; a row about no item has `ItemRef::NONE`
    let mut d = Design::new();
    let (title, top) = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let title = b.title(top, "t", Span::NONE);
        (title, top)
    };
    let body = {
        let mut b = d.builder();
        b.subckt(top, "s", &[], &[], Span::NONE).1
    };
    {
        let mut e = d.editor();
        let t = e.items_mut(top).remove(0);
        e.items_mut(body).push(t);
    }
    let mut diags = Diags::new();
    check(&d, &mut diags);
    assert_eq!(diags.rows().len(), 1);
    assert_eq!(diags.rows()[0].item, title, "the moved title");
    // check_legal names the item that uses the feature
    let mut legal = Diags::new();
    crate::check::check_legal(&d, &crate::feature::FeatureSet::EMPTY, &mut legal);
    assert!(legal.rows().iter().any(|r| r.item == title));
    // push_at takes the span from the item; push names none
    let mut rows = Diags::new();
    rows.push_at(violation_code(Violation::FieldDomain), &d, title, 7);
    rows.push(violation_code(Violation::FieldDomain), Span::NONE, 7);
    assert_eq!(rows.rows()[0].span, d.span_of(title));
    assert_eq!(
        (rows.rows()[0].item, rows.rows()[1].item),
        (title, ItemRef::NONE)
    );
}

#[test]
fn a_dead_body_is_no_violation() {
    // A28: a pass that takes a scope-owning item out of every list leaves its body dead; check
    // walks reachable scopes only, so the shrunk design is valid (its live neighbour too)
    let mut d = Design::new();
    let top = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        let one = b.num(1.0);
        b.param(body, "k", one, Span::NONE);
        let (_, inner) = b.subckt(body, "t", &[], &[], Span::NONE);
        b.param(inner, "j", one, Span::NONE);
        top
    };
    assert_eq!(violations(&d), vec![], "the live design");
    // through the door (round 7b): the raw list would leave the subckt's definition stale
    let outer = d.items(top)[0];
    d.editor().remove_item(top, outer);
    assert_eq!(
        violations(&d),
        vec![],
        "the outer subckt dropped: its body and the inner one are dead"
    );
}

#[test]
fn a_definition_cycle_is_its_own_violation() {
    // A28: `subckt s` whose formal `t` defaults to a read of `t` (bound to the formal itself, a
    // cycle of one), and a body parameter that reads `t`: one DefinitionCycle at the formal, no
    // ProbeContext at the reader
    // (link never makes it, A29; a reader that binds by its own rule can, as the ngspice
    // reader once did)
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let read = b.param_ref("t");
        let (_, body) = b.subckt(top, "s", &[], &[("t", read)], Span::NONE);
        let formal = b
            .design()
            .lookup(body, DefKind::Param, b.design().find_name("t"));
        b.resolve(read, formal);
        let inner = b.param_ref("t");
        b.resolve(inner, formal);
        b.param(body, "k", inner, Span::NONE);
    }
    assert_eq!(violations(&d), vec![Violation::DefinitionCycle]);
    // its neighbour: the default reads a top parameter of another name
    let mut ok = Design::new();
    {
        let mut b = ok.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let one = b.num(1.0);
        b.param(top, "u", one, Span::NONE);
        let read = b.param_ref("u");
        let (_, body) = b.subckt(top, "s", &[], &[("t", read)], Span::NONE);
        let inner = b.param_ref("t");
        b.param(body, "k", inner, Span::NONE);
    }
    link(&mut ok, &mut Diags::new());
    assert_eq!(violations(&ok), vec![]);
}
