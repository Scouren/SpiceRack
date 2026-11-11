//! The lexicon calls of a [`DialectDef`](super::DialectDef): each sets a
//! field or adds a row of the lexicon being declared (`Lex`, copied from
//! the shared lexicon on the first such call); and `set!`, the call that
//! sets one scalar field, which `mod.rs`'s behaviour calls use too (it is
//! `#[macro_use]`).
//!
//! Cites: none (the tables' own files cite their manuals).
//!
//! Contents:
//! - `macro_rules! set`
//! - the lexicon calls: `title`, `fold_case`, `comment_line`, `comment_col1`, `blank_led_comment`,
//!   `cont_skip`, `comment_tail`, `block_comment`, `long_strings`, `dollar`, `plus`, `backslash`,
//!   `end`, `stop`, `join`, `quote`, `string`, `string_expr`, `name_escape`, `ident_start`,
//!   `ident_continue`, `directive`, `head_words`, `head_after`, `operators`, `operator`,
//!   `expr_braces`, `caret`, `infix`, `right`, `prefix`, `prefix_bp`, `probe`, `index`,
//!   `lib_sections`, `exponent`, `scale`, `tail`, `arithmetic`, `keywords`, `keyword`, `named`,
//!   `devices`, `device`, `master`, `analysis`, `region`

use super::{Devices, DialectDef, Keywords, Operators, add, push_new, put};
use crate::syntax::Lang;
use crate::syntax::device;
use crate::syntax::lexicon::{
    self, Arithmetic, Backslash, Caret, Dollar, End, Join, Quote, Quoting, Scale, Tail,
};
use crate::syntax::statement::{Analysis, Statement};
use crate::syntax::token::Tag;

