//! Validity, checked rather than assumed: a design in, one `X` diagnostic per broken invariant
//! out (`check`), and one `U` diagnostic per feature the design uses that a target lacks
//! (`check_legal`). `check` runs after every read and every pass; `check_legal` before every
//! write. A violation is a bug in whoever produced the design, never in the user's deck.
//!
//! Both are total over any table contents (a wrong handle is a diagnostic, not a panic), walk
//! with explicit stacks (C11), and report in table order, so the same design gives the same rows.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, C1–C12; `REWRITE-PLAN.md` §2.2 rule 4.
//!
//! Contents: `pub enum Violation`, `Violation::ALL`, `Violation::COUNT`,
//! `pub fn check(&Design, &mut Diags)`, `pub fn check_legal(&Design, &FeatureSet, &mut Diags)`

use std::collections::HashSet;

use crate::design::{Design, ItemRow};
use crate::diag::{Diags, UNSUPPORTED, violation_code};
use crate::expr::{BinOp, Dist, Expr, ExprTag, Func, UnOp};
use crate::facts::{Handle, RowFacts};
use crate::feature::{ExprWalk, FeatureSet, design_level, item_features};
use crate::id::{DefId, ExprId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, ItemKind, ScopeKind};
use crate::span::Span;

/// One invariant of a well-formed design. Each has its `X` code (`diag::violation_code`); the
/// span is the offending item's (or `Span::NONE` for a non-item row) and `arg` is stated per
/// variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Violation {
    /// A handle in some row, list or pool is neither `NONE` (where `NONE` is allowed) nor in its
    /// table's range, a list's range overruns its pool, or an empty list is not `List::EMPTY`. `arg`: the handle's value.
    HandleRange,
    /// The scope tree is not a forest of file scopes: a cycle, a scope that reaches no `File`
    /// scope, a file scope with a parent, or an item in two lists. `arg`: the scope.
    ScopeTree,
    /// A scope's kind does not match its owner's item kind, or the owner's row does not name it
    /// as its body. `arg`: the scope.
    ScopeOwner,
    /// An expression's child id is not lower than its own. `arg`: the parent expression.
    ExprOrder,
    /// A call, draw or operator has an argument count outside its arity, or a `Ternary`'s pool
    /// pair overruns. `arg`: the expression.
    ExprArity,
    /// An element's terminal count is outside its kind's range. `arg`: the element row.
    TerminalCount,
    /// A detail row is missing for a kind that has one, or present for one that has none.
    /// `arg`: the item row.
    DetailRow,
    /// A resolved `Ref` names a definition of a kind the referring field does not accept, or
    /// whose name differs; also a `Param`-tag expression bound to a `Measure` definition outside
    /// a `Measure` item (A7). `arg`: the definition.
    RefTarget,
    /// A literal's value is not finite (`1e400`). `arg`: the expression.
    NonFinite,
    /// A literal breaks an invariant of `Literal` (mantissa range, `frac`, an `exp10` its form
    /// does not allow, a non-canonical zero). `arg`: the expression.
    LiteralShape,
    /// A name used by a row is empty. `arg`: the name.
    EmptyName,
    /// A `V`, `I`, `Time`, `Freq`, `Ddt` or `Idt` appears in an expression a row lists as a
    /// [`Handle::Expr`] (a field that must be constant in time: a parameter, a model value, an
    /// analysis argument). Under [`Handle::ExprDyn`] they are legal. `arg`: the expression.
    ProbeContext,
    /// Two definitions of one kind and name in one scope. `arg`: the second definition.
    DuplicateDef,
    /// An `Opaque` has no dialect or empty text. `arg`: the opaque row.
    OpaqueEmpty,
    /// A row's field is outside its domain, or two of its fields disagree (an `ic_mode` of
    /// `None` with an `ic` present, an AC phase without a magnitude, port fields on a non-port, a
    /// master named where the kind takes none). Reported by the row's own `RowFacts::check`.
    /// `arg`: the row's index in its kind's table (the span names the item).
    FieldDomain,
}

/// The design under check and where its rows go.
struct Out<'a> {
    design: &'a Design,
    diags: &'a mut Diags,
}

