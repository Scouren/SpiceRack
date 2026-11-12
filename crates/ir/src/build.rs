//! The append-only door: a reader's or builder's calls in, new rows out. Readers, the Python and
//! Rust builders and the IR text parser all build through it; nothing here reads source text.
//!
//! Every value goes through its constructor once, at this edge: a reader's number is the
//! [`Literal`] it read (digits in, no floating arithmetic in a reader, Addendum A3), checked
//! against the literal's invariants; a built number (`num`) is refused unless finite (the NaN
//! gate, C9) and becomes its shortest literal; names unless non-empty, handles unless in range, capacities asserted
//! (C10). These are asserts: a reader must turn a bad number into a diagnostic and an `Opaque`
//! before it calls here. Every item push takes the scope it goes into and its [`Span`] (last),
//! appends the row, appends the item to the scope's list, and returns the [`ItemRef`].
//! A push of a defining item (`subckt`, `section`, `param`, `func`, the owners' `model`) also
//! appends its definition; modules and anything else are defined with [`Builder::define`].
//! A dialect reader binds references by its own simulator's rule: [`Builder::resolve`] for an
//! expression, a resolved [`Ref`] in the row it pushes. A builder that has no such rule leaves
//! them unresolved and calls `link` (the neutral rule, A4) when it is done.
//!
//! ```ignore
//! let mut b = design.builder();
//! let (_, top) = b.file("rc.sp", Role::Deck);
//! b.title(top, "rc divider", Span::NONE);
//! let r = b.num(1e3);
//! b.param(top, "rval", r, Span::NONE);
//! let rval = b.param_ref("rval");
//! let (_, body) = b.subckt(top, "div", &["in", "out", "gnd"], &[("r", rval)], Span::NONE);
//! let half = { let two = b.num(2.0); let r = b.param_ref("r"); b.binary(BinOp::Div, r, two) };
//! let ok = { let t = b.temp(); let lim = b.num(125.0); b.binary(BinOp::Lt, t, lim) };
//! let (_, branches) = b.if_(body, &[ok, ExprId::NONE], Span::NONE);
//! // circuit and run methods (`build_circuit.rs`, `build_run.rs`) push elements and analyses.
//! ```
//!
//! Cites: `rewrite/IR-KERNEL.md` C2, C6–C10, C13; `REWRITE-PLAN.md` §2.4 (caller code first).
//!
//! Contents:
//! - `pub struct Builder<'d>`, `Builder::design(&self) -> &Design`
//! - files and pools: `Builder::file(&mut self, &str, Role) -> (FileId, ScopeId)`,
//!   `Builder::name(&mut self, &str) -> NameId`, `Builder::text(&mut self, &str) -> TextId`,
//!   `Builder::names(&mut self, &[&str]) -> NameList`,
//!   `Builder::name_ids(&mut self, &[NameId]) -> NameList`,
//!   `Builder::exprs(&mut self, &[ExprId]) -> ExprList`,
//!   `Builder::params(&mut self, &[(&str, ExprId)]) -> ParamList`
//! - expressions: `Builder::literal`, `Builder::num`, `Builder::string`, `Builder::param_ref`, `Builder::call`,
//!   `Builder::user_call`, `Builder::unary`, `Builder::binary`, `Builder::ternary`, `Builder::v`,
//!   `Builder::i`, `Builder::time`, `Builder::temp`, `Builder::freq`, `Builder::tstep`,
//!   `Builder::tstop`, `Builder::tnom`, `Builder::draw`, `Builder::list` (each `-> ExprId`)
//! - items: `Builder::push<R: ItemRow>(&mut self, ScopeId, R, Span) -> ItemRef`,
//!   `Builder::push_scoped<R: ItemRow>(…) -> (ItemRef, ScopeId)`, `Builder::title`,
//!   `Builder::comment`, `Builder::include`, `Builder::global`, `Builder::load`,
//!   `Builder::opaque`, `Builder::subckt`, `Builder::section`, `Builder::if_`, `Builder::param`,
//!   `Builder::func`
//! - definitions: `Builder::define(&mut self, DefKind, NameId, ScopeId, ItemRef) -> DefId`,
//!   `Builder::resolve(&mut self, ExprId, DefId)`

use crate::design::{Design, ItemRow};
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, Func, Literal, UnOp};
#[cfg(doc)]
use crate::id::Ref;
use crate::id::{
    BranchList, DefId, ExprId, ExprList, FileId, ItemRef, NameId, NameList, ParamList, ScopeId,
    TextId,
};
use crate::item::{DefKind, LoadKind, OpaqueClass, Role, ScopeKind};
use crate::span::Span;

