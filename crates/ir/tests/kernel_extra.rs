//! Each kernel `Extra` beside a legal neighbour that does not set it: one hand-built design that
//! sets the feature and one that differs from it in the rule alone. Split from
//! `kernel_features.rs` at its size limit.
//!
//! Oracle: a legal neighbour per `Extra`, written by hand; the covered members are compared with
//! `Extra::ALL` by name.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    BinOp, DefKind, Design, ExprId, Extra, Feature, FeatureSet, Role, Span, features_used,
};

// ---- private ----

/// The features of a design built by `f` into a fresh deck's top scope.
fn used(f: impl FnOnce(&mut spicerack_ir::Builder<'_>, spicerack_ir::ScopeId)) -> FeatureSet {
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        f(&mut b, top);
    }
    features_used(&d)
}

#[test]
fn each_kernel_extra_has_a_row_and_a_neighbour() {
    type Build = fn(&mut spicerack_ir::Builder<'_>, spicerack_ir::ScopeId);
    // (extra, a design that sets it, its legal neighbour that does not)
    let cases: [(Extra, Build, Build); Extra::COUNT] = [
        (
            Extra::IncludeSection,
            |b, top| {
                b.include(top, "m.lib", Some("tt"), Span::NONE);
            },
            |b, top| {
                b.include(top, "m.lib", None, Span::NONE);
            },
        ),
        (
            Extra::SectionLibrary,
            |b, top| {
                b.section(top, "tt", Some("lib"), Span::NONE);
            },
            |b, top| {
                b.section(top, "tt", None, Span::NONE);
            },
        ),
        (
            Extra::CaseSensitiveNames,
            |b, top| {
                b.global(top, &["vdd", "VDD"], Span::NONE);
            },
            |b, top| {
                b.global(top, &["vdd", "vss"], Span::NONE);
            },
        ),
        (
            Extra::IntLiteral,
            |b, top| {
                let three = spicerack_ir::Literal {
                    mantissa: 3,
                    frac: 0,
                    exp10: 0,
                    form: spicerack_ir::NumForm::Int,
                };
                let v = b.literal(three);
                b.param(top, "k", v, Span::NONE);
            },
            |b, top| {
                let v = b.num(3.0);
                b.param(top, "k", v, Span::NONE);
            },
        ),
        (
            Extra::EnclosingRef,
            |b, top| {
                // outer body defines p; the inner subckt's param reads it: an enclosing body
                let (_, outer) = b.subckt(top, "outer", &[], &[], Span::NONE);
                let one = b.num(1.0);
                b.param(outer, "p", one, Span::NONE);
                let (_, inner) = b.subckt(outer, "inner", &[], &[], Span::NONE);
                let read = b.param_ref("p");
                let p = b.design().lookup(
                    outer,
                    spicerack_ir::DefKind::Param,
                    b.design().find_name("p"),
                );
                b.resolve(read, p);
                b.param(inner, "q", read, Span::NONE);
            },
            |b, top| {
                // the neighbour reads its own subcircuit's p and the top's t
                let one = b.num(1.0);
                b.param(top, "t", one, Span::NONE);
                let (_, outer) = b.subckt(top, "outer", &[], &[], Span::NONE);
                let (_, inner) = b.subckt(outer, "inner", &[], &[], Span::NONE);
                let two = b.num(2.0);
                b.param(inner, "p", two, Span::NONE);
                let own = b.param_ref("p");
                let p = b.design().lookup(
                    inner,
                    spicerack_ir::DefKind::Param,
                    b.design().find_name("p"),
                );
                b.resolve(own, p);
                let at_top = b.param_ref("t");
                let t =
                    b.design()
                        .lookup(top, spicerack_ir::DefKind::Param, b.design().find_name("t"));
                b.resolve(at_top, t);
                let sum = b.binary(BinOp::Add, own, at_top);
                b.param(inner, "q", sum, Span::NONE);
            },
        ),
        (
            Extra::FormalNotConstant,
            |b, top| {
                let one = b.num(1.0);
                b.param(top, "w0", one, Span::NONE);
                let read = b.param_ref("w0");
                b.subckt(top, "s", &[], &[("w", read)], Span::NONE);
            },
            |b, top| {
                let one = b.num(1.0);
                b.subckt(top, "s", &[], &[("w", one)], Span::NONE);
            },
        ),
        (
            Extra::DeckWithoutTitle,
            |b, top| {
                b.comment(top, "no title", Span::NONE);
            },
            |b, top| {
                b.title(top, "", Span::NONE);
                b.comment(top, "titled", Span::NONE);
            },
        ),
        (
            Extra::FormalWithoutDefault,
            |b, top| {
                b.subckt(top, "s", &[], &[("w", ExprId::NONE)], Span::NONE);
            },
            |b, top| {
                let one = b.num(1.0);
                b.subckt(top, "s", &[], &[("w", one)], Span::NONE);
            },
        ),
        (
            Extra::IfConditionDynamic,
            |b, top| {
                let t = b.temp();
                let lim = b.num(100.0);
                let hot = b.binary(BinOp::Gt, t, lim);
                b.if_(top, &[hot], Span::NONE);
            },
            |b, top| {
                let one = b.num(1.0);
                let zero = b.literal(spicerack_ir::Literal::ZERO);
                let yes = b.binary(BinOp::Gt, one, zero);
                b.if_(top, &[yes], Span::NONE);
            },
        ),
        (
            Extra::LongLiteral,
            |b, top| {
                let v =
                    b.long_literal("0.12345678901234567890123", 0, spicerack_ir::NumForm::Plain);
                b.param(top, "p", v, Span::NONE);
            },
            |b, top| {
                let v = b.num(0.123_456_789_012_345_67);
                b.param(top, "p", v, Span::NONE);
            },
        ),
        (
            Extra::SubcktConstantParam,
            |b, top| {
                let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
                let one = b.num(1.0);
                b.param(body, "k", one, Span::NONE);
            },
            |b, top| {
                // a dependent local (it reads the formal) and a constant at the top
                let one = b.num(1.0);
                let (_, body) = b.subckt(top, "s", &[], &[("w", one)], Span::NONE);
                let w = b.param_ref("w");
                b.param(body, "k", w, Span::NONE);
                let two = b.num(2.0);
                b.param(top, "c", two, Span::NONE);
            },
        ),
        (
            // A25 item 4: a string as a parameter's value; the neighbour is a list there
            Extra::StringValue,
            |b, top| {
                let s = b.string("fast");
                b.param(top, "corner", s, Span::NONE);
            },
            |b, top| {
                let one = b.num(1.0);
                let l = b.list(&[one]);
                b.param(top, "v", l, Span::NONE);
            },
        ),
        (
            // a list as a formal's default; the neighbour is a string there
            Extra::ListValue,
            |b, top| {
                let (one, two) = (b.num(1.0), b.num(2.0));
                let l = b.list(&[one, two]);
                b.subckt(top, "s", &[], &[("w", l)], Span::NONE);
            },
            |b, top| {
                let s = b.string("fast");
                b.subckt(top, "s", &[], &[("w", s)], Span::NONE);
            },
        ),
        (
            // R62/A37: a default bound to a definition outside the subcircuit; the neighbour
            // reads a sibling formal, bound to it
            Extra::FormalOuterRead,
            |b, top| {
                let one = b.num(1.0);
                b.param(top, "w0", one, Span::NONE);
                let w0 = b
                    .design()
                    .lookup(top, DefKind::Param, b.design().find_name("w0"));
                let read = b.param_ref("w0");
                b.resolve(read, w0);
                b.subckt(top, "s", &[], &[("w", read)], Span::NONE);
            },
            |b, top| {
                let one = b.num(1.0);
                let read = b.param_ref("a");
                let (_, body) = b.subckt(top, "s", &[], &[("a", one), ("w", read)], Span::NONE);
                let a = b
                    .design()
                    .lookup(body, DefKind::Param, b.design().find_name("a"));
                b.resolve(read, a);
            },
        ),
        (
            // an unbound read no sibling names; the neighbours: an unbound read of a sibling's
            // name, and a bound outer read
            Extra::FormalUnboundRead,
            |b, top| {
                let read = b.param_ref("vdd");
                b.subckt(top, "s", &[], &[("w", read)], Span::NONE);
            },
            |b, top| {
                let one = b.num(1.0);
                let sibling = b.param_ref("a");
                b.subckt(top, "s", &[], &[("a", one), ("w", sibling)], Span::NONE);
                let two = b.num(2.0);
                b.param(top, "w0", two, Span::NONE);
                let w0 = b
                    .design()
                    .lookup(top, DefKind::Param, b.design().find_name("w0"));
                let outer = b.param_ref("w0");
                b.resolve(outer, w0);
                b.subckt(top, "t", &[], &[("w", outer)], Span::NONE);
            },
        ),
        (
            // `"tt" == "tt"`; the neighbour: the same strings as a list
            Extra::StringCompare,
            |b, top| {
                let (x, y) = (b.string("tt"), b.string("tt"));
                let eq = b.binary(BinOp::Eq, x, y);
                b.param(top, "c", eq, Span::NONE);
            },
            |b, top| {
                let (x, y) = (b.string("tt"), b.string("tt"));
                let l = b.list(&[x, y]);
                b.param(top, "c", l, Span::NONE);
            },
        ),
    ];
    for (extra, sets, neighbour) in cases {
        let f = Feature::Extra(extra);
        assert!(used(sets).contains(f), "{extra:?} is set");
        assert!(
            !used(neighbour).contains(f),
            "{extra:?}'s neighbour does not set it"
        );
    }
    assert_eq!(
        cases.map(|c| c.0),
        Extra::ALL,
        "every kernel Extra has its pair"
    );
}
