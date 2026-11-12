//! The element-name index (R23, A13: an element's name is unique in its scope): a reader asks
//! the builder whether a name is taken before pushing, in O(1) amortized, so a duplicate can be
//! refused with nothing pushed. The index follows the design through the builder's pushes and is
//! rebuilt after a pass moves items.
//!
//! Oracles: the scope's item list (an element of that name listed there, first in order); the
//! design after an editor move.
//!
//! Contents: none (tests only).

use spicerack_ir::{Design, Diags, ItemKind, ItemRef, parse};

// ---- private ----

fn deck(body: &str) -> Design {
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(
        &format!("file \"t.sp\" deck\n{body}"),
        &mut d.builder(),
        &mut diags,
    );
    assert!(diags.is_empty(), "{:?}", diags.rows());
    d
}

#[test]
fn a_taken_name_is_found_in_its_scope() {
    let mut d = deck(
        "  element resistor r1 (a b) value=1k\n  subckt s (a b)\n    element resistor r1 (a b) value=2k\n",
    );
    let top = d.file(d.entry()).top;
    let r1 = d.find_name("r1");
    let first = d.items(top)[0];
    assert_eq!(first.kind, ItemKind::Element);
    let body = d
        .row::<spicerack_ir::item::Subckt>(d.items(top)[1].index)
        .body;
    let inner = d.items(body)[0];
    let mut b = d.builder();
    assert_eq!(b.element_named(top, r1), Some(first), "the top's r1");
    assert_eq!(b.element_named(body, r1), Some(inner), "the body's own r1");
    let r9 = b.name("r9");
    assert_eq!(b.element_named(top, r9), None);
}

#[test]
fn the_index_follows_pushes_and_moves() {
    let mut d = deck("  element resistor r1 (a b) value=1k\n");
    let top = d.file(d.entry()).top;
    // a push after the index was built is in it
    let r2 = {
        let mut b = d.builder();
        let r1 = b.name("r1");
        assert!(b.element_named(top, r1).is_some());
        let mut diags = Diags::new();
        parse(
            "file \"u.sp\" library\n  element resistor r2 (a b) value=1k\n",
            &mut b,
            &mut diags,
        );
        let r2 = b.name("r2");
        let u = b.design().file(b.design().find_file("u.sp")).top;
        assert!(b.element_named(u, r2).is_some(), "noted by the push");
        u
    };
    // a pass moves r1 out of the top: the index answers for the design as it is
    let r1_item: ItemRef = d.items(top)[0];
    {
        let mut e = d.editor();
        e.items_mut(top).remove(0);
        e.items_mut(r2).push(r1_item);
    }
    let mut b = d.builder();
    let r1 = b.name("r1");
    assert_eq!(b.element_named(top, r1), None, "moved away");
    assert_eq!(b.element_named(r2, r1), Some(r1_item), "moved here");
}
