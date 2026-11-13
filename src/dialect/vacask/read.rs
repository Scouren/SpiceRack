//! The reader's spine: sources → one concrete syntax tree per file (`syntax::tokenize` and
//! `syntax::parse` with VACASK's lexicon, a private tree) → declare (what every file defines
//! that binding needs first: ground names, the files loads and includes name) → lower (one walk
//! in source order, one dispatch on each statement's CST tag and head word to an area
//! function) → bind (every parameter read, by VACASK's rule). Every branch on which work
//! happens is here; an area function pushes rows or one `Opaque` and returns.
//!
//! VACASK's binding rule (RUN n1, m1; VD cir-hier "Scope"): the own subcircuit's scope, then
//! the top; never an enclosing subcircuit. `@if` branches and sections are not scopes of their
//! own for it. Names are exact (RUN n4); a forward reference is legal (RUN n5).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.1; `DIALECT-NGSPICE.md` §3 (the same four steps).
//!
//! Contents: `Cst`, `Lower` (`Lower::span`, `Lower::text`, `Lower::start`, `Lower::body`,
//! `Lower::comments_until`, `Lower::bind_master_later`, `Lower::element_here`), `references`,
//! `read`, `unread`, `opaque`,
//! `statement_text`, `field_name`, `unclosed` (crate-private)

use std::collections::{HashMap, HashSet};

use super::bind;
use super::codes;
use super::fold;
use super::{read_elements, read_models, read_run, read_sources, read_structure};
use crate::dialect::{Reference, Sources};
use crate::ir;
use crate::syntax::ast::{Ast, Name, Node, Tag};
use crate::syntax::token::{Tag as Tok, Tokens};
use crate::syntax::{Lang, parse, tokenize};

/// One file's concrete syntax tree. A library file is tokenized behind one empty line, since
/// VACASK's lexicon reads line 1 as a title and a library has none (S 14.12): `shift` is that
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
    /// Every name a top-level `ground` declares, in every file (VD cir-nodes).
    pub(super) grounds: &'a HashSet<String>,
    /// Every path a `load` or an `include` names, as written (an `embed` of one is `Private`).
    pub(super) named_files: &'a HashSet<String>,
    /// Every model name declared with a source-like module (`tables::SOURCE_MODULES`): an
    /// instance of one is the sources area's (S4.2), every other instance the devices area's.
    pub(super) source_models: &'a HashSet<String>,
    /// Each parameter read, the scope it is read from and its statement, bound by `bind`.
    pub(super) reads: &'a mut Vec<(ir::ExprId, ir::ScopeId, ir::Span)>,
    /// Each element whose master to bind, bound by `bind::masters` at the end.
    pub(super) masters: &'a mut Vec<bind::Master>,
    /// Every master every file declares (`read_elements::declare`): an instance's kind.
    #[allow(
        dead_code,
        reason = "read_elements (S4.1) and read_sources (S4.2) read it"
    )]
    pub(super) declared: &'a read_elements::Declared,
    /// Each source a `type` alter reaches and the alters that are framing (R42: `fold::folds`).
    #[allow(dead_code, reason = "read_sources (S4.2) and read_run (S4.3) read it")]
    pub(super) folds: &'a fold::Folds,
    /// Each scope's element names and how many of its items they cover (`Lower::element_here`).
    pub(super) elements: HashMap<ir::ScopeId, (usize, HashSet<ir::NameId>)>,
    /// `parameters` entries a subcircuit's head took (their `Assign` nodes).
    pub(super) taken: HashSet<u32>,
    /// The next token whose leading gap has not been searched for comments.
    pub(super) gap_token: u32,
    /// The end of the gaps already searched (a byte of the tokenized text).
    pub(super) gap_end: u32,
}

/// Which area reads an instance statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Area {
    /// `read_sources`: an instance of a model of a source-like module.
    Sources,
    /// `read_elements`: every other instance, a behavioural source (no master) included.
    Devices,
}

