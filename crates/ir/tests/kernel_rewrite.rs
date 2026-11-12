//! The passes' door for expressions (A23): `RowFacts::map_exprs` visits exactly the roots
//! `handles` lists, in its order, with how each field reads; `rewrite_exprs` with an identity
//! rule changes no id, and with a rule that wraps every node changes every visited root and no
//! root's value; `check` finds nothing after either.
//!
//! Oracles: `handles` (the roots each row already lists, checked by `check`); `eval` of each
//! root before and after; `check`; byte and table equality for the identity rule.
//!
//! Contents: none (tests only).

#![cfg(feature = "gen")]

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{
    Bindings, Design, Diags, Dist, DrawPer, DrawWhen, ExprId, ExprUse, Feature, FeatureSet, Form,
    Handle, ItemKind, ItemRef, NameId, Ref, UnOp, check, eval, features_used, rewrite_exprs,
    rewrite_exprs_in, rewrite_tree, root_form,
};

const SIZE: Size = Size {
    items: 40,
    scope_depth: 3,
    expr_depth: 4,
    files: 2,
};

/// Every read answers a fixed value, so a root's value is a function of its tree.
struct Fixed;

impl Bindings for Fixed {
    fn param(&self, _: Ref) -> Option<f64> {
        Some(2.0)
    }
    fn voltage(&self, _: NameId, _: NameId) -> Option<f64> {
        Some(0.5)
    }
    fn current(&self, _: NameId) -> Option<f64> {
        Some(1e-3)
    }
    fn time(&self) -> Option<f64> {
        Some(1e-6)
    }
    fn temp(&self) -> Option<f64> {
        Some(27.0)
    }
    fn freq(&self) -> Option<f64> {
        Some(1e3)
    }
    fn tstep(&self) -> Option<f64> {
        Some(1e-9)
    }
    fn tstop(&self) -> Option<f64> {
        Some(1e-6)
    }
    fn tnom(&self) -> Option<f64> {
        Some(27.0)
    }
    fn draw(&self, _: Dist, _: DrawWhen, _: DrawPer, _: &[f64]) -> Option<f64> {
        Some(0.25)
    }
}

// ---- private ----

/// The (root, use) list `handles` implies for `item`: a behavioural element's one `ExprDyn` is
/// its expression (R66).
fn from_handles(d: &Design, item: ItemRef) -> Vec<(ExprId, ExprUse)> {
    let mut handles = Vec::new();
    d.item_handles(item, &mut handles);
    let behavioural = item.kind == ItemKind::Element
        && d.row::<spicerack_ir::Element>(item.index).kind.detail()
            == spicerack_ir::element::DetailKind::Behav;
    let dyn_use = if behavioural {
        ExprUse::Behavioural
    } else {
        ExprUse::Circuit
    };
    let mut out = Vec::new();
    for h in handles {
        match h {
            Handle::Expr(e) => out.push((e, ExprUse::Constant)),
            Handle::ExprDyn(e) => out.push((e, dyn_use)),
            Handle::Exprs(l) => out.extend(d.exprs(l).iter().map(|&e| (e, ExprUse::Constant))),
            Handle::Params(l) => out.extend(d.params(l).iter().map(|p| (p.1, ExprUse::Constant))),
            Handle::Name(_)
            | Handle::Ref(_)
            | Handle::Names(_)
            | Handle::Scope(_)
            | Handle::Text(_)
            | Handle::Branches(_) => {}
        }
    }
    out
}

/// The (root, use) list `map_exprs` visits for `item`, on a copy.
fn visited(d: &Design, item: ItemRef) -> Vec<(ExprId, ExprUse)> {
    let mut copy = d.clone();
    let mut out = Vec::new();
    copy.editor().map_exprs(item, &mut |e, u| out.push((*e, u)));
    out
}

