//! The reader's spine: sources → a CST per file (`syntax::tokenize` and `syntax::parse` with
//! ngspice's lexicon, used as a private concrete syntax tree) → declare (which definition of a
//! name wins in its scope) → lower (one walk in source order on an explicit stack, one dispatch
//! on the statement's tag to an area function) → bind (each parameter read and user call bound
//! by ngspice's rule: the nearest enclosing scope; ruling R3, never `ir::link`).
//!
//! Law L1 is asserted per statement: each one an area function is handed pushes at least one
//! item into its scope (an `Opaque` with a coded note when nothing reads it), or is framing
//! (`.end`; line 1 of a library file, which has none). Comment lines (`*`) between statements
//! are `Comment` items of the scope they stand in; other comment forms are not kept.
//!
//! A library file (every file after the entries, or every entry of a library read) is read
//! behind one empty line, so that its line 1 is no title (the kit rule of `syntax::resolve`);
//! its spans are shifted back.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §3; `REWRITE-PLAN.md` §4.2 (L1), §4.3; `rewrite/RULINGS.md`
//! R3; ngspice-45 `inpcom.c` (`gnd`, case), `inp_sort_params` (a `.param` may read one defined
//! later).
//!
//! Contents: `pub(super) struct Lower<'a, 'd>`, `pub(super) struct Stmt`,
//! `pub(super) enum Def`,
//! `pub(super) fn read(src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags)`,
//! `pub(super) fn references(text: &str) -> Vec<Reference>`,
//! `Lower::{open, opaque, opaque_text, deck_script, script_index, deck_has_card_analysis, unread, note, framing, span, raw, text, rest, keyword, expr, number, wins,
//! class, subckt_formals, refer, pending, rescope, rescope_name, measure_reads, include_path}`; the CST reads are `cst.rs`

use std::collections::HashSet;

use super::bind::{RowRef, bind};
use super::codes;
use super::cst::{body, extent, joined, line, next, rest, unquote};
use super::declare::{self, Class, Declared, Script, declare};
use super::expr_read::Tree;
use super::name::{self, Folder};
use super::number;
use super::{
    read_control, read_elements, read_models, read_outputs, read_run, read_sources, read_structure,
};
use crate::dialect::{Reference, Sources};
use crate::ir;
use crate::syntax::ast::{Ast, Node, Tag};
use crate::syntax::lexicon::Lexicon;
use crate::syntax::token::Tokens;
use crate::syntax::{Lang, parse, tokenize};

/// The lowering state of one file, handed to every area function.
pub(super) struct Lower<'a, 'd> {
    /// The append door.
    pub(super) b: &'a mut ir::Builder<'d>,
    /// Diagnostics.
    pub(super) diags: &'a mut ir::Diags,
    /// Case folding across the whole read.
    pub(super) names: &'a mut Folder,
    /// The CST of this file.
    pub(super) ast: &'a Ast<'a>,
    /// Its tokens.
    pub(super) toks: &'a Tokens<'a>,
    /// Every source (an include resolves against them).
    pub(super) src: &'a Sources,
    /// This file's path in `src`.
    pub(super) path: &'a str,
    /// This file in the design.
    pub(super) file: ir::FileId,
    /// Its role.
    pub(super) role: ir::Role,
    pending: &'a mut Vec<(ir::ExprId, ir::ScopeId)>,
    rows: &'a mut Vec<RowRef>,
    measured: &'a mut Vec<(usize, usize)>,
    declared: &'a Declared,
    script: &'a Script,
    /// `(scope, element name)` of every element card so far (folded).
    elements: &'a mut HashSet<(ir::ScopeId, String)>,
    /// Bytes read before the file's own text (1 for a library file).
    shift: u32,
    frames: Vec<Frame<'a>>,
    framed: bool,
}

