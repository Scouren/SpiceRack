//! The append-only door: a reader's or builder's calls in, new rows out. Readers, the Python and
//! Rust builders and the IR text parser all build through it; nothing here reads source text.
//!
//! An empty list is always `List::EMPTY` (start 0), whichever constructor made it, so equal
//! designs compare equal; `check` reports any other empty range. Every value goes through its
//! constructor once, at this edge: a reader's number is the
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
//! ```
//! use spicerack_ir::{BinOp, Design, Diags, ExprId, Role, Span, check, link};
//!
//! let mut design = Design::new();
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
//! b.param(body, "w", half, Span::NONE);
//! // circuit and run methods (`build_circuit.rs`, `build_run.rs`) push elements and analyses.
//! let mut diags = Diags::new();
//! link(&mut design, &mut diags); // the neutral rule binds `rval` and `r`
//! check(&design, &mut diags);
//! assert!(diags.is_empty());
//! # let _ = branches;
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
//! - expressions: in `build_expr.rs`
//! - detail rows: `Builder::detail<D: DetailRow>(&mut self, D) -> u32`
//! - items: `Builder::push<R: ItemRow>(&mut self, ScopeId, R, Span) -> ItemRef`,
//!   `Builder::push_scoped<R: ItemRow>(…) -> (ItemRef, ScopeId)`, `Builder::title`,
//!   `Builder::comment`, `Builder::include`, `Builder::global`, `Builder::load`,
//!   `Builder::opaque`, `Builder::subckt`, `Builder::section`, `Builder::if_`, `Builder::param`,
//!   `Builder::func`
//! - definitions: `Builder::define(&mut self, DefKind, NameId, ScopeId, ItemRef) -> DefId`,
//!   `Builder::bind_ref(&mut self, ItemRef, u32, DefId)`,
//!   `Builder::resolve(&mut self, ExprId, DefId)`