impl Violation {
    /// Every invariant, in declaration order (the order of the `X` codes).
    pub const ALL: [Violation; 15] = [
        Violation::HandleRange,
        Violation::ScopeTree,
        Violation::ScopeOwner,
        Violation::ExprOrder,
        Violation::ExprArity,
        Violation::TerminalCount,
        Violation::DetailRow,
        Violation::RefTarget,
        Violation::NonFinite,
        Violation::LiteralShape,
        Violation::EmptyName,
        Violation::ProbeContext,
        Violation::DuplicateDef,
        Violation::OpaqueEmpty,
        Violation::FieldDomain,
    ];
    /// Invariant count.
    pub const COUNT: usize = Self::ALL.len();
}

/// Appends one `X` row per violation found. Appends nothing for a valid design. O(rows). The
/// kernel checks the scope tree, definitions, expressions and the ranges of every handle a row
/// lists in `RowFacts::handles`; each row's own invariants are its `RowFacts::check` (one
/// exhaustive dispatch over `ItemKind`).
pub fn check(design: &Design, diags: &mut Diags) {
    let mut out = Out { design, diags };
    files_and_scopes(&mut out);
    definitions(&mut out);
    let bad = expression_rows(&mut out);
    items(&mut out, &bad);
}

/// Appends one `UNSUPPORTED` row per (item, feature) for each feature an item uses outside
/// `legal` (span: the item; `arg`: `Feature::index`), and one at `Span::NONE` per feature of the
/// whole design outside it (the case clash, the owners' `design_features`). It appends nothing
/// exactly when `features_used(design).minus(legal)` is empty: these rows are `legalize`'s
/// leftovers (R10). Assumes `check` passed. O(rows).
pub fn check_legal(design: &Design, legal: &FeatureSet, diags: &mut Diags) {
    let mut walk = ExprWalk::new(design);
    for (item, scope) in design.reachable() {
        let mut used = FeatureSet::EMPTY;
        item_features(design, item, scope, &mut walk, &mut used);
        for f in used.minus(*legal).iter() {
            diags.push(UNSUPPORTED, design.span_of(item), f.index() as u32);
        }
    }
    let mut whole = FeatureSet::EMPTY;
    design_level(design, &mut whole);
    for f in whole.minus(*legal).iter() {
        diags.push(UNSUPPORTED, Span::NONE, f.index() as u32);
    }
}

// ---- private ----

impl Out<'_> {
    fn report(&mut self, v: Violation, span: Span, arg: u32) {
        self.diags.push(violation_code(v), span, arg);
    }
}

/// Files, scopes and item lists: handle ranges, the tree's shape, each scope's owner.
fn files_and_scopes(out: &mut Out<'_>) {
    let d = out.design;
    let n_scopes = d.scopes.len();
    for (k, f) in d.files.iter().enumerate() {
        if f.path.index_checked(d.names.len()).is_none() {
            out.report(Violation::HandleRange, Span::NONE, f.path.0);
        }
        let top_ok = f.top.0 < n_scopes as u32
            && d.scopes[f.top.index()].kind == ScopeKind::File
            && d.scopes[f.top.index()].file.0 as usize == k;
        if !top_ok {
            out.report(Violation::ScopeTree, Span::NONE, f.top.0);
        }
    }
    let mut listed: HashSet<ItemRef> = HashSet::new();
    // the scope that first lists each item, for the owner test (one pass, not a search per scope)
    let mut listing: std::collections::HashMap<ItemRef, ScopeId> = std::collections::HashMap::new();
    for (k, items) in d.scope_items.iter().enumerate() {
        for &item in items {
            listing.entry(item).or_insert(ScopeId(k as u32));
        }
    }
    for (k, s) in d.scopes.iter().enumerate() {
        let id = k as u32;
        let parent_ok = s.parent.is_none() || s.parent.index() < n_scopes;
        let owner_ok =
            s.owner == ItemRef::NONE || (s.owner.index as usize) < table_len(d, s.owner.kind);
        if !parent_ok || (s.file.0 as usize) >= d.files.len() || !owner_ok {
            out.report(Violation::HandleRange, Span::NONE, id);
            continue;
        }
        if !reaches_a_file(d, ScopeId(id)) {
            out.report(Violation::ScopeTree, Span::NONE, id);
            continue;
        }
        for &item in &d.scope_items[k] {
            if (item.index as usize) >= table_len(d, item.kind) {
                out.report(Violation::HandleRange, Span::NONE, item.index);
            } else if !listed.insert(item) {
                out.report(Violation::ScopeTree, d.span_of(item), id);
            }
        }
        if s.kind != ScopeKind::File && !owner_matches(d, ScopeId(id), &listing) {
            out.report(Violation::ScopeOwner, Span::NONE, id);
        }
    }
}

