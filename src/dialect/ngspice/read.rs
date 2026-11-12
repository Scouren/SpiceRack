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
//! `Lower::{open, opaque, unread, note, framing, span, raw, text, rest, keyword, expr, wins,
//! include_path}`; the CST reads are `cst.rs`

use std::collections::HashMap;

use super::codes;
use super::cst::{body, extent, joined, line, next, unquote};
use super::expr_read::Tree;
use super::name::{self, Folder};
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
    winners: &'a HashMap<(u32, Def, String), (u32, usize)>,
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
    todo!()
}

/// The files `text` names, in text order: each `.include` path and `.lib` call's file.
pub(super) fn references(text: &str) -> Vec<Reference> {
    todo!()
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
        let (a, b) = line(self.toks, self.ast.main_token(n));
        let mut out = String::new();
        for k in a + 1..=b {
            if k > a + 1 && self.toks.start(k) > self.toks.end(k - 1) {
                out.push(' ');
            }
            out.push_str(self.toks.text(k));
        }
        out
    }

    /// The lowercased text of a statement's head token (`.param`, `r1`).
    pub(super) fn keyword(&self, n: Node) -> String {
        self.toks.text(self.ast.main_token(n)).to_ascii_lowercase()
    }

    /// Appends `tree` for an item in `scope`, notes a rounded number, and returns its root.
    pub(super) fn expr(&mut self, st: Stmt, tree: &Tree, scope: ir::ScopeId) -> ir::ExprId {
        if tree.rounded() {
            self.note(st, codes::ROUNDED);
        }
        tree.emit(self.b, self.names, scope, self.pending)
    }

    /// Whether the definition `(def, name)` of field `k` of `st` is the one ngspice keeps in
    /// its block (the declare step).
    pub(super) fn wins(&self, st: Stmt, def: Def, name: &str, k: usize) -> bool {
        let key = (st.container.0, def, name::key(name));
        self.winners.get(&key) == Some(&(st.node.0, k))
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

/// The one dispatch: a statement's tag → its area function.
fn dispatch(lw: &mut Lower<'_, '_>, st: Stmt) {
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
        lw.b.comment(scope, text, span);
    }
}

/// Which definition of each `(block, kind, name)` ngspice keeps: the last `.param` (INV 3.01),
/// the first of any other kind (the later ones are `Unread`). A `.param` naming its
/// subcircuit's head parameter keeps none.
fn declare(ast: &Ast<'_>, toks: &Tokens<'_>) -> HashMap<(u32, Def, String), (u32, usize)> {
    let mut win = HashMap::new();
    let mut stack: Vec<Node> = vec![ast.root()];
    // Bounded: each block is pushed once.
    while let Some(container) = stack.pop() {
        let items: &[u32] = if container == ast.root() {
            ast.fields(ast.root())
        } else {
            body(ast, container)
        };
        let head: Vec<String> = if ast.tag(container) == Tag::Subckt {
            read_structure::head_params(ast, toks, container)
        } else {
            Vec::new()
        };
        for &s in items {
            let s = Node(s);
            let mut put = |def: Def, name: &str, k: usize, last: bool| {
                let key = (container.0, def, name::key(name));
                if last || !win.contains_key(&key) {
                    win.insert(key, (s.0, k));
                }
            };
            match ast.tag(s) {
                Tag::Param => {
                    for (k, name) in read_structure::param_names(ast, toks, s) {
                        let clash = head.iter().any(|h| name::key(h) == name::key(&name));
                        put(Def::Param, &name, if clash { usize::MAX } else { k }, true);
                    }
                }
                Tag::Func => {
                    if let Some(name) = read_structure::def_name(ast, toks, s) {
                        put(Def::Func, &name, 0, false);
                    }
                }
                Tag::Subckt | Tag::Section => {
                    let def = if ast.tag(s) == Tag::Subckt {
                        Def::Subckt
                    } else {
                        Def::Section
                    };
                    if let Some(name) = read_structure::def_name(ast, toks, s) {
                        put(def, &name, 0, false);
                    }
                    stack.push(s);
                }
                Tag::If => {
                    let mut b = Some(s);
                    // Bounded: a chain's branches are distinct nodes.
                    for _ in 0..ast.len() {
                        let Some(at) = b else { break };
                        stack.push(at);
                        b = next(ast, at);
                    }
                }
                _ => {}
            }
        }
    }
    win
}

/// The bind step: each pending parameter read and user call, still unresolved, bound to the
/// definition ngspice reads from its scope (the nearest enclosing one: `Design::lookup`).
fn bind(b: &mut ir::Builder<'_>, pending: &[(ir::ExprId, ir::ScopeId)]) {
    for &(e, scope) in pending {
        let (kind, r) = match b.design().expr(e) {
            ir::Expr::Param(r) => (ir::DefKind::Param, r),
            ir::Expr::UserCall(r, _) => (ir::DefKind::Func, r),
            other => unreachable!("only reads and calls are pending: {other:?}"),
        };
        if r.def.is_none() {
            let def = b.design().lookup(scope, kind, r.name);
            if !def.is_none() {
                b.resolve(e, def);
            }
        }
    }
}
