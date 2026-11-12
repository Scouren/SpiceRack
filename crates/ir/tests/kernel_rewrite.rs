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
    Bindings, Design, Diags, Dist, DrawPer, DrawWhen, ExprId, ExprUse, FeatureSet, Handle,
    ItemKind, ItemRef, NameId, Ref, check, eval, rewrite_exprs,
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

/// The (root, use) list `handles` implies for `item`.
fn from_handles(d: &Design, item: ItemRef) -> Vec<(ExprId, ExprUse)> {
    let mut handles = Vec::new();
    d.item_handles(item, &mut handles);
    let mut out = Vec::new();
    for h in handles {
        match h {
            Handle::Expr(e) => out.push((e, ExprUse::Constant)),
            Handle::ExprDyn(e) => out.push((e, ExprUse::Circuit)),
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
    // known gap, a named marker: the owners' rows write their part in round 4 (A23); a kind
    // that stops drifting fails this until its line goes
    let not_yet = [
        ItemKind::Statistics,
        ItemKind::Analysis,
        ItemKind::Loop,
        ItemKind::Alter,
        ItemKind::Option,
        ItemKind::Env,
        ItemKind::NodeSet,
        ItemKind::Save,
        ItemKind::Print,
        ItemKind::Measure,
    ];
    assert_eq!(drift, not_yet, "kinds whose map_exprs drifts from handles");
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