/// Whether the parent chain of a valid-ranged scope ends at a file scope with no parent, owner
/// `NONE` and its file's top, without a cycle.
fn reaches_a_file(d: &Design, scope: ScopeId) -> bool {
    let mut at = scope;
    for _ in 0..=d.scopes.len() {
        let s = d.scopes[at.index()];
        if s.kind == ScopeKind::File {
            let file = d.files.get(s.file.index());
            return s.parent.is_none()
                && s.owner == ItemRef::NONE
                && file.is_some_and(|f| f.top == at);
        }
        if s.parent.is_none() || s.parent.index() >= d.scopes.len() {
            return false;
        }
        at = s.parent;
    }
    false
}

/// Whether a non-file scope's owner has the scope's kind, names it as a body, and is listed in
/// the scope's parent.
fn owner_matches(
    d: &Design,
    scope: ScopeId,
    listing: &std::collections::HashMap<ItemRef, ScopeId>,
) -> bool {
    let s = d.scopes[scope.index()];
    let kind_ok = match s.kind {
        ScopeKind::Subckt => s.owner.kind == ItemKind::Subckt,
        ScopeKind::Section => s.owner.kind == ItemKind::Section,
        ScopeKind::Branch => s.owner.kind == ItemKind::If,
        ScopeKind::Loop => s.owner.kind == ItemKind::Loop,
        ScopeKind::File => false,
    };
    if !kind_ok || listing.get(&s.owner) != Some(&s.parent) {
        return false;
    }
    let mut handles = Vec::new();
    for_kind!(s.owner.kind, R => d.row::<R>(s.owner.index).handles(d, &mut handles));
    handles.contains(&Handle::Scope(scope))
}

/// Definitions: handle ranges, one per (scope, kind, name).
fn definitions(out: &mut Out<'_>) {
    let d = out.design;
    let mut seen = HashSet::new();
    for (k, def) in d.defs.iter().enumerate() {
        let item_ok = (def.item.index as usize) < table_len(d, def.item.kind);
        if def.scope.index_checked(d.scopes.len()).is_none()
            || def.name.index_checked(d.names.len()).is_none()
            || !item_ok
        {
            out.report(Violation::HandleRange, Span::NONE, k as u32);
        } else if !seen.insert((def.scope, def.kind, def.name)) {
            out.report(Violation::DuplicateDef, d.span_of(def.item), k as u32);
        }
    }
}

/// Every expression row: handle ranges, child order, arities, literals. Returns the rows that
/// may not be decoded (the item pass skips them).
fn expression_rows(out: &mut Out<'_>) -> Vec<bool> {
    let d = out.design;
    let x = &d.exprs;
    let n = x.tags.len();
    let mut bad = vec![false; n];
    let mut kids = Vec::new();
    for (i, &tag) in x.tags.iter().enumerate() {
        let (sub, a, b) = (x.subs[i] as usize, x.a[i], x.b[i]);
        let names = d.names.len() as u32;
        let in_pool = |start: u32, len: u32| {
            start as usize + len as usize <= d.expr_pool.len() && (len > 0 || start == 0)
        };
        let (range_ok, arity_ok) = match tag {
            ExprTag::Num => (
                match x.nums.get(a as usize) {
                    Some(l) => {
                        if !l.shape_ok() {
                            out.report(Violation::LiteralShape, Span::NONE, i as u32);
                        } else if !l.finite() {
                            out.report(Violation::NonFinite, Span::NONE, i as u32);
                        }
                        true
                    }
                    None => false,
                },
                true,
            ),
            ExprTag::Str | ExprTag::I => (a < names, true),
            ExprTag::Param => (a < names && def_ok(d, DefId(b)), true),
            ExprTag::V => (a < names && (NameId(b).is_none() || b < names), true),
            ExprTag::Call => match Func::ALL.get(sub) {
                Some(f) => (in_pool(a, b), arity(f.arity(), b)),
                None => (false, true),
            },
            ExprTag::Draw => {
                let dist = Dist::ALL.get(sub & 0xF);
                // bits 4-6 hold DrawWhen and DrawPer, whose widths expr.rs asserts; bit 7 is zero
                let shape = sub >> 7 == 0;
                match dist {
                    Some(dist) if shape => (in_pool(a, b), arity(dist.arity(), b)),
                    Some(_) | None => (false, true),
                }
            }
            ExprTag::List => (in_pool(a, b), true),
            ExprTag::UserCall => match x.calls.get(a as usize) {
                Some(c) => (
                    c.func.name.0 < names
                        && def_ok(d, c.func.def)
                        && in_pool(c.args.start, c.args.len),
                    true,
                ),
                None => (false, true),
            },
            ExprTag::Unary => (sub < UnOp::COUNT && (a as usize) < n, true),
            ExprTag::Binary => (
                sub < BinOp::COUNT && (a as usize) < n && (b as usize) < n,
                true,
            ),
            ExprTag::Ternary => ((a as usize) < n, in_pool(b, 2)),
            ExprTag::Time
            | ExprTag::Temp
            | ExprTag::Freq
            | ExprTag::TStep
            | ExprTag::TStop
            | ExprTag::Tnom => (true, true),
        };
        if !range_ok {
            out.report(Violation::HandleRange, Span::NONE, i as u32);
            bad[i] = true;
            continue;
        }
        if !arity_ok {
            out.report(Violation::ExprArity, Span::NONE, i as u32);
            bad[i] = true;
            continue;
        }
        kids.clear();
        d.children(ExprId(i as u32), &mut kids);
        if kids.iter().any(|k| k.index_checked(n).is_none()) {
            out.report(Violation::HandleRange, Span::NONE, i as u32);
            bad[i] = true;
        } else if kids.iter().any(|k| k.0 >= i as u32) {
            out.report(Violation::ExprOrder, Span::NONE, i as u32);
            bad[i] = true;
        }
    }
    bad
}

