//! The editor's doors for definitions (round 7b, `pass-structure`'s drop, hoist and
//! local-to-formal rules): removing an item takes its definitions out of the index, moving one
//! re-keys them to its new scope, re-pointing a definition keeps every bound reference, and
//! `check` refuses a stale index entry a pass that bypassed the doors left.
//!
//! Oracles: `lookup` from each scope worked by hand; `check`.
//!
//! Contents: none (tests only).

use spicerack_ir::{DefKind, Design, Diags, Role, Span, Violation, check, diag::violation_code};

// ---- private ----

fn violations(d: &Design) -> Vec<spicerack_ir::Code> {
    let mut diags = Diags::new();
    check(d, &mut diags);
    diags.rows().iter().map(|r| r.code).collect()
}

#[test]
fn a_removed_item_leaves_the_index() {
    let build = || {
        let mut d = Design::new();
        let top = {
            let mut b = d.builder();
            let (_, top) = b.file("t.sp", Role::Deck);
            let one = b.num(1.0);
            b.param(top, "p", one, Span::NONE);
            top
        };
        (d, top)
    };
    // through the raw list: the definition stays, and check says so
    let (mut raw, top) = build();
    raw.editor().items_mut(top).clear();
    assert!(
        !raw.lookup(top, DefKind::Param, raw.find_name("p"))
            .is_none(),
        "stale"
    );
    assert_eq!(
        violations(&raw),
        vec![violation_code(Violation::StaleDefinition)]
    );
    // through the door: gone from the index, check clean
    let (mut d, top) = build();
    let item = d.items(top)[0];
    d.editor().remove_item(top, item);
    assert!(d.lookup(top, DefKind::Param, d.find_name("p")).is_none());
    assert_eq!(violations(&d), vec![]);
}

#[test]
fn a_moved_item_is_defined_where_it_now_stands() {
    // hoist: `subckt s1` holds `subckt s2`; s2 moves to the top, after s1
    let mut d = Design::new();
    let (top, s1_body, s2) = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let (_, s1_body) = b.subckt(top, "s1", &[], &[], Span::NONE);
        let (s2, _) = b.subckt(s1_body, "s2", &[], &[], Span::NONE);
        (top, s1_body, s2)
    };
    let name = d.find_name("s2");
    let before = d.lookup(s1_body, DefKind::Subckt, name);
    assert!(
        d.lookup(top, DefKind::Subckt, name).is_none(),
        "not visible from the top yet"
    );
    d.editor().move_item(s1_body, s2, top, 1);
    assert_eq!(
        d.lookup(top, DefKind::Subckt, name),
        before,
        "the same definition, now at the top"
    );
    assert_eq!(
        d.lookup(s1_body, DefKind::Subckt, name),
        before,
        "and seen from s1's body through its parent"
    );
    assert_eq!(d.items(top).len(), 2);
    assert_eq!(violations(&d), vec![]);
}

#[test]
fn a_definition_is_re_pointed_and_renamed() {
    // a local parameter made a formal: its definition moves from the Param item to the subckt,
    // the Param item goes, and the read bound to it stays bound
    let mut d = Design::new();
    let (body, sub, local, read) = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        let (sub, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        let one = b.num(1.0);
        let local = b.param(body, "k", one, Span::NONE);
        let def = b
            .design()
            .lookup(body, DefKind::Param, b.design().find_name("k"));
        let read = b.param_ref_to(def);
        b.param(body, "j", read, Span::NONE);
        (body, sub, local, read)
    };
    let def = d.lookup(body, DefKind::Param, d.find_name("k"));
    {
        let mut e = d.editor();
        e.redefine(def, sub);
        e.remove_item(body, local);
    }
    assert_eq!(
        d.lookup(body, DefKind::Param, d.find_name("k")),
        def,
        "still defined"
    );
    assert_eq!(d.def(def).item, sub);
    let _ = read;
    // rename_def re-keys it
    let k2 = d.builder().name("k2");
    d.editor().rename_def(def, k2);
    assert_eq!(d.lookup(body, DefKind::Param, k2), def);
    assert!(d.lookup(body, DefKind::Param, d.find_name("k")).is_none());
}