#[test]
fn map_exprs_visits_the_roots_handles_lists() {
    let mut drift: Vec<ItemKind> = Vec::new();
    for seed in 0..200u64 {
        let d = generate(seed, &FeatureSet::ALL, SIZE);
        for (item, _) in d.reachable() {
            if from_handles(&d, item) != visited(&d, item) && !drift.contains(&item.kind) {
                drift.push(item.kind);
            }
        }
    }
    drift.sort_by_key(|k| *k as u8);
    assert_eq!(
        drift,
        Vec::<ItemKind>::new(),
        "kinds whose map_exprs drifts from handles"
    );
}

#[test]
fn an_identity_rule_changes_no_id() {
    for seed in 0..100u64 {
        let d = generate(seed, &FeatureSet::ALL, SIZE);
        let mut after = d.clone();
        let changed = rewrite_exprs(&mut after, &mut |_, _| None);
        assert_eq!(changed, 0, "seed {seed}");
        assert_eq!(
            after.expr_count(),
            d.expr_count(),
            "seed {seed}: nothing appended"
        );
        assert!(after.eq_ignoring_spans(&d), "seed {seed}");
        let mut diags = Diags::new();
        check(&after, &mut diags);
        assert!(diags.is_empty(), "seed {seed}: {:?}", diags.rows());
    }
}

#[test]
fn a_wrapping_rule_changes_every_root_and_no_value() {
    // every node e becomes `1 ? e : e`: each visited root changes, each keeps its value
    for seed in 0..100u64 {
        let d = generate(seed, &FeatureSet::ALL, SIZE);
        let items: Vec<ItemRef> = d.reachable().into_iter().map(|(i, _)| i).collect();
        let before: Vec<Vec<(ExprId, ExprUse)>> = items.iter().map(|&i| visited(&d, i)).collect();
        let roots = before
            .iter()
            .flatten()
            .filter(|(e, _)| !e.is_none())
            .count() as u32;
        let mut after = d.clone();
        let changed = rewrite_exprs(&mut after, &mut |b, e| {
            let one = b.num(1.0);
            Some(b.ternary(one, e, e))
        });
        assert_eq!(changed, roots, "seed {seed}: every visited root changes");
        for (&item, old) in items.iter().zip(&before) {
            let new = visited(&after, item);
            assert_eq!(new.len(), old.len(), "seed {seed}, {item:?}");
            for (&(o, u), &(n, v)) in old.iter().zip(&new) {
                assert_eq!(u, v, "seed {seed}, {item:?}: the use is the field's");
                if o.is_none() {
                    assert!(n.is_none(), "seed {seed}, {item:?}: NONE stays NONE");
                    continue;
                }
                assert_ne!(o, n, "seed {seed}, {item:?}");
                match (eval(&d, o, &Fixed), eval(&after, n, &Fixed)) {
                    (Ok(x), Ok(y)) => assert!(
                        x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()),
                        "seed {seed}, {item:?}: {x} became {y}"
                    ),
                    (Err(_), Err(_)) => {}
                    (x, y) => panic!("seed {seed}, {item:?}: {x:?} became {y:?}"),
                }
            }
        }
        let mut diags = Diags::new();
        check(&after, &mut diags);
        assert!(diags.is_empty(), "seed {seed}: {:?}", diags.rows());
    }
}

#[test]
fn a_shared_subtree_is_rewritten_once() {
    // two parameters hold one expression id: the rule sees each node once, and both roots
    // point at the one rewrite
    let mut d = Design::new();
    let (p, q) = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", spicerack_ir::Role::Deck);
        let one = b.num(1.0);
        let two = b.num(2.0);
        let sum = b.binary(spicerack_ir::BinOp::Add, one, two);
        let p = b.param(top, "p", sum, spicerack_ir::Span::NONE);
        let q = b.param(top, "q", sum, spicerack_ir::Span::NONE);
        (p, q)
    };
    let mut calls = 0;
    let changed = rewrite_exprs(&mut d, &mut |b, e| {
        calls += 1;
        let one = b.num(1.0);
        Some(b.ternary(one, e, e))
    });
    assert_eq!(calls, 3, "1, 2 and their sum, once each");
    assert_eq!(changed, 2, "both roots");
    let row = |i: ItemRef| d.row::<spicerack_ir::item::Param>(i.index).value;
    assert_eq!(row(p), row(q), "one rewrite for the shared tree");
    // a node holding one child twice (`x + x`): the child is rewritten once
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", spicerack_ir::Role::Deck);
        let x = b.num(1.0);
        let twice = b.binary(spicerack_ir::BinOp::Add, x, x);
        b.param(top, "p", twice, spicerack_ir::Span::NONE);
    }
    let mut calls = 0;
    rewrite_exprs(&mut d, &mut |_, _| {
        calls += 1;
        None
    });
    assert_eq!(calls, 2, "x and x + x");
}