/// A statement as an area function gets it: its CST node, the IR scope its items go into, and
/// the CST block holding it (the declare step's scope key).
#[derive(Clone, Copy, Debug)]
pub(super) struct Stmt {
    /// The statement's node.
    pub(super) node: Node,
    /// Where its items go.
    pub(super) scope: ir::ScopeId,
    /// The root, `.subckt`, `.lib` section or branch node it stands in.
    pub(super) container: Node,
}

/// What a name is defined as, for the declare step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Def {
    /// `.param`, and a subcircuit's head parameters.
    Param,
    /// `.func`.
    Func,
    /// `.subckt`.
    Subckt,
    /// `.lib name` … `.endl`.
    Section,
}

/// A body still to lower: its statements, the next one, and the bytes that close it.
struct Frame<'a> {
    scope: ir::ScopeId,
    items: &'a [u32],
    at: usize,
    container: Node,
    /// The byte range of the line that ends the body (a closer or the next branch's head).
    close: Option<(u32, u32)>,
}

/// Every file of `src` into `out`, in `src`'s order; then the bind step.
pub(super) fn read(src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
    assert!(!src.files().is_empty(), "ngspice read: a file to read");
    let lex = lexicon();
    let roles: Vec<ir::Role> = (0..src.files().len())
        .map(|k| {
            if k < src.entries().len() {
                src.form()
            } else {
                ir::Role::Library
            }
        })
        .collect();
    // a library file is read behind one empty line (its line 1 is no title)
    let texts: Vec<String> = src
        .files()
        .iter()
        .zip(&roles)
        .map(|((_, t), r)| {
            if *r == ir::Role::Library {
                format!("\n{t}")
            } else {
                t.clone()
            }
        })
        .collect();
    let toks: Vec<Tokens<'_>> = texts.iter().map(|t| tokenize(t, lex)).collect();
    let asts: Vec<_> = toks.iter().map(|t| parse(t, lex)).collect();
    let mut names = Folder::default();
    let mut pending = Vec::new();
    let mut rows = Vec::new();
    let mut measured = Vec::new();
    let mut elements = HashSet::new();
    for (k, (path, _)) in src.files().iter().enumerate() {
        let (file, top) = out.file(path, roles[k]);
        let declared = declare(&asts[k], &toks[k]);
        // ponytail: one file's script; a deck's includes inlined when a fixture needs them
        let script = declare::script(&asts[k], &toks[k]);
        let mut lw = Lower {
            b: &mut *out,
            diags: &mut *diags,
            names: &mut names,
            ast: &asts[k],
            toks: &toks[k],
            src,
            path,
            file,
            role: roles[k],
            pending: &mut pending,
            rows: &mut rows,
            measured: &mut measured,
            declared: &declared,
            script: &script,
            elements: &mut elements,
            shift: u32::from(roles[k] == ir::Role::Library),
            frames: Vec::new(),
            framed: false,
        };
        lower(&mut lw, top);
    }
    bind(out, &pending, &measured, &rows);
    // checks that need each element's bound model
    super::read_elements::bound(out.design(), diags);
}

/// The files `text` names, in text order: each `.include` path and `.lib` call's file.
pub(super) fn references(text: &str) -> Vec<Reference> {
    let lex = lexicon();
    let toks = tokenize(text, lex);
    let ast = parse(&toks, lex);
    let mut out = Vec::new();
    let mut stack: Vec<Node> = ast
        .fields(ast.root())
        .iter()
        .rev()
        .map(|&n| Node(n))
        .collect();
    // Bounded: each statement is visited once.
    while let Some(n) = stack.pop() {
        match ast.tag(n) {
            Tag::Include | Tag::LibCall => {
                if let Some(&p) = ast.fields(n).first() {
                    out.push(Reference::File(unquote(&joined(&toks, &ast, Node(p)))));
                }
            }
            _ => {
                stack.extend(next(&ast, n));
                stack.extend(body(&ast, n).iter().rev().map(|&s| Node(s)));
            }
        }
    }
    out
}

