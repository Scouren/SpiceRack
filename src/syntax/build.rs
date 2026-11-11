//! Calls → the triple a parse gives: a deck's text, its token rows and its
//! [`Ast`], appended together by [`Build`], so that a deck made by calls is
//! what the reader makes of the same text and every stage after the parser
//! takes it unchanged. The text is the deck's own spelling (what a stored
//! deck holds); the rows are the reader's rows of it, moved in beside it:
//! [`Build::text`] reads a run of statements with the one tokenizer and
//! parser and relocates their rows, each word by its kind
//! ([`Tag::word_kinds`]).
//!
//! The guard is the differential ([`unlike_parse`]): a built deck is, row
//! for row, the parse of its own text.
//!
//! Cites: DESIGN-python-ast-builders §2 (text, token rows and tree rows
//! appended together, the text whole before the tokens borrow it), §3 (the
//! second writer's guards: `Ast::check`, the differential), §7 (relocation
//! by the tag table); Zig 0.17 `lib/std/zig/Ast.zig` (rows, `extra_data`).
//!
//! Contents:
//! - `pub(crate) struct Build`, `Build::new(title: &str, lex: &'static Lexicon) -> Build`,
//!   `Build::text(&mut self, lines: &str)`,
//!   `Build::finish(self) -> (String, TokRows, Ast<'static>)`
//! - `pub(crate) fn unlike_parse(t: &Tokens, a: &Ast) -> Option<String>`

use std::borrow::Cow;

use crate::syntax::ast::{Ast, NONE, Name, Node, Tag, WordKind};
use crate::syntax::lexicon::Lexicon;
use crate::syntax::token::{Tag as T, TokRows, Tokens};
use crate::syntax::{parse, tokenize};

/// A deck under construction: its text so far and the rows over it.
///
/// Invariants: `src` ends with a newline; every token row is inside `src`;
/// every id in a tree row names a row already appended; `top` lists the
/// top statements in text order.
pub(crate) struct Build {
    src: String,
    tok: TokRows,
    ast: Ast<'static>,
    /// The deck's lexicon: what reads every statement outside a region.
    lex: &'static Lexicon,
    /// The top statements so far: the root's list at `finish`.
    top: Vec<u32>,
}

/// Where a parse's rows land in the tree they move into: a node, a token
/// and an `extra_data` offset each move by their own base, a name through
/// the map.
struct Shift<'n> {
    /// The id the first moved node gets.
    node: u32,
    /// The index the first moved token gets.
    token: u32,
    /// The offset the parse's `extra_data[0]` lands at.
    extra: u32,
    /// The parse's name ids → the tree's.
    names: &'n [u32],
}

/// What a parse of a chunk holds before the chunk's own rows, in nodes and
/// in tokens alike: the root and the (empty) title's node; the title's
/// token and its end of statement.
const LEAD: u32 = 2;

impl Build {
    /// A deck whose line 1 is `title`, with nothing after it yet.
    ///
    /// # Panics
    /// When `title` holds a newline, or `lex` reads no title line.
    pub(crate) fn new(title: &str, lex: &'static Lexicon) -> Build {
        assert!(lex.title, "a built deck's line 1 is its title");
        assert!(!title.contains('\n'), "a title is one line");
        let end = u32::try_from(title.len()).expect("a title under 4 GiB");
        let mut b = Build {
            src: format!("{title}\n"),
            tok: TokRows::default(),
            ast: Ast::new(0),
            lex,
            top: Vec::new(),
        };
        // As `tokenize` and `parse` read line 1: a `Title` token, its end
        // of statement, a `Title` node.
        b.tok.tags.extend([T::Title, T::EndOfStatement]);
        b.tok.starts.extend([0, end]);
        b.tok.ends.extend([end, end]);
        b.top.push(b.ast.push(Tag::Title, 0, NONE, NONE).0);
        debug_assert_eq!(b.ast.len(), LEAD as usize, "the root and the title");
        debug_assert_eq!(b.tok.tags.len(), LEAD as usize, "the title's two tokens");
        b
    }

