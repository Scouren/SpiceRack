//! The tokenizer's output: one row per token, `{tag: u8, start: u32}` SoA,
//! plus the side tables a later stage reads instead of re-lexing (number
//! values, the physical line starts, the lexicon switches).
//!
//! A token's end is stored beside its start (`ends`), so [`Tokens::end`] is
//! a load: the renderer asks for it once per token per text it reads, and
//! re-lexing it there (Zig's `Ast.tokenSlice`) was a fifth of the render.
//! Comments, blank lines and continuation marks
//! are not tokens: they stay in the gaps between token spans, where a
//! renderer finds them (Zig `Render.zig:3074-3099`). Statement structure is
//! tokens ([`Tag::EndOfStatement`], [`Tag::LangSpice`]).
//!
//! Cites: Zig 0.17 `lib/std/zig/tokenizer.zig`, `Ast.zig:28-31`,
//! `279-295`.
//!
//! Contents:
//! - `pub enum Tag`, `Tag::lexeme(self) -> Option<&'static str>`
//! - `pub struct Tokens<'s>`
//! - `pub struct Location { pub line: u32, pub column: u32 }`
//! - `pub(crate) struct TokRows`,
//!   `Tokens::from_rows(source: &'s str, lexicon: &'static Lexicon, rows: TokRows) -> Tokens<'s>`
//! - `Tokens::source(&self) -> &'s str`, `Tokens::tags(&self) -> &[Tag]`,
//!   `Tokens::len(&self) -> usize`, `Tokens::is_empty(&self) -> bool`,
//!   `Tokens::start(&self, i: u32) -> u32`, `Tokens::end(&self, i: u32) -> u32`,
//!   `Tokens::end_relexed(&self, i: u32) -> u32`,
//!   `Tokens::text(&self, i: u32) -> &'s str`, `Tokens::value(&self, i: u32) -> f64`,
//!   `Tokens::location(&self, i: u32) -> Location`,
//!   `Tokens::lexicon_at(&self, i: u32) -> &'static Lexicon`

use crate::syntax::lexicon::Lexicon;