#[test]
fn a_deep_tree_rewrites_without_recursion() {
    // C11: a 10,000-deep chain of negations, every node rewritten, in a debug build
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", spicerack_ir::Role::Deck);
        let mut e = b.num(1.0);
        for _ in 0..10_000 {
            e = b.unary(spicerack_ir::UnOp::Neg, e);
        }
        b.param(top, "deep", e, spicerack_ir::Span::NONE);
    }
    let before = d.expr_count();
    let mut calls = 0u32;
    let changed = rewrite_exprs(&mut d, &mut |b, _| {
        calls += 1;
        (calls == 1).then(|| b.num(3.0)) // the leaf changes: the whole spine is appended again
    });
    assert_eq!((changed, calls), (1, 10_001));
    assert_eq!(
        d.expr_count(),
        before + 1 + 10_000,
        "the new leaf and a new spine"
    );
    let mut diags = Diags::new();
    check(&d, &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
}

#[test]
fn a_rewrite_of_one_form_keeps_the_other() {
    // A26: wrapping every node of the roots of one form in `-(-e)` adds that form's Neg feature
    // and never the other's; the other form's roots keep their ids; every value is unchanged
    for form in [Form::Static, Form::Dynamic] {
        let (wrapper, other) = match form {
            Form::Static => (Feature::Unary(UnOp::Neg), Feature::UnaryDyn(UnOp::Neg)),
            Form::Dynamic => (Feature::UnaryDyn(UnOp::Neg), Feature::Unary(UnOp::Neg)),
            Form::Both => unreachable!("a pass rewrites one form"),
        };
        let mut reached = false;
        for seed in 0..100u64 {
            let d = generate(seed, &FeatureSet::ALL, SIZE);
            let items: Vec<ItemRef> = d.reachable().into_iter().map(|(i, _)| i).collect();
            let before: Vec<Vec<(ExprId, ExprUse)>> =
                items.iter().map(|&i| visited(&d, i)).collect();
            let forms: Vec<Vec<Form>> = before
                .iter()
                .map(|roots| roots.iter().map(|&(e, u)| root_form(&d, e, u)).collect())
                .collect();
            let mut after = d.clone();
            rewrite_exprs_in(&mut after, form, &mut |b, e| {
                let once = b.unary(UnOp::Neg, e);
                Some(b.unary(UnOp::Neg, once))
            });
            let (used_before, used_after) = (features_used(&d), features_used(&after));
            assert_eq!(
                used_after.contains(other),
                used_before.contains(other),
                "seed {seed}, {form:?}: the other form's feature is not added"
            );
            for ((&item, old), kinds) in items.iter().zip(&before).zip(&forms) {
                let new = visited(&after, item);
                for ((&(o, _), &(n, _)), &f) in old.iter().zip(&new).zip(kinds) {
                    if o.is_none() {
                        continue;
                    }
                    if f == form {
                        assert_ne!(o, n, "seed {seed}, {item:?}: a root of the form changes");
                        reached = true;
                    } else {
                        assert_eq!(o, n, "seed {seed}, {item:?}: the other form keeps its id");
                    }
                    match (eval(&d, o, &Fixed), eval(&after, n, &Fixed)) {
                        (Ok(x), Ok(y)) => assert!(
                            x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()),
                            "seed {seed}, {item:?}: {x} became {y}"
                        ),
                        (Err(_), Err(_)) => {}
                        (x, y) => panic!("seed {seed}, {item:?}: {x:?} became {y:?}"),
                    }
                }
            }
            if forms.iter().flatten().any(|&f| f == form) {
                assert!(
                    used_after.contains(wrapper),
                    "seed {seed}, {form:?}: the wrapper counts"
                );
            }
            let mut diags = Diags::new();
            check(&after, &mut diags);
            assert!(diags.is_empty(), "seed {seed}: {:?}", diags.rows());
        }
        assert!(reached, "{form:?}: some root of the form was rewritten");
    }
}