    /// Appends `lines`: whole statements, each starting its own line, read
    /// as the deck's lexicon reads them where they stand. A block or a
    /// `simulator lang=` region the lines open, they also close. Allocates
    /// the chunk's tokens and tree, then moves their rows in.
    ///
    /// # Panics
    /// When the deck passes 4 GiB (a token offset is a `u32`).
    pub(crate) fn text(&mut self, lines: &str) {
        if lines.is_empty() {
            return;
        }
        let from = self.src.len() - 1;
        debug_assert_eq!(self.src.as_bytes()[from], b'\n', "a chunk starts a line");
        self.src.push_str(lines);
        if !lines.ends_with('\n') {
            self.src.push('\n');
        }
        let byte = u32::try_from(from).expect("a deck under 4 GiB");
        assert!(u32::try_from(self.src.len()).is_ok(), "a deck under 4 GiB");
        // Read from the newline before the chunk: line 1 of that text is
        // empty, and is the title the lexicon takes whatever it holds, so
        // the chunk's lines are statements, as they are in the whole text.
        let t = tokenize(&self.src[from..], self.lex);
        let a = parse(&t, self.lex);
        debug_assert_eq!(t.tags[..2], [T::Title, T::EndOfStatement]);
        debug_assert_eq!(a.tag(Node(1)), Tag::Title, "node 1 is the empty title");
        let names: Vec<u32> = (0..a.names_len() as u32)
            .map(|k| self.ast.intern(Cow::Owned(a.name(Name(k)).to_string())).0)
            .collect();
        let to = Shift {
            node: self.ast.len() as u32,
            token: self.tok.tags.len() as u32,
            extra: self.ast.extra_len() as u32,
            names: &names,
        };
        let lead = LEAD as usize;
        self.tok.tags.extend_from_slice(&t.tags[lead..]);
        self.tok
            .starts
            .extend(t.starts[lead..].iter().map(|s| s + byte));
        self.tok
            .ends
            .extend(t.ends[lead..].iter().map(|e| e + byte));
        let numbers = t.number_tokens.iter().map(|&n| to.token(n));
        self.tok.number_tokens.extend(numbers);
        self.tok.number_values.extend_from_slice(&t.number_values);
        self.tok
            .switches
            .extend(t.switches.iter().map(|&s| to.token(s)));
        self.tok
            .switch_lexicons
            .extend_from_slice(&t.switch_lexicons);
        self.top.extend(to.rows(&a, &mut self.ast));
    }

    /// The deck's text, its token rows and its tree; the tokens the tree is
    /// over are `Tokens::from_rows(&text, lexicon, rows)`, `lexicon` the one
    /// given to [`Build::new`].
    pub(crate) fn finish(mut self) -> (String, TokRows, Ast<'static>) {
        debug_assert!(self.src.ends_with('\n'), "every statement ended its line");
        let list = self.ast.push_list(&self.top);
        self.ast.set_root(list);
        debug_assert_eq!(self.ast.fields(self.ast.root()), &self.top[..]);
        (self.src, self.tok, self.ast)
    }
}

/// Where the built rows part from the reader's own of the same text (`a`
/// over `t`, both from a [`Build`]): the first column and row that differ,
/// or `None` when every token row, node row, extra word, name and
/// diagnostic is the same. It reads the text again, so it is a debug check
/// and the tests' oracle, never a production path.
pub(crate) fn unlike_parse(t: &Tokens, a: &Ast) -> Option<String> {
    let rt = tokenize(t.source(), t.lexicon);
    let ra = parse(&rt, t.lexicon);
    unlike("token tag", &t.tags, &rt.tags)
        .or_else(|| unlike("token start", &t.starts, &rt.starts))
        .or_else(|| unlike("token end", &t.ends, &rt.ends))
        .or_else(|| unlike("number token", &t.number_tokens, &rt.number_tokens))
        .or_else(|| unlike("number value (bits)", &bits(t), &bits(&rt)))
        .or_else(|| unlike("switch token", &t.switches, &rt.switches))
        .or_else(|| unlike("switch lexicon", &langs(t), &langs(&rt)))
        .or_else(|| unlike("node", &nodes(a), &nodes(&ra)))
        .or_else(|| unlike("extra word", &extra(a), &extra(&ra)))
        .or_else(|| unlike("name", &names(a), &names(&ra)))
        .or_else(|| unlike("diagnostic", a.diags(), ra.diags()))
}

// ---- private ----