impl<'a, 'd> Lower<'a, 'd> {
    /// Schedules `body` (statements of `container`) into `scope`, after the statement being
    /// lowered and before its next sibling; `close` is the node of the line that ends it.
    pub(super) fn open(
        &mut self,
        scope: ir::ScopeId,
        body: &'a [u32],
        container: Node,
        close: Option<Node>,
    ) {
        let close = close.map(|c| {
            let (a, b) = line(self.toks, self.ast.main_token(c));
            (self.toks.start(a), self.toks.end(b))
        });
        self.frames.push(Frame {
            scope,
            items: body,
            at: 0,
            container,
            close,
        });
    }

    /// Pushes `text` at `span` into `scope` as an `Opaque` of `class`, with the note `code`: a
    /// statement with no CST node (a `*#` line).
    pub(super) fn opaque_text(
        &mut self,
        scope: ir::ScopeId,
        text: &str,
        span: ir::Span,
        class: ir::OpaqueClass,
        code: ir::Code,
    ) {
        self.b.opaque(scope, "ngspice", class, text, span);
        self.diags.push(code, span, 0);
    }

    /// The file's script statements in file order (R49): each `.control` … `.endc` block as
    /// [`Lower::raw`] gives it, each `*#` line outside one as written.
    pub(super) fn deck_script(&self) -> &[String] {
        &self.script.text
    }

    /// The position of `st` (a `.control` block) in [`Lower::deck_script`].
    #[allow(dead_code, reason = "the control area calls it (S3.4)")]
    pub(super) fn script_index(&self, st: Stmt) -> usize {
        let at = self.toks.start(self.ast.main_token(st.node));
        self.script
            .at
            .binary_search(&at)
            .expect("ngspice read: a .control block is a script statement")
    }

    /// Whether a dot card of the file runs an analysis (`.op`, `.dc`, `.ac`, `.tran`, `.noise`,
    /// `.tf`, `.sens`, `.pz`, `.disto`, `.sp`, `.pss`), in any body, outside the blocks.
    #[allow(dead_code, reason = "the control area calls it (S3.4)")]
    pub(super) fn deck_has_card_analysis(&self) -> bool {
        self.script.card_analysis
    }

    /// Pushes `st` as an `Opaque` of `class`, its whole text, with the note `code`.
    pub(super) fn opaque(&mut self, st: Stmt, class: ir::OpaqueClass, code: ir::Code) {
        let text = self.raw(st.node);
        let span = self.span(st.node);
        self.b.opaque(st.scope, "ngspice", class, &text, span);
        self.diags.push(code, span, 0);
    }

    /// [`Lower::opaque`] of class `Unread` with the note `P1001`.
    pub(super) fn unread(&mut self, st: Stmt) {
        self.opaque(st, ir::OpaqueClass::Unread, codes::UNREAD);
    }

    /// A note at `st`.
    pub(super) fn note(&mut self, st: Stmt, code: ir::Code) {
        let span = self.span(st.node);
        self.diags.push(code, span, 0);
    }

    /// Marks the statement being lowered as framing: it has no item (`.end`, a library's
    /// line 1).
    pub(super) fn framing(&mut self) {
        self.framed = true;
    }

    /// The span of a statement: its first token to the end of its last line (a block's closer).
    pub(super) fn span(&self, n: Node) -> ir::Span {
        let (a, b) = extent(self.ast, self.toks, n);
        let start = self.toks.start(a).saturating_sub(self.shift);
        let end = self.toks.end(b).saturating_sub(self.shift);
        ir::Span {
            file: self.file,
            start,
            len: end.saturating_sub(start),
        }
    }

    /// A statement's text as written, continuation lines and a block's body included.
    pub(super) fn raw(&self, n: Node) -> String {
        let (a, b) = extent(self.ast, self.toks, n);
        let src = self.toks.source();
        src[self.toks.start(a) as usize..self.toks.end(b) as usize].to_string()
    }

    /// A field node's tokens, a blank where the source has a gap (comments and continuations
    /// read as one blank).
    pub(super) fn text(&self, n: Node) -> String {
        joined(self.toks, self.ast, n)
    }

