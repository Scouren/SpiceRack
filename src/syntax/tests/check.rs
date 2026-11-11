//! `Ast::check` (`check.rs`) against the layouts `ast.rs`'s header and tag
//! docs state: every tree these tests read passes it (`mod.rs`'s `read`
//! asserts so, one tree per node tag and dialect), and one hand-broken row
//! per word kind is refused, blamed by its node id.
//!
//! Contents: none (tests only).

use crate::syntax::Lang;
use crate::syntax::ast::{Ast, CheckError, NONE, Node, Tag, WordKind};
use crate::syntax::{parse, tokenize};

/// A deck whose tree holds a header, a delimited ternary and a body.
const SRC: &str = "t\n.subckt s a b\nR1 a b {x ? 1 : 2}\n.ends\n.end\n";

/// No node, token, name or `extra_data` offset of [`SRC`]'s tree.
const FAR: u32 = 1_000_000;

// ---- private ----

/// [`SRC`]'s tree with the row `add` appends: that row's id, and what
/// `check` says of the tree.
fn checked(add: impl FnOnce(&mut Ast<'static>) -> Node) -> (u32, Result<(), CheckError>) {
    let lex = Lang::NGSPICE.lexicon().expect("a reader");
    let t = tokenize(SRC, lex);
    let mut a = parse(&t, lex);
    assert_eq!(a.check(&t), Ok(()), "the parse itself is sound");
    let n = add(&mut a);
    (n.0, a.check(&t))
}

#[test]
fn p8_002_check_refuses_one_broken_row_per_word_kind() {
    // Oracle: the layouts in `ast.rs`'s tag docs, broken by hand. Each row
    // below is well formed but for the one word of the kind named beside
    // it. `Value` has no broken form (any number reads). Checks: 3 per row.
    let rows = [
        (Tag::Title, 0, NONE, WordKind::Unused),
        (Tag::Unary, FAR, NONE, WordKind::Node),
        (Tag::Unary, NONE, NONE, WordKind::Node),
        (Tag::Assign, 0, FAR, WordKind::NodeOpt),
        (Tag::Bad, FAR, NONE, WordKind::Token),
        (Tag::Str, FAR, NONE, WordKind::TokenOpt),
        (Tag::Ident, FAR, 0, WordKind::Name),
        (Tag::Seq, FAR, NONE, WordKind::List),
        (Tag::Instance, FAR, NONE, WordKind::Header),
        (Tag::Param, NONE, FAR, WordKind::Body),
        (Tag::Vector, NONE, FAR, WordKind::Group),
        (Tag::Ternary, 0, FAR, WordKind::Pair),
        (Tag::Index, 0, FAR, WordKind::IndexTail),
    ];
    for (tag, lhs, rhs, kind) in rows {
        assert!(tag.word_kinds().contains(&kind), "{tag:?} has a {kind:?}");
        let (n, got) = checked(|a| a.push(tag, 0, lhs, rhs));
        let e = got.expect_err("a broken row");
        assert_eq!((e.node, e.token), (n, NONE), "{tag:?} {kind:?}: {}", e.what);
    }
}

#[test]
fn p8_002_check_reads_the_words_behind_an_extra_offset() {
    // Oracle: as above, one level down: a list item, a header's node, a
    // body statement and a group's closing token that name nothing.
    // A sound row of each shape passes first (the legal neighbour).
    // Checks: 5.
    let sound = checked(|a| {
        let l = a.push_list(&[NONE, 0]);
        a.push(Tag::Seq, 0, l, NONE)
    });
    assert_eq!(sound.1, Ok(()), "a list may hold NONE and nodes");
    let item = checked(|a| {
        let l = a.push_list(&[FAR]);
        a.push(Tag::Seq, 0, l, NONE)
    });
    let header = checked(|a| {
        let h = a.push_header(NONE, NONE, NONE, &[], &[]);
        a.set_extra(h + 3, 1);
        a.push(Tag::Instance, 0, h, NONE)
    });
    let body = checked(|a| {
        let fields = a.push_list(&[]);
        let b = a.push_body(NONE, NONE, &[NONE]);
        a.push(Tag::Param, 0, fields, b)
    });
    let group = checked(|a| {
        let g = a.push_extra(&[FAR, 0]);
        a.push(Tag::Vector, 0, NONE, g)
    });
    for (what, (n, got)) in [
        ("item", item),
        ("header", header),
        ("body", body),
        ("group", group),
    ] {
        assert_eq!(got.map_err(|e| e.node), Err(n), "{what}");
    }
}
