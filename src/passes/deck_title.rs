//! Pass `deck_title`: a design in, the same design out with an empty `Title` as the first item
//! of every `Deck` file whose top scope holds none. It removes `Extra::DeckWithoutTitle`
//! (ngspice and VACASK read a deck's first line as its title whatever it holds, so "no title"
//! and "an empty title" are one text there) and requires nothing. Nothing else moves: every
//! other item keeps its place, and the only feature the design can gain is `Item(Title)`.
//!
//! Cites: the lead's ruling on decks without a title (VACASK scaffold); `rewrite/SPINE.md` §4.
//!
//! Contents: `pub fn deck_title(design: &mut ir::Design, diags: &mut ir::Diags)`

use crate::ir;

/// Gives each `Deck` file with no `Title` in its top scope an empty one, first. Adds no
/// diagnostic: an empty title is what those decks mean.
pub fn deck_title(design: &mut ir::Design, _diags: &mut ir::Diags) {
    let bare: Vec<ir::ScopeId> = design
        .files()
        .map(|f| design.file(f))
        .filter(|f| f.role == ir::Role::Deck)
        .map(|f| f.top)
        .filter(|&top| {
            design
                .items(top)
                .iter()
                .all(|i| i.kind != ir::ItemKind::Title)
        })
        .collect();
    let mut editor = design.editor();
    for &top in &bare {
        let title = editor.builder().title(top, "", ir::Span::NONE);
        let items = editor.items_mut(top);
        assert!(
            items.last() == Some(&title),
            "the builder appends the title last"
        );
        items.pop();
        items.insert(0, title);
        assert!(
            items[0].kind == ir::ItemKind::Title,
            "the title leads the deck"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` read as IR text, `deck_title` run, and written back.
    fn run(text: &str) -> (String, ir::FeatureSet, ir::FeatureSet) {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        let before = ir::features_used(&design);
        deck_title(&mut design, &mut diags);
        ir::check(&design, &mut diags);
        assert!(
            diags.is_empty(),
            "the pass leaves a checked design: {:?}",
            diags.rows()
        );
        let mut out = String::new();
        for f in design.files() {
            ir::dump(&design, f, &mut out);
        }
        (out, before, ir::features_used(&design))
    }

    #[test]
    fn a_deck_without_a_title_gets_an_empty_one_first_and_nothing_else_moves() {
        let text = "file \"a.sp\" deck\n  param p = 1\n  subckt s (x y)\n    param q = 2\nfile \"b.lib\" library\n  param r = 3\nfile \"c.sp\" deck\n  title \"kept\"\n  param t = 4\nfile \"d.sp\" deck\n";
        let expected = "file \"a.sp\" deck\n  title \"\"\n  param p = 1\n  subckt s (x y)\n    param q = 2\nfile \"b.lib\" library\n  param r = 3\nfile \"c.sp\" deck\n  title \"kept\"\n  param t = 4\nfile \"d.sp\" deck\n  title \"\"\n";
        let (out, before, after) = run(text);
        assert_eq!(out, expected);
        let title = ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Title));
        assert!(
            after.minus(before).minus(title).is_empty(),
            "it gains only Item(Title)"
        );
        // The cue: when the kernel adds `Extra::DeckWithoutTitle`, `features_used` reports it
        // and this goes red. Then assert `before.minus(after)` is exactly that feature, and
        // register the pass in `PASSES` (`removes` it, `requires` nothing).
        assert!(before.minus(after).is_empty());
    }

    #[test]
    fn a_design_whose_decks_all_have_titles_is_untouched() {
        let text = "file \"c.sp\" deck\n  title \"kept\"\n  param t = 4\nfile \"b.lib\" library\n  param r = 3\n";
        assert_eq!(run(text).0, text);
    }
}