    /// A statement's head line after its first token, one blank per gap.
    pub(super) fn rest(&self, n: Node) -> String {
        rest(self.toks, self.ast, n)
    }

    /// The lowercased text of a statement's head token (`.param`, `r1`).
    pub(super) fn keyword(&self, n: Node) -> String {
        self.toks.text(self.ast.main_token(n)).to_ascii_lowercase()
    }

    /// Appends `tree` for an item in `scope`, gives its reading's notes, and returns its root.
    pub(super) fn expr(&mut self, st: Stmt, tree: &Tree, scope: ir::ScopeId) -> ir::ExprId {
        for &code in tree.notes() {
            self.note(st, code);
        }
        tree.emit(self.b, self.names, scope, self.pending)
    }

    /// Appends a card number (`number::Scan`'s literal and fit) and returns its root: a long one
    /// as its digits (A18).
    pub(super) fn number(&mut self, lit: ir::Literal, fit: &number::Fit) -> ir::ExprId {
        fit.build(self.b, lit)
    }

    /// Whether the definition `(def, name)` of field `k` of `st` is the one ngspice keeps in
    /// its block (the declare step).
    pub(super) fn wins(&self, st: Stmt, def: Def, name: &str, k: usize) -> bool {
        self.declared.wins(st.container, def, name, st.node, k)
    }

    /// The class of `text` read in `block` (R14, `declare.rs`), `formals` shadowing.
    pub(super) fn class(&self, block: Node, text: &str, formals: &[String]) -> Class {
        self.declared.class(block, text, formals)
    }

    /// Queues slot `k` (in `RowFacts::map_refs` order) of a pushed `item` to be bound in the
    /// bind step to the `kind` definition named `name` (as pushed: folded) nearest `scope`
    /// outward, the reader's rule (R3); left unresolved when none is found (R4). The one way an
    /// area binds a row's reference (a master or model defined later, or in a later file of
    /// the same scope chain): `Builder::bind_ref` in `bind`.
    #[allow(dead_code, reason = "the element and model areas call it (S3.1, S3.2)")]
    pub(super) fn refer(
        &mut self,
        item: ir::ItemRef,
        k: u32,
        scope: ir::ScopeId,
        kind: ir::DefKind,
        name: ir::NameId,
    ) {
        assert!(!name.is_none(), "refer: a name");
        self.rows.push(RowRef {
            item,
            k,
            scope,
            kind,
            name,
        });
    }

    /// Marks the parameter reads queued since `mark` as reads inside a `.meas … param=`: the bind
    /// step binds each to a measure of its name first, then to a parameter (ngspice-45
    /// `measure.c` `do_measure`: every other measure runs first and enters its result with
    /// `nupa_add_param`, so a measure, even a later one, hides a `.param` of its name).
    #[allow(dead_code, reason = "the output area calls it (S3.3b)")]
    pub(super) fn measure_reads(&mut self, mark: usize) {
        assert!(
            mark <= self.pending.len(),
            "measure_reads: a mark from before"
        );
        self.measured.push((mark, self.pending.len()));
    }

    /// The head parameter names (folded, `name::key`) of the `.subckt name` ngspice binds from
    /// `st`'s block: the nearest block outward in this file, else a top-level `.subckt` of an
    /// earlier file of the read; `None` when none is visible.
    #[allow(dead_code, reason = "the element area calls it (S3.1a)")]
    pub(super) fn subckt_formals(&self, st: Stmt, name: &str) -> Option<Vec<String>> {
        if let Some(f) = self.declared.formals(st.container, name) {
            return Some(f.to_vec());
        }
        let d = self.b.design();
        let key = name::key(name);
        let def = d.definitions().iter().find(|def| {
            def.kind == ir::DefKind::Subckt
                && d.scope(def.scope).kind == ir::ScopeKind::File
                && name::key(d.str(def.name)) == key
        })?;
        let s: ir::item::Subckt = d.row(def.item.index);
        Some(
            d.params(s.params)
                .iter()
                .map(|&(n, _)| name::key(d.str(n)))
                .collect(),
        )
    }