/// Appends to one design. Holds the design mutably for its lifetime; drop it to read the design
/// as a writer.
#[derive(Debug)]
pub struct Builder<'d> {
    pub(crate) design: &'d mut Design,
}

impl<'d> Builder<'d> {
    /// The design built so far, read only (a reader looks up names and definitions here).
    pub fn design(&self) -> &Design {
        todo!()
    }

    // ---- files and pools ----

    /// Adds a file and its top scope. Asserts `path` is not already a file and [`crate::id::FILES_MAX`].
    pub fn file(&mut self, path: &str, role: Role) -> (FileId, ScopeId) {
        todo!()
    }

    /// Interns a name (exact bytes). Asserts it is non-empty.
    pub fn name(&mut self, name: &str) -> NameId {
        todo!()
    }

    /// Appends uninterned text (may be empty).
    pub fn text(&mut self, text: &str) -> TextId {
        todo!()
    }

    /// Interns each name and appends the list.
    pub fn names(&mut self, names: &[&str]) -> NameList {
        todo!()
    }

    /// Appends a list of names already interned. Asserts none is `NONE`.
    pub fn name_ids(&mut self, names: &[NameId]) -> NameList {
        todo!()
    }

    /// Appends a list of expressions. Asserts each is an existing row.
    pub fn exprs(&mut self, exprs: &[ExprId]) -> ExprList {
        todo!()
    }

    /// Interns each name and appends the pairs. Asserts the names are distinct; a value may be
    /// `NONE` only where the receiving field says so.
    pub fn params(&mut self, params: &[(&str, ExprId)]) -> ParamList {
        todo!()
    }

    // ---- expressions ----

    /// A number as a reader read it. Asserts `Literal::is_valid` (a reader rounds a literal
    /// past 18 significant digits, with a note, and folds `1e3k` into `Exponent`, before it
    /// calls).
    pub fn literal(&mut self, literal: Literal) -> ExprId {
        todo!()
    }

    /// A built number (Python, Schemify, passes), stored as `Literal::from_f64(value)`. Asserts
    /// it is finite (the NaN gate, C9).
    pub fn num(&mut self, value: f64) -> ExprId {
        todo!()
    }

    /// A string value.
    pub fn string(&mut self, value: &str) -> ExprId {
        todo!()
    }

    /// A parameter read, unresolved. (`param` is the item push.)
    pub fn param_ref(&mut self, name: &str) -> ExprId {
        todo!()
    }

    /// A built-in function call. Asserts the argument count is within `func.arity()`.
    pub fn call(&mut self, func: Func, args: &[ExprId]) -> ExprId {
        todo!()
    }

    /// A user-function call, unresolved.
    pub fn user_call(&mut self, name: &str, args: &[ExprId]) -> ExprId {
        todo!()
    }

    /// A unary operator.
    pub fn unary(&mut self, op: UnOp, operand: ExprId) -> ExprId {
        todo!()
    }

    /// A binary operator.
    pub fn binary(&mut self, op: BinOp, left: ExprId, right: ExprId) -> ExprId {
        todo!()
    }

    /// `cond ? then : other`.
    pub fn ternary(&mut self, cond: ExprId, then: ExprId, other: ExprId) -> ExprId {
        todo!()
    }

    /// `v(net)` (`reference` `None`: ground) or `v(net, reference)`.
    pub fn v(&mut self, net: &str, reference: Option<&str>) -> ExprId {
        todo!()
    }

    /// `i(element)`: the element's branch current, positive into its first terminal.
    pub fn i(&mut self, element: &str) -> ExprId {
        todo!()
    }

    /// The simulation time.
    pub fn time(&mut self) -> ExprId {
        todo!()
    }

    /// The circuit temperature.
    pub fn temp(&mut self) -> ExprId {
        todo!()
    }

    /// The analysis frequency.
    pub fn freq(&mut self) -> ExprId {
        todo!()
    }

    /// The transient step leaf.
    pub fn tstep(&mut self) -> ExprId {
        todo!()
    }

    /// The transient stop leaf.
    pub fn tstop(&mut self) -> ExprId {
        todo!()
    }

    /// The nominal temperature leaf.
    pub fn tnom(&mut self) -> ExprId {
        todo!()
    }