/// A token's kind. Every tag but the words and the zero-width structure
/// tags spells itself ([`Tag::lexeme`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Tag {
    // ── structure ──
    /// Line 1 of a top-level file, the whole physical line, maybe empty.
    Title,
    /// Zero width, at the end of a logical statement's last token line.
    EndOfStatement,
    /// Zero width, after a `simulator lang=<name>` statement that opens a
    /// region (Spectre's `lang=spice`; one of [`Lexicon::regions`]): the
    /// tokens up to the next [`Tag::LangSpectre`] are lexed by the
    /// region's lexicon.
    LangSpice,
    /// Zero width, after the statement that closes a region (`simulator
    /// lang=spectre` in a Spectre file, `lang=spice` in a SPICE one): back
    /// to the file's own lexicon.
    LangSpectre,
    /// One byte no rule reads (a control byte); the parser reports it.
    Invalid,

    // ── words ──
    /// A name: letters, digits, `_`, non-ASCII, the lexicon's extra bytes.
    Ident,
    /// A number with its scale and unit tail; its value is
    /// [`Tokens::value`].
    Number,
    /// `"…"` (or VACASK `<<<MARK … >>>MARK`), quotes included.
    String,
    /// A directive word (`.foo`, VACASK `@foo`) that is no keyword.
    Directive,

    // ── keywords: one tag per meaning, whatever the dialect's spelling ──
    /// `.param`, `parameters`.
    KwParam,
    /// `.func`.
    KwFunc,
    /// `.subckt`, `subckt`.
    KwSubckt,
    /// `.ends`, `ends`.
    KwEnds,
    /// `.model`, `model`.
    KwModel,
    /// `.include`, `include`.
    KwInclude,
    /// `.lib`.
    KwLib,
    /// `.endl`.
    KwEndl,
    /// `.end`.
    KwEnd,
    /// `.global`, `global`.
    KwGlobal,
    /// `.options`.
    KwOptions,
    /// `.temp`.
    KwTemp,
    /// `.ic`, `ic`.
    KwIc,
    /// `.nodeset`, `nodeset`.
    KwNodeset,
    /// `.save`, `save`.
    KwSave,
    /// `.print`, `.plot`.
    KwPrint,
    /// `.probe` (HSPICE).
    KwProbe,
    /// `.meas`.
    KwMeas,
    /// `.op`.
    KwOp,
    /// `.dc`.
    KwDc,
    /// `.ac`.
    KwAc,
    /// `.tran`.
    KwTran,
    /// `.noise`.
    KwNoise,
    /// `.tf`.
    KwTf,
    /// `.sens`.
    KwSens,
    /// `.pz`.
    KwPz,
    /// `.four`.
    KwFour,
    /// `.sp` (ngspice).
    KwSp,
    /// `.step` (LTspice, Xyce).
    KwStep,
    /// `.data`.
    KwData,
    /// `.enddata`.
    KwEnddata,
    /// `.variation`.
    KwVariation,
    /// `.end_variation`.
    KwEndVariation,
    /// `.control`, VACASK `control`.
    KwControl,
    /// `.endc`, VACASK `endc`.
    KwEndc,
    /// `.if`, Spectre `if`, VACASK `@if`.
    KwIf,
    /// `.elseif`, VACASK `@elseif`.
    KwElseif,
    /// `.else`, Spectre `else`, VACASK `@else`.
    KwElse,
    /// `.endif`, VACASK `@end`.
    KwEndif,
    /// Spectre `simulator`.
    KwSimulator,
    /// Spectre `ahdl_include`.
    KwAhdlInclude,
    /// Spectre `library`.
    KwLibrary,
    /// Spectre `endlibrary`.
    KwEndlibrary,
    /// Spectre and VACASK `section`.
    KwSection,
    /// Spectre and VACASK `endsection`.
    KwEndsection,
    /// Spectre `inline`.
    KwInline,
    /// Spectre `statistics`.
    KwStatistics,
    /// Spectre `process`.
    KwProcess,
    /// Spectre `mismatch`.
    KwMismatch,
    /// Spectre `vary`.
    KwVary,
    /// Spectre `correlate`.
    KwCorrelate,
    /// Spectre `truncate`.
    KwTruncate,
    /// Spectre `real`.
    KwReal,
    /// Spectre `return`.
    KwReturn,
    /// VACASK `load`.
    KwLoad,
    /// VACASK `embed`.
    KwEmbed,
    /// VACASK `ground`.
    KwGround,
    /// VACASK `analysis`.
    KwAnalysis,
    /// VACASK `sweep`.
    KwSweep,

    // ── punctuation and operators: each spells itself ──
    /// `(`.
    LParen,
    /// `)`.
    RParen,
    /// `[`.
    LBracket,
    /// `]`.
    RBracket,
    /// `{`.
    LBrace,
    /// `}`.
    RBrace,
    /// `,`.
    Comma,
    /// `=`.
    Equal,
    /// `'`, an expression quote where the lexicon says so.
    Quote,
    /// `"`, an expression quote (HSPICE, `Lexicon::string_expr`); else a
    /// `"` starts a [`Tag::String`].
    DoubleQuote,
    /// `:`.
    Colon,
    /// `;`.
    Semicolon,
    /// `?`.
    Question,
    /// `+`.
    Plus,
    /// `-`.
    Minus,
    /// `*`.
    Star,
    /// `**`.
    StarStar,
    /// `/`.
    Slash,
    /// `\`.
    Backslash,
    /// `%`.
    Percent,
    /// `^`.
    Caret,
    /// `!`.
    Bang,
    /// `!=`.
    BangEqual,
    /// `==`.
    EqualEqual,
    /// `<>`.
    LessGreater,
    /// `<`.
    Less,
    /// `<=`.
    LessEqual,
    /// `<<`.
    LessLess,
    /// `>`.
    Greater,
    /// `>=`.
    GreaterEqual,
    /// `>>`.
    GreaterGreater,
    /// `&`.
    Amp,
    /// `&&`.
    AmpAmp,
    /// `|`.
    Pipe,
    /// `||`.
    PipePipe,
    /// `~`.
    Tilde,
    /// `.`.
    Dot,
    /// `@`.
    At,
    /// `#`.
    Hash,
    /// `` ` ``.
    Backtick,
}

