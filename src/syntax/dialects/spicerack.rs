//! SpiceRack's own text, declared with the definition API: ngspice-given's
//! statements (`ngspice_given.rs`, the Python decks) with the neutral
//! expression grammar of decision D16, one syntax with one meaning that
//! every target's writer lowers. It narrows what it extends. A number ends
//! at its scale (`f p n u m k meg g t`, case-blind: no `mil`, no atto, no
//! unit). The levels, loosest first: `?:`, `||`, `&&`, `== !=`, `< <= >
//! >=`, `+ -`, `* / %`, the sign, `**`. `**` groups to the right and binds
//! tighter than the sign (`-2**2` is -4, `2**-1` is 0.5); the two
//! comparison levels do not associate; a ternary nests in parentheses or
//! in the else branch; `<>` and `\` are no operators, and `^` is read as
//! `**` only to be noted (`P0023`). A reader only: written, it is ngspice.
//!
//! Cites: none (SpiceRack's own grammar, decision D16: C's levels, with
//! `**` above the sign as Fortran and Python have it).
//!
//! Contents:
//! - `pub(super) fn spicerack() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::def::DialectDef;
use crate::syntax::lexicon::{Caret, Nesting, Tail};
use crate::syntax::token::Tag;

/// ngspice-given's statements with the neutral expression grammar.
pub(super) fn spicerack() -> DialectDef {
    DialectDef::new("spicerack")
        .extends(Lang::NGSPICE_GIVEN)
        // Numbers: a mantissa, an exponent, one scale, nothing after it.
        .no_scale("mil")
        .no_scale("a")
        .tail(Tail::None)
        // Levels 2 to 7; `?:` is level 1 (`Ops::TERNARY`).
        .no_infix(Tag::LessGreater)
        .no_infix(Tag::Backslash)
        .infix(Tag::PipePipe, 2)
        .infix(Tag::AmpAmp, 3)
        .infix(Tag::EqualEqual, 4)
        .infix(Tag::BangEqual, 4)
        .infix(Tag::Less, 5)
        .infix(Tag::LessEqual, 5)
        .infix(Tag::Greater, 5)
        .infix(Tag::GreaterEqual, 5)
        .infix(Tag::Plus, 6)
        .infix(Tag::Minus, 6)
        .infix(Tag::Star, 7)
        .infix(Tag::Slash, 7)
        .infix(Tag::Percent, 7)
        // Level 8 is the sign: its operand takes only level 9.
        .prefix_bp(8)
        .infix(Tag::StarStar, 9)
        .right(Tag::StarStar)
        // `^` reads as `**` and is noted, so the rest is checked.
        .infix(Tag::Caret, 9)
        .right(Tag::Caret)
        .caret(Caret::Refused)
        .nonassoc(Tag::EqualEqual)
        .nonassoc(Tag::BangEqual)
        .nonassoc(Tag::Less)
        .nonassoc(Tag::LessEqual)
        .nonassoc(Tag::Greater)
        .nonassoc(Tag::GreaterEqual)
        .ternary_nesting(Nesting::Explicit)
}