impl Lower<'_, '_, '_> {
    /// The span of statement `n` (its first token to its last, a block's closer included).
    pub(super) fn span(&self, n: Node) -> ir::Span {
        let (start, end) = statement_bytes(self.cst, n);
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

    /// Records that element `item` names its master by name (`Element.model`, unresolved):
    /// `bind::masters` binds it after every file is read (a `model` may come later or from an
    /// include), by VACASK's rule, to the definition kind the element's kind takes.
    #[allow(
        dead_code,
        reason = "read_elements (S4.1) and read_sources (S4.2) call it"
    )]
    pub(super) fn bind_master_later(&mut self, item: ir::ItemRef) {
        self.masters.push(bind::Master {
            item,
            scope: self.scope,
            span: self.stmt,
        });
    }

    /// Whether an element named `name` already stands in the current scope (run dup). A scope's
    /// item list only grows, so its names are indexed once, as it grows: a read stays linear in
    /// its instances (a scan of the scope per instance was quadratic: the scaling step).
    pub(super) fn element_here(&mut self, name: &str) -> bool {
        let d = self.b.design();
        let items = d.items(self.scope);
        let (seen, names) = self.elements.entry(self.scope).or_default();
        assert!(*seen <= items.len(), "a scope's item list only grows");
        for at in items[*seen..]
            .iter()
            .filter(|at| at.kind == ir::ItemKind::Element)
        {
            names.insert(d.row::<ir::Element>(at.index).name);
        }
        *seen = items.len();
        let id = d.find_name(name);
        !id.is_none() && names.contains(&id)
    }

    /// The start byte (in the file's own text) of statement `n`.
    pub(super) fn start(&self, n: Node) -> u32 {
        statement_bytes(self.cst, n).0
    }

    /// Lowers the statements `body` into `scope`, comments of the gaps between them included,
    /// then those before byte `until` (a block's closer), then restores the current scope.
    pub(super) fn body(&mut self, scope: ir::ScopeId, body: &[u32], until: Option<u32>) {
        let saved = (self.scope, self.stmt);
        self.scope = scope;
        for &s in body {
            let n = Node(s);
            let (start, end) = statement_bytes(self.cst, n);
            self.comments_until(start, true);
            self.stmt = self.span(n);
            statement(self, n);
            self.comments_until(end, true);
        }
        if let Some(until) = until {
            self.comments_until(until, true);
        }
        (self.scope, self.stmt) = saved;
    }

    /// Pushes a `Comment` item (when `keep`) for each `//` or `/* */` comment in the gaps that
    /// end at or before byte `until` (of the file's own text) and that no earlier call took.
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
            if i < count {
                self.gap_end = self.gap_end.max(tokens.end(i));
            } else {
                self.gap_end = gap_end;
            }
            self.gap_token += 1;
        }
    }

    /// The comments of `source[from..to]` (a gap: blanks, comments, `\` before a newline).
    fn gap_comments(&mut self, from: usize, to: usize) {
        let source = self.cst.tokens.source();
        let gap = &source.as_bytes()[from..to];
        let mut at = 0;
        while at < gap.len() {
            let rest = &gap[at..];
            let (text_from, text_to, next) = if rest.starts_with(b"//") {
                let end = rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
                (at + 2, at + end, at + end)
            } else if rest.starts_with(b"/*") {
                let close = rest
                    .windows(2)
                    .position(|w| w == b"*/")
                    .unwrap_or(rest.len() - 2);
                (at + 2, at + close, at + close + 2)
            } else {
                at += 1;
                continue;
            };
            let text = &source[from + text_from..from + text_to];
            let start = u32::try_from(from + at).expect("a file under 4 GiB") - self.cst.shift;
            let len = u32::try_from(next - at).expect("a short comment");
            let span = ir::Span {
                file: self.cst.file,
                start,
                len,
            };
            // A comment item is one line (`ir::check`): a block comment is one per line.
            for line in text.split('\n') {
                self.b
                    .comment(self.scope, line.strip_suffix('\r').unwrap_or(line), span);
            }
            at = next;
        }
    }
}

/// The files `text` names, in text order: `include` paths and `load`s of Verilog-A sources
/// (a compiled `.osdi` is not followed).
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
            "load" if path.ends_with(".va") => out.push(Reference::VerilogA(path)),
            _ => {}
        }
    }
    out
}

