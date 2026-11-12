//! Features and diagnostics: `Feature::index`/`from_index` as inverses over every feature, the
//! bitset's algebra, `features_used` of hand-built designs against hand-listed sets, each kernel
//! `Extra` beside a legal neighbour that does not set it, `check_legal`'s refusals, and the
//! kernel's code table.
//!
//! Oracles: the inverse (`from_index(index(f)) == f` for every index); spec examples (the expected
//! feature sets are written out in each test); a legal neighbour per `Extra`.
//!
//! Contents: none (tests only).

use spicerack_ir::diag::{self, TEXT_SYNTAX, UNSUPPORTED};
use spicerack_ir::{
    AnalysisKind, BinOp, CircuitExtra, Code, Design, Diags, Dist, DrawPer, DrawWhen, ElemKind,
    ExprId, ExprTag, Extra, Feature, FeatureSet, Func, ItemKind, Kind, LoadKind, OpaqueClass,
    OptionKey, Role, RunExtra, Span, UnOp, Violation, WaveKind, check_legal, features_used,
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
    // the count follows the enums it is built from: a member added to any of them moves it,
    // and no hand edit here is needed (the inverse below shows each index is one feature)
    let groups = [
        ItemKind::ALL.len(),
        ElemKind::ALL.len(),
        WaveKind::ALL.len(),
        AnalysisKind::ALL.len(),
        ExprTag::ALL.len(),
        2 * Func::ALL.len(),
        2 * BinOp::ALL.len(),
        2 * UnOp::ALL.len(),
        Dist::ALL.len(),
        DrawWhen::ALL.len(),
        DrawPer::ALL.len(),
        Extra::ALL.len(),
        CircuitExtra::ALL.len(),
        RunExtra::ALL.len(),
        OptionKey::ALL.len(),
        spicerack_ir::ModelFamily::ALL.len(),
        (spicerack_ir::ScopeKind::ALL.len() - 1) * ItemKind::ALL.len(),
    ];
    assert_eq!(Feature::COUNT as usize, groups.iter().sum::<usize>());
    let mut seen = vec![false; Feature::COUNT as usize];
    for i in 0..Feature::COUNT {
        let f = Feature::from_index(i);
        assert_eq!(f.index(), i, "{f:?}");
        assert!(!std::mem::replace(&mut seen[i as usize], true));
    }
    assert_eq!(Feature::Item(ItemKind::Title).index(), 0);
    assert_eq!(
        Feature::Item(ItemKind::Measure).index(),
        ItemKind::Measure as u16
    );
    // the groups in declaration order: the last feature is the last placement (A15 K1)
    let last = Feature::from_index(Feature::COUNT - 1);
    assert_eq!(
        last,
        Feature::Placed(spicerack_ir::ScopeKind::Loop, ItemKind::Elaborate)
    );
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
    let cases: [(Extra, Build, Build); Extra::COUNT] = [
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
            Extra::FormalReadsParam,
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
    // A12 K3: Hint is appended, so the codes before it keep their numbers
    let hint = diag::info(Code::new(b'S', 6)).expect("S0006 is Hint");
    assert_eq!(diag::opaque_code(OpaqueClass::Hint), Code::new(b'S', 6));
    assert_eq!(hint.kind, Kind::Note);
    assert_ne!(
        hint.explain,
        diag::info(diag::opaque_code(OpaqueClass::Inert))
            .expect("S0003")
            .explain,
        "its own note, not Inert's"
    );
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
fn a_library_without_a_title_is_no_deck_without_one() {
    // the legal neighbour of DeckWithoutTitle's other side: only a `Deck` file needs a title
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("lib.sp", Role::Library);
        b.comment(top, "a library", Span::NONE);
    }
    assert!(!features_used(&d).contains(Feature::Extra(Extra::DeckWithoutTitle)));
}

#[test]
fn placement_is_enclosure_at_any_depth() {
    // A15 K1 (R27, ruled "enclosed"): Placed(S, K) for each scope kind S on the item's chain,
    // File excluded; spec examples worked by hand
    use spicerack_ir::ScopeKind as S;
    let placed = |s: S, k: ItemKind| Feature::Placed(s, k);
    let used = used(|b, top| {
        let (_, outer) = b.subckt(top, "outer", &[], &[], Span::NONE);
        let one = b.num(1.0);
        let (_, branches) = b.if_(outer, &[one], Span::NONE);
        let branch = b.design().branches(branches)[0].body;
        let (_, inner) = b.subckt(branch, "inner", &[], &[], Span::NONE);
        let two = b.num(2.0);
        b.param(inner, "p", two, Span::NONE);
    });
    let got: Vec<Feature> = used
        .iter()
        .filter(|f| matches!(f, Feature::Placed(..)))
        .collect();
    let mut want = vec![
        placed(S::Subckt, ItemKind::If),
        placed(S::Subckt, ItemKind::Subckt),
        placed(S::Branch, ItemKind::Subckt),
        placed(S::Subckt, ItemKind::Param),
        placed(S::Branch, ItemKind::Param),
    ];
    want.sort_by_key(|f| f.index());
    assert_eq!(got, want, "the outer subckt in the top sets nothing");
    // the helper
    let set = FeatureSet::placed(S::Loop, &[ItemKind::Loop, ItemKind::Analysis]);
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        vec![
            placed(S::Loop, ItemKind::Analysis),
            placed(S::Loop, ItemKind::Loop)
        ]
    );
}
