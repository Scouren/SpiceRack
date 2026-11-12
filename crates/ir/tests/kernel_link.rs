//! The neutral linker (A4): a forward reference binds, an inner scope's definition shadows an
//! outer one, a reference no definition matches stays unresolved without a diagnostic, a kind
//! mismatch does not bind, a bound reference is never changed, and user calls bind to functions.
//!
//! Oracle: hand-built designs whose expected bindings are written out in each test.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    BinOp, DefId, DefKind, Design, Diags, Expr, ExprId, Ref, Role, ScopeId, Span, link,
};

// ---- private ----

fn param_def(d: &Design, scope: ScopeId, name: &str) -> DefId {
    d.lookup(scope, DefKind::Param, d.find_name(name))
}

fn bound(d: &Design, e: ExprId) -> DefId {
    match d.expr(e) {
        Expr::Param(r) | Expr::UserCall(r, _) => r.def,
        other => panic!("a reference, not {other:?}"),
    }
}

#[test]
fn forward_references_and_shadowing() {
    let mut d = Design::new();
    let (top, body, fwd, inner_read, outer_read) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        // `a` reads `b` before `b` is defined
        let fwd = b.param_ref("b");
        b.param(top, "a", fwd, Span::NONE);
        let one = b.num(1.0);
        b.param(top, "b", one, Span::NONE);
        // `k` is defined at the top and again in the subcircuit
        b.param(top, "k", one, Span::NONE);
        let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        b.param(body, "k", one, Span::NONE);
        let inner_read = b.param_ref("k");
        b.param(body, "use_inner", inner_read, Span::NONE);
        let outer_read = b.param_ref("k");
        b.param(top, "use_outer", outer_read, Span::NONE);
        (top, body, fwd, inner_read, outer_read)
    };
    let mut diags = Diags::new();
    link(&mut d, &mut diags);
    assert!(diags.is_empty(), "link is silent");
    assert_eq!(bound(&d, fwd), param_def(&d, top, "b"));
    assert_eq!(
        bound(&d, inner_read),
        param_def(&d, body, "k"),
        "the inner scope wins"
    );
    assert_eq!(bound(&d, outer_read), param_def(&d, top, "k"));
    assert_ne!(param_def(&d, body, "k"), param_def(&d, top, "k"));
}

#[test]
fn unmatched_and_mismatched_stay_unresolved() {
    let mut d = Design::new();
    let (nothing, wrong_kind) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        b.subckt(top, "inv", &[], &[], Span::NONE);
        let nothing = b.param_ref("absent");
        let wrong_kind = b.param_ref("inv"); // names a subckt, not a parameter
        let sum = b.binary(BinOp::Add, nothing, wrong_kind);
        b.param(top, "p", sum, Span::NONE);
        (nothing, wrong_kind)
    };
    let mut diags = Diags::new();
    link(&mut d, &mut diags);
    assert!(diags.is_empty());
    assert_eq!(bound(&d, nothing), DefId::NONE);
    assert_eq!(bound(&d, wrong_kind), DefId::NONE);
}

#[test]
fn a_bound_reference_is_kept_and_calls_bind_to_functions() {
    let mut d = Design::new();
    let (top, pinned, call, f_def) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let one = b.num(1.0);
        b.param(top, "k", one, Span::NONE);
        let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        b.param(body, "k", one, Span::NONE);
        // a reader bound this read to the outer `k` by its own rule; link must not move it
        let pinned = b.param_ref("k");
        let outer = b
            .design()
            .lookup(top, DefKind::Param, b.design().find_name("k"));
        b.resolve(pinned, outer);
        b.param(body, "q", pinned, Span::NONE);
        let x = b.param_ref("x");
        b.func(top, "f", &["x"], x, Span::NONE);
        let f_def = b
            .design()
            .lookup(top, DefKind::Func, b.design().find_name("f"));
        let call = b.user_call("f", &[one]);
        b.param(top, "r", call, Span::NONE);
        (top, pinned, call, f_def)
    };
    link(&mut d, &mut Diags::new());
    assert_eq!(bound(&d, pinned), param_def(&d, top, "k"));
    assert_eq!(bound(&d, call), f_def);
    let Expr::UserCall(Ref { def, .. }, _) = d.expr(call) else {
        panic!("a call")
    };
    assert_eq!(def, f_def);
}

#[test]
fn a_subckt_default_reads_its_sibling_head_parameter() {
    // A10: `subckt s () r=1 w=(2 * r)` beside an outer `r`: `w`'s `r` is the sibling, not the outer
    let mut d = Design::new();
    let (top, read, sub) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let five = b.num(5.0);
        b.param(top, "r", five, Span::NONE);
        let one = b.num(1.0);
        let read = b.param_ref("r");
        let two = b.num(2.0);
        let w = b.binary(BinOp::Mul, two, read);
        let (sub, _) = b.subckt(top, "s", &[], &[("r", one), ("w", w)], Span::NONE);
        (top, read, sub)
    };
    link(&mut d, &mut Diags::new());
    let def = bound(&d, read);
    assert_eq!(
        d.def(def).item,
        sub,
        "bound to the subckt's own head parameter"
    );
    assert_ne!(def, param_def(&d, top, "r"), "the outer `r` is shadowed");
}
