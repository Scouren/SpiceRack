//! Law P2 on one pass (`tests/pass_laws.rs`, apart for its 500-line cap): every item still
//! carrying the pass's feature has exactly one of its notes (R69 d); and P2's second self-test,
//! a pass that notes an item twice.
//!
//! Contents: `pub fn p2_on(&Pass) -> Fails`

use spicerack::ir;
use spicerack::passes::Pass;

use super::{Fails, LEAKY, carrying, each_with};

/// P2 on one pass: every item still carrying `removes` after it has exactly one of its notes at
/// that item (`Diag::item`).
pub fn p2_on(pass: &Pass) -> Fails {
    each_with(pass, |seed, mut design| {
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        for item in carrying(&design, pass.removes) {
            // exactly one (R69 d): a note repeated at one item is the user reading it twice
            let notes = diags
                .rows()
                .iter()
                .filter(|d| d.item == item && pass.notes.contains(&d.code))
                .count();
            match notes {
                1 => {}
                0 => {
                    return Err(format!(
                        "seed {seed}: the item {item:?} keeps it with no note"
                    ));
                }
                n => {
                    return Err(format!(
                        "seed {seed}: the item {item:?} keeps it with {n} notes"
                    ));
                }
            }
        }
        Ok(())
    })
}

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
    let found = p2_on(&twice).evidence();
    assert!(
        found.contains("keeps it with 2 notes"),
        "P2 sees the repeated note: {found}"
    );
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