/// Reachable items: the row's handles, its references, its expressions' contexts and its own
/// invariants (`RowFacts::check`).
fn items(out: &mut Out<'_>, bad: &[bool]) {
    let d = out.design;
    let mut walk = ExprWalk::new(d);
    let mut handles = Vec::new();
    // `map_refs` reaches a row's references through `&mut Design`, and `check` only reads: it
    // reads them from one copy of the design (O(design), once per check; A9)
    let mut scratch = d.clone();
    for (item, scope) in d.reachable() {
        if (item.index as usize) >= table_len(d, item.kind) {
            continue; // reported with the scope's list
        }
        handles.clear();
        let mut refs: Vec<(crate::Ref, DefKind)> = Vec::new();
        for_kind!(item.kind, R => {
            let row: R = d.row(item.index);
            row.handles(d, &mut handles);
            R::map_refs(&mut scratch, item, &mut |r, kind| refs.push((*r, kind)));
            row.check(d, item, scope, out.diags);
        });
        let span = d.span_of(item);
        for (r, kind) in refs {
            if !r.is_resolved() {
                continue;
            }
            if !def_ok(d, r.def) {
                out.report(Violation::HandleRange, span, r.def.0);
            } else if d.defs[r.def.index()].kind != kind || d.defs[r.def.index()].name != r.name {
                out.report(Violation::RefTarget, span, r.def.0);
            }
        }
        for h in handles.clone() {
            handle(out, item, span, h, bad, &mut walk);
        }
    }
}

/// One handle of a reachable item.
fn handle(
    out: &mut Out<'_>,
    item: ItemRef,
    span: Span,
    h: Handle,
    bad: &[bool],
    walk: &mut ExprWalk,
) {
    let d = out.design;
    // a list lies in its pool, and an empty one is `List::EMPTY` (start 0)
    let in_pool = |start: u32, len: u32, pool: usize| {
        start as usize + len as usize <= pool && (len > 0 || start == 0)
    };
    match h {
        Handle::Name(n) => name(out, span, n),
        Handle::Names(l) => {
            if !in_pool(l.start, l.len, d.name_pool.len()) {
                return out.report(Violation::HandleRange, span, l.start);
            }
            for &n in d.names(l) {
                name(out, span, n);
            }
        }
        Handle::Params(l) => {
            if !in_pool(l.start, l.len, d.param_pool.len()) {
                return out.report(Violation::HandleRange, span, l.start);
            }
            for &(n, v) in d.params(l) {
                name(out, span, n);
                expression(out, item, span, v, false, bad, walk);
            }
        }
        Handle::Exprs(l) => {
            if !in_pool(l.start, l.len, d.expr_pool.len()) {
                return out.report(Violation::HandleRange, span, l.start);
            }
            for &e in d.exprs(l) {
                expression(out, item, span, e, false, bad, walk);
            }
        }
        Handle::Expr(e) => expression(out, item, span, e, false, bad, walk),
        Handle::ExprDyn(e) => expression(out, item, span, e, true, bad, walk),
        Handle::Ref(_) => {} // references are reached through `map_refs`
        Handle::Scope(s) => {
            if !s.is_none() && s.index() >= d.scopes.len() {
                out.report(Violation::HandleRange, span, s.0);
            }
        }
        Handle::Text(t) => {
            if !t.is_none() && t.index() >= d.texts.len() {
                out.report(Violation::HandleRange, span, t.0);
            }
        }
        Handle::Branches(l) => {
            if !in_pool(l.start, l.len, d.branch_pool.len()) {
                out.report(Violation::HandleRange, span, l.start);
            }
        }
    }
}