impl Shift<'_> {
    /// Node id `w` of the parse as the tree names it; `NONE` stays.
    fn node(&self, w: u32) -> u32 {
        debug_assert!(w == NONE || w >= LEAD, "no row names the root or the title");
        match w {
            NONE => NONE,
            w => w - LEAD + self.node,
        }
    }

    /// Token index `w` of the parse as the deck numbers it; `NONE` stays.
    fn token(&self, w: u32) -> u32 {
        debug_assert!(w == NONE || w >= LEAD, "no row names the title's tokens");
        match w {
            NONE => NONE,
            w => w - LEAD + self.token,
        }
    }

    /// Appends every row of `a` past its lead to `into`, each word moved
    /// by its kind, and `a`'s diagnostics; returns `a`'s top statements as
    /// `into` names them.
    fn rows(&self, a: &Ast, into: &mut Ast<'static>) -> Vec<u32> {
        // The root's list is a parse's last push: the words before it are
        // the statements'. They are copied as they stand, then each row
        // moves the words its own layout holds.
        let root = a.lhs(a.root());
        let words: Vec<u32> = (0..root).map(|k| a.extra(k)).collect();
        let at = into.push_extra(&words);
        debug_assert_eq!(at, self.extra, "the words land at the shift's base");
        for n in (LEAD..a.len() as u32).map(Node) {
            let [l, r] = a.tag(n).word_kinds();
            let lhs = self.word(a, into, l, a.lhs(n));
            let rhs = self.word(a, into, r, a.rhs(n));
            let id = into.push(a.tag(n), self.token(a.main_token(n)), lhs, rhs);
            debug_assert_eq!(id.0, self.node(n.0), "a node lands at its moved id");
        }
        for d in a.diags() {
            debug_assert_eq!(d.detail, 0, "a parser's diagnostic has no detail");
            into.diag(d.code, self.token(d.token));
        }
        let top = a.list(root);
        debug_assert_eq!(top.first(), Some(&1), "the title is the first statement");
        top.iter().skip(1).map(|&n| self.node(n)).collect()
    }

    /// Word `w` of kind `kind` as `into` reads it; the words behind an
    /// `extra_data` offset (copied as they stood) are moved with it.
    fn word(&self, a: &Ast, into: &mut Ast<'static>, kind: WordKind, w: u32) -> u32 {
        use WordKind as K;
        match kind {
            K::Unused | K::Value => return w,
            K::Node | K::NodeOpt => return self.node(w),
            K::Token | K::TokenOpt => return self.token(w),
            K::Name => return self.names[w as usize],
            K::List | K::Body if w == NONE => return NONE,
            K::List => self.list(a, into, w),
            K::Header => {
                self.set_node(a, into, w);
                self.set_node(a, into, w + 1);
                self.list(a, into, w + 3);
                self.list(a, into, w + 4 + a.extra(w + 3));
            }
            K::Body => {
                self.set_node(a, into, w);
                self.set_node(a, into, w + 1);
                self.list(a, into, w + 2);
            }
            K::Group => {
                self.set_token(a, into, w);
                self.list(a, into, w + 1);
            }
            K::Pair => {
                self.set_node(a, into, w);
                self.set_node(a, into, w + 1);
            }
            K::IndexTail => {
                self.set_node(a, into, w);
                self.set_token(a, into, w + 1);
            }
        }
        self.extra + w
    }

    /// Moves the list at `at` of `a`: `[n, item0 .. item(n-1)]`, each item
    /// a node id or `NONE`.
    fn list(&self, a: &Ast, into: &mut Ast<'static>, at: u32) {
        for k in at + 1..=at + a.extra(at) {
            self.set_node(a, into, k);
        }
    }

    /// Moves the node id at word `at` of `a`'s `extra_data`.
    fn set_node(&self, a: &Ast, into: &mut Ast<'static>, at: u32) {
        into.set_extra(self.extra + at, self.node(a.extra(at)));
    }

    /// Moves the token index at word `at` of `a`'s `extra_data`.
    fn set_token(&self, a: &Ast, into: &mut Ast<'static>, at: u32) {
        into.set_extra(self.extra + at, self.token(a.extra(at)));
    }
}

/// `built` against `read`: the first index they differ at (or where the
/// shorter one ends), spelled for an assertion's message.
fn unlike<R: PartialEq + std::fmt::Debug>(what: &str, built: &[R], read: &[R]) -> Option<String> {
    let at = built
        .iter()
        .zip(read)
        .position(|(b, r)| b != r)
        .or_else(|| (built.len() != read.len()).then(|| built.len().min(read.len())))?;
    let (b, r) = (built.get(at), read.get(at));
    Some(format!("{what} {at}: built {b:?}, read {r:?}"))
}

/// Every number token's value, as bits: `-0.0` and a NaN's payload count.
fn bits(t: &Tokens) -> Vec<u64> {
    t.number_values.iter().map(|v| v.to_bits()).collect()
}

/// The language of the lexicon after each switch.
fn langs(t: &Tokens) -> Vec<crate::syntax::Lang> {
    t.switch_lexicons.iter().map(|l| l.lang).collect()
}

/// Every node row.
fn nodes(a: &Ast) -> Vec<(Tag, u32, u32, u32)> {
    let row = |n: Node| (a.tag(n), a.main_token(n), a.lhs(n), a.rhs(n));
    (0..a.len() as u32).map(Node).map(row).collect()
}

/// Every word of `extra_data`.
fn extra(a: &Ast) -> Vec<u32> {
    (0..a.extra_len() as u32).map(|k| a.extra(k)).collect()
}

/// Every interned name, in id order.
fn names(a: &Ast) -> Vec<String> {
    let name = |k: u32| a.name(Name(k)).to_string();
    (0..a.names_len() as u32).map(name).collect()
}
