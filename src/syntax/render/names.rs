//! The deck's elements → the name map, both writers' (`spice`, `named`)
//! (names are renamed as a whole): each element's written name (its type letter, then its name
//! as the reader keeps it: `r1` is `R1`; a Spectre `in1 (a 0) vsource` is
//! `Vin1`), built once per deck, and each declaration found from a
//! reference to it in its scope, so a reference is spelled as its
//! declaration is. The letter is added here and nowhere else; the named
//! writer spells it lowercase and escapes the name after it (Spectre `\\`,
//! VACASK `'…'`), as the old writers did.
//!
//! A writer that spells an element under another letter (HSPICE's `E`/`G`
//! for a `B` source, a Laplace lift's `EB1`/`GB1`/`AB1`) renames it here,
//! as the map is built: the new name is made unique in its scope (`_1`,
//! `_2`, … after it when the deck declares it already), and a reference
//! `i(B1)` in an output or an expression is spelled as the declaration
//! ([`Names::refs`]).
//!
//! Where the old writer spelled a reference otherwise (as written, while its
//! declaration gained a letter), the writer keeps the old bytes beside a
//! `// quirk:` until phase 7 (`tests/fixtures/transpiler/PHASE4-STATUS.md`).
//!
//! Cites: ngspice manual §2.1.2 (the element letter), §3.2.12 (`K`),
//! §5.2-§5.3 (the controlling source of `F`, `H`); ngspice and HSPICE fold
//! name case, Spectre does not (Spectre UG p.66).
//!
//! Contents:
//! - `pub(super) struct Names`
//! - `Names::new(rd: &Rd, deck: &Deck, rename: impl Fn(u32, &Card) -> Option<String>) -> Names`,
//!   `Names::decl(&self, n: u32) -> &str`,
//!   `Names::reference(&self, scope: u32, name: &str, letter: char) -> Option<&str>`,
//!   `Names::refs(&self, scope: u32, text: &str) -> String`

use std::collections::{HashMap, HashSet};

use crate::syntax::ast::{NONE, Node};
use crate::syntax::render::read::{Block, Card, Comp, Deck, Rd};

/// The deck's element names, written.
pub(super) struct Names {
    /// Each element's written name, by its node.
    decl: HashMap<u32, String>,
    /// Each element's written name, by its scope (the subcircuit's node,
    /// `NONE` at the top) and its name as written (case folded in a SPICE
    /// deck).
    by_name: HashMap<(u32, String), String>,
    /// The elements a writer renamed, by scope and name as written: the
    /// names a reference `i(name)` is respelled to.
    renamed: HashMap<(u32, String), String>,
    /// Names fold case (a SPICE deck).
    fold: bool,
}

/// The suffixes a renamed element tries before giving up: more names
/// in one scope than any deck declares.
const SUFFIX_CAP: u32 = 1 << 20;