    /// How many parameter reads and calls wait for the bind step (a mark for [`Lower::rescope`]).
    pub(super) fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Makes the reads queued since `mark` bind from `scope` (a head default reads from its
    /// subcircuit's body, A10).
    pub(super) fn rescope(&mut self, mark: usize, scope: ir::ScopeId) {
        assert!(mark <= self.pending.len(), "rescope: a mark from before");
        for p in &mut self.pending[mark..] {
            p.1 = scope;
        }
    }

    /// Makes the parameter reads of `name` queued in `from..to` bind from `scope`: a head
    /// default's read of its own name is the enclosing scope's (ngspice runs outer.cir,
    /// selfplus.cir: `t={t*2}` reads the outer `t`).
    pub(super) fn rescope_name(&mut self, from: usize, to: usize, name: &str, scope: ir::ScopeId) {
        assert!(
            from <= to && to <= self.pending.len(),
            "rescope_name: marks from before"
        );
        for p in &mut self.pending[from..to] {
            let own = matches!(self.b.design().expr(p.0), ir::Expr::Param(r) if self.b.design().str(r.name) == name);
            if own {
                p.1 = scope;
            }
        }
    }

    /// The path an include names, as the design's file table names it: resolved against this
    /// file when the sources hold that file, else as written (with a `R1003` note at `st`).
    pub(super) fn include_path(&mut self, st: Stmt, written: &str) -> String {
        let resolved = Reference::File(written.to_string()).resolve(self.path);
        if self.src.has(&resolved) {
            return resolved;
        }
        self.note(st, codes::MISSING_FILE);
        written.to_string()
    }
}

// ---- private ----

/// ngspice's lexicon (the `syntax` front end's row).
fn lexicon() -> &'static Lexicon {
    Lang::NGSPICE
        .lexicon()
        .expect("the ngspice lexicon is built in")
}

/// Lowers one file: the root's statements, then every body an area function opens, in source
/// order. Asserts L1 at each statement.
fn lower(lw: &mut Lower<'_, '_>, top: ir::ScopeId) {
    let root = lw.ast.fields(lw.ast.root());
    lw.open(top, root, lw.ast.root(), None);
    let mut cursor = 0u32;
    let (mut statements, mut accounted) = (0usize, 0usize);
    // Bounded: each step takes one statement or closes one frame.
    while let Some(f) = lw.frames.last_mut() {
        let Some(&n) = f.items.get(f.at) else {
            let f = lw.frames.pop().expect("a frame is open");
            let end = f.close.map_or(lw.toks.source().len() as u32, |c| c.0);
            comments(lw, f.scope, cursor, end);
            cursor = f.close.map_or(end, |c| c.1);
            continue;
        };
        f.at += 1;
        let st = Stmt {
            node: Node(n),
            scope: f.scope,
            container: f.container,
        };
        let first = lw.toks.start(lw.ast.main_token(st.node));
        comments(lw, st.scope, cursor, first);
        let depth = lw.frames.len();
        let before = lw.b.design().items(st.scope).len();
        lw.framed = false;
        dispatch(lw, st);
        statements += 1;
        let pushed = lw.b.design().items(st.scope).len() > before;
        assert!(
            pushed || lw.framed,
            "L1: statement {n} ({:?}) left no item",
            lw.ast.tag(st.node)
        );
        accounted += 1;
        let (a, b) = if lw.frames.len() > depth {
            line(lw.toks, lw.ast.main_token(st.node))
        } else {
            extent(lw.ast, lw.toks, st.node)
        };
        debug_assert!(a <= b, "a statement has a token");
        cursor = cursor.max(lw.toks.end(b));
    }
    assert_eq!(
        statements, accounted,
        "L1: every statement in is accounted for"
    );
}

