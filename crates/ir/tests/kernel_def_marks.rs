//! A bound reference's `#k` mark in IR text names its definition by the place the text gives it
//! (the definitions numbered in item order, the order `parse` creates them), not by the
//! design's raw definition index. Two designs that differ only in the order their definitions
//! were created (a pass that defines last what stands first: pair-2's twinless OSDI decks) dump
//! to one text, and that text reads back to a design that dumps to it again.
//!
//! Oracle: a round trip and the equality of two dumps built apart by hand.
//!
//! Contents: tests only.

use spicerack_ir::{Builder, Design, Diags, Role, ScopeId, Span, dump, parse};

// ---- private ----

/// `param a = 1` at the top; `subckt s ()` whose body defines its own `a = 2` and reads the
/// top's `a` in `b = a` (bound past the shadow, so dumped as `a#k`). `late`: the top's `a` is
/// created after the subcircuit, then moved first, so its definition index is the last.
fn design(late: bool) -> Design {
    let mut d = Design::new();
    let top = {
        let mut b = d.builder();
        let (_, top) = b.file("t.sp", Role::Deck);
        if !late {
            top_param(&mut b, top);
        }
        let (_, body) = b.subckt(top, "s", &[], &[], Span::NONE);
        let two = b.num(2.0);
        b.param(body, "a", two, Span::NONE);
        if late {
            top_param(&mut b, top);
        }
        let top_a = b
            .design()
            .lookup(top, spicerack_ir::DefKind::Param, b.design().find_name("a"));
        let read = b.param_ref_to(top_a);
        b.param(body, "b", read, Span::NONE);
        top
    };
    if late {
        let mut e = d.editor();
        let items = e.items_mut(top);
        let a = items.pop().expect("the top's a");
        items.insert(0, a);
    }
    d
}

fn top_param(b: &mut Builder<'_>, top: ScopeId) {
    let one = b.num(1.0);
    b.param(top, "a", one, Span::NONE);
}

fn text(d: &Design) -> String {
    let mut out = String::new();
    dump(d, d.entry(), &mut out);
    out
}

#[test]
fn a_definition_mark_does_not_depend_on_creation_order() {
    let (early, late) = (design(false), design(true));
    assert!(text(&early).contains("a#"), "{}", text(&early));
    assert_eq!(text(&late), text(&early));
    let mut back = Design::new();
    let mut diags = Diags::new();
    parse(&text(&late), &mut back.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    assert_eq!(text(&back), text(&late), "the text reads back to itself");
}
