//! The reader's spine: sources → one concrete syntax tree per file (`syntax::tokenize` and
//! `syntax::parse` with Spectre's lexicon, a private tree) → declare (every model and
//! subcircuit name of every file, which an instance's kind needs first) → lower (one walk in
//! source order, one dispatch on each statement's CST tag and head word to an area function)
//! → bind (every parameter read, user call and row reference, by Spectre's rule, `bind.rs`).
//! Every branch on which work happens is here; an area function pushes rows or one `Opaque`
//! and returns.
//!
//! A deck's line 1 is its title whatever it holds; an included file has none (MAN: UG p.29,
//! p.50), so a library is tokenized behind one empty line. `//` anywhere and `*` in column 1
//! start a comment (MAN: UG p.59). A statement no rule reads is `Unread` (`codes::UNREAD`).
//!
//! Cites: `rewrite/DIALECT-SPECTRE.md` §2, §5; `DIALECT-VACASK.md` §3 (the same steps).
//!
//! Contents: `Cst`, `Lower` (`Lower::span`, `Lower::text`, `Lower::body`,
//! `Lower::comments_until`, `Lower::refer`, `Lower::field`, `Lower::token`), `references`,
//! `read`, `opaque`, `unread`, `statement_text`, `field_name`, `extent` (crate-private)

use std::collections::{HashMap, HashSet};

use super::bind::{self, RowRef};
use super::codes;
use super::{read_elements, read_run, read_structure};
use crate::dialect::{Reference, Sources};
use crate::ir;
use crate::syntax::ast::{Ast, Name, Node, Tag};
use crate::syntax::token::Tokens;
use crate::syntax::{Lang, parse, tokenize};

/// One file's concrete syntax tree. A library file is tokenized behind one empty line, since
/// Spectre's lexicon reads line 1 as a title and an included file has none: `shift` is that
/// line's byte, taken off every span.
pub(super) struct Cst<'s> {
    /// The tokens (of the shifted text for a library).
    pub(super) tokens: Tokens<'s>,
    /// The statements.
    pub(super) ast: Ast<'s>,
    /// 1 for a library file, else 0.
    pub(super) shift: u32,
    /// The file's id in the design.
    pub(super) file: ir::FileId,
}

/// The lowering walk's state: the builder, the diagnostics, the file being read and the scope
/// a statement's rows go to.
pub(super) struct Lower<'a, 'b, 's> {
    /// The append-only door to the design.
    pub(super) b: &'a mut ir::Builder<'b>,
    /// Where every diagnostic goes.
    pub(super) diags: &'a mut ir::Diags,
    /// The file's tree.
    pub(super) cst: &'a Cst<'s>,
    /// The scope the current statement's rows go to.
    pub(super) scope: ir::ScopeId,
    /// The span of the statement being lowered.
    pub(super) stmt: ir::Span,
    /// Every model and subcircuit name of every file, with what it is.
    pub(super) declared: &'a read_elements::Declared,
    /// Each parameter read and user call, and the scope it is read from (`bind::reads`).
    pub(super) reads: &'a mut Vec<(ir::ExprId, ir::ScopeId)>,
    /// Each row reference to bind (`bind::rows`).
    pub(super) refs: &'a mut Vec<RowRef>,
    /// Each scope's element names so far (a second one of a name is refused).
    pub(super) elements: HashMap<ir::ScopeId, HashSet<String>>,
    /// The next token whose leading gap has not been searched for comments.
    gap_token: u32,
    /// The end of the gaps already searched (a byte of the tokenized text).
    gap_end: u32,
}

