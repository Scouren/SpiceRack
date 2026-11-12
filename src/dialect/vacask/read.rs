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
//! Contents: `Cst`, `Lower` (and its methods), `references`, `read`, `unread`, `opaque`,
//! `statement_text` (crate-private)

use std::collections::HashSet;

use super::codes;
use super::{read_elements, read_models, read_run, read_structure};
use crate::dialect::{Reference, Sources};
use crate::ir;
use crate::syntax::ast::{Ast, Node};
use crate::syntax::token::Tokens;

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
    /// Every name a top-level `ground` declares, in every file (VD cir-nodes).
    pub(super) grounds: &'a HashSet<String>,
    /// Every path a `load` or an `include` names, as written (an `embed` of one is `Private`).
    pub(super) named_files: &'a HashSet<String>,
    /// Each parameter read and the scope it is read from, bound by [`bind`] at the end.
    pub(super) reads: &'a mut Vec<(ir::ExprId, ir::ScopeId)>,
    /// The next token whose leading gap has not been searched for comments.
    pub(super) gap_token: u32,
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

    /// Lowers the statements `body` into `scope`, comments of the gaps between them included,
    /// then restores the current scope.
    pub(super) fn body(&mut self, scope: ir::ScopeId, body: &[u32]) {
        let _ = (scope, body);
        todo!("S4.0: Lower::body")
    }

    /// Pushes a `Comment` item for each `//` or `/* */` comment in the gaps before byte `until`
    /// that no earlier call took (a comment inside an opaque statement's text is skipped with it).
    pub(super) fn comments_until(&mut self, until: u32, keep: bool) {
        let _ = (until, keep);
        todo!("S4.0: Lower::comments_until")
    }
}

/// The files `text` names, in text order: `include` paths and `load`s of Verilog-A sources.
pub(super) fn references(text: &str) -> Vec<Reference> {
    let _ = text;
    todo!("S4.0: read::references")
}

/// Reads every file of `src` into `out`: entries in `src`'s role, the rest as libraries.
pub(super) fn read(src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
    let _ = (src, out, diags, codes::UNREAD);
    todo!("S4.0: read::read")
}

/// Statement `n` as an `Opaque` of `class` with the code `code` (a note or a refusal), its text
/// as written.
pub(super) fn opaque(l: &mut Lower<'_, '_, '_>, n: Node, class: ir::OpaqueClass, code: ir::Code) {
    let _ = (l, n, class, code);
    todo!("S4.0: read::opaque")
}

/// Statement `n` as an `Unread` opaque: the stub of every area not written yet.
pub(super) fn unread(l: &mut Lower<'_, '_, '_>, n: Node) {
    opaque(l, n, ir::OpaqueClass::Unread, codes::UNREAD);
}

/// Statement `n`'s text: from its first token's first byte to its last token's last byte.
pub(super) fn statement_text<'s>(cst: &Cst<'s>, n: Node) -> &'s str {
    let _ = (cst, n);
    todo!("S4.0: read::statement_text")
}

// ---- private ----

/// One statement's dispatch: by its CST tag, and for a `Card` by its head word.
fn statement(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (
        l,
        n,
        read_structure::FEATURES,
        read_elements::FEATURES,
        read_models::FEATURES,
        read_run::FEATURES,
    );
    todo!("S4.0: read::statement")
}

/// Statement `n`'s byte range in the file's own text (the library shift taken off).
fn statement_bytes(cst: &Cst<'_>, n: Node) -> (u32, u32) {
    let _ = (cst, n);
    todo!("S4.0: read::statement_bytes")
}

/// Binds every recorded parameter read by VACASK's rule.
fn bind(b: &mut ir::Builder<'_>, reads: &[(ir::ExprId, ir::ScopeId)], diags: &mut ir::Diags) {
    let _ = (b, reads, diags);
    todo!("S4.0: read::bind")
}
