//! One statement of a SPICE-family file → its place in the plan: the
//! statement dispatch ([`Rd::card`]) the block walk (`blocks`) calls once a
//! card, a section's body read as at the top ([`Rd::section`]), the cards
//! kept as text (`raw`, `unparsed`, `keep`), and the deck-wide `.model`
//! names (`model_names`) the elements' masters are looked up in.
//!
//! Cites: the old lowering's `Cx::card`, `raw`, `section`, `leaks`; its
//! SPICE grammar's `ifblock` and `Branch`.
//!
//! Contents:
//! - `Rd::card(&mut self, n: u32, here: &mut Block, scope: Scope, acc: &mut Acc) -> Option<Opened>`
//! - `Rd::raw(&mut self, n: u32, here: &mut Block, block: bool)`, `Rd::unparsed(&mut self, n: u32, here: &mut Block)`,
//!   `Rd::keep(&mut self, n: u32, here: &mut Block, k: Kept)`
//! - `Rd::section(&mut self, n: u32, library: Option<String>, acc: &mut Acc) -> Section`
//! - `Rd::close_of(&self, open: u32, opener: T, closer: T) -> Option<u32>`
//! - `Rd::model_names(&self, stmts: &[u32]) -> HashMap<String, u32>`

use std::collections::HashMap;

use super::blocks::Opened;
use super::{Acc, Bench, Block, Comp, Kept, Raw, Rd, Scope, Section};
use crate::syntax::ast::{Node, Tag};
use crate::syntax::behaviour::NamedRules;
use crate::syntax::diag::Diag;
use crate::syntax::token::Tag as T;

impl<'a> Rd<'a> {
    /// One card, as the adapter's `Cx::card`; a block with a body of cards
    /// is handed back for [`Rd::cards`] to walk.
    pub(super) fn card(
        &mut self,
        n: u32,
        here: &mut Block,
        scope: Scope,
        acc: &mut Acc,
    ) -> Option<Opened> {
        if self.named() {
            return self.named_card(n, here, scope, acc);
        }
        let inside = scope == Scope::Subckt;
        if scope == Scope::Section && self.leaks(n) {
            // A `.control` kept raw is one line holding its newlines
            // (quirk: the old lowering's `raw` splits only `Block`-like rows).
            match self.tag(n) {
                Tag::Control => here.raw.push(Raw::Joined(n)),
                _ => self.raw(n, here, false),
            }
            return None;
        }
        // A line its own simulator rejects has no meaning: left out, noted.
        if let Some(code) = self.rejected(n) {
            acc.notes.push(Diag::new(code, self.main(n), 0));
            return None;
        }
        match self.tag(n) {
            Tag::Title | Tag::End => {}
            Tag::Verbatim | Tag::Line => self.unparsed(n, here),
            // A region of another dialect's text (`Lang::NGSPICE_GIVEN`):
            // kept as that dialect's text, which only it writes; but a
            // Spectre analysis at the top is an analysis (`Ana`), which
            // each target spells or refuses (VACASK's `mc` for a
            // `montecarlo`).
            Tag::Region => {
                for &k in self.body(n) {
                    let ana = scope == Scope::Top
                        && self.text_behaviours(self.main(k)).named_rules == NamedRules::Spectre
                        && self.tag(k) == Tag::Analysis
                        && self.spectre_analysis(k).is_some();
                    match ana {
                        true => acc.bench.push(Bench::Analysis(k)),
                        false => here.raw.push(Raw::Lines(k)),
                    }
                }
            }
            Tag::Subckt => match self.subckt_head(n) {
                Some(_) => return Some(Opened::Subckt(n)),
                // quirk: a `.subckt` head the old SPICE grammar does not read is
                // `Unparsed`; its body then reads as cards here, and its
                // `.ends` is a `Raw` card.
                None => {
                    self.unparsed(n, here);
                    return Some(Opened::Dissolve(n));
                }
            },
            Tag::Section if scope != Scope::Top => self.raw(n, here, true),
            Tag::Section => {
                let s = self.section(n, None, acc);
                acc.sections.push(s);
            }
            Tag::If => match self.if_head_ok(n) {
                true => return Some(Opened::If(n)),
                // quirk: the old SPICE grammar's `ifblock`: an `.if` with no
                // condition in parentheses is a `Block`, kept whole.
                false => self.raw(n, here, true),
            },
            Tag::Data => self.raw(n, here, true),
            Tag::Control => self.control(n, scope, acc),
            Tag::Instance => self.element(n, here),
            _ => self.dot_card(n, here, scope, inside, acc),
        }
        None
    }