/// A file's tokens.
///
/// Invariants: `tags`, `starts` and `ends` have one row per token, `starts`
/// never decreases, `starts[i] <= ends[i]`; `number_tokens` lists every [`Tag::Number`] token in order,
/// `number_values` its value; `switches` lists every [`Tag::LangSpice`] and
/// [`Tag::LangSpectre`] token in order.
pub struct Tokens<'s> {
    pub(super) source: &'s str,
    pub(super) lexicon: &'static Lexicon,
    pub(super) tags: Vec<Tag>,
    pub(super) starts: Vec<u32>,
    /// One past each token's last byte: every token writer fills it with
    /// the end its lexer found (`tokenize`'s `push`).
    pub(super) ends: Vec<u32>,
    /// The byte offset of each physical line's first byte.
    pub(super) line_starts: Vec<u32>,
    pub(super) number_tokens: Vec<u32>,
    /// The value of each `number_tokens` entry, NaN for a spelling the
    /// lexicon's number grammar does not read (`1k5` in Spectre).
    pub(super) number_values: Vec<f64>,
    pub(super) switches: Vec<u32>,
    /// The lexicon in force after each switch, by `switches` index.
    pub(super) switch_lexicons: Vec<&'static Lexicon>,
}

/// A position for a diagnostic, both 1-based; the column counts bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location {
    /// The physical line, counting every `\n`.
    pub line: u32,
    /// The byte offset in that line, plus one.
    pub column: u32,
}

/// The rows of a [`Tokens`] whose text is not borrowed yet: what a writer
/// other than the tokenizer appends while it still owns the text, column
/// for column as [`Tokens`] (the line starts are found from the text).
// `syntax::build` fills it, which only the tests and the `python` feature
// compile.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Default)]
pub(crate) struct TokRows {
    /// Each token's tag.
    pub(crate) tags: Vec<Tag>,
    /// Each token's first byte.
    pub(crate) starts: Vec<u32>,
    /// One past each token's last byte.
    pub(crate) ends: Vec<u32>,
    /// Every [`Tag::Number`] token, in order.
    pub(crate) number_tokens: Vec<u32>,
    /// The value of each `number_tokens` entry.
    pub(crate) number_values: Vec<f64>,
    /// Every [`Tag::LangSpice`] and [`Tag::LangSpectre`] token, in order.
    pub(crate) switches: Vec<u32>,
    /// The lexicon in force after each switch, by `switches` index.
    pub(crate) switch_lexicons: Vec<&'static Lexicon>,
}

impl Tag {
    /// The fixed spelling of a punctuation tag; `None` for words and the
    /// structure tags, whose text is re-lexed.
    pub fn lexeme(self) -> Option<&'static str> {
        use Tag::*;
        Some(match self {
            LParen => "(",
            RParen => ")",
            LBracket => "[",
            RBracket => "]",
            LBrace => "{",
            RBrace => "}",
            Comma => ",",
            Equal => "=",
            Quote => "'",
            DoubleQuote => "\"",
            Colon => ":",
            Semicolon => ";",
            Question => "?",
            Plus => "+",
            Minus => "-",
            Star => "*",
            StarStar => "**",
            Slash => "/",
            Backslash => "\\",
            Percent => "%",
            Caret => "^",
            Bang => "!",
            BangEqual => "!=",
            EqualEqual => "==",
            LessGreater => "<>",
            Less => "<",
            LessEqual => "<=",
            LessLess => "<<",
            Greater => ">",
            GreaterEqual => ">=",
            GreaterGreater => ">>",
            Amp => "&",
            AmpAmp => "&&",
            Pipe => "|",
            PipePipe => "||",
            Tilde => "~",
            Dot => ".",
            At => "@",
            Hash => "#",
            Backtick => "`",
            Title | EndOfStatement | LangSpice | LangSpectre | Invalid | Ident | Number
            | String | Directive | KwParam | KwFunc | KwSubckt | KwEnds | KwModel | KwInclude
            | KwLib | KwEndl | KwEnd | KwGlobal | KwOptions | KwTemp | KwIc | KwNodeset
            | KwSave | KwPrint | KwProbe | KwMeas | KwOp | KwDc | KwAc | KwTran | KwNoise
            | KwTf | KwSens | KwPz | KwFour | KwSp | KwStep | KwData | KwEnddata | KwVariation
            | KwEndVariation | KwControl | KwEndc | KwIf | KwElseif | KwElse | KwEndif
            | KwSimulator | KwAhdlInclude | KwLibrary | KwEndlibrary | KwSection | KwEndsection
            | KwInline | KwStatistics | KwProcess | KwMismatch | KwVary | KwCorrelate
            | KwTruncate | KwReal | KwReturn | KwLoad | KwEmbed | KwGround | KwAnalysis
            | KwSweep => return None,
        })
    }
}