impl Lower<'_, '_, '_> {
    /// The span of statement `n` (its first token to its last, a block's closer included).
    pub(super) fn span(&self, n: Node) -> ir::Span {
        let (start, end) = extent(self.cst, n);
        ir::Span {
            file: self.cst.file,
            start,
            len: end - start,
        }
    }

    /// The text of statement `n`, as written.
    pub(super) fn text(&self, n: Node) -> &str {
        statement_text(self.cst, n)
    }

    /// The token text at node `n`'s main token.
    pub(super) fn token(&self, n: Node) -> &str {
        self.cst.tokens.text(self.cst.ast.main_token(n))
    }

    /// The name a `Word`, `Ident` or `Str` field holds.
    pub(super) fn field(&self, field: u32) -> String {
        field_name(self.cst, field)
    }

    /// Queues row reference `k` of `item` (its slot in `RowFacts::map_refs` order), read from
    /// the current scope, for the bind step.
    pub(super) fn refer(&mut self, item: ir::ItemRef, k: u32, kind: ir::DefKind, name: &str) {
        assert!(!name.is_empty(), "refer: a name");
        let name = self.b.name(name);
        self.refs.push(RowRef {
            item,
            k,
            scope: self.scope,
            kind,
            name,
        });
    }

    /// Lowers the statements `body` into `scope`, comments of the gaps between them included,
    /// then those before byte `until` (a block's closer), then restores the current scope.
    pub(super) fn body(&mut self, scope: ir::ScopeId, body: &[u32], until: Option<u32>) {
        let saved = (self.scope, self.stmt);
        self.scope = scope;
        for &s in body {
            let n = Node(s);
            let (start, end) = extent(self.cst, n);
            self.comments_until(start, true);
            self.stmt = self.span(n);
            statement(self, n);
            self.comments_until(end, false);
        }
        if let Some(until) = until {
            self.comments_until(until, true);
        }
        (self.scope, self.stmt) = saved;
    }

    /// Pushes a `Comment` item (when `keep`) for each comment in the gaps that end at or before
    /// byte `until` (of the file's own text) and that no earlier call took.
    pub(super) fn comments_until(&mut self, until: u32, keep: bool) {
        let tokens = &self.cst.tokens;
        let source = tokens.source();
        let shift = self.cst.shift;
        let count = u32::try_from(tokens.len()).expect("fewer than 2^32 tokens");
        // Bounded: each pass takes one token's leading gap, or the tail once.
        while self.gap_token <= count {
            let i = self.gap_token;
            let gap_end = match i < count {
                true => tokens.start(i),
                false => u32::try_from(source.len()).expect("a file under 4 GiB"),
            };
            if gap_end.saturating_sub(shift) > until {
                break;
            }
            let gap_start = self.gap_end;
            if keep && gap_end > gap_start {
                self.gap_comments(gap_start as usize, gap_end as usize);
            }
            self.gap_end = match i < count {
                true => self.gap_end.max(tokens.end(i)),
                false => gap_end,
            };
            self.gap_token += 1;
        }
    }

    /// The comments of `source[from..to]` (a gap: blanks, comments, continuations): `//` to
    /// the line's end, and a `*` that starts a line.
    fn gap_comments(&mut self, from: usize, to: usize) {
        let source = self.cst.tokens.source();
        let bytes = source.as_bytes();
        let mut at = from;
        while at < to {
            let line_start = at == 0 || bytes[at - 1] == b'\n';
            let slashes = bytes[at..to].starts_with(b"//");
            let star = bytes[at] == b'*' && line_start;
            if !slashes && !star {
                at += 1;
                continue;
            }
            let body = at + if slashes { 2 } else { 1 };
            let end = bytes[body..to]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(to, |k| body + k);
            let text = &source[body..end];
            let start = u32::try_from(at).expect("a file under 4 GiB") - self.cst.shift;
            let span = ir::Span {
                file: self.cst.file,
                start,
                len: u32::try_from(end - at).expect("a short comment"),
            };
            self.b
                .comment(self.scope, text.strip_suffix('\r').unwrap_or(text), span);
            at = end;
        }
    }
}

/// The files `text` names, in text order: each `include "path"`; an `ahdl_include` of a
/// Verilog-A source.
pub(super) fn references(text: &str) -> Vec<Reference> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_start();
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let Some(path) = first_string(rest) else {
            continue;
        };
        match word {
            "include" => out.push(Reference::File(path)),
            "ahdl_include" => out.push(Reference::VerilogA(path)),
            _ => {}
        }
    }
    out
}