use crate::design::{Design, DetailRow, ItemRow};
use crate::expr::ExprTag;
#[cfg(doc)]
use crate::expr::Literal;
#[cfg(doc)]
use crate::id::Ref;
use crate::id::{
    BranchList, DEFS_MAX, DefId, ExprId, ExprList, FILES_MAX, FileId, ItemRef, List, NameId,
    NameList, POOL_MAX, ParamList, ROWS_MAX, SCOPES_MAX, ScopeId, TextId,
};
use crate::item::{
    Branch, Comment, DefKind, Func as FuncRow, Global, If, Include, ItemKind, Load, LoadKind,
    Opaque, OpaqueClass, Param, Role, ScopeKind, Section, Subckt, Title,
};
use crate::scope::{Def, File, Scope};
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
        self.design
    }

    // ---- files and pools ----

    /// Adds a file and its top scope. Asserts `path` is not already a file and [`crate::id::FILES_MAX`].
    pub fn file(&mut self, path: &str, role: Role) -> (FileId, ScopeId) {
        assert!(
            self.design.find_file(path).is_none(),
            "a file path is unique"
        );
        assert!(
            self.design.files.len() < FILES_MAX,
            "at most FILES_MAX files"
        );
        let path = self.name(path);
        let file = FileId(self.design.files.len() as u16);
        let top = self.new_scope(ScopeKind::File, ScopeId::NONE, ItemRef::NONE, file);
        self.design.files.push(File { path, role, top });
        (file, top)
    }

    /// Interns a name (exact bytes). Asserts it is non-empty.
    pub fn name(&mut self, name: &str) -> NameId {
        self.design.names.intern(name)
    }

    /// Appends uninterned text (may be empty).
    pub fn text(&mut self, text: &str) -> TextId {
        self.design.texts.push(text)
    }

    /// Interns each name and appends the list.
    pub fn names(&mut self, names: &[&str]) -> NameList {
        if names.is_empty() {
            return NameList::EMPTY;
        }
        let start = self.design.name_pool.len();
        assert!(
            start + names.len() <= POOL_MAX,
            "a pool holds at most POOL_MAX elements"
        );
        for name in names {
            let id = self.name(name);
            self.design.name_pool.push(id);
        }
        List::range(start as u32, names.len() as u32)
    }

    /// Appends a list of names already interned. Asserts none is `NONE`.
    pub fn name_ids(&mut self, names: &[NameId]) -> NameList {
        let count = self.design.names.len();
        assert!(
            names.iter().all(|n| n.index() < count),
            "every name is an interned id"
        );
        pooled(&mut self.design.name_pool, names)
    }

    /// Appends a list of expressions. Asserts each is an existing row.
    pub fn exprs(&mut self, exprs: &[ExprId]) -> ExprList {
        let count = self.design.exprs.tags.len();
        assert!(
            exprs.iter().all(|e| e.index() < count),
            "every expression exists"
        );
        pooled(&mut self.design.expr_pool, exprs)
    }

    /// Interns each name and appends the pairs. Asserts the names are distinct; a value may be
    /// `NONE` only where the receiving field says so.
    pub fn params(&mut self, params: &[(&str, ExprId)]) -> ParamList {
        if params.is_empty() {
            return ParamList::EMPTY;
        }
        let count = self.design.exprs.tags.len();
        let start = self.design.param_pool.len();
        assert!(
            start + params.len() <= POOL_MAX,
            "a pool holds at most POOL_MAX elements"
        );
        for (k, (name, value)) in params.iter().enumerate() {
            assert!(
                value.is_none() || value.index() < count,
                "every value exists"
            );
            assert!(
                params[..k].iter().all(|(n, _)| n != name),
                "parameter names in one list are distinct"
            );
            let id = self.name(name);
            self.design.param_pool.push((id, *value));
        }
        List::range(start as u32, params.len() as u32)
    }

    // ---- detail rows ----

    /// Appends one detail or side row to `D`'s table (an owner's typed push calls this, then
    /// pushes the header that indexes it) and returns its index. Asserts the row's handles are
    /// in range and `ROWS_MAX`.
    pub fn detail<D: DetailRow>(&mut self, row: D) -> u32 {
        let rows = D::rows_mut(self.design);
        assert!(rows.len() < ROWS_MAX, "a table holds at most ROWS_MAX rows");
        rows.push(row);
        (rows.len() - 1) as u32
    }

    // ---- items ----

    /// Appends any item row to `scope` (the low tier every kind's push uses). Asserts `scope`
    /// and the table's capacity; the row's own fields are validated by `check` (through its
    /// `RowFacts`), not here. Does not define anything.
    pub fn push<R: ItemRow>(&mut self, scope: ScopeId, row: R, span: Span) -> ItemRef {
        self.design.classes.take();
        assert!(
            scope.index() < self.design.scopes.len(),
            "a scope id in range"
        );
        let index = R::table_mut(self.design).push(row, span);
        let item = ItemRef {
            kind: R::KIND,
            index,
        };
        self.design.scope_items[scope.index()].push(item);
        item
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
        let file = self.design.scope(scope).file;
        let body_scope = self.new_scope(body, scope, ItemRef::NONE, file);
        let item = self.push(scope, row(body_scope), span);
        self.design.scopes[body_scope.index()].owner = item;
        (item, body_scope)
    }

    /// The deck's title. Asserts `scope` is a `Deck` file's top scope.
    pub fn title(&mut self, scope: ScopeId, text: &str, span: Span) -> ItemRef {
        let s = self.design.scope(scope);
        let deck = s.kind == ScopeKind::File && self.design.file(s.file).role == Role::Deck;
        assert!(deck, "a title is in a Deck file's top scope");
        let text = self.text(text);
        self.push(scope, Title { text }, span)
    }

    /// A comment statement, its marker stripped.
    pub fn comment(&mut self, scope: ScopeId, text: &str, span: Span) -> ItemRef {
        let text = self.text(text);
        self.push(scope, Comment { text }, span)
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
        let path = self.name(path);
        let section = section.map_or(NameId::NONE, |s| self.name(s));
        self.push(scope, Include { path, section }, span)
    }

    /// Global nets. Asserts none is ground.
    pub fn global(&mut self, scope: ScopeId, nets: &[&str], span: Span) -> ItemRef {
        assert!(
            nets.iter().all(|n| *n != "0"),
            "a global net is never ground"
        );
        let nets = self.names(nets);
        self.push(scope, Global { nets }, span)
    }

    /// A module load; define its modules with [`Builder::define`] (`DefKind::Module`, item the
    /// returned load).
    pub fn load(&mut self, scope: ScopeId, kind: LoadKind, path: &str, span: Span) -> ItemRef {
        let path = self.name(path);
        self.push(scope, Load { kind, path }, span)
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
        let dialect = self.name(dialect);
        let text = self.text(text);
        self.push(
            scope,
            Opaque {
                dialect,
                class,
                text,
            },
            span,
        )
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
        let name = self.name(name);
        let ports = self.names(ports);
        let params = self.params(params);
        let row = |body| Subckt {
            name,
            ports,
            params,
            body,
        };
        let (item, body) = self.push_scoped(scope, ScopeKind::Subckt, row, span);
        self.define(DefKind::Subckt, name, scope, item);
        for k in 0..params.len() {
            let (param, _) = self.design.params(params)[k];
            self.define(DefKind::Param, param, body, item);
        }
        (item, body)
    }

    /// A library section and its empty body; defines the section.
    pub fn section(
        &mut self,
        scope: ScopeId,
        name: &str,
        library: Option<&str>,
        span: Span,
    ) -> (ItemRef, ScopeId) {
        let name = self.name(name);
        let library = library.map_or(NameId::NONE, |l| self.name(l));
        let row = |body| Section {
            name,
            library,
            body,
        };
        let (item, body) = self.push_scoped(scope, ScopeKind::Section, row, span);
        self.define(DefKind::Section, name, scope, item);
        (item, body)
    }

    /// A conditional with one branch per condition, each with an empty body scope; `NONE` as
    /// the last condition is the `else`. Branch `k`'s scope is
    /// `design().branches(list)[k].body`. Asserts at least one condition and `NONE` only last.
    pub fn if_(&mut self, scope: ScopeId, conds: &[ExprId], span: Span) -> (ItemRef, BranchList) {
        assert!(!conds.is_empty(), "an if has at least one branch");
        let (last, rest) = conds.split_last().expect("non-empty");
        assert!(
            rest.iter().all(|c| !c.is_none()),
            "only the last branch may be the else"
        );
        for c in conds.iter().filter(|c| !c.is_none()) {
            self.assert_expr(*c);
        }
        let _ = last;
        // the If row's index is known before it is pushed: its branch scopes name it as owner
        let owner = ItemRef {
            kind: ItemKind::If,
            index: self.design.ifs.len() as u32,
        };
        let file = self.design.scope(scope).file;
        let start = self.design.branch_pool.len();
        assert!(
            start + conds.len() <= POOL_MAX,
            "a pool holds at most POOL_MAX elements"
        );
        for &cond in conds {
            let body = self.new_scope(ScopeKind::Branch, scope, owner, file);
            self.design.branch_pool.push(Branch { cond, body });
        }
        let branches = List::range(start as u32, conds.len() as u32);
        let item = self.push(scope, If { branches }, span);
        assert!(item == owner, "the If row lands at its predicted index");
        (item, branches)
    }

    /// A parameter definition; defines it in `scope`. Asserts `value` is not `NONE`.
    pub fn param(&mut self, scope: ScopeId, name: &str, value: ExprId, span: Span) -> ItemRef {
        self.assert_expr(value);
        let name = self.name(name);
        let item = self.push(scope, Param { name, value }, span);
        self.define(DefKind::Param, name, scope, item);
        item
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
        self.assert_expr(body);
        let name = self.name(name);
        let args = self.names(args);
        let item = self.push(scope, FuncRow { name, args, body }, span);
        let def = self.define(DefKind::Func, name, scope, item);
        self.bind_formals(body, args, def);
        item
    }

    // ---- definitions ----

    /// Sets the `k`-th reference of `item` (in its `RowFacts::map_refs` order) to `def`: a reader
    /// binding a row's reference to a definition it pushed later, by its own simulator's rule
    /// (`resolve` covers expressions). Asserts `k` is in range and `def` has the kind that slot
    /// expects.
    pub fn bind_ref(&mut self, item: ItemRef, k: u32, def: DefId) {
        let target = self.design.def(def).kind;
        let mut kinds = Vec::new();
        self.design
            .editor()
            .map_refs(item, &mut |_, kind| kinds.push(kind));
        assert!(
            (k as usize) < kinds.len(),
            "bind_ref names one of the item's references"
        );
        assert!(
            kinds[k as usize] == target,
            "bind_ref binds a definition of the kind its slot expects"
        );
        let mut slot = 0;
        self.design.editor().map_refs(item, &mut |r, _| {
            if slot == k {
                r.def = def;
            }
            slot += 1;
        });
    }

    /// Adds a definition. Asserts no definition of `kind` named `name` exists in `scope` yet:
    /// the reader keeps one winner by its simulator's rule before it calls.
    pub fn define(&mut self, kind: DefKind, name: NameId, scope: ScopeId, item: ItemRef) -> DefId {
        self.design.classes.take();
        assert!(
            scope.index() < self.design.scopes.len(),
            "a scope id in range"
        );
        let key = (scope, kind, name);
        assert!(
            !self.design.def_index.contains_key(&key),
            "one definition per scope, kind and name"
        );
        assert!(
            self.design.defs.len() < DEFS_MAX,
            "at most DEFS_MAX definitions"
        );
        let def = DefId(self.design.defs.len() as u32);
        self.design.defs.push(Def {
            kind,
            name,
            scope,
            item,
        });
        self.design.def_index.insert(key, def);
        def
    }

    /// Binds the [`Ref`] of a `Param` or `UserCall` expression to `def`. Asserts the tag, and
    /// that `def` has the name and a kind the expression accepts: `Param` (or `Measure`, for a
    /// measure reading an earlier measure, A7) for a `Param` tag, `Func` for a `UserCall`.
    pub fn resolve(&mut self, expr: ExprId, def: DefId) {
        self.design.classes.take();
        self.assert_expr(expr);
        let target = self.design.def(def);
        let i = expr.index();
        let x = &mut self.design.exprs;
        match x.tags[i] {
            ExprTag::Param => {
                assert!(
                    target.name == NameId(x.a[i]),
                    "resolve binds a definition of the same name"
                );
                let kind_ok = matches!(target.kind, DefKind::Param | DefKind::Measure);
                assert!(kind_ok, "a parameter read binds a Param or a Measure");
                x.b[i] = def.0;
            }
            ExprTag::UserCall => {
                let call = &mut x.calls[x.a[i] as usize];
                assert!(
                    target.name == call.func.name,
                    "resolve binds a definition of the same name"
                );
                assert!(target.kind == DefKind::Func, "a user call binds a Func");
                call.func.def = def;
            }
            ExprTag::Num
            | ExprTag::Str
            | ExprTag::Call
            | ExprTag::Unary
            | ExprTag::Binary
            | ExprTag::Ternary
            | ExprTag::V
            | ExprTag::I
            | ExprTag::Time
            | ExprTag::Temp
            | ExprTag::Freq
            | ExprTag::TStep
            | ExprTag::TStop
            | ExprTag::Tnom
            | ExprTag::Draw
            | ExprTag::List => panic!("resolve binds a Param or UserCall expression"),
        }
    }
}