impl Names {
    /// The map of every element `deck` writes, in every block; `rename`
    /// gives an element (its node and card) the name the writer spells it
    /// under instead of its letter and name, made unique in its scope.
    pub(super) fn new(
        rd: &Rd,
        deck: &Deck,
        rename: impl Fn(u32, &Card) -> Option<String>,
    ) -> Names {
        let mut names = Names {
            decl: HashMap::new(),
            by_name: HashMap::new(),
            renamed: HashMap::new(),
            fold: !rd.named(),
        };
        // The renames wait for every declared name, which they avoid.
        let mut later: Vec<(u32, u32, String, String)> = Vec::new();
        let tops = std::iter::once(&deck.top).chain(deck.sections.iter().map(|s| &s.body));
        let defs = deck
            .defs
            .iter()
            .chain(deck.sections.iter().flat_map(|s| &s.defs));
        let mut work: Vec<(&Block, u32)> = tops.map(|b| (b, NONE)).collect();
        work.extend(defs.map(|b| (b, b.node)));
        while let Some((b, scope)) = work.pop() {
            let cards = b.comps.iter().filter_map(|c| match c {
                Comp::Card(n) => Some(*n),
                Comp::Raw(_) => None,
            });
            for n in cards.chain(b.insts.iter().map(|&(_, n)| n)) {
                let card = rd.card_view(n);
                let source = rd.name_of(rd.ast.header(Node(n)).name);
                if let Some(r) = rename(n, &card) {
                    later.push((scope, n, source.clone(), r));
                }
                let written = written(card);
                names
                    .by_name
                    .entry((scope, names.key(&source)))
                    .or_insert_with(|| written.clone());
                names.decl.insert(n, written);
            }
            let branches = b.conds.iter().flat_map(|(_, c)| &c.branches);
            work.extend(branches.map(|(_, body)| (body, scope)));
        }
        let mut taken: HashSet<(u32, String)> = names
            .by_name
            .iter()
            .map(|((scope, _), w)| (*scope, names.key(w)))
            .collect();
        for (scope, n, source, r) in later {
            let free = |w: &String| !taken.contains(&(scope, names.key(w)));
            let w = std::iter::once(r.clone())
                .chain((1..SUFFIX_CAP).map(|i| format!("{r}_{i}")))
                .find(free)
                .expect("a free name under the cap");
            taken.insert((scope, names.key(&w)));
            names.by_name.insert((scope, names.key(&source)), w.clone());
            names.renamed.insert((scope, names.key(&source)), w.clone());
            names.decl.insert(n, w);
        }
        names
    }

    /// Element `n`'s written name.
    ///
    /// # Panics
    /// When `n` is no element the deck writes.
    pub(super) fn decl(&self, n: u32) -> &str {
        self.decl.get(&n).expect("an element the map holds")
    }

    /// The written name of the element `name` (as a reference wrote it)
    /// declares in `scope`, when one does with type letter `letter`.
    pub(super) fn reference(&self, scope: u32, name: &str, letter: char) -> Option<&str> {
        let w = self.by_name.get(&(scope, self.key(name)))?;
        w.starts_with(letter).then_some(w.as_str())
    }

    /// `text` with each current reference `i(name)` (either case, the
    /// name as written) to an element renamed in `scope` spelled as its
    /// declaration; `text` itself when nothing in the deck was renamed.
    pub(super) fn refs(&self, scope: u32, text: &str) -> String {
        if self.renamed.is_empty() {
            return text.to_string();
        }
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let b = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        let mut i = 0;
        while i + 1 < b.len() {
            let call = matches!(b[i], b'i' | b'I') && b[i + 1] == b'(';
            if call && (i == 0 || !word(b[i - 1])) {
                let start = i + 2;
                let len = b[start..].iter().take_while(|&&c| word(c)).count();
                let close = b.get(start + len) == Some(&b')');
                if let (true, Some(w)) = (
                    close,
                    self.renamed
                        .get(&(scope, self.key(&text[start..start + len]))),
                ) {
                    out.push_str(&text[at..start]);
                    out.push_str(w);
                    at = start + len;
                    i = at;
                    continue;
                }
            }
            i += 1;
        }
        out.push_str(&text[at..]);
        out
    }
}

// ---- private ----

impl Names {
    fn key(&self, name: &str) -> String {
        match self.fold {
            true => name.to_ascii_lowercase(),
            false => name.to_string(),
        }
    }
}

/// A card's written name: its type letter, then its name.
fn written(c: Card) -> String {
    let (letter, name) = match c {
        Card::Rcl { letter, name, .. }
        | Card::Source { letter, name, .. }
        | Card::Linear { letter, name, .. }
        | Card::Poly { letter, name, .. }
        | Card::Semi { letter, name, .. }
        | Card::Switch { letter, name, .. } => (letter, name),
        Card::Charge { name, .. } => ('C', name),
        Card::Coupling { name, .. } => ('K', name),
        Card::Behavioural { name, .. } => ('B', name),
        Card::Line { name, .. } => ('T', name),
        Card::Inst { name, .. } => ('X', name),
    };
    assert!(letter.is_ascii_uppercase(), "a type letter");
    format!("{letter}{name}")
}
