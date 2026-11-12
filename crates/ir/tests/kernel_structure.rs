//! The design's structure as callers see it: `Design::reachable`'s order (files in id order,
//! depth first, each body right after its item), the one empty list, and `Builder::bind_ref`
//! (A11: a row reference bound after its definition is pushed).
//!
//! Oracles: hand-built designs whose expected order and rows are written out in the test; for
//! `bind_ref`, the same pushes with the reference resolved from the start (`eq_ignoring_spans`).
//!
//! Contents: none (tests only).

use spicerack_ir::item::Subckt;
use spicerack_ir::{DefId, DefKind, Design, ExprId, Ref, Role, Span};

// ---- private ----

#[test]
fn reachable_is_files_in_order_bodies_after_their_item() {
    let mut d = Design::new();
    let want = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        let c = b.comment(top, "first", Span::NONE);
        let (outer, outer_body) = b.subckt(top, "outer", &[], &[], Span::NONE);
        let (inner, inner_body) = b.subckt(outer_body, "inner", &[], &[], Span::NONE);
        let deep = b.comment(inner_body, "deep", Span::NONE);
        let one = b.num(1.0);
        let (iff, branches) = b.if_(outer_body, &[one, ExprId::NONE], Span::NONE);
        let (then_body, else_body) = {
            let br = b.design().branches(branches);
            (br[0].body, br[1].body)
        };
        let t = b.comment(then_body, "then", Span::NONE);
        let e = b.comment(else_body, "else", Span::NONE);
        let after = b.comment(top, "after", Span::NONE);
        let (_, lib) = b.file("lib.sp", Role::Library);
        let l = b.comment(lib, "lib", Span::NONE);
        vec![
            (c, top),
            (outer, top),
            (inner, outer_body),
            (deep, inner_body),
            (iff, outer_body),
            (t, then_body),
            (e, else_body),
            (after, top),
            (l, lib),
        ]
    };
    assert_eq!(d.reachable(), want);
}

#[test]
fn an_empty_list_is_the_one_empty_list() {
    let mut d = Design::new();
    let (lists, sub) = {
        let mut b = d.builder();
        let (_, top) = b.file("top.sp", Role::Deck);
        b.global(top, &["vdd"], Span::NONE); // the name pool is no longer empty
        let one = b.num(1.0);
        b.exprs(&[one]);
        let lists = (b.names(&[]), b.name_ids(&[]), b.exprs(&[]), b.params(&[]));
        let (sub, _) = b.subckt(top, "s", &[], &[], Span::NONE);
        (lists, sub)
    };
    use spicerack_ir::{ExprList, NameList, ParamList};
    assert_eq!(
        lists,
        (
            NameList::EMPTY,
            NameList::EMPTY,
            ExprList::EMPTY,
            ParamList::EMPTY
        )
    );
    let row: Subckt = d.row(sub.index);
    assert_eq!((row.ports, row.params), (NameList::EMPTY, ParamList::EMPTY));
}

#[test]
fn bind_ref_binds_a_row_reference_pushed_before_its_definition() {
    use spicerack_ir::{ElemKind, Element, NO_ROW, NameList, ParamList};
    // an instance pushed before its subcircuit, bound afterwards by the reader's own rule,
    // equals the same pushes with the reference resolved from the start
    let build = |bind_later: bool| {
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let (_, top) = b.file("top.sp", Role::Deck);
            let master = b.name("inv");
            let name = b.name("x1");
            // the subckt pushed next gets definition 0
            let def = if bind_later { DefId::NONE } else { DefId(0) };
            let row = Element {
                kind: ElemKind::Instance,
                name,
                terminals: NameList::EMPTY,
                model: Ref { name: master, def },
                params: ParamList::EMPTY,
                mult: ExprId::NONE,
                detail: NO_ROW,
            };
            let x1 = b.push(top, row, Span::NONE);
            b.subckt(top, "inv", &[], &[], Span::NONE);
            if bind_later {
                let sub = b.design().lookup(top, DefKind::Subckt, master);
                b.bind_ref(x1, 0, sub);
            }
        }
        d
    };
    assert!(build(true).eq_ignoring_spans(&build(false)));
}

#[test]
fn entry_is_total() {
    assert_eq!(
        Design::new().entry(),
        spicerack_ir::FileId::NONE,
        "no file: no entry"
    );
    let mut d = Design::new();
    {
        let mut b = d.builder();
        b.file("lib.sp", Role::Library);
        b.file("top.sp", Role::Deck);
    }
    assert_eq!(d.entry(), spicerack_ir::FileId(1), "the first deck");
    let mut libs = Design::new();
    libs.builder().file("only.lib", Role::Library);
    assert_eq!(libs.entry(), spicerack_ir::FileId(0), "no deck: file 0");
}