// ---- private ----

impl Builder<'_> {
    /// A new empty scope. Asserts `SCOPES_MAX`.
    fn new_scope(
        &mut self,
        kind: ScopeKind,
        parent: ScopeId,
        owner: ItemRef,
        file: FileId,
    ) -> ScopeId {
        assert!(
            self.design.scopes.len() < SCOPES_MAX,
            "at most SCOPES_MAX scopes"
        );
        self.design.scopes.push(Scope {
            kind,
            parent,
            owner,
            file,
        });
        self.design.scope_items.push(Vec::new());
        ScopeId((self.design.scopes.len() - 1) as u32)
    }

    /// Binds every unresolved `Param` read of a formal argument in `body` to the function `def`.
    /// Explicit stack (C11); each row of the body is visited once.
    fn bind_formals(&mut self, body: ExprId, args: NameList, def: DefId) {
        let formals: Vec<NameId> = self.design.names(args).to_vec();
        let mut stack = vec![body];
        let mut kids = Vec::new();
        // the rows of this body only: a set, not a vector sized by the design (O(body), not
        // O(expressions so far), per function)
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = stack.pop() {
            if !seen.insert(e) {
                continue;
            }
            let i = e.index();
            let x = &mut self.design.exprs;
            if x.tags[i] == ExprTag::Param
                && DefId(x.b[i]).is_none()
                && formals.contains(&NameId(x.a[i]))
            {
                x.b[i] = def.0;
            }
            kids.clear();
            self.design.children(e, &mut kids);
            stack.extend_from_slice(&kids);
        }
    }
}

/// Appends `items` to `pool` and returns their range. Asserts `POOL_MAX`.
fn pooled<T: Copy>(pool: &mut Vec<T>, items: &[T]) -> List<T> {
    if items.is_empty() {
        return List::EMPTY; // the one empty list: equality does not depend on how it was made
    }
    let start = pool.len();
    assert!(
        start + items.len() <= POOL_MAX,
        "a pool holds at most POOL_MAX elements"
    );
    pool.extend_from_slice(items);
    List::range(start as u32, items.len() as u32)
}