/// A name handle: in range, and non-empty.
fn name(out: &mut Out<'_>, span: Span, n: NameId) {
    if n.is_none() {
        return;
    }
    if n.index() >= out.design.names.len() {
        out.report(Violation::HandleRange, span, n.0);
    } else if out.design.str(n).is_empty() {
        out.report(Violation::EmptyName, span, n.0);
    }
}

/// An expression handle: in range, its references, and (when constant in time) no circuit read.
#[allow(clippy::too_many_arguments)]
fn expression(
    out: &mut Out<'_>,
    item: ItemRef,
    span: Span,
    e: ExprId,
    dynamic: bool,
    bad: &[bool],
    walk: &mut ExprWalk,
) {
    let d = out.design;
    if e.is_none() {
        return;
    }
    if e.index() >= bad.len() {
        return out.report(Violation::HandleRange, span, e.0);
    }
    for &node in walk.tree(d, e) {
        if bad[node.index()] {
            continue;
        }
        match d.expr(node) {
            Expr::V(..) | Expr::I(_) | Expr::Time | Expr::Freq if !dynamic => {
                out.report(Violation::ProbeContext, span, node.0)
            }
            Expr::Call(f, _) if f.reads_circuit() && !dynamic => {
                out.report(Violation::ProbeContext, span, node.0)
            }
            Expr::Param(r) if r.is_resolved() && !param_target_ok(d, item, r) => {
                out.report(Violation::RefTarget, span, r.def.0)
            }
            Expr::UserCall(r, _) if r.is_resolved() && !same(d, r, DefKind::Func) => {
                out.report(Violation::RefTarget, span, r.def.0)
            }
            Expr::Num(_)
            | Expr::Str(_)
            | Expr::Param(_)
            | Expr::Call(..)
            | Expr::UserCall(..)
            | Expr::Unary(..)
            | Expr::Binary(..)
            | Expr::Ternary(..)
            | Expr::V(..)
            | Expr::I(_)
            | Expr::Time
            | Expr::Temp
            | Expr::Freq
            | Expr::TStep
            | Expr::TStop
            | Expr::Tnom
            | Expr::Draw(..)
            | Expr::List(_) => {}
        }
    }
}

/// What a parameter read may bind to: a `Param`; a `Func` whose formal arguments hold the name;
/// a `Measure` only from a `Measure` item (A7).
fn param_target_ok(d: &Design, item: ItemRef, r: crate::Ref) -> bool {
    let def = d.defs[r.def.index()];
    match def.kind {
        DefKind::Param => def.name == r.name,
        DefKind::Measure => def.name == r.name && item.kind == ItemKind::Measure,
        // a formal argument: bound to its function, whose name is not the argument's
        DefKind::Func => {
            def.item.kind == ItemKind::Func
                && d.names(d.row::<crate::item::Func>(def.item.index).args)
                    .contains(&r.name)
        }
        DefKind::Subckt | DefKind::Model | DefKind::Section | DefKind::Module => false,
    }
}

/// Whether a resolved reference names a definition of `kind` with its name.
fn same(d: &Design, r: crate::Ref, kind: DefKind) -> bool {
    let def = d.defs[r.def.index()];
    def.kind == kind && def.name == r.name
}

/// Whether `def` is `NONE` or a definition in range (`HELD` never is: it is parse-internal).
fn def_ok(d: &Design, def: DefId) -> bool {
    def.is_none() || def.index_checked(d.defs.len()).is_some()
}

/// Whether a count lies in an arity range (`max == u8::MAX`: unbounded).
fn arity((min, max): (u8, u8), n: u32) -> bool {
    n >= min as u32 && (max == u8::MAX || n <= max as u32)
}

/// The row count of `kind`'s table.
fn table_len(d: &Design, kind: ItemKind) -> usize {
    for_kind!(kind, R => R::table(d).len())
}
