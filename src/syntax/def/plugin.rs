//! The calls a plugin makes that no built-in needs: its own codes and the
//! cells that raise them, the post-pass's maps (a keyword's written head, a
//! master's written word, an option's written name), write hooks, the
//! simulator that reads its text, and `simulator lang=` regions of any
//! registered language (`tests/fixtures/transpiler/DIALECT-PLUGIN.md`
//! §6.2-§6.5). Each is open to every definition; the built-ins call only
//! `regions_registered` (ngspice-given's).
//!
//! Cites: none.
//!
//! Contents:
//! - `pub type WriteHook`, `pub enum Hook`, `pub struct Card<'a>`, `pub enum Answer`,
//!   `pub struct RunsOn`, `From<Dialect> for RunsOn`
//! - `DialectDef::{code, refuse_code, omit_code, write_head, write_master, option, hook,
//!   external_hook, runs_on, regions_registered}`

use super::{DialectDef, put};
use crate::syntax::diag::{Code, Kind};
use crate::syntax::render::capability::{Action, Alt, Construct};
use crate::syntax::token::Tag;
use crate::syntax::{Dialect, Lang};

/// Writes the cards of one hooked head of a deck, as the definition's
/// writer wrote them, in output order: one answer per card, pushed onto
/// `out` (`DIALECT-PLUGIN.md` §6.3).
pub type WriteHook = fn(cards: &[Card<'_>], out: &mut Vec<Answer>);

/// Who answers a hooked head's cards.
#[derive(Clone, Copy, Debug)]
pub enum Hook {
    /// A Rust function, called by the writer once per deck.
    Rust(WriteHook),
    /// A caller outside `syntax` (the Python binding), by its own id: the
    /// writer returns a request per card (`render::render_pending`) and
    /// the caller hands the answers to `render::splice`.
    External(u16),
}

/// One written card a hook answers.
#[derive(Clone, Copy, Debug)]
pub struct Card<'a> {
    /// Its first word, as written (`.noise`).
    pub head: &'a str,
    /// The words after the head that are no `name=value`, in order.
    pub words: &'a [&'a str],
    /// The `name=value` words, in order.
    pub params: &'a [(&'a str, &'a str)],
    /// The whole card, continuations joined by one space.
    pub text: &'a str,
}

/// A hook's answer for one card.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Answer {
    /// The card's text instead of the writer's (one or more lines).
    Text(String),
    /// The deck is refused with the definition's code `id` (`W0201`).
    Refuse(&'static str),
}

/// The simulator a definition's `runs_on` names: the name it is registered
/// under and the language it reads. A built-in is `Dialect::into`; a
/// registered simulator is `backend::Sim::into` (the registry is
/// `backend`'s, which `syntax` does not see). No invariant ties the
/// fields here: `backend` gives the pair its registry holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunsOn {
    /// The name the simulator is registered under (`backend::by_name`).
    pub sim: &'static str,
    /// The language the simulator's decks are written in.
    pub dialect: Lang,
}

impl From<Dialect> for RunsOn {
    /// The built-in simulator of `d`: named as the dialect, reading it.
    fn from(d: Dialect) -> RunsOn {
        RunsOn {
            sim: d.name(),
            dialect: d.into(),
        }
    }
}

impl DialectDef {
    /// Declares code `id` of this definition: a refusal (`W0201`) or a
    /// note (`W0202`) whose message is `message`. A later call with the
    /// same `id` replaces it.
    pub fn code(mut self, id: &str, kind: Kind, message: &str) -> Self {
        match self.codes.iter_mut().find(|c| c.0 == id) {
            Some(c) => (c.1, c.2) = (kind, message.to_string()),
            None => self.codes.push((id.to_string(), kind, message.to_string())),
        }
        self
    }

    /// `c` refuses the deck with this definition's code `id` (`W0201`).
    pub fn refuse_code(mut self, c: Construct, id: &str) -> Self {
        self = self.cell(c, Action::Refuse(Code::W0201));
        self.code_ids[c as usize] = Some(id.to_string());
        self
    }

    /// `c` is left out, noted with this definition's code `id` (`W0202`).
    pub fn omit_code(mut self, c: Construct, id: &str) -> Self {
        self = self.cell(c, Action::Spell(Alt::Omit(Code::W0202)));
        self.code_ids[c as usize] = Some(id.to_string());
        self
    }

    /// A card the writer heads with keyword `t` is headed `text` instead.
    pub fn write_head(mut self, t: Tag, text: &str) -> Self {
        put(&mut self.heads, t, text.to_string());
        self
    }

    /// A named statement's master `word` is written `text` instead.
    pub fn write_master(mut self, word: &str, text: &str) -> Self {
        put(&mut self.write_masters, word.to_string(), text.to_string());
        self
    }

    /// Option `spice` (as the writer writes it) is written `own`.
    pub fn option(mut self, spice: &str, own: &str) -> Self {
        put(&mut self.options, spice.to_string(), own.to_string());
        self
    }

    /// The cards the writer heads `head` are answered by `f`, once a deck.
    pub fn hook(mut self, head: &str, f: WriteHook) -> Self {
        put(&mut self.hooks, head.to_string(), Hook::Rust(f));
        self
    }

    /// The cards headed `head` are answered outside `syntax`, by the
    /// caller's hook `id` (a binding's: `render::render_pending`).
    pub fn external_hook(mut self, head: &str, id: u16) -> Self {
        put(&mut self.hooks, head.to_string(), Hook::External(id));
        self
    }

    /// Simulator `sim` reads this language's text (the author's claim):
    /// a deck in it can be run there. `sim` is a built-in (`Dialect`) or a
    /// registered simulator (`backend::Sim`; by name:
    /// `backend::by_name(name)`), so the simulator is registered before the
    /// language that runs on it. Registration allows it only when the
    /// simulator's language is in the `extends` chain.
    pub fn runs_on(mut self, sim: impl Into<RunsOn>) -> Self {
        self.runs_on = Some(sim.into());
        self
    }

    /// `simulator lang=<name>` also opens a region of any registered
    /// plugin language named `name` (ngspice-given's: the Python decks').
    pub fn regions_registered(mut self, v: bool) -> Self {
        self.lex().regions_registered = v;
        self
    }
}
