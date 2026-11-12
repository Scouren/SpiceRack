//! The neutral linker (A4): a forward reference binds, an inner scope's definition shadows an
//! outer one, a reference no definition matches stays unresolved without a diagnostic, a kind
//! mismatch does not bind, a bound reference is never changed, and user calls bind to functions.
//!
//! Oracle: hand-built designs whose expected bindings are written out in each test.
//!
//! Contents: none (tests only).

use spicerack_ir::{
    BinOp, DefId, DefKind, Design, Diags, Expr, ExprId, Ref, Role, ScopeId, Span, UnboundRead,
    link, parse, unbound_reads,
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

#[test]
fn a_default_never_reads_its_own_formal() {
    // A29 (R53): `subckt s () t=(t + 1) u=(t * 2)` beside an outer `t`: `u`'s `t` is the sibling
    // formal (as A10), but `t`'s own `t` is never the formal itself: it is what is visible from
    // where the subcircuit is defined (the outer `t`), or unbound when nothing is
    let mut d = Design::new();
    let (top, own, sibling, sub) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let five = b.num(5.0);
        b.param(top, "t", five, Span::NONE);
        let own = b.param_ref("t");
        let one = b.num(1.0);
        let t = b.binary(BinOp::Add, own, one);
        let sibling = b.param_ref("t");
        let two = b.num(2.0);
        let u = b.binary(BinOp::Mul, sibling, two);
        let (sub, _) = b.subckt(top, "s", &[], &[("t", t), ("u", u)], Span::NONE);
        (top, own, sibling, sub)
    };
    link(&mut d, &mut Diags::new());
    assert_eq!(bound(&d, own), param_def(&d, top, "t"), "the outer t");
    assert_eq!(
        d.def(bound(&d, sibling)).item,
        sub,
        "u reads the sibling formal"
    );
    // with no outer `t`, the default's read stays unbound
    let mut lone = Design::new();
    let own = {
        let mut b = lone.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let own = b.param_ref("t");
        b.subckt(top, "s", &[], &[("t", own)], Span::NONE);
        own
    };
    link(&mut lone, &mut Diags::new());
    assert!(
        bound(&lone, own).is_none(),
        "unbound, read where the instance stands"
    );
    let mut diags = Diags::new();
    spicerack_ir::check(&lone, &mut diags);
    assert!(diags.is_empty(), "no cycle: {:?}", diags.rows());
    // an unbound read in a default is not constant (A29): the formal is FormalNotConstant
    let used = spicerack_ir::features_used(&lone);
    assert!(used.contains(spicerack_ir::Feature::Extra(
        spicerack_ir::Extra::FormalNotConstant
    )));
}

#[test]
fn the_unbound_reads_are_listed_at_their_items_held_ones_apart() {
    // A38 (R63; oracle: the reads written by hand): an unbound read in an element's value and
    // one in a model parameter are listed at their items, in listed order; a formal default's
    // held read is listed apart; a bound read is not listed
    let src = concat!(
        "file \"t.sp\" deck\n",
        "  param k = 1\n",
        "  element resistor r1 (a b) value=(zz + k)\n",
        "  model dmod diode params=(is=yy)\n",
        "  subckt s (p q) w=ww?\n",
    );
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(src, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let top = d.file(d.entry()).top;
    let items = d.items(top);
    let rows: Vec<(spicerack_ir::ItemRef, &str, bool)> = unbound_reads(&d)
        .into_iter()
        .map(|r: UnboundRead| (r.item, d.str(r.name), r.held))
        .collect();
    assert_eq!(
        rows,
        vec![
            (items[1], "zz", false),
            (items[2], "yy", false),
            (items[3], "ww", true)
        ]
    );
}