impl<'s> Tokens<'s> {
    /// The tokens of `source` from rows built beside it, `lexicon` the
    /// file's own: the twin of `tokenize` for a writer that appends text
    /// and rows together.
    ///
    /// # Panics
    /// When the rows break [`Tokens`]' invariants: a start and an end per
    /// tag, starts that never decrease, every token inside `source`,
    /// `number_tokens` every [`Tag::Number`] token in order with one value
    /// each, `switches` every [`Tag::LangSpice`] and [`Tag::LangSpectre`]
    /// token in order with one lexicon each.
    // `syntax::build`'s decks call it, which only the tests and the
    // `python` feature compile.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_rows(
        source: &'s str,
        lexicon: &'static Lexicon,
        rows: TokRows,
    ) -> Tokens<'s> {
        let n = rows.tags.len();
        assert!(
            rows.starts.len() == n && rows.ends.len() == n,
            "a start and an end per token"
        );
        assert!(rows.starts.is_sorted(), "token starts never decrease");
        let inside = |(s, e): (&u32, &u32)| s <= e && *e as usize <= source.len();
        assert!(
            rows.starts.iter().zip(&rows.ends).all(inside),
            "every token is inside the source"
        );
        let of = |want: fn(Tag) -> bool| {
            let tags = &rows.tags;
            (0..n as u32).filter(move |&i| want(tags[i as usize]))
        };
        assert!(
            of(|t| t == Tag::Number).eq(rows.number_tokens.iter().copied()),
            "number_tokens lists every Number token, in order"
        );
        assert_eq!(
            rows.number_tokens.len(),
            rows.number_values.len(),
            "a value per number token"
        );
        assert!(
            of(|t| matches!(t, Tag::LangSpice | Tag::LangSpectre))
                .eq(rows.switches.iter().copied()),
            "switches lists every lexicon switch, in order"
        );
        assert_eq!(
            rows.switches.len(),
            rows.switch_lexicons.len(),
            "a lexicon per switch"
        );
        let line_ends = source.bytes().enumerate().filter(|&(_, b)| b == b'\n');
        Tokens {
            source,
            lexicon,
            tags: rows.tags,
            starts: rows.starts,
            ends: rows.ends,
            line_starts: std::iter::once(0)
                .chain(line_ends.map(|(i, _)| i as u32 + 1))
                .collect(),
            number_tokens: rows.number_tokens,
            number_values: rows.number_values,
            switches: rows.switches,
            switch_lexicons: rows.switch_lexicons,
        }
    }

    /// The text the tokens index.
    pub fn source(&self) -> &'s str {
        self.source
    }

    /// Every token's tag, in source order.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    /// The number of tokens.
    pub fn len(&self) -> usize {
        self.tags.len()
    }

    /// Whether there are no tokens.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// Token `i`'s first byte.
    pub fn start(&self, i: u32) -> u32 {
        self.starts[i as usize]
    }

    /// One past token `i`'s last byte.
    pub fn end(&self, i: u32) -> u32 {
        self.ends[i as usize]
    }

    /// Token `i`'s end re-lexed from its start: O(token length), plus
    /// O(log switches) to find its lexicon. The reference [`Tokens::end`] is
    /// held to (by `tokenize` in a debug build, and over every golden input
    /// by `tests/test_syntax.rs`); nothing else calls it.
    pub fn end_relexed(&self, i: u32) -> u32 {
        use crate::syntax::scan;
        let start = self.start(i);
        match self.tags[i as usize] {
            Tag::EndOfStatement | Tag::LangSpice | Tag::LangSpectre => start,
            Tag::Title => scan::line_end(self.source.as_bytes(), start as usize) as u32,
            tag => match tag.lexeme() {
                Some(s) => start + s.len() as u32,
                None => {
                    scan::token_at(self.source.as_bytes(), start as usize, self.lexicon_at(i)).1
                        as u32
                }
            },
        }
    }

    /// Token `i`'s text, as written.
    pub fn text(&self, i: u32) -> &'s str {
        &self.source[self.start(i) as usize..self.end(i) as usize]
    }

    /// Number token `i`'s value, decoded once by its lexicon's suffix table;
    /// NaN when that grammar does not read its spelling.
    ///
    /// # Panics
    /// When token `i` is not a [`Tag::Number`].
    pub fn value(&self, i: u32) -> f64 {
        let row = self
            .number_tokens
            .binary_search(&i)
            .expect("value: token is a Number");
        self.number_values[row]
    }

    /// Where token `i` starts.
    pub fn location(&self, i: u32) -> Location {
        let at = self.start(i);
        let line = self.line_starts.partition_point(|&s| s <= at);
        Location {
            line: line as u32,
            column: at - self.line_starts[line - 1] + 1,
        }
    }

    /// The lexicon token `i` was lexed by: the file's, or a region's
    /// inside a `simulator lang=` region.
    pub fn lexicon_at(&self, i: u32) -> &'static Lexicon {
        let before = self.switches.partition_point(|&s| s < i);
        match before.checked_sub(1) {
            Some(k) => self.switch_lexicons[k],
            None => self.lexicon,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_nine_bytes_per_token_and_twelve_per_number() {
        // A token row is a tag byte plus a u32 start and a u32 end (Zig
        // `Ast.TokenList` plus the end); a number row is a u32 token index
        // plus an f64 value.
        assert_eq!(size_of::<Tag>(), 1);
        assert_eq!(size_of::<Tag>() + 2 * size_of::<u32>(), 9);
        assert_eq!(size_of::<u32>() + size_of::<f64>(), 12);
        assert_eq!(size_of::<Location>(), 8);
    }

    #[test]
    fn p8_002_rows_built_beside_the_source_read_as_tokenized() {
        // Oracle: the tokenizer's own rows (a round trip). A deck with
        // numbers and a `simulator lang=` region (two lexicon switches),
        // its columns handed to `from_rows`, reads token for token as
        // `tokenize` left it. Checks: 2 + 4 per token.
        use crate::syntax::{Lang, tokenize};
        let src = "t\nR1 a 0 1k\nsimulator lang=spectre\nr2 (a 0) resistor r=2k\nsimulator lang=spice\n.op\n";
        let lex = Lang::NGSPICE_GIVEN.lexicon().expect("a reader");
        let a = tokenize(src, lex);
        assert!(a.switches.len() == 2 && a.number_tokens.len() >= 2);
        let rows = TokRows {
            tags: a.tags.clone(),
            starts: a.starts.clone(),
            ends: a.ends.clone(),
            number_tokens: a.number_tokens.clone(),
            number_values: a.number_values.clone(),
            switches: a.switches.clone(),
            switch_lexicons: a.switch_lexicons.clone(),
        };
        let b = Tokens::from_rows(src, lex, rows);
        assert_eq!(a.tags(), b.tags());
        for i in 0..a.len() as u32 {
            assert_eq!((a.start(i), a.end(i)), (b.start(i), b.end(i)), "{i}");
            assert_eq!(a.location(i), b.location(i), "token {i}");
            assert!(std::ptr::eq(a.lexicon_at(i), b.lexicon_at(i)), "token {i}");
            if a.tags()[i as usize] == Tag::Number {
                assert_eq!(a.value(i).to_bits(), b.value(i).to_bits(), "{i}");
            }
        }
    }
}
