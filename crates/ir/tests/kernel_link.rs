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

/// The A43 fixture deck: one alter, loop, DC axis or elaborate per case of `unbound_targets`.
const TARGETS: &str = "file \"t.sp\" deck
  param p = 1
  subckt t (a b)
    element resistor r3 (a b) value=1
  subckt s (a b)
    element resistor r2 (a b) value=1
    element instance y1 (a b) model=t
  element resistor r1 (n1 0) value=1
  element resistor r4 (n1 0) value=1
  element instance x1 (n1 0) model=s
  element instance xu (n1 0) model=nokit
  alter (param p) value=2
  alter (param q) value=2
  alter (device_param r1 r) value=2
  alter (device_param __topinst__ a) value=2
  alter (device_param r2 r path=(x1)) value=2
  alter (device_param r3 r path=(x1 y1)) value=2
  alter (device_param r9 r path=(x1 y1)) value=2
  alter (device_param r5 r path=(xu)) value=2
  alter (device_param r5 r path=(r4)) value=2
  elaborate (s nosuch)
  loop sweep (param zz) (list values=[1])
    analysis op
  analysis dc (source_dc vmissing) (lin start=0 stop=1 step=1)
";

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

/// `unbound_targets` of `d` as (the item's position in the deck's top, the name).
fn unbound_at(d: &Design) -> Vec<(usize, String)> {
    let top = d.file(d.entry()).top;
    spicerack_ir::unbound_targets(d)
        .iter()
        .map(|u| {
            let k = d
                .items(top)
                .iter()
                .position(|&i| i == u.item)
                .expect("a top item");
            (k, d.str(u.name).to_string())
        })
        .collect()
}

fn targets_deck() -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(TARGETS, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    d
}

#[test]
fn the_unbound_targets_are_the_ones_that_provably_name_nothing() {
    // A43 (R70); oracle: the expected list written out by hand, case by case (positions in the
    // deck's top: 0 p, 1 t, 2 s, 3 r1, 4 r4, 5 x1, 6 xu, then the items in order from 7)
    let d = targets_deck();
    let want: Vec<(usize, String)> = [
        (8, "q"),            // an unresolved parameter
        (10, "__topinst__"), // F7's shape: no such element at the top
        (13, "r9"),          // no such element two levels down
        (15, "r5"),          // nothing lives below a resistor
        (16, "nosuch"),      // an elaborate of no subcircuit
        (17, "zz"),          // a loop over no parameter
        (18, "vmissing"),    // a DC axis over no source
    ]
    .into_iter()
    .map(|(k, n)| (k, n.to_string()))
    .collect();
    // not listed: p (bound), r1, r2 through x1, r3 through x1 y1 (bound), r5 through xu (an
    // unresolved master: the absent kit may hold it), s (bound)
    assert_eq!(unbound_at(&d), want);
}

#[test]
fn deleting_a_bound_target_s_element_lists_exactly_its_item() {
    // the metamorphic check: removing the element a bound target names makes exactly that
    // item appear, with the element's name
    for (element, at, alter) in [
        ("r1", None, 9),
        ("r2", Some("s"), 11),
        ("r3", Some("t"), 12),
    ] {
        let mut d = targets_deck();
        let top = d.file(d.entry()).top;
        let alter = d.items(top)[alter];
        let listed = |d: &Design| -> Vec<(spicerack_ir::ItemRef, String)> {
            let all = spicerack_ir::unbound_targets(d);
            all.iter()
                .map(|u| (u.item, d.str(u.name).to_string()))
                .collect()
        };
        let before = listed(&d);
        let scope = match at {
            None => top,
            Some(sub) => {
                let def = d.lookup(top, DefKind::Subckt, d.find_name(sub));
                d.row::<spicerack_ir::item::Subckt>(d.def(def).item.index)
                    .body
            }
        };
        let item = *d
            .items(scope)
            .iter()
            .find(|i| {
                i.kind == spicerack_ir::ItemKind::Element
                    && d.row::<spicerack_ir::Element>(i.index).name == d.find_name(element)
            })
            .expect("the element");
        d.editor().remove_item(scope, item);
        let mut after = listed(&d);
        after.retain(|r| !before.contains(r));
        assert_eq!(after, vec![(alter, element.to_string())], "{element}");
    }
}