/// Reads every file of `src` into `out`: entries in `src`'s role, the rest as libraries.
pub(super) fn read(src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
    assert!(!src.files().is_empty(), "vacask::read: a file to read");
    let lex = Lang::VACASK.lexicon().expect("VACASK has a lexicon");
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
    let (grounds, named_files, source_models) = declare(&csts);
    let mut reads = Vec::new();
    let mut masters = Vec::new();
    let declared = read_elements::declare(&csts);
    let folds = fold::folds(&csts);
    for (cst, top) in &csts {
        let mut l = Lower {
            b: &mut *out,
            diags: &mut *diags,
            cst,
            scope: *top,
            stmt: ir::Span::NONE,
            grounds: &grounds,
            named_files: &named_files,
            source_models: &source_models,
            reads: &mut reads,
            masters: &mut masters,
            declared: &declared,
            folds: &folds,
            elements: Default::default(),
            taken: HashSet::new(),
            gap_token: 0,
            gap_end: 0,
        };
        let root = cst.ast.fields(cst.ast.root()).to_vec();
        l.body(*top, &root, None);
        l.comments_until(u32::MAX, true);
    }
    let visible = super::visible::Visible::new(out.design());
    for (second, first) in visible.redefinitions(out.design()) {
        diags.push_at(
            read_structure::INCLUDED_TWICE,
            out.design(),
            second,
            first.0,
        );
    }
    bind::reads(out, &visible, &reads, diags);
    bind::masters(out, &visible, &masters, diags);
}

/// Statement `n` as an `Opaque` of `class`, its text as written: the class's `S` note and,
/// when given, `code`. Comments inside its text go with it.
pub(super) fn opaque(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    class: ir::OpaqueClass,
    code: Option<ir::Code>,
) {
    let span = l.span(n);
    let text = l.text(n).to_string();
    l.b.opaque(l.scope, "vacask", class, &text, span);
    l.diags.push(ir::diag::opaque_code(class), span, 0);
    if let Some(code) = code {
        l.diags.push(code, span, 0);
    }
    l.comments_until(span.start + span.len, false);
}

/// Statement `n`, which no reader rule reads: VACASK has no syntax for it (run fx; settle
/// star_deck, star_in_section: "Syntax error, unexpected string."), so it is invalid in source
/// (P2013), except at a library's top outside every section when some include reads that
/// library by `section=`, which VACASK skips (settle star_outside_section,
/// junk_outside_section): Inert (P2014). A library read alone or included whole is the deck
/// case (settle plain_include: VACASK rejects even the `section` line there). Nothing is left `Unread` (the corpus zero gate).
pub(super) fn unread(l: &mut Lower<'_, '_, '_>, n: Node) {
    let design = l.b.design();
    let file = design.file(l.cst.file);
    let included = l.declared.by_section(design.str(file.path));
    let skipped = file.role != ir::Role::Deck && l.scope == file.top && included;
    match skipped {
        true => opaque(l, n, ir::OpaqueClass::Inert, Some(codes::SKIPPED)),
        false => opaque(
            l,
            n,
            ir::OpaqueClass::InvalidInSource,
            Some(codes::NO_SYNTAX),
        ),
    }
}

/// Statement `n`'s text: from its first token's first byte to its last token's last byte.
pub(super) fn statement_text<'s>(cst: &Cst<'s>, n: Node) -> &'s str {
    let (start, end) = statement_bytes(cst, n);
    &cst.tokens.source()[(start + cst.shift) as usize..(end + cst.shift) as usize]
}

/// The name a `Word`, `Ident` or `Str` field holds (a string's quotes and escapes taken off).
pub(super) fn field_name(cst: &Cst<'_>, field: u32) -> String {
    let n = Node(field);
    let ast = &cst.ast;
    match ast.tag(n) {
        Tag::Word => super::name::unquote(ast.name(Name(ast.rhs(n)))),
        Tag::Ident => super::name::unquote(ast.name(Name(ast.lhs(n)))),
        Tag::Str => {
            let t = cst.tokens.text(ast.main_token(n));
            let inner = t.strip_prefix('"').and_then(|t| t.strip_suffix('"'));
            super::expr_read::unescape(inner.unwrap_or(t))
        }
        _ => super::name::unquote(cst.tokens.text(ast.main_token(n))),
    }
}

