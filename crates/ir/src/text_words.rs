//! IR text, the statement cursor: one line's bytes in, the fields a row's `parse` asks for
//! out (words, names, numbers, strings, references, values, expressions, lists), each failure
//! one `TEXT_SYNTAX` row at the current word. The grammar is `text.rs`'s header; expressions are
//! read by `text_parse`, literals, names and quoted runs by `text_lex`.
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (A2: `Words`), §9.
//!
//! Contents: `pub struct Words<'t>`,
//! `Words::next(&mut self) -> Option<&'t str>`, `Words::peek(&self) -> Option<&'t str>`,
//! `Words::at_end(&self) -> bool`, `Words::expect(&mut self, &str, &mut Diags) -> bool`,
//! `Words::key(&mut self) -> Option<&'t str>`,
//! `Words::name(&mut self, &mut Builder<'_>, &mut Diags) -> NameId`,
//! `Words::names(&mut self, &mut Builder<'_>, &mut Diags) -> NameList`,
//! `Words::number(&mut self, &mut Diags) -> Option<Literal>`,
//! `Words::string(&mut self, &mut Diags) -> Option<Cow<'t, str>>`,
//! `Words::reference(&mut self, &mut Builder<'_>, &mut Diags) -> Ref`,
//! `Words::value(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ExprId`,
//! `Words::expr(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ExprId`,
//! `Words::exprs(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ExprList`,
//! `Words::params(&mut self, ScopeId, &mut Builder<'_>, &mut Diags) -> ParamList`,
//! `Words::error(&self, &mut Diags)`, `Words::span(&self) -> Span`,
//! `Words::name_text(&mut self, &mut Diags) -> Option<Cow<'t, str>>`,
//! `Words::names_text(&mut self, &mut Diags) -> Option<Vec<Cow<'t, str>>>`,
//! `Words::line_span(&self) -> Span`

use std::borrow::Cow;

use crate::build::Builder;
use crate::diag::{Diags, TEXT_SYNTAX};
use crate::expr::Literal;
use crate::id::{DefId, ExprId, ExprList, FileId, NameId, NameList, ParamList, Ref, ScopeId};
use crate::span::Span;
use crate::text_lex::{Mark, lex_literal, lex_mark, lex_name, name_byte, unquote};
use crate::text_parse::parse_expr;

/// The IR text reader's cursor over one statement line (after its indentation). A word is a
/// maximal run of non-blank bytes outside a string or backtick name, or one of `( ) [ ] , =`.
/// Every reading method that fails appends a [`TEXT_SYNTAX`] row at the current word, consumes
/// nothing more than the bad word, and returns its failure value (`None`, `NONE`, an empty list).
#[derive(Debug)]
pub struct Words<'t> {
    /// The whole text (spans are offsets into it).
    pub(crate) text: &'t str,
    /// Byte offset of the next word.
    pub(crate) pos: usize,
    /// Byte offset of the end of this line.
    pub(crate) end: usize,
    /// Byte offset of the statement's first byte (after its indentation).
    pub(crate) line: usize,
    /// The file block the line sits in: every span this cursor makes names it.
    pub(crate) file: FileId,
}

