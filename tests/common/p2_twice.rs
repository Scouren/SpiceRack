//! Law P2's second self-test (R69 d), apart from `tests/pass_laws.rs` for its 500-line cap: a
//! pass that notes an item it leaves twice fails P2, which asks for exactly one note.
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::passes::Pass;

use super::{LEAKY, p2_on};

// ---- private ----

/// P2's second self-test (R69 d): a pass that notes every comment it leaves twice fails P2.
#[test]
fn p2_catches_a_pass_that_notes_an_item_twice() {
    let twice = Pass {
        name: "twice",
        removes: ir::Feature::Item(ir::ItemKind::Comment),
        requires: ir::FeatureSet::EMPTY,
        notes: &[LEAKY],
        run: note_every_comment_twice,
    };
    let found = p2_on(&twice).expect_err("P2 sees the repeated note");
    assert!(found.contains("keeps it with 2 notes"), "{found}");
}

/// The repeating self-test pass: two notes at every comment, and no change.
fn note_every_comment_twice(design: &mut ir::Design, diags: &mut ir::Diags) {
    for (item, _) in design.reachable() {
        if item.kind == ir::ItemKind::Comment {
            diags.push_at(LEAKY, design, item, 0);
            diags.push_at(LEAKY, design, item, 0);
        }
    }
}