/// Generates the call that sets one scalar field.
macro_rules! set {
    ($(#[$doc:meta])* $name:ident, $part:ident . $field:ident : $ty:ty) => {
        $(#[$doc])*
        pub fn $name(mut self, v: $ty) -> Self {
            self.$part().$field = v;
            self
        }
    };
}

impl DialectDef {
    // ---- the lexicon ----

    set!(title, lex.title: bool);
    set!(fold_case, lex.fold_case: bool);
    set!(blank_led_comment, lex.blank_led_comment: bool);
    set!(block_comment, lex.block_comment: bool);
    set!(long_strings, lex.long_strings: bool);
    set!(plus, lex.plus: bool);
    set!(string_expr, lex.string_expr: bool);
    set!(name_escape, lex.name_escape: bool);
    set!(head_words, lex.head_words: bool);
    set!(expr_braces, lex.expr_braces: bool);
    set!(prefix_bp, lex.prefix_bp: u8);
    set!(index, lex.index: bool);
    set!(lib_sections, lex.lib_sections: bool);
    set!(named, lex.named: bool);
    set!(end, lex.end: Option<End>);
    set!(stop, lex.stop: Option<u8>);
    set!(directive, lex.directive: Option<u8>);

    /// A line whose first non-blank text starts with `s` is a comment.
    pub fn comment_line(mut self, s: &str) -> Self {
        push_new(&mut self.lex().comment_lines, s);
        self
    }

    /// A line whose first byte is `b` is a comment.
    pub fn comment_col1(mut self, b: u8) -> Self {
        let l = self.lex();
        if !l.comment_col1.contains(&b) {
            l.comment_col1.push(b);
        }
        self
    }

    /// A `+` continuation reads past comment lines starting with `s`.
    pub fn cont_skip(mut self, s: &str) -> Self {
        push_new(&mut self.lex().cont_skip, s);
        self
    }

    /// `s` starts a comment to the end of the line.
    pub fn comment_tail(mut self, s: &str) -> Self {
        push_new(&mut self.lex().comment_tails, s);
        self
    }

    /// When `$` starts a comment.
    pub fn dollar(mut self, v: Dollar) -> Self {
        self.lex().dollar = Some(v);
        self
    }

    /// A trailing backslash's meaning.
    pub fn backslash(mut self, v: Backslash) -> Self {
        self.lex().backslash = Some(v);
        self
    }

    /// Which open brackets carry a statement past its line.
    pub fn join(mut self, v: Join) -> Self {
        self.lex().join = Some(v);
        self
    }

    /// What `'` delimits.
    pub fn quote(mut self, v: Quote) -> Self {
        self.lex().quote = Some(v);
        self
    }

    /// How a `"…"` string reads.
    pub fn string(mut self, v: Quoting) -> Self {
        self.lex().string = Some(v);
        self
    }

    /// The bytes past `[A-Za-z_]` that start a name.
    pub fn ident_start(mut self, bytes: &str) -> Self {
        self.lex().ident_start = bytes.as_bytes().to_vec();
        self
    }

    /// The bytes past `[A-Za-z0-9_]` that continue a name.
    pub fn ident_continue(mut self, bytes: &str) -> Self {
        self.lex().ident_continue = bytes.as_bytes().to_vec();
        self
    }

    /// After token `t` the next word is at a statement head again.
    pub fn head_after(mut self, t: Tag) -> Self {
        let l = self.lex();
        if !l.heads_after.contains(&t) {
            l.heads_after.push(t);
        }
        self
    }

    /// Adds a shared operator table's rows the definition does not hold.
    pub fn operators(mut self, table: Operators) -> Self {
        let rows = match table {
            Operators::Spice => lexicon::SPICE_OPERATORS,
        };
        for &(s, t) in rows {
            add(&mut self.lex().operators, s.to_string(), t);
        }
        self
    }

    /// `text` is one operator token `t`.
    pub fn operator(mut self, text: &str, t: Tag) -> Self {
        put(&mut self.lex().operators, text.to_string(), t);
        self
    }

    /// What `^` means.
    pub fn caret(mut self, v: Caret) -> Self {
        self.lex().caret = Some(v);
        self
    }

    /// Binary operator `t` binds with power `bp` (higher binds tighter).
    pub fn infix(mut self, t: Tag, bp: u8) -> Self {
        put(&mut self.lex().infix, t, bp);
        self
    }

    /// Binary operator `t` groups to the right.
    pub fn right(mut self, t: Tag) -> Self {
        let l = self.lex();
        if !l.right.contains(&t) {
            l.right.push(t);
        }
        self
    }

    /// `t` is a prefix operator.
    pub fn prefix(mut self, t: Tag) -> Self {
        let l = self.lex();
        if !l.prefix.contains(&t) {
            l.prefix.push(t);
        }
        self
    }

    /// `name(...)` is a probe: its arguments are names.
    pub fn probe(mut self, name: &str) -> Self {
        push_new(&mut self.lex().probes, name);
        self
    }

    /// The letters that start an exponent.
    pub fn exponent(mut self, letters: &str) -> Self {
        self.lex().exponent = letters.as_bytes().to_vec();
        self
    }

    /// `text` after a mantissa scales it by `s`.
    pub fn scale(mut self, text: &str, s: Scale) -> Self {
        put(&mut self.lex().scales, text.to_string(), s);
        self
    }

    /// What may follow a number's mantissa and scale.
    pub fn tail(mut self, v: Tail) -> Self {
        self.lex().tail = Some(v);
        self
    }

    /// How a number's digits become a double.
    pub fn arithmetic(mut self, v: Arithmetic) -> Self {
        self.lex().arithmetic = Some(v);
        self
    }

    /// Adds a shared keyword table's rows the definition does not hold.
    pub fn keywords(mut self, table: Keywords) -> Self {
        let rows = match table {
            Keywords::Spice => lexicon::SPICE_KEYWORDS,
        };
        for &(s, t) in rows {
            add(&mut self.lex().keywords, s.to_string(), t);
        }
        self
    }

    /// `word` spells keyword `t`.
    pub fn keyword(mut self, word: &str, t: Tag) -> Self {
        put(&mut self.lex().keywords, word.to_string(), t);
        self
    }

    /// Adds a shared device table's rows the definition does not hold.
    pub fn devices(mut self, table: Devices) -> Self {
        let rows = match table {
            Devices::Spice => device::SPICE,
            Devices::NgspiceOwn => device::NGSPICE_OWN,
            Devices::LossyLine => device::LOSSY_LINE,
            Devices::XycePort => device::XYCE_PORT,
            Devices::Spectre => device::SPECTRE,
            Devices::Vacask => device::VACASK,
        };
        for d in rows {
            add(&mut self.lex().devices, d.word.to_string(), *d);
        }
        self
    }

    /// `word` names the element `like` names, with its shape.
    ///
    /// # Panics
    /// When no device row is spelled `like`.
    pub fn device(mut self, word: &str, like: &str) -> Self {
        let l = self.lex();
        let row = l.devices.iter().find(|d| d.0 == like).map(|d| d.1);
        let row = row.expect("device: a row spelled `like`");
        put(&mut l.devices, word.to_string(), row);
        self
    }

    /// Master `word` makes a statement `s` (Spectre).
    pub fn master(mut self, word: &str, s: Statement) -> Self {
        put(&mut self.lex().masters, word.to_string(), s);
        self
    }

    /// `analysis name word` is analysis `a` (VACASK).
    pub fn analysis(mut self, word: &str, a: Analysis) -> Self {
        put(&mut self.lex().analyses, word.to_string(), a);
        self
    }

    /// `simulator lang=name` opens a region read by `lang`'s lexicon.
    pub fn region(mut self, name: &str, lang: Lang) -> Self {
        put(&mut self.lex().regions, name.to_string(), lang);
        self
    }
}
