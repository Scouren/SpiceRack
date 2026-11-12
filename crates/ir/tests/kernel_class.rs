//! The dynamic class (A11, R14): transitive through parameter and function reads, the `*Dyn`
//! features that follow it, and the per-design class cache, which every door's mutation clears.
//!
//! Oracles: spec examples (each class worked by hand from the reads written in the test); each
//! cache case is read before and after one mutation, and goes red with that mutation's clear
//! removed.
//!
//! Contents: none (tests only).

use spicerack_ir::{BinOp, Design, Diags, Feature, FeatureSet, Func, Role, Span, features_used};

// ---- private ----

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

#[test]
fn a_class_read_after_a_mutation_sees_the_mutation() {
    // the class cache (A11) is cleared by the doors' mutations: a reader binding a read inside
    // a definition's value after classifying mid-build, and a pass replacing a value with an
    // expression built before the cache was filled, both read the new class
    use spicerack_ir::{DefKind, ExprClass, expr_class, item::Param};
    let mut d = Design::new();
    let (p_read, t_early, p_index) = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let t = b.temp();
        b.param(top, "hot", t, Span::NONE);
        let t_early = b.temp(); // for the pass below
        let one = b.num(1.0);
        let p_item = b.param(top, "p", one, Span::NONE);
        // q = late (unbound), r = q
        let late = b.param_ref("hot");
        b.param(top, "q", late, Span::NONE);
        let q_read = b.param_ref("q");
        b.param(top, "r", q_read, Span::NONE);
        let find = |b: &spicerack_ir::Builder<'_>, n: &str| {
            b.design()
                .lookup(top, DefKind::Param, b.design().find_name(n))
        };
        let q = find(&b, "q");
        b.resolve(q_read, q);
        assert_eq!(
            expr_class(b.design(), q_read),
            ExprClass::Constant,
            "q is unbound"
        );
        let hot = find(&b, "hot");
        b.resolve(late, hot); // q's value now reads hot: q's definition class moves
        assert_eq!(
            expr_class(b.design(), q_read),
            ExprClass::Temp,
            "q reads $temp"
        );
        let p_read = b.param_ref("p");
        b.param(top, "s", p_read, Span::NONE);
        let p = find(&b, "p");
        b.resolve(p_read, p);
        (p_read, t_early, p_item.index)
    };
    assert_eq!(expr_class(&d, p_read), ExprClass::Constant, "p = 1");
    // a pass sets p's value to a $temp built before the cache was filled
    {
        let mut e = d.editor();
        let row: Param = e.design().row(p_index);
        assert_eq!(
            expr_class(e.design(), p_read),
            ExprClass::Constant,
            "read in the pass"
        );
        e.set(
            p_index,
            Param {
                value: t_early,
                ..row
            },
        );
        assert_eq!(
            expr_class(e.design(), p_read),
            ExprClass::Temp,
            "and after its set"
        );
    }
    assert_eq!(expr_class(&d, p_read), ExprClass::Temp, "p = $temp now");
}
