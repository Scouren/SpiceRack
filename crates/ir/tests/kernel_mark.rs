//! The builder's undo (`Builder::mark`, `Builder::truncate`; the dialects' audit, row 5): what
//! is pushed after a mark and truncated leaves the design as it stood, its next ids the same.
//!
//! Oracles: a design read with a detour (a mark, a whole other generated text pushed, the
//! truncate) against the same text read straight (`eq_ignoring_spans`, the spans, and the text
//! `dump` writes); names and definitions dropped are not found.
//!
//! Contents: none (tests only).

#![cfg(feature = "gen")]

use spicerack_ir::r#gen::{Size, generate};
use spicerack_ir::{DefKind, Design, Diags, FeatureSet, Role, Span, dump, parse_unchecked};

const SIZE: Size = Size {
    items: 30,
    scope_depth: 3,
    expr_depth: 3,
    files: 2,
};

// ---- private ----

/// The IR text of the design generated from `seed`.
fn text_of(seed: u64) -> String {
    let d = generate(seed, &FeatureSet::ALL, SIZE);
    let mut out = String::new();
    for f in d.files() {
        dump(&d, f, &mut out);
    }
    out
}

/// Every file of `d` as IR text.
fn written(d: &Design) -> String {
    let mut out = String::new();
    for f in d.files() {
        dump(d, f, &mut out);
    }
    out
}

/// Whether `a` and `b` hold the same rows, spans and text.
fn same(a: &Design, b: &Design) -> bool {
    a.eq_ignoring_spans(b) && written(a) == written(b)
}

#[test]
fn a_detour_truncated_leaves_the_design_as_it_stood() {
    for seed in 0..100u64 {
        let (first, detour) = (text_of(seed), text_of(seed + 1000));
        // straight: the first text alone
        let mut straight = Design::new();
        parse_unchecked(&first, &mut straight.builder(), &mut Diags::new());
        // at the start: a detour taken back, then the first text, gives the same ids
        let mut d = Design::new();
        {
            let mut b = d.builder();
            let mark = b.mark();
            parse_unchecked(&detour, &mut b, &mut Diags::new());
            b.truncate(mark);
        }
        assert!(same(&d, &Design::new()), "seed {seed}: back to empty");
        parse_unchecked(&first, &mut d.builder(), &mut Diags::new());
        assert!(
            same(&d, &straight),
            "seed {seed}: the same ids after a detour"
        );
        // in the middle: after the first text, a detour taken back
        let mut b = d.builder();
        let mark = b.mark();
        parse_unchecked(&detour, &mut b, &mut Diags::new());
        b.truncate(mark);
        assert!(same(&d, &straight), "seed {seed}: as it stood at the mark");
    }
}

#[test]
fn a_truncated_name_and_definition_are_not_found() {
    let mut d = Design::new();
    let mut b = d.builder();
    let (_, top) = b.file("t.sp", Role::Deck);
    let one = b.num(1.0);
    b.param(top, "kept", one, Span::NONE);
    let mark = b.mark();
    let two = b.num(2.0);
    b.param(top, "dropped", two, Span::NONE);
    let (_, sub) = b.file("u.sp", Role::Library);
    let three = b.num(3.0);
    b.param(sub, "kept", three, Span::NONE);
    b.truncate(mark);
    assert!(d.find_name("dropped").is_none(), "the name is gone");
    assert_eq!(d.files().count(), 1, "the file is gone");
    let found = |name| d.lookup(top, DefKind::Param, d.find_name(name));
    assert!(!found("kept").is_none(), "what stood at the mark stays");
    assert_eq!(d.items(top).len(), 1, "one item listed");
    // the names table restarts where it stood: a new name takes the dropped one's id
    let dropped_id_again = d.builder().name("again");
    assert_eq!(dropped_id_again.index(), d.find_name("kept").index() + 1);
}
