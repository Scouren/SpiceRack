//! A swept name (a `.step`'s, a `.dc` axis's) → what it names at the
//! deck's top: the temperature, a parameter, a source's DC value, an R, C
//! or L's value, another device or a device's parameter. The one place
//! the named writers and the LTspice writer ask; each then spells the
//! sweep its target has, or refuses it.
//!
//! Cites: Xyce RG 7.10 `.STEP` (a bare device name is its default
//! parameter: a source's DC value, an R, C or L's value; a parameter of
//! the name comes first: Xyce 7.10, run,
//! `research/impl/evidence/P4-121/summary.txt`); LTspice help `.STEP`
//! (`param <name>` names a parameter); Spectre Reference 19.1 p.406
//! (`sweep param=` with no `dev`: a netlist parameter).
//!
//! Contents:
//! - `pub(super) enum Swept`
//! - `fn swept(rd: &Rd, deck: &Deck, names: &Names, name: &str) -> Swept`
//! - `fn step_swept(rd: &Rd, deck: &Deck, names: &Names, name: &str) -> Swept`

use super::names::Names;
use super::read::{Card, Comp, Deck, Rd};
use crate::syntax::ast::NONE;
use crate::syntax::behaviour::CardRules;

/// What a swept name names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Swept {
    /// `temp`: the circuit temperature.
    Temp,
    /// A parameter the deck's top declares.
    Param,
    /// A top-level V or I card's DC value; `plain`: a card of the top's
    /// own, with no waveform.
    Source { plain: bool },
    /// A top-level R, C or L card's value, the card with no model: its
    /// parameter's name (`r`, `c`, `l`).
    Value(char),
    /// Another device, or a device's or model's parameter (`R1:TC1`).
    Other,
    /// A name the top neither declares nor names an element by (a
    /// parameter or a device of an included file).
    Unseen,
}

/// What `name` names at the top of `deck`: a parameter before a device of
/// its name.
///
/// # Panics
/// On an empty `name`.
// ponytail: the top's own cards only; a V or I card in a branch of a
// conditional is a source of unseen waveform, any other card there `Other`.
pub(super) fn swept(rd: &Rd, deck: &Deck, names: &Names, name: &str) -> Swept {
    assert!(!name.is_empty(), "swept: a sweep names what it sweeps");
    if name.eq_ignore_ascii_case("temp") {
        return Swept::Temp;
    }
    let declares = |&a: &u32| rd.param_view(a).0.eq_ignore_ascii_case(name);
    if deck.top.params.iter().any(declares) {
        return Swept::Param;
    }
    if name.contains(':') {
        return Swept::Other;
    }
    let letter = name.as_bytes()[0].to_ascii_uppercase() as char;
    let Some(decl) = names.reference(NONE, name, letter) else {
        return Swept::Unseen;
    };
    let card = deck.top.comps.iter().find_map(|c| match c {
        Comp::Card(n) if names.decl(*n) == decl => Some(rd.card_view(*n)),
        _ => None,
    });
    match card {
        Some(Card::Source { wave, .. }) => Swept::Source {
            plain: wave.is_none(),
        },
        Some(Card::Rcl {
            letter: l,
            model: None,
            ..
        }) => {
            assert!(matches!(l, 'R' | 'C' | 'L'), "swept: an R, C or L card");
            Swept::Value(l.to_ascii_lowercase())
        }
        _ if matches!(letter, 'V' | 'I') => Swept::Source { plain: false },
        _ => Swept::Other,
    }
}

/// What a `.step` over `name` sweeps: [`swept`], but a parameter whatever
/// the top holds where the source's own step names parameters only
/// (LTspice's `.step param`, Spectre's `sweep param=`, VACASK's sweep of
/// a variable); `temp` is the temperature in each.
///
/// # Panics
/// On an empty `name`.
pub(super) fn step_swept(rd: &Rd, deck: &Deck, names: &Names, name: &str) -> Swept {
    assert!(
        !name.is_empty(),
        "step_swept: a `.step` names what it sweeps"
    );
    let by_syntax = rd.b.card_rules == CardRules::Ltspice || rd.named();
    match swept(rd, deck, names, name) {
        Swept::Temp => Swept::Temp,
        _ if by_syntax => Swept::Param,
        s => s,
    }
}
