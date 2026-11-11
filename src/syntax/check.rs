//! [`Ast`] rows → `Ok`, or the first row that breaks its tag's layout
//! ([`Tag::word_kinds`]): the one structural validator every writer of the
//! tree is held to (the parser, and a builder appending rows beside it).
//! Each row is read once: `main_token`, `lhs` and `rhs` name what their
//! kind says (a node, a token, a name, an `extra_data` layout whose own
//! words are held to the same kinds), and every token's stored end is the
//! end its lexer finds again from its start.
//!
//! Cites: `ast.rs`'s header (the layouts: list, fields, header, body);
//! DESIGN-python-ast-builders §3 (the second writer's guards).
//!
//! Contents:
//! - `Ast::check(&self, t: &Tokens) -> Result<(), CheckError>`

use crate::syntax::ast::{Ast, CheckError, NONE, Node, Tag, WordKind};
use crate::syntax::token::Tokens;

impl Ast<'_> {
    /// Holds every row to its tag's layout, and every token of `t` (the
    /// tokens the tree was built over) to its re-lexed end. O(rows + extra
    /// words + source bytes); allocates nothing.
    ///
    /// # Errors
    /// The first broken token, else the first broken row.
    pub fn check(&self, t: &Tokens) -> Result<(), CheckError> {
        let tokens = u32::try_from(t.len()).expect("fewer than u32::MAX tokens");
        if let Some(i) = (0..tokens).find(|&i| t.end(i) != t.end_relexed(i)) {
            let what = "a token's stored end is not its re-lexed end";
            return Err(CheckError {
                node: NONE,
                token: i,
                what,
            });
        }
        for n in 0..self.len() as u32 {
            let node = Node(n);
            let main = self.main_token(node);
            let [l, r] = self.tag(node).word_kinds();
            let row = if (n == 0) != (self.tag(node) == Tag::Root) {
                Err("node 0, and no other, is the Root")
            } else if main >= tokens && (n, main) != (0, 0) {
                Err("main_token is no token")
            } else {
                self.word_ok(l, self.lhs(node), tokens)
                    .and_then(|()| self.word_ok(r, self.rhs(node), tokens))
            };
            row.map_err(|what| CheckError {
                node: n,
                token: NONE,
                what,
            })?;
        }
        Ok(())
    }
}

// ---- private ----

impl Ast<'_> {
    /// Whether word `w` reads as `kind` says, in a tree over `tokens`
    /// tokens.
    fn word_ok(&self, kind: WordKind, w: u32, tokens: u32) -> Result<(), &'static str> {
        let nodes = self.len() as u32;
        let (at, extra) = (w as usize, self.extra_len());
        let ok = |good: bool, what: &'static str| if good { Ok(()) } else { Err(what) };
        let opt = WordKind::NodeOpt;
        match kind {
            WordKind::Unused => ok(w == NONE, "an unused word is not NONE"),
            WordKind::Value => Ok(()),
            WordKind::Node => ok(w < nodes, "a word names no node"),
            WordKind::NodeOpt => ok(w == NONE || w < nodes, "a word names no node"),
            WordKind::Token => ok(w < tokens, "a word names no token"),
            WordKind::TokenOpt => ok(w == NONE || w < tokens, "a word names no token"),
            WordKind::Name => ok(at < self.names_len(), "a word names no interned name"),
            WordKind::List if w == NONE => Ok(()),
            WordKind::List => self.items_ok(at, true),
            WordKind::Header => {
                ok(at.saturating_add(4) <= extra, "a header is cut short")?;
                self.word_ok(opt, self.extra(w), tokens)?;
                self.word_ok(opt, self.extra(w + 1), tokens)?;
                self.items_ok(at + 3, false)?;
                self.items_ok(at + 4 + self.extra(w + 3) as usize, true)
            }
            WordKind::Body if w == NONE => Ok(()),
            WordKind::Body => {
                ok(at.saturating_add(3) <= extra, "a body is cut short")?;
                self.word_ok(opt, self.extra(w), tokens)?;
                self.word_ok(opt, self.extra(w + 1), tokens)?;
                self.items_ok(at + 2, false)
            }
            WordKind::Group => {
                ok(at.saturating_add(2) <= extra, "a group is cut short")?;
                self.word_ok(WordKind::Token, self.extra(w), tokens)?;
                self.items_ok(at + 1, false)
            }
            WordKind::Pair => {
                ok(at.saturating_add(2) <= extra, "a pair is cut short")?;
                self.word_ok(WordKind::Node, self.extra(w), tokens)?;
                self.word_ok(WordKind::Node, self.extra(w + 1), tokens)
            }
            WordKind::IndexTail => {
                ok(at.saturating_add(2) <= extra, "an index tail is cut short")?;
                self.word_ok(WordKind::Node, self.extra(w), tokens)?;
                self.word_ok(WordKind::Token, self.extra(w + 1), tokens)
            }
        }
    }

    /// Whether `extra_data[at..]` is a list `[n, item0 .. item(n-1)]` of
    /// node ids, `NONE` among them only when `holes`.
    fn items_ok(&self, at: usize, holes: bool) -> Result<(), &'static str> {
        let (nodes, extra) = (self.len() as u32, self.extra_len());
        if at >= extra {
            return Err("a list's length word is past the extra words");
        }
        let n = self.extra(at as u32) as usize;
        if n > extra - at - 1 {
            return Err("a list runs past the extra words");
        }
        let item = |k: usize| self.extra(k as u32);
        let good = (at + 1..at + 1 + n).all(|k| item(k) < nodes || (holes && item(k) == NONE));
        if good {
            Ok(())
        } else {
            Err("a list item names no node")
        }
    }
}