#[test]
fn the_form_decision_is_the_feature_count_s() {
    // A26, R66: a root counts dynamic when its class is Temp or above or it is a behavioural
    // source's expression: root_form is what features_used counts (oracle: the R14 and R66
    // spec examples)
    let read = |text: &str| {
        let mut d = Design::new();
        spicerack_ir::parse(text, &mut d.builder(), &mut Diags::new());
        d
    };
    let d = read("file \"t.sp\" deck\n  param p = 2 + 3\n  param t = $temp + 1\n");
    let roots: Vec<(ExprId, ExprUse)> = d
        .reachable()
        .into_iter()
        .flat_map(|(i, _)| visited(&d, i))
        .collect();
    let forms: Vec<Form> = roots.iter().map(|&(e, u)| root_form(&d, e, u)).collect();
    assert_eq!(
        forms,
        vec![Form::Static, Form::Dynamic],
        "a constant, then $temp"
    );
    // R66: the same constant in a field that may read the circuit counts static, in a
    // behavioural source's expression dynamic
    assert_eq!(root_form(&d, roots[0].0, ExprUse::Circuit), Form::Static);
    assert_eq!(
        root_form(&d, roots[0].0, ExprUse::Behavioural),
        Form::Dynamic
    );
}

#[test]
fn one_tree_is_rewritten_alone() {
    // round 7b door 3: `rewrite_tree` on one root: an identity rule returns the root itself and
    // appends nothing; a wrapping rule returns a tree of equal value and leaves the original
    for seed in 0..100u64 {
        let mut d = generate(seed, &FeatureSet::ALL, SIZE);
        let roots: Vec<ExprId> = d
            .reachable()
            .into_iter()
            .flat_map(|(i, _)| visited(&d, i))
            .map(|(e, _)| e)
            .filter(|e| !e.is_none())
            .collect();
        let Some(&root) = roots.first() else { continue };
        let before = d.clone();
        let n = d.expr_count();
        let same = rewrite_tree(&mut d.builder(), root, &mut |_, _| None);
        assert_eq!((same, d.expr_count()), (root, n), "seed {seed}: identity");
        let new = rewrite_tree(&mut d.builder(), root, &mut |b, e| {
            let one = b.num(1.0);
            Some(b.ternary(one, e, e))
        });
        assert_ne!(new, root, "seed {seed}");
        match (eval(&before, root, &Fixed), eval(&d, new, &Fixed)) {
            (Ok(x), Ok(y)) => assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())),
            (Err(_), Err(_)) => {}
            (x, y) => panic!("seed {seed}: {x:?} became {y:?}"),
        }
        // the original rows are untouched: the old root still evaluates as before
        assert!(d.eq_meaning(&d.clone()));
        let old_now = eval(&d, root, &Fixed).ok().map(f64::to_bits);
        assert_eq!(
            old_now,
            eval(&before, root, &Fixed).ok().map(f64::to_bits),
            "seed {seed}"
        );
    }
}

#[test]
fn a_bound_read_in_one_call() {
    let mut d = Design::new();
    let mut b = d.builder();
    let (_, top) = b.file("t.sp", spicerack_ir::Role::Deck);
    let one = b.num(1.0);
    b.param(top, "p", one, spicerack_ir::Span::NONE);
    let def = b
        .design()
        .lookup(top, spicerack_ir::DefKind::Param, b.design().find_name("p"));
    let read = b.param_ref_to(def);
    match b.design().expr(read) {
        spicerack_ir::Expr::Param(r) => assert_eq!((r.def, b.design().str(r.name)), (def, "p")),
        other => panic!("{other:?}"),
    }
}
