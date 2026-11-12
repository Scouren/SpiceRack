//! A function body's form follows its callers (A28, R51): fixtures for `root_form` on bodies
//! and the inlining property over generated designs (crate tests: the substitution rebuilds
//! nodes through `Builder::rebuild`).
//!
//! Oracles: spec examples worked by hand (a body called from a static root, a dynamic one,
//! both, none); inlining every call by the test's own substitution adds no feature (law P3 for
//! `inline_funcs`).
//!
//! Contents: none (tests only).

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::expr::Expr;
use crate::facts::ExprUse;
use crate::feature::{Feature, FeatureSet, Form, features_used, root_form};
use crate::r#gen::{Size, generate};
use crate::id::{DefId, ExprId, NameId};
use crate::item::{DefKind, ItemKind, ScopeKind};
use crate::rewrite::rewrite_exprs;

// ---- private ----

/// A deck read from IR text.
fn read(text: &str) -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    crate::text::parse(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    d
}

/// The body of the function named `name` in the entry's top.
fn body(d: &Design, name: &str) -> ExprId {
    let top = d.file(d.entry()).top;
    let def = d.lookup(top, DefKind::Func, d.find_name(name));
    d.row::<crate::item::Func>(d.def(def).item.index).body
}

/// `e` with every read of a formal of `func` replaced by the argument in its position: the
/// test's own substitution (recursion on a generated tree's small depth).
fn subst(
    b: &mut Builder<'_>,
    e: ExprId,
    func: DefId,
    formals: &[NameId],
    args: &[ExprId],
) -> ExprId {
    if let Expr::Param(r) = b.design().expr(e)
        && r.def == func
    {
        let k = formals
            .iter()
            .position(|&f| f == r.name)
            .expect("a formal of the function");
        return args[k];
    }
    let mut kids = Vec::new();
    b.design().children(e, &mut kids);
    let new: Vec<ExprId> = kids
        .iter()
        .map(|&k| subst(b, k, func, formals, args))
        .collect();
    if new == kids { e } else { b.rebuild(e, &new) }
}

/// Inlines every resolved call of `d` until none is left, then lists no `Func` item.
fn inline_all(d: &mut Design) {
    for _ in 0..16 {
        let changed = rewrite_exprs(d, &mut |b, e| {
            let Expr::UserCall(r, args) = b.design().expr(e) else {
                return None;
            };
            if !r.is_resolved() || b.design().def(r.def).kind != DefKind::Func {
                return None;
            }
            let item = b.design().def(r.def).item;
            let f: crate::item::Func = b.design().row(item.index);
            let formals = b.design().names(f.args).to_vec();
            let args = b.design().exprs(args).to_vec();
            Some(subst(b, f.body, r.def, &formals, &args))
        });
        if changed == 0 {
            break;
        }
    }
    let scopes = d.scopes.len();
    let mut e = d.editor();
    for s in 0..scopes {
        e.items_mut(crate::id::ScopeId(s as u32))
            .retain(|i| i.kind != ItemKind::Func);
    }
}

#[test]
fn a_body_counts_in_its_callers_forms() {
    let d = read(concat!(
        "file \"t.sp\" deck\n",
        "  func s() = 2 + 3\n",
        "  func dy() = 4 * 5\n",
        "  func both() = 6 - 7\n",
        "  func none() = 8 / 9\n",
        "  param a = @s()\n",
        "  param b = @dy() + $temp\n",
        "  param c = @both()\n",
        "  param e = @both() + $temp\n",
    ));
    let form = |n: &str| root_form(&d, body(&d, n), ExprUse::Circuit);
    assert_eq!(form("s"), Form::Static, "called from a static root");
    assert_eq!(form("dy"), Form::Dynamic, "called from a dynamic root");
    assert_eq!(form("both"), Form::Both, "called from both");
    assert_eq!(form("none"), Form::Static, "called from none");
    let used = features_used(&d);
    use crate::expr::BinOp;
    assert!(
        used.contains(Feature::Binary(BinOp::Add))
            && !used.contains(Feature::BinaryDyn(BinOp::Add))
    );
    assert!(
        used.contains(Feature::BinaryDyn(BinOp::Mul))
            && !used.contains(Feature::Binary(BinOp::Mul))
    );
    assert!(
        used.contains(Feature::Binary(BinOp::Sub)) && used.contains(Feature::BinaryDyn(BinOp::Sub))
    );
    assert!(
        used.contains(Feature::Binary(BinOp::Div))
            && !used.contains(Feature::BinaryDyn(BinOp::Div))
    );
}

#[test]
fn inlining_every_call_adds_no_feature() {
    // A28: what a pass that inlines adds is what the forms already counted; it removes the
    // calls, the function items and their placements (and formal reads, when those were all)
    let size = Size {
        items: 40,
        scope_depth: 3,
        expr_depth: 4,
        files: 2,
    };
    let mut calls = 0;
    for seed in 0..200u64 {
        let d = generate(seed, &FeatureSet::ALL, size);
        let before = features_used(&d);
        if before.contains(Feature::Expr(crate::expr::ExprTag::UserCall)) {
            calls += 1;
        }
        let mut after = d.clone();
        inline_all(&mut after);
        let used = features_used(&after);
        let added: Vec<Feature> = used.minus(before).iter().collect();
        assert_eq!(
            added,
            Vec::<Feature>::new(),
            "seed {seed}: inlining adds no feature"
        );
        let gone: Vec<Feature> = before
            .minus(used)
            .iter()
            .filter(|f| {
                !matches!(
                    f,
                    Feature::Expr(crate::expr::ExprTag::UserCall | crate::expr::ExprTag::Param)
                        | Feature::Item(ItemKind::Func)
                        | Feature::Placed(_, ItemKind::Func)
                )
            })
            .collect();
        assert_eq!(
            gone,
            Vec::<Feature>::new(),
            "seed {seed}: and removes only the calls"
        );
    }
    assert!(calls > 20, "the generator made calls ({calls} seeds)");
    let _ = ScopeKind::File;
}