impl<'t> Words<'t> {
    /// The next word, consumed; `None` at the end of the line.
    #[allow(clippy::should_implement_trait)] // a cursor over one line, not an `Iterator`
    pub fn next(&mut self) -> Option<&'t str> {
        self.blank();
        if self.pos >= self.end {
            return None;
        }
        let e = self.word_end(self.pos);
        let word = &self.text[self.pos..e];
        self.pos = e;
        Some(word)
    }

    /// The next word, not consumed.
    pub fn peek(&self) -> Option<&'t str> {
        let mut ahead = Words {
            pos: self.pos,
            ..*self
        };
        ahead.next()
    }

    /// Whether the line has no word left.
    pub fn at_end(&self) -> bool {
        self.text.as_bytes()[self.pos..self.end]
            .iter()
            .all(|c| *c == b' ' || *c == b'\t')
    }

    /// Consumes `word` if it is next; else reports and returns false.
    pub fn expect(&mut self, word: &str, diags: &mut Diags) -> bool {
        self.blank();
        let at = self.pos;
        if self.next() == Some(word) {
            return true;
        }
        self.error_at(at, diags);
        false
    }

    /// If the next words are `key =`, consumes them and returns the key; else `None`, consuming
    /// nothing (fields are optional, so this never reports).
    pub fn key(&mut self) -> Option<&'t str> {
        let start = self.pos;
        let key = self
            .next()
            .filter(|w| !w.is_empty() && w.bytes().all(name_byte));
        if key.is_some() && self.next() == Some("=") {
            return key;
        }
        self.pos = start;
        None
    }

    /// A name (bare in a name position, or backtick-quoted), interned.
    pub fn name(&mut self, b: &mut Builder<'_>, diags: &mut Diags) -> NameId {
        match self.name_text(diags) {
            Some(n) => b.name(&n),
            None => NameId::NONE,
        }
    }

    /// A parenthesized, blank-separated name list: `(a b c)`, possibly `()`.
    pub fn names(&mut self, b: &mut Builder<'_>, diags: &mut Diags) -> NameList {
        match self.names_text(diags) {
            Some(names) => {
                let refs: Vec<&str> = names.iter().map(|n| n.as_ref()).collect();
                b.names(&refs)
            }
            None => NameList::EMPTY,
        }
    }

    /// A number in the grammar's spelling, as its literal (no floating arithmetic).
    pub fn number(&mut self, diags: &mut Diags) -> Option<Literal> {
        self.blank();
        let at = self.pos;
        let literal = self.next().and_then(lex_literal);
        if literal.is_none() {
            self.error_at(at, diags);
        }
        literal
    }

    /// A double-quoted string, unescaped: borrowed from the text when it has no escape, owned
    /// when it has.
    pub fn string(&mut self, diags: &mut Diags) -> Option<Cow<'t, str>> {
        self.blank();
        let at = self.pos;
        if at < self.end
            && self.text.as_bytes()[at] == b'"'
            && let Some((s, q)) = unquote(self.text, at, self.end)
        {
            self.pos = q;
            return Some(s);
        }
        self.next();
        self.error_at(at, diags);
        None
    }

    /// A reference: `name` (unresolved; `link` binds it after the whole text), `name?` (held
    /// unresolved through the link) or `name#k` (bound to definition `k`, checked by `check`).
    /// The row stores the returned `Ref` as it is.
    pub fn reference(&mut self, b: &mut Builder<'_>, diags: &mut Diags) -> Ref {
        self.blank();
        let at = self.pos;
        let Some((name, q)) = lex_name(self.text, at, self.end) else {
            self.next();
            self.error_at(at, diags);
            return Ref {
                name: NameId::NONE,
                def: DefId::NONE,
            };
        };
        let (mark, q) = lex_mark(self.text, q, self.end);
        self.pos = q;
        let def = match mark {
            Mark::Bare => DefId::NONE,
            Mark::Held => DefId::HELD,
            Mark::Def(k) => DefId(k),
        };
        Ref {
            name: b.name(&name),
            def,
        }
    }

    /// A field value: an atom, or a parenthesized expression (references in it are linked after
    /// the whole text). `scope` is where the expression is used.
    pub fn value(&mut self, _scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ExprId {
        self.blank();
        let at = self.pos;
        let s = self.text.as_bytes();
        let (start, stop, after) = if at < self.end && s[at] == b'(' {
            match self.matching_paren(at) {
                Some(close) => (at + 1, close, close + 1),
                None => {
                    self.pos = self.end;
                    self.error_at(at, diags);
                    return ExprId::NONE;
                }
            }
        } else {
            let e = self.word_end(at);
            (at, e, e)
        };
        match parse_expr(self.text, start, stop, b) {
            Ok((e, q)) if q == stop => {
                self.pos = after;
                e
            }
            Ok((_, q)) | Err(q) => {
                self.pos = after;
                self.error_at(q, diags);
                ExprId::NONE
            }
        }
    }

    /// An expression to the end of the line or the next unmatched `)`/`,`/`]` (the `param` and
    /// `func` right-hand sides). Explicit operator stack: no recursion on nesting depth (C11).
    pub fn expr(&mut self, _scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ExprId {
        self.blank();
        match parse_expr(self.text, self.pos, self.end, b) {
            Ok((e, q)) => {
                self.pos = q;
                e
            }
            Err(q) => {
                self.pos = self.end;
                self.error_at(q, diags);
                ExprId::NONE
            }
        }
    }

    /// A bracketed expression list: `[e1, e2]`, possibly `[]`.
    pub fn exprs(&mut self, _scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ExprList {
        if !self.expect("[", diags) {
            return ExprList::EMPTY;
        }
        let mut items = Vec::new();
        // bounded by the line: each turn consumes an element and its separator
        loop {
            self.blank();
            if self.pos < self.end && self.text.as_bytes()[self.pos] == b']' && items.is_empty() {
                self.pos += 1;
                break;
            }
            match parse_expr(self.text, self.pos, self.end, b) {
                Ok((e, q)) if q < self.end && matches!(self.text.as_bytes()[q], b',' | b']') => {
                    items.push(e);
                    self.pos = q + 1;
                    if self.text.as_bytes()[q] == b']' {
                        break;
                    }
                }
                Ok((_, q)) | Err(q) => {
                    self.pos = self.end;
                    self.error_at(q, diags);
                    return ExprList::EMPTY;
                }
            }
        }
        b.exprs(&items)
    }

    /// A parenthesized `key=value` list: `(w=1e-6 l=(2 * lmin))`, possibly `()`.
    pub fn params(&mut self, scope: ScopeId, b: &mut Builder<'_>, diags: &mut Diags) -> ParamList {
        if !self.expect("(", diags) {
            return ParamList::EMPTY;
        }
        let mut pairs: Vec<(Cow<'t, str>, ExprId)> = Vec::new();
        // bounded by the line: each turn consumes one `key=value`
        while self.peek() != Some(")") {
            let Some(key) = self.name_text(diags) else {
                return ParamList::EMPTY;
            };
            if !self.expect("=", diags) {
                return ParamList::EMPTY;
            }
            let value = self.value(scope, b, diags);
            if value.is_none() {
                return ParamList::EMPTY;
            }
            pairs.push((key, value));
        }
        self.next();
        if pairs
            .iter()
            .enumerate()
            .any(|(k, p)| pairs[..k].iter().any(|q| q.0 == p.0))
        {
            self.error(diags);
            return ParamList::EMPTY;
        }
        let refs: Vec<(&str, ExprId)> = pairs.iter().map(|(k, v)| (k.as_ref(), *v)).collect();
        b.params(&refs)
    }

    /// Appends a [`TEXT_SYNTAX`] row at the current word.
    pub fn error(&self, diags: &mut Diags) {
        diags.push(TEXT_SYNTAX, self.span(), 0);
    }

    /// The current word's bytes (file `NONE`: the text is not a design file).
    pub fn span(&self) -> Span {
        let mut at = self.pos;
        let s = self.text.as_bytes();
        while at < self.end && (s[at] == b' ' || s[at] == b'\t') {
            at += 1;
        }
        let end = self.word_end(at);
        Span {
            file: self.file,
            start: at as u32,
            len: (end - at) as u32,
        }
    }

    /// A name (bare `[A-Za-z0-9_.$]+` or backtick-quoted) as text, not interned: a reader
    /// passes it to the push, which interns it in a builder caller's order.
    pub fn name_text(&mut self, diags: &mut Diags) -> Option<Cow<'t, str>> {
        self.blank();
        let at = self.pos;
        match lex_name(self.text, at, self.end) {
            Some((n, q)) if q == self.word_end(at) => {
                self.pos = q;
                Some(n)
            }
            Some(_) | None => {
                self.next();
                self.error_at(at, diags);
                None
            }
        }
    }

    /// A parenthesized, blank-separated name list as text: `(a b c)`, possibly `()`.
    pub fn names_text(&mut self, diags: &mut Diags) -> Option<Vec<Cow<'t, str>>> {
        if !self.expect("(", diags) {
            return None;
        }
        let mut names = Vec::new();
        // bounded by the line: each turn consumes a name
        while self.peek() != Some(")") {
            names.push(self.name_text(diags)?);
        }
        self.next();
        Some(names)
    }

    /// The whole statement's bytes (the span a reader gives the item it pushes).
    pub fn line_span(&self) -> Span {
        Span {
            file: self.file,
            start: self.line as u32,
            len: (self.end - self.line) as u32,
        }
    }
}

// ---- private ----

impl Words<'_> {
    fn blank(&mut self) {
        let s = self.text.as_bytes();
        while self.pos < self.end && (s[self.pos] == b' ' || s[self.pos] == b'\t') {
            self.pos += 1;
        }
    }

    /// The end of the word starting at `at`: one punctuation byte, a quoted run, or a run up to
    /// a blank or punctuation (quoted runs inside it included).
    fn word_end(&self, at: usize) -> usize {
        let s = self.text.as_bytes();
        if at >= self.end {
            return at;
        }
        match s[at] {
            b'(' | b')' | b'[' | b']' | b',' | b'=' => at + 1,
            b'"' | b'`' => unquote(self.text, at, self.end).map_or(self.end, |(_, q)| q),
            _ => {
                let mut q = at;
                while q < self.end
                    && !matches!(s[q], b' ' | b'\t' | b'(' | b')' | b'[' | b']' | b',' | b'=')
                {
                    q = if matches!(s[q], b'"' | b'`') {
                        unquote(self.text, q, self.end).map_or(self.end, |(_, e)| e)
                    } else {
                        q + 1
                    };
                }
                q
            }
        }
    }

    /// The `)` matching the `(` at `open`, skipping quoted runs.
    fn matching_paren(&self, open: usize) -> Option<usize> {
        let s = self.text.as_bytes();
        let (mut depth, mut p) = (0usize, open);
        while p < self.end {
            match s[p] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(p);
                    }
                }
                b'"' | b'`' => {
                    p = unquote(self.text, p, self.end)?.1;
                    continue;
                }
                _ => {}
            }
            p += 1;
        }
        None
    }

    fn error_at(&self, at: usize, diags: &mut Diags) {
        let at = at.min(self.end);
        let end = self.word_end(at).max(at);
        diags.push(
            TEXT_SYNTAX,
            Span {
                file: self.file,
                start: at as u32,
                len: (end - at) as u32,
            },
            0,
        );
    }
}
