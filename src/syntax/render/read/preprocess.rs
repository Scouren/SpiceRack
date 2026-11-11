//! Xyce's `.PREPROCESS` card (RG 7.10 §2.1.28–30) → what the reader
//! applies of it as the deck is read: the letters a `REMOVEUNUSED` card
//! lists (`Rd::removeunused`, a prepass over the top's statements), the
//! elements it removes (`Rd::unused`), and the cards placed with nothing
//! left behind (`Rd::preprocess`): `REMOVEUNUSED`, `ADDRESISTORS` (a
//! request for a second netlist file, W0023) and `REPLACEGROUND FALSE`
//! (the default). `REPLACEGROUND TRUE` renames ground nodes, which the
//! reader does not do: it, and a `REMOVEUNUSED` beside it, stay the card's
//! kept text (W0039).
//!
//! Cites: Xyce RG 7.10 §2.1.28 (`REPLACEGROUND`), §2.1.29
//! (`REMOVEUNUSED`), §2.1.30 (`ADDRESISTORS`); ZERO-LEDGER P6-014.
//!
//! Contents:
//! - `Rd::removeunused(&self, stmts: &[u32]) -> String`, `Rd::unused(&self, n: u32) -> bool`,
//!   `Rd::preprocess(&self, n: u32, acc: &mut Acc) -> bool`

use super::{Acc, Rd};
use crate::syntax::behaviour::DotRule;

impl Rd<'_> {
    /// The element letters, uppercased, the Xyce `.PREPROCESS REMOVEUNUSED`
    /// card among `stmts` (the top's statements) lists (RG 7.10 §2.1.29:
    /// C, D, I, L, M, Q, R, V). Empty, the card then staying kept text:
    /// with no such card, with a list Xyce stops on (none, or another
    /// word), and beside a `REPLACEGROUND TRUE`, whose ground names the
    /// reader does not rename.
    pub(super) fn removeunused(&self, stmts: &[u32]) -> String {
        if self.dot_rule("preprocess") != Some(DotRule::Preprocess) {
            return String::new();
        }
        let mut cards = Vec::new();
        for &n in stmts {
            let head = self.t.text(self.main(n));
            if head.eq_ignore_ascii_case(".preprocess") {
                cards.push(self.words_after(n));
            }
        }
        let renames = cards.iter().any(|w| *w == ["replaceground", "true"]);
        let list = cards
            .iter()
            .find(|w| w.first().is_some_and(|x| x == "removeunused"));
        let letters = list.map_or(<&[String]>::default(), |w| &w[1..]);
        let known = |l: &String| l.len() == 1 && "cdilmqrv".contains(l.as_str());
        let out = match renames || !letters.iter().all(known) {
            true => String::new(),
            false => letters.concat().to_ascii_uppercase(),
        };
        assert!(out.bytes().all(|b| b"CDILMQRV".contains(&b)), "{out}");
        out
    }

    /// Whether Xyce's `.PREPROCESS REMOVEUNUSED` removes element `n` (RG
    /// 7.10 §2.1.29): its letter is listed (`Rd::removed`) and its
    /// terminals, a MOSFET's and a BJT's first three and any other's two,
    /// are one node (Xyce compares names without case: 7.10 run).
    pub(super) fn unused(&self, n: u32) -> bool {
        let letter = self.letter(n);
        if !self.removed.contains(letter) {
            return false;
        }
        let items = self.items(n);
        let terminals = if matches!(letter, 'M' | 'Q') { 3 } else { 2 };
        let node = |i: usize| self.node_text(&items, i);
        (1..terminals).all(|i| match (node(0), node(i)) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(&b),
            _ => false,
        })
    }

    /// Xyce's `.PREPROCESS` card `n` (RG 7.10 §2.1.28–30), `true` when the
    /// reader applies it: `REMOVEUNUSED` when its devices are left out
    /// ([`Rd::removeunused`]), `ADDRESISTORS` (the run adds no resistor:
    /// the card asks for a second netlist file, W0023), `REPLACEGROUND
    /// FALSE` (the default). Any other form stays the card's kept text.
    pub(super) fn preprocess(&self, n: u32, acc: &mut Acc) -> bool {
        let words = self.words_after(n);
        let words: Vec<&str> = words.iter().map(String::as_str).collect();
        match words[..] {
            ["removeunused", ..] => !self.removed.is_empty(),
            ["addresistors", "oneterminal" | "nodcpath", _] => {
                let note = crate::syntax::diag::Code::W0023;
                acc.notes
                    .push(crate::syntax::diag::Diag::new(note, self.main(n), 0));
                true
            }
            ["replaceground", "false"] => true,
            _ => false,
        }
    }
}

// ---- private ----

impl Rd<'_> {
    /// The lowercased words of flat statement `n` after its first, split
    /// at blanks and commas (`.PREPROCESS REMOVEUNUSED R,C`: `removeunused`,
    /// `r`, `c`).
    fn words_after(&self, n: u32) -> Vec<String> {
        let line = self.line(n).to_ascii_lowercase();
        let words = line.split(|c: char| c == ',' || c.is_whitespace());
        words
            .filter(|w| !w.is_empty())
            .skip(1)
            .map(String::from)
            .collect()
    }
}