/// Whether leaf statement `n` leaves a `(`, `[` or `{` open at its end, closes one it did not
/// open, or holds a string a line break ended (the tokenizer joins lines while a bracket is
/// open, as VACASK does: settle unclosed_*; `syntax::scan::string_end` stops at the line).
pub(super) fn unclosed(cst: &Cst<'_>, n: Node) -> bool {
    let tags = cst.tokens.tags();
    let mut depth = [0i32; 3];
    // Bounded by the token count: one step per token of the statement.
    for (i, &tag) in tags.iter().enumerate().skip(cst.ast.main_token(n) as usize) {
        let k = match tag {
            Tok::EndOfStatement => break,
            Tok::LParen | Tok::RParen => 0,
            Tok::LBracket | Tok::RBracket => 1,
            Tok::LBrace | Tok::RBrace => 2,
            Tok::String => {
                let t = cst.tokens.text(i as u32).as_bytes();
                let escapes = t.iter().rev().skip(1).take_while(|&&b| b == b'\\').count();
                // An embed's `<<<MARK … >>>MARK` body is a string token too, and has no quotes.
                let open = t.first() == Some(&b'"') && (t.len() < 2 || t.last() != Some(&b'"'));
                if open || (t.first() == Some(&b'"') && escapes % 2 == 1) {
                    return true;
                }
                continue;
            }
            _ => continue,
        };
        let open = matches!(tag, Tok::LParen | Tok::LBracket | Tok::LBrace);
        depth[k] += if open { 1 } else { -1 };
        if depth[k] < 0 {
            return true;
        }
    }
    depth != [0; 3]
}

// ---- private ----

/// One statement's dispatch: by its CST tag, and for a `Card` by its head word.
fn statement(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    // A leaf, or a subcircuit's head line (`unclosed` stops at the first statement end, so a
    // body is never scanned): `subckt s (a b` with no `)` to the end of the file is VACASK's
    // syntax error (the reader fixtures' bracket mutants, b_redef_sub).
    let checked = !matches!(
        ast.tag(n),
        Tag::Title | Tag::Section | Tag::If | Tag::Else | Tag::Control
    );
    if checked && unclosed(l.cst, n) {
        return opaque(
            l,
            n,
            ir::OpaqueClass::InvalidInSource,
            Some(codes::UNCLOSED),
        );
    }
    match ast.tag(n) {
        Tag::Title => read_structure::title(l, n),
        Tag::Include => read_structure::include(l, n),
        Tag::Section => read_structure::section(l, n),
        Tag::Subckt => read_structure::subckt(l, n),
        Tag::If => read_structure::if_chain(l, n),
        Tag::Param => read_structure::param(l, n),
        Tag::Global => read_structure::global(l, n),
        Tag::Model => read_models::model(l, n),
        Tag::Instance => match instance_area(l.cst, n, l.source_models) {
            Area::Sources => read_sources::source(l, n),
            Area::Devices => read_elements::instance(l, n),
        },
        Tag::Control => read_run::control(l, n),
        Tag::Card => match l.cst.tokens.text(ast.main_token(n)) {
            "load" => read_structure::load(l, n),
            "ground" => read_structure::ground(l, n),
            "embed" => read_structure::embed(l, n),
            _ => unread(l, n),
        },
        _ => unread(l, n),
    }
}

/// The area of instance `n`: by its master's name, against the models declared with a
/// source-like module (in any scope: a name declared both ways is the sources area's).
fn instance_area(cst: &Cst<'_>, n: Node, source_models: &HashSet<String>) -> Area {
    let master = cst.ast.header(n).master;
    match master != crate::syntax::ast::NONE && source_models.contains(&field_name(cst, master)) {
        true => Area::Sources,
        false => Area::Devices,
    }
}

/// Statement `n`'s byte range in the file's own text (the library shift taken off): its
/// first token to the last token of its line, a block to its closer's, an `@if` chain to its
/// last branch's closer.
fn statement_bytes(cst: &Cst<'_>, n: Node) -> (u32, u32) {
    let ast = &cst.ast;
    let tokens = &cst.tokens;
    let start = tokens.start(ast.main_token(n));
    let mut last = n;
    // Bounded by the node count: each step moves to a later branch.
    for _ in 0..ast.len() {
        match ast.tag(last) {
            Tag::If | Tag::Else => match ast.next(last) {
                Some(next) => last = next,
                None => break,
            },
            _ => break,
        }
    }
    let block = matches!(
        ast.tag(last),
        Tag::Subckt | Tag::Section | Tag::If | Tag::Else | Tag::Control
    );
    // Only a block's `rhs` is a body: another tag's may be anything.
    let end_node = match block {
        true => ast.closer(last).unwrap_or(last),
        false => last,
    };
    let end = line_end(tokens, ast.main_token(end_node));
    // A library's leading empty line (its title token) starts before the file's own text.
    (
        start.saturating_sub(cst.shift),
        end.saturating_sub(cst.shift),
    )
}