/// Reads every file of `src` into `out`: entries in `src`'s role, the rest as libraries.
pub(super) fn read(src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
    assert!(!src.files().is_empty(), "spectre::read: a file to read");
    let lex = Lang::SPECTRE.lexicon().expect("Spectre has a lexicon");
    let entries = src.entries().len();
    let role = |k: usize| match k < entries {
        true => src.form(),
        false => ir::Role::Library,
    };
    let texts: Vec<String> = src
        .files()
        .iter()
        .enumerate()
        .map(|(k, (_, t))| match role(k) {
            ir::Role::Deck => t.clone(),
            _ => format!("\n{t}"),
        })
        .collect();
    let mut csts = Vec::with_capacity(texts.len());
    for (k, (path, _)) in src.files().iter().enumerate() {
        let (file, top) = out.file(path, role(k));
        let tokens = tokenize(&texts[k], lex);
        let ast = parse(&tokens, lex);
        let shift = u32::from(role(k) != ir::Role::Deck);
        csts.push((
            Cst {
                tokens,
                ast,
                shift,
                file,
            },
            top,
        ));
    }
    let declared = read_elements::declare(csts.iter().map(|(c, _)| c));
    let mut reads = Vec::new();
    let mut refs = Vec::new();
    for (cst, top) in &csts {
        let mut l = Lower {
            b: &mut *out,
            diags: &mut *diags,
            cst,
            scope: *top,
            stmt: ir::Span::NONE,
            declared: &declared,
            reads: &mut reads,
            refs: &mut refs,
            elements: HashMap::new(),
            gap_token: 0,
            gap_end: 0,
        };
        let root = cst.ast.fields(cst.ast.root()).to_vec();
        l.body(*top, &root, None);
        l.comments_until(u32::MAX, true);
    }
    bind::bind(out, &reads, &refs);
}

/// Statement `n` as an `Opaque` of `class` with `code`, its text as written. Comments inside
/// its text go with it.
pub(super) fn opaque(l: &mut Lower<'_, '_, '_>, n: Node, class: ir::OpaqueClass, code: ir::Code) {
    let span = l.span(n);
    let text = l.text(n).to_string();
    l.b.opaque(l.scope, "spectre", class, &text, span);
    l.diags.push(ir::diag::opaque_code(class), span, 0);
    l.diags.push(code, span, 0);
    l.comments_until(span.start + span.len, false);
}

/// Statement `n`, which no reader rule reads: `Unread` (`codes::UNREAD`).
pub(super) fn unread(l: &mut Lower<'_, '_, '_>, n: Node) {
    opaque(l, n, ir::OpaqueClass::Unread, codes::UNREAD);
}

/// Statement `n`'s text: from its first token's first byte to its last token's last byte.
pub(super) fn statement_text<'s>(cst: &Cst<'s>, n: Node) -> &'s str {
    let (start, end) = extent(cst, n);
    &cst.tokens.source()[(start + cst.shift) as usize..(end + cst.shift) as usize]
}

/// The name a `Word`, `Ident` or `Str` field holds (a string's quotes taken off).
pub(super) fn field_name(cst: &Cst<'_>, field: u32) -> String {
    let n = Node(field);
    let ast = &cst.ast;
    match ast.tag(n) {
        Tag::Word => ast.name(Name(ast.rhs(n))).to_string(),
        Tag::Ident => ast.name(Name(ast.lhs(n))).to_string(),
        Tag::Str => {
            let t = cst.tokens.text(ast.main_token(n));
            let inner = t.strip_prefix('"').and_then(|t| t.strip_suffix('"'));
            inner.unwrap_or(t).to_string()
        }
        _ => cst.tokens.text(ast.main_token(n)).to_string(),
    }
}