    /// A statistical draw: when it samples and how often are the source simulator's rule, made
    /// explicit. Asserts the argument count is within `dist.arity()`.
    pub fn draw(&mut self, dist: Dist, when: DrawWhen, per: DrawPer, args: &[ExprId]) -> ExprId {
        todo!()
    }

    /// A vector value.
    pub fn list(&mut self, elements: &[ExprId]) -> ExprId {
        todo!()
    }

    // ---- items ----

    /// Appends any item row to `scope` (the low tier every kind's push uses). Asserts the row's
    /// handles are in range and its numbers finite; does not define anything.
    pub fn push<R: ItemRow>(&mut self, scope: ScopeId, row: R, span: Span) -> ItemRef {
        todo!()
    }

    /// Appends an item that opens a scope: creates the body scope (`body` kind, parent `scope`,
    /// owner the new item), passes its id to `row` to build the row, and pushes it.
    pub fn push_scoped<R: ItemRow>(
        &mut self,
        scope: ScopeId,
        body: ScopeKind,
        row: impl FnOnce(ScopeId) -> R,
        span: Span,
    ) -> (ItemRef, ScopeId) {
        todo!()
    }

    /// The deck's title. Asserts `scope` is a `Deck` file's top scope.
    pub fn title(&mut self, scope: ScopeId, text: &str, span: Span) -> ItemRef {
        todo!()
    }

    /// A comment statement, its marker stripped.
    pub fn comment(&mut self, scope: ScopeId, text: &str, span: Span) -> ItemRef {
        todo!()
    }

    /// An include of `path` (a design file's path, or as spelled when missing), of one section
    /// or (`None`) the whole file.
    pub fn include(
        &mut self,
        scope: ScopeId,
        path: &str,
        section: Option<&str>,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// Global nets. Asserts none is ground.
    pub fn global(&mut self, scope: ScopeId, nets: &[&str], span: Span) -> ItemRef {
        todo!()
    }

    /// A module load; define its modules with [`Builder::define`] (`DefKind::Module`, item the
    /// returned load).
    pub fn load(&mut self, scope: ScopeId, kind: LoadKind, path: &str, span: Span) -> ItemRef {
        todo!()
    }

    /// A statement with no node, as `dialect`'s text. Asserts `dialect` is non-empty.
    pub fn opaque(
        &mut self,
        scope: ScopeId,
        dialect: &str,
        class: OpaqueClass,
        text: &str,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A subcircuit and its empty body; defines the subcircuit and each head parameter.
    pub fn subckt(
        &mut self,
        scope: ScopeId,
        name: &str,
        ports: &[&str],
        params: &[(&str, ExprId)],
        span: Span,
    ) -> (ItemRef, ScopeId) {
        todo!()
    }

    /// A library section and its empty body; defines the section.
    pub fn section(
        &mut self,
        scope: ScopeId,
        name: &str,
        library: Option<&str>,
        span: Span,
    ) -> (ItemRef, ScopeId) {
        todo!()
    }

    /// A conditional with one branch per condition, each with an empty body scope; `NONE` as
    /// the last condition is the `else`. Branch `k`'s scope is
    /// `design().branches(list)[k].body`. Asserts at least one condition and `NONE` only last.
    pub fn if_(&mut self, scope: ScopeId, conds: &[ExprId], span: Span) -> (ItemRef, BranchList) {
        todo!()
    }

    /// A parameter definition; defines it in `scope`. Asserts `value` is not `NONE`.
    pub fn param(&mut self, scope: ScopeId, name: &str, value: ExprId, span: Span) -> ItemRef {
        todo!()
    }

    /// A user function; defines it. Formal arguments read in `body` are bound to the new
    /// definition by this call.
    pub fn func(
        &mut self,
        scope: ScopeId,
        name: &str,
        args: &[&str],
        body: ExprId,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    // ---- definitions ----

    /// Adds a definition. Asserts no definition of `kind` named `name` exists in `scope` yet:
    /// the reader keeps one winner by its simulator's rule before it calls.
    pub fn define(&mut self, kind: DefKind, name: NameId, scope: ScopeId, item: ItemRef) -> DefId {
        todo!()
    }

    /// Binds the [`Ref`] of a `Param` or `UserCall` expression to `def`. Asserts the tag, and
    /// that `def` has the name and the kind (`Param`/`Func`) the expression expects.
    pub fn resolve(&mut self, expr: ExprId, def: DefId) {
        todo!()
    }
}
