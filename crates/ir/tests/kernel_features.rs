//! Features and diagnostics: `Feature::index`/`from_index` as inverses over every feature, the
//! bitset's algebra, `features_used` of hand-built designs against hand-listed sets, each kernel
//! `Extra` beside a legal neighbour that does not set it, `check_legal`'s refusals, and the
//! kernel's code table.
//!
//! Oracles: the inverse (`from_index(index(f)) == f` for all 496); spec examples (the expected
//! feature sets are written out in each test); a legal neighbour per `Extra`.
//!
//! Contents: none (tests only).

use spicerack_ir::diag::{self, TEXT_SYNTAX, UNSUPPORTED};
use spicerack_ir::{
    BinOp, Code, Design, Diags, Dist, DrawPer, DrawWhen, ExprTag, Extra, Feature, FeatureSet, Func,
    ItemKind, Kind, LoadKind, OpaqueClass, Role, Span, UnOp, Violation, check_legal, features_used,
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

fn set(fs: &[Feature]) -> FeatureSet {
    fs.iter().fold(FeatureSet::EMPTY, |s, f| s.with(*f))
}

#[test]
fn index_and_from_index_are_inverses() {
    assert_eq!(Feature::COUNT, 502); // A11: three more `Func` members, each twice (plain, Dyn)
    let mut seen = vec![false; Feature::COUNT as usize];
    for i in 0..Feature::COUNT {
        let f = Feature::from_index(i);
        assert_eq!(f.index(), i, "{f:?}");
        assert!(!std::mem::replace(&mut seen[i as usize], true));
    }
    assert_eq!(Feature::Item(ItemKind::Title).index(), 0);
    assert_eq!(Feature::Item(ItemKind::Measure).index(), 22);
    // the groups in declaration order: the last feature is the last option key
    let last = Feature::from_index(Feature::COUNT - 1);
    assert!(matches!(last, Feature::Option(_)), "{last:?}");
    assert_ne!(
        Feature::Func(Func::Sin).index(),
        Feature::FuncDyn(Func::Sin).index()
    );
}

#[test]
fn the_bitset_is_a_set() {
    let a: FeatureSet = FeatureSet::EMPTY
        .with(Feature::Item(ItemKind::Param))
        .with(Feature::Func(Func::Exp));
    let b = FeatureSet::EMPTY
        .with(Feature::Func(Func::Exp))
        .with(Feature::Extra(Extra::LocalFunc));
    assert!(a.contains(Feature::Item(ItemKind::Param)));
    assert!(!a.contains(Feature::Item(ItemKind::Title)));
    assert!(FeatureSet::EMPTY.is_empty() && !a.is_empty());
    assert_eq!(
        a.minus(b).iter().collect::<Vec<_>>(),
        vec![Feature::Item(ItemKind::Param)]
    );
    assert_eq!(
        a.union(b).iter().collect::<Vec<_>>(),
        vec![
            Feature::Item(ItemKind::Param),
            Feature::Func(Func::Exp),
            Feature::Extra(Extra::LocalFunc)
        ]
    );
    let all = (0..Feature::COUNT).fold(FeatureSet::EMPTY, |s, i| s.with(Feature::from_index(i)));
    assert_eq!(all.iter().count(), Feature::COUNT as usize);
    assert!(all.minus(all).is_empty());
}

#[test]
fn features_of_a_hand_built_deck() {
    let got = used(|b, top| {
        b.title(top, "t", Span::NONE);
        let x = b.param_ref("x");
        let e = b.call(Func::Exp, &[x]);
        let two = b.num(2.0);
        let v = b.binary(BinOp::Mul, e, two);
        b.param(top, "k", v, Span::NONE);
        // a function body that reads the circuit: its members are the Dyn features
        let probe = b.v("out", None);
        let neg = b.unary(UnOp::Neg, probe);
        let body = b.call(Func::Abs, &[neg]);
        b.func(top, "f", &["y"], body, Span::NONE);
        let one = b.num(1.0);
        let d = b.draw(
            Dist::AbsGauss,
            DrawWhen::Always,
            DrawPer::Reference,
            &[one, one, one],
        );
        b.param(top, "g", d, Span::NONE);
    });
    let want = set(&[
        Feature::Item(ItemKind::Title),
        Feature::Item(ItemKind::Param),
        Feature::Item(ItemKind::Func),
        Feature::Expr(ExprTag::Param),
        Feature::Expr(ExprTag::Call),
        Feature::Expr(ExprTag::Num),
        Feature::Expr(ExprTag::Binary),
        Feature::Expr(ExprTag::V),
        Feature::Expr(ExprTag::Unary),
        Feature::Expr(ExprTag::Draw),
        Feature::Func(Func::Exp),
        Feature::Binary(BinOp::Mul),
        Feature::FuncDyn(Func::Abs),
        Feature::UnaryDyn(UnOp::Neg),
        Feature::Dist(Dist::AbsGauss),
        Feature::DrawWhen(DrawWhen::Always),
        Feature::DrawPer(DrawPer::Reference),
    ]);
    assert_eq!(
        got.iter().collect::<Vec<_>>(),
        want.iter().collect::<Vec<_>>()
    );
}

#[test]
fn each_kernel_extra_has_a_row_and_a_neighbour() {
    type Build = fn(&mut spicerack_ir::Builder<'_>, spicerack_ir::ScopeId);
    // (extra, a design that sets it, its legal neighbour that does not)
    let cases: [(Extra, Build, Build); 8] = [
        (
            Extra::NestedSubckt,
            |b, top| {
                let (_, body) = b.subckt(top, "outer", &[], &[], Span::NONE);
                b.subckt(body, "inner", &[], &[], Span::NONE);
            },
            |b, top| {
                b.subckt(top, "outer", &[], &[], Span::NONE);
                b.subckt(top, "inner", &[], &[], Span::NONE);
            },
        ),
        (
            Extra::LocalModel,
            |_, _| {}, // a Model is the circuit owner's row: its pair is in the owner's tests
            |_, _| {},
        ),
        (
            Extra::LocalFunc,
            |b, top| {
                let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
                let one = b.num(1.0);
                b.func(body, "f", &[], one, Span::NONE);
            },
            |b, top| {
                b.subckt(top, "s", &[], &[], Span::NONE);
                let one = b.num(1.0);
                b.func(top, "f", &[], one, Span::NONE);
            },
        ),
        (
            Extra::ParamInBranch,
            |b, top| {
                let c = b.num(1.0);
                let (_, branches) = b.if_(top, &[c], Span::NONE);
                let body = b.design().branches(branches)[0].body;
                b.param(body, "k", c, Span::NONE);
            },
            |b, top| {
                let c = b.num(1.0);
                b.if_(top, &[c], Span::NONE);
                b.param(top, "k", c, Span::NONE);
            },
        ),
        (
            Extra::LoadInBranch,
            |b, top| {
                let c = b.num(1.0);
                let (_, branches) = b.if_(top, &[c], Span::NONE);
                let body = b.design().branches(branches)[0].body;
                b.load(body, LoadKind::Osdi, "m.osdi", Span::NONE);
            },
            |b, top| {
                let c = b.num(1.0);
                b.if_(top, &[c], Span::NONE);
                b.load(top, LoadKind::Osdi, "m.osdi", Span::NONE);
            },
        ),
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
    ];
    for (extra, sets, neighbour) in cases {
        if extra == Extra::LocalModel {
            continue;
        }
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

#[test]
fn check_legal_refuses_what_the_target_lacks() {
    let mut d = Design::new();
    let (title, opaque) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let t = b.title(
            top,
            "t",
            Span {
                file: spicerack_ir::FileId(0),
                start: 0,
                len: 1,
            },
        );
        let o = b.opaque(top, "xyce", OpaqueClass::Private, "YADC x", Span::NONE);
        (t, o)
    };
    let legal = FeatureSet::EMPTY.with(Feature::Item(ItemKind::Title));
    let mut diags = Diags::new();
    check_legal(&d, &legal, &mut diags);
    let rows = diags.rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].code, UNSUPPORTED);
    assert_eq!(rows[0].arg, Feature::Item(ItemKind::Opaque).index() as u32);
    assert_eq!(rows[0].span, d.span_of(opaque));
    let mut none = Diags::new();
    check_legal(&d, &legal.with(Feature::Item(ItemKind::Opaque)), &mut none);
    assert!(none.is_empty());
    let _ = title;
}

#[test]
fn the_kernel_code_table() {
    assert_eq!(Code::new(b'X', 1).to_string(), "X0001");
    assert_eq!(TEXT_SYNTAX.to_string(), "P9001");
    assert_eq!(UNSUPPORTED.to_string(), "U0001");
    for (k, v) in Violation::ALL.iter().enumerate() {
        let code = diag::violation_code(*v);
        assert_eq!(code, Code::new(b'X', k as u16 + 1), "{v:?}");
        let info = diag::info(code).expect("a kernel code has its row");
        assert_eq!(info.kind, Kind::Refusal, "an invariant violation refuses");
        assert!(!info.title.is_empty() && !info.explain.is_empty() && !info.cite.is_empty());
    }
    for (k, c) in OpaqueClass::ALL.iter().enumerate() {
        let code = diag::opaque_code(*c);
        assert_eq!(code, Code::new(b'S', k as u16 + 1));
        assert_eq!(diag::info(code).expect("S row").kind, Kind::Note);
    }
    assert_eq!(diag::info(UNSUPPORTED).expect("U row").kind, Kind::Refusal);
    assert!(diag::info(TEXT_SYNTAX).is_some());
    assert_eq!(diag::info(Code::new(b'W', 1)), None, "another owner's code");
    assert_eq!(
        diag::info(Code::new(b'X', Violation::COUNT as u16 + 1)),
        None
    );
}

#[test]
fn the_full_set_holds_every_feature() {
    let names: Vec<Feature> = FeatureSet::ALL.iter().collect();
    let want: Vec<Feature> = (0..Feature::COUNT).map(Feature::from_index).collect();
    assert_eq!(names, want);
    assert!(FeatureSet::ALL.minus(FeatureSet::ALL).is_empty());
}

#[test]
fn the_dynamic_class_is_transitive() {
    use spicerack_ir::{ExprClass, expr_class};
    // R14 / A11: a = exp($temp) is Temp; b = exp(a) is Temp through a; c = exp(1) is Constant;
    // p = exp(@h(1)) with h(x) = v(out) + x is Circuit through h; x = y, y = x is a cycle
    let mut d = Design::new();
    let (a, b, c, p, x) = {
        let mut bl = d.builder();
        let (_, top) = bl.file("top.sp", Role::Deck);
        let t = bl.temp();
        let a = bl.call(Func::Exp, &[t]);
        bl.param(top, "a", a, Span::NONE);
        let ra = bl.param_ref("a");
        let b = bl.call(Func::Exp, &[ra]);
        bl.param(top, "b", b, Span::NONE);
        let one = bl.num(1.0);
        let c = bl.call(Func::Exp, &[one]);
        bl.param(top, "c", c, Span::NONE);
        let probe = bl.v("out", None);
        let fx = bl.param_ref("x");
        let hbody = bl.binary(BinOp::Add, probe, fx);
        bl.func(top, "h", &["x"], hbody, Span::NONE);
        let one = bl.num(1.0);
        let call = bl.user_call("h", &[one]);
        let p = bl.call(Func::Exp, &[call]);
        bl.param(top, "p", p, Span::NONE);
        let ry = bl.param_ref("y");
        bl.param(top, "x", ry, Span::NONE);
        let rx = bl.param_ref("x");
        bl.param(top, "y", rx, Span::NONE);
        (a, b, c, p, ry)
    };
    spicerack_ir::link(&mut d, &mut Diags::new());
    assert_eq!(expr_class(&d, a), ExprClass::Temp);
    assert_eq!(
        expr_class(&d, b),
        ExprClass::Temp,
        "through the parameter it reads"
    );
    assert_eq!(expr_class(&d, c), ExprClass::Constant);
    assert_eq!(
        expr_class(&d, p),
        ExprClass::Circuit,
        "through the function it calls"
    );
    assert_eq!(
        expr_class(&d, x),
        ExprClass::Circuit,
        "a reference cycle counts as Circuit"
    );
    assert!(ExprClass::Constant < ExprClass::Temp && ExprClass::Temp < ExprClass::Circuit);
    // the Dyn features are class >= Temp: a and b's `exp` is Dyn, c's is not
    let used = features_used(&d);
    assert!(used.contains(Feature::FuncDyn(Func::Exp)) && used.contains(Feature::Func(Func::Exp)));
    let only_c = used_with(|bl, top| {
        let one = bl.num(1.0);
        let c = bl.call(Func::Exp, &[one]);
        bl.param(top, "c", c, Span::NONE);
    });
    assert!(
        only_c.contains(Feature::Func(Func::Exp)) && !only_c.contains(Feature::FuncDyn(Func::Exp))
    );
    let through_a = used_with(|bl, top| {
        let t = bl.temp();
        bl.param(top, "a", t, Span::NONE);
        let ra = bl.param_ref("a");
        let b = bl.call(Func::Exp, &[ra]);
        bl.param(top, "b", b, Span::NONE);
    });
    assert!(
        through_a.contains(Feature::FuncDyn(Func::Exp)),
        "b reads Temp through a"
    );
    assert!(!through_a.contains(Feature::Func(Func::Exp)));
}

/// `features_used` of a linked deck built by `f`.
fn used_with(f: impl FnOnce(&mut spicerack_ir::Builder<'_>, spicerack_ir::ScopeId)) -> FeatureSet {
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        f(&mut b, top);
    }
    spicerack_ir::link(&mut d, &mut Diags::new());
    features_used(&d)
}