/// Statement `n`'s byte range in the file's own text (the library shift taken off): its
/// first token to the end of its last line; a block to its closer's line, or, unclosed, to
/// its last body statement's end; an `if` chain to its last branch's.
pub(super) fn extent(cst: &Cst<'_>, n: Node) -> (u32, u32) {
    let ast = &cst.ast;
    let tokens = &cst.tokens;
    let start = tokens.start(ast.main_token(n));
    let mut end = line_end(tokens, ast.main_token(n));
    let mut todo = vec![n];
    // Bounded by the node count: each statement of the (acyclic) block tree is taken once.
    while let Some(at) = todo.pop() {
        if !is_block(ast.tag(at)) {
            continue;
        }
        if let Some(next) = ast.next(at) {
            end = end.max(line_end(tokens, ast.main_token(next)));
            todo.push(next);
        }
        match ast.closer(at) {
            Some(c) => end = end.max(line_end(tokens, ast.main_token(c))),
            None => {
                for &s in ast.body(at) {
                    end = end.max(line_end(tokens, ast.main_token(Node(s))));
                    todo.push(Node(s));
                }
            }
        }
    }
    (
        start.saturating_sub(cst.shift),
        end.saturating_sub(cst.shift),
    )
}

// ---- private ----

/// One statement's dispatch: by its CST tag, and for a `Card` by its head word.
fn statement(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    if !placed(l.b.design().scope(l.scope).kind, ast.tag(n)) {
        return opaque(l, n, ir::OpaqueClass::Unread, codes::PLACEMENT);
    }
    match ast.tag(n) {
        Tag::Title => read_structure::title(l, n),
        Tag::Param => read_structure::parameters(l, n),
        Tag::Subckt => read_structure::subckt(l, n),
        Tag::If => read_structure::if_chain(l, n),
        Tag::Func => read_structure::func(l, n),
        Tag::Include => read_structure::include(l, n),
        Tag::Global => read_structure::global(l, n),
        Tag::Statistics => read_structure::statistics(l, n),
        Tag::Region => read_structure::region(l, n),
        Tag::Model => read_elements::model(l, n),
        Tag::Instance => read_elements::instance(l, n),
        Tag::Analysis => read_run::analysis(l, n),
        Tag::Options => read_run::options(l, n),
        Tag::Save => read_run::save(l, n),
        Tag::Card if l.token(n) == "simulator" => read_structure::simulator(l, n),
        _ => unread(l, n),
    }
}

/// Whether this reader takes a statement of `tag` in a scope of `kind` (a subcircuit holds
/// parameters, subcircuits, `if`s, models and instances; a branch the same but subcircuits; a
/// loop analyses; a statement no rule reads stands anywhere as text).
fn placed(kind: ir::ScopeKind, tag: Tag) -> bool {
    let circuit = matches!(tag, Tag::Param | Tag::If | Tag::Model | Tag::Instance);
    match kind {
        ir::ScopeKind::File => true,
        ir::ScopeKind::Subckt => circuit || tag == Tag::Subckt,
        ir::ScopeKind::Branch => circuit,
        ir::ScopeKind::Loop => tag == Tag::Analysis,
        _ => false,
    }
}

/// Whether a statement of `tag` has a body this reader walks for its extent.
fn is_block(tag: Tag) -> bool {
    matches!(
        tag,
        Tag::Subckt
            | Tag::Section
            | Tag::Library
            | Tag::If
            | Tag::Else
            | Tag::Region
            | Tag::Statistics
            | Tag::Process
            | Tag::Func
            | Tag::Analysis
    )
}

/// The end of the last token of the statement that starts at token `first`.
fn line_end(tokens: &Tokens<'_>, first: u32) -> u32 {
    use crate::syntax::token::Tag as Tok;
    let tags = tokens.tags();
    let mut end = tokens.end(first);
    let mut i = first as usize + 1;
    while i < tags.len() && tags[i] != Tok::EndOfStatement {
        end = end.max(tokens.end(i as u32));
        i += 1;
    }
    end
}

/// The first `"…"` string of `text`, its quotes taken off.
fn first_string(text: &str) -> Option<String> {
    let rest = text.trim_start().strip_prefix('"')?;
    rest.find('"').map(|end| rest[..end].to_string())
}