    /// A card kept as text (the old lowering's `raw`): a block one raw line
    /// per line, a SPICE element in its place among the elements.
    pub(super) fn raw(&mut self, n: u32, here: &mut Block, block: bool) {
        if block {
            here.raw.push(Raw::Lines(n));
            return;
        }
        let letter = self
            .t
            .text(self.main(n))
            .starts_with(|c: char| c.is_ascii_alphabetic());
        if !self.named() && letter {
            here.comps.push(Comp::Raw(n));
        } else {
            here.raw.push(Raw::Line(n));
        }
    }

    /// A card the grammar refused: a raw line.
    pub(super) fn unparsed(&mut self, n: u32, here: &mut Block) {
        here.raw.push(Raw::Line(n));
    }

    /// A card the old reader could not lower, kept as `k` says.
    pub(super) fn keep(&mut self, n: u32, here: &mut Block, k: Kept) {
        match k {
            Kept::Raw => self.raw(n, here, false),
            Kept::Unparsed => self.unparsed(n, here),
        }
    }

    /// `.lib name` … `.endl`: its cards read as at the top, into the
    /// section's own body and definitions (the old lowering's `section`).
    pub(super) fn section(&mut self, n: u32, library: Option<String>, acc: &mut Acc) -> Section {
        assert_eq!(self.tag(n), Tag::Section, "a section");
        let name_item = self.fields(n)[0];
        let mut body = Block {
            name: self.name_of(name_item),
            ..Block::default()
        };
        let (first, first_f) = (acc.defs.len(), acc.funcs.len());
        let stmts = self.body(n).to_vec();
        self.cards(&stmts, &mut body, Scope::Section, acc);
        assert!(acc.defs.len() >= first, "a section's definitions follow");
        Section {
            node: n,
            library,
            body,
            defs: acc.defs.split_off(first),
            funcs: acc.funcs.split_off(first_f),
        }
    }

    /// The closer matching the opener at token `open` (`T::LParen` and
    /// `T::RParen`, `T::LBrace` and `T::RBrace`), on its statement.
    pub(super) fn close_of(&self, open: u32, opener: T, closer: T) -> Option<u32> {
        assert_eq!(self.ttag(open), opener, "an opener at `open`");
        let last = self.line_last(open);
        let mut depth = 0i32;
        // Bounded by the statement: one pass per token.
        for k in open..=last {
            match self.ttag(k) {
                t if t == opener => depth += 1,
                t if t == closer => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(k);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The `.model` names the grammar read, lowercased, anywhere in the
    /// deck but kept-whole blocks.
    pub(super) fn model_names(&self, stmts: &[u32]) -> HashMap<String, u32> {
        let mut out = HashMap::new();
        let mut work: Vec<u32> = stmts.iter().rev().copied().collect();
        while let Some(s) = work.pop() {
            match self.tag(s) {
                Tag::Model => {
                    if let Some(m) = self.model_head(s) {
                        out.insert(self.text(m.0).to_ascii_lowercase(), s);
                    }
                }
                Tag::Data | Tag::Control => {}
                Tag::If if !self.if_head_ok(s) => {}
                Tag::Subckt | Tag::Section | Tag::If | Tag::Else => {
                    if let Some(nx) = self.ast.next(Node(s)) {
                        work.push(nx.0);
                    }
                    work.extend(self.body(s).iter().rev());
                }
                _ => {}
            }
        }
        out
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// Whether a card in a section would reach past it (the old lowering's
    /// `leaks`).
    fn leaks(&self, n: u32) -> bool {
        match self.tag(n) {
            Tag::Global | Tag::Control => true,
            Tag::Save => self.ttag(self.main(n)) == T::KwSave,
            _ => false,
        }
    }

    /// quirk: the old SPICE grammar's `Branch <- '.if' s? Cond s? nl`: the
    /// condition is one parenthesized group ending the line.
    fn if_head_ok(&self, n: u32) -> bool {
        let first = self.main(n);
        let last = self.line_last(first);
        first < last
            && self.ttag(first + 1) == T::LParen
            && self.close_of(first + 1, T::LParen, T::RParen) == Some(last)
    }
}