/// The one dispatch: a statement's tag → its area function. An element card whose name the
/// scope has already is no element: ngspice bails out ("device already exists", run dup.cir).
fn dispatch(lw: &mut Lower<'_, '_>, st: Stmt) {
    if lw.ast.tag(st.node) == Tag::Instance {
        let name = name::key(&lw.keyword(st.node));
        if !lw.elements.insert((st.scope, name)) {
            return lw.opaque(
                st,
                ir::OpaqueClass::InvalidInSource,
                codes::DUPLICATE_ELEMENT,
            );
        }
    }
    match lw.ast.tag(st.node) {
        Tag::Title => read_structure::title(lw, st),
        Tag::End => read_structure::end(lw, st),
        Tag::Include | Tag::LibCall => read_structure::include(lw, st),
        Tag::Subckt => read_structure::subckt(lw, st),
        Tag::Section => read_structure::section(lw, st),
        Tag::If => read_structure::if_chain(lw, st),
        Tag::Param => read_structure::param(lw, st),
        Tag::Func => read_structure::func(lw, st),
        Tag::Global => read_structure::global(lw, st),
        Tag::Instance if source_letter(lw, st) => read_sources::source(lw, st),
        Tag::Instance => read_elements::element(lw, st),
        Tag::Model => read_models::model(lw, st),
        Tag::Analysis | Tag::Options | Tag::Temp | Tag::Ic | Tag::Nodeset | Tag::Step => {
            read_run::card(lw, st);
        }
        Tag::Save | Tag::Print | Tag::Measure => read_outputs::card(lw, st),
        Tag::Control => read_control::control(lw, st),
        Tag::Card => read_structure::card(lw, st),
        Tag::Else
        | Tag::Verbatim
        | Tag::Region
        | Tag::Data
        | Tag::Line
        | Tag::Library
        | Tag::Command
        | Tag::Statistics
        | Tag::Process => lw.unread(st),
        // not statements: the parser never lists these as one; total anyway
        Tag::Root
        | Tag::Word
        | Tag::Assign
        | Tag::Group
        | Tag::Seq
        | Tag::Delimited
        | Tag::Num
        | Tag::Ident
        | Tag::Str
        | Tag::Unary
        | Tag::Binary
        | Tag::Ternary
        | Tag::Call
        | Tag::Probe
        | Tag::Index
        | Tag::Paren
        | Tag::Vector
        | Tag::List
        | Tag::Bad => lw.unread(st),
    }
}

/// Whether an element card's letter is an independent source's (`V`, `I`): `read_sources`.
fn source_letter(lw: &Lower<'_, '_>, st: Stmt) -> bool {
    matches!(lw.keyword(st.node).as_bytes().first(), Some(b'v' | b'i'))
}

/// `Comment` items for the `*` lines of bytes `from..to` of the file, into `scope`.
fn comments(lw: &mut Lower<'_, '_>, scope: ir::ScopeId, from: u32, to: u32) {
    if from >= to {
        return;
    }
    let src = lw.toks.source();
    let mut at = from as usize;
    for line in src[from as usize..to as usize].split_inclusive('\n') {
        let start = at;
        at += line.len();
        let body = line.trim_end_matches(['\n', '\r']);
        let Some(text) = body.trim_start().strip_prefix('*') else {
            continue;
        };
        // a gap line that is no whole line (the tail of a statement's line) is no comment
        if start != 0 && src.as_bytes()[start - 1] != b'\n' {
            continue;
        }
        let shift = lw.shift;
        let span = ir::Span {
            file: lw.file,
            start: (start as u32).saturating_sub(shift),
            len: body.len() as u32,
        };
        if text.starts_with('#') {
            // a `*#` line is a script command (R49): the control area's
            let index = lw
                .script
                .at
                .binary_search(&(start as u32))
                .expect("ngspice read: a *# line is a script statement");
            let before = lw.b.design().items(scope).len();
            read_control::script_line(lw, scope, index, span);
            assert!(
                lw.b.design().items(scope).len() > before,
                "L1: a *# line left no item"
            );
            continue;
        }
        lw.b.comment(scope, text, span);
    }
}