/// The end of the last token of the statement that starts at token `first`.
fn line_end(tokens: &Tokens<'_>, first: u32) -> u32 {
    let tags = tokens.tags();
    let mut end = tokens.end(first);
    let mut i = first as usize + 1;
    while i < tags.len() && tags[i] != Tok::EndOfStatement {
        end = end.max(tokens.end(i as u32));
        i += 1;
    }
    end
}

/// The ground names, the named files and the source-like model names of every file.
fn declare(csts: &[(Cst<'_>, ir::ScopeId)]) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let mut grounds = HashSet::new();
    let mut named = HashSet::new();
    let mut sources = HashSet::new();
    for (cst, _) in csts {
        let ast = &cst.ast;
        let mut todo: Vec<u32> = ast.fields(ast.root()).to_vec();
        // Bounded: each node is taken once (the statement tree is acyclic).
        while let Some(s) = todo.pop() {
            let n = Node(s);
            let head = cst.tokens.text(ast.main_token(n));
            match (ast.tag(n), head) {
                (Tag::Card, "ground") => {
                    grounds.extend(ast.fields(n).iter().map(|&f| field_name(cst, f)));
                }
                (Tag::Model, _) => {
                    if let [name, module, ..] = ast.fields(n)
                        && super::tables::SOURCE_MODULES
                            .contains(&field_name(cst, *module).as_str())
                    {
                        sources.insert(field_name(cst, *name));
                    }
                }
                (Tag::Card, "load") | (Tag::Include, _) => {
                    if let Some(&f) = ast.fields(n).first() {
                        named.insert(field_name(cst, f));
                    }
                }
                (Tag::Subckt | Tag::Section | Tag::If | Tag::Else, _) => {
                    todo.extend_from_slice(ast.body(n));
                    if let Some(next) = ast.next(n) {
                        todo.push(next.0);
                    }
                }
                _ => {}
            }
        }
    }
    (grounds, named, sources)
}

/// The first `"…"` string of `text`, its quotes taken off.
fn first_string(text: &str) -> Option<String> {
    let rest = text.trim_start().strip_prefix('"')?;
    rest.find('"').map(|end| rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: DIALECT-VACASK §2 and S4-AREAS (an instance of a model of a builtin module or
    /// `tline_ideal` is the sources area's; every other, a behavioural source too, the devices
    /// area's); VD cir-instance (`name (nodes) master params`).
    #[test]
    fn an_instance_routes_by_its_masters_module() {
        let text = "routes\nmodel vs vsource\nmodel res sp_resistor\nv1 (a 0) vs dc=1\n\
                    r1 (a 0) res r=1\nb1 (a 0) v=1\nx1 (a) sub\n";
        let lex = Lang::VACASK.lexicon().expect("a lexicon");
        let tokens = tokenize(text, lex);
        let ast = parse(&tokens, lex);
        let cst = Cst {
            tokens,
            ast,
            shift: 0,
            file: ir::FileId::NONE,
        };
        let sources: HashSet<String> = ["vs".to_string()].into();
        let areas: Vec<(String, Area)> = cst
            .ast
            .fields(cst.ast.root())
            .iter()
            .map(|&s| Node(s))
            .filter(|&n| cst.ast.tag(n) == Tag::Instance)
            .map(|n| {
                (
                    statement_text(&cst, n).to_string(),
                    instance_area(&cst, n, &sources),
                )
            })
            .collect();
        let want = [
            ("v1 (a 0) vs dc=1", Area::Sources),
            ("r1 (a 0) res r=1", Area::Devices),
            ("b1 (a 0) v=1", Area::Devices),
            ("x1 (a) sub", Area::Devices),
        ];
        let got: Vec<(&str, Area)> = areas.iter().map(|(t, a)| (t.as_str(), *a)).collect();
        assert_eq!(got, want);
    }
}
