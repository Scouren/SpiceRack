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
//! `pub fn check(&Design, &mut Diags)`, `pub fn check_legal(&Design, &FeatureSet, &mut Diags)`; crate:
//! `pub(crate) struct Out` (`Out::report`), `pub(crate) fn table_len(&Design, ItemKind) -> usize`

use std::collections::HashSet;

use crate::class::ExprClass;
use crate::design::{Design, ItemRow};
use crate::diag::{Diags, UNSUPPORTED, violation_code};
use crate::expr::{BinOp, Dist, Expr, ExprTag, Func, NumForm, UnOp};
use crate::facts::{Handle, RowFacts};
use crate::feature::{ExprWalk, FeatureSet, design_level, item_features};
use crate::id::{DefId, ExprId, ItemRef, NameId};
use crate::item::{DefKind, ItemKind};
use crate::literal;
use crate::span::Span;

/// One invariant of a well-formed design. Each has its `X` code (`diag::violation_code`); the
/// item is the offending one (its span the item's; `ItemRef::NONE` and `Span::NONE` for a non-item
/// row, A25) and `arg` is stated per
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
    /// does not allow, a non-canonical zero), or an `idt` whose initial value is a literal 0 (the
    /// absent one written again, A28; at the item). `arg`: the expression.
    LiteralShape,
    /// A name used by a row is empty. `arg`: the name.
    EmptyName,
    /// A `V`, `I`, `Time`, `Freq`, `Ddt` or `Idt` appears in an expression a row lists as a
    /// [`Handle::Expr`] (a field that must be constant in time: a parameter, a model value, an
    /// analysis argument), or the expression is of class `Circuit` through a parameter or
    /// function it reads (A15 K3; reported only on a design with no other violation, since the
    /// class table reads definitions through their rows). Under [`Handle::ExprDyn`] they are
    /// legal. `arg`: the expression (the probe's row, or the field's root when through a read).
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
    /// Two elements of one name in one scope, of any kinds (A13, R23: a by-name reference
    /// names one element). Reported by the circuit owner's `element::design_check`. `arg`: the
    /// second element's row in the `Element` table.
    DuplicateElement,
    /// Parameter or function definitions that read one another in a cycle (A28; A29: a default
    /// bound to its own formal): invalid on every simulator. Reported once per definition on
    /// the cycle, at its item; what merely reads the cycle is not reported again. `arg`: the
    /// definition.
    DefinitionCycle,
    /// The definition index holds an entry, in a live scope, whose definition's item no live
    /// scope lists (a pass removed or moved an item through `Editor::items_mut`, bypassing
    /// `Editor::remove_item` and `Editor::move_item`), or a reference is bound to a definition
    /// the index does not hold. `arg`: the definition.
    StaleDefinition,
    /// A user call bound to a function whose formals number differently from its arguments
    /// (the lead's ruling on A28's finding b: no simulator we know accepts one). `arg`: the call's
    /// expression.
    CallArity,
    /// A function body whose root is a string or a list (A25 item 4: a body is a number).
    /// `arg`: the root.
    StringOrListBody,
    /// A typed field (a `Handle::Expr` or `Handle::ExprDyn` of a row other than a `Param`'s
    /// value) whose root is a list, or a string where the row's `RowFacts::string_roots` does not
    /// name it (A25 item 4). `arg`: the root.
    StringOrListField,
}

/// The design under check and where its rows go (crate: `check_scopes` reports through it).
pub(crate) struct Out<'a> {
    pub(crate) design: &'a Design,
    pub(crate) diags: &'a mut Diags,
}

impl Violation {
    /// Every invariant, in declaration order (the order of the `X` codes).
    pub const ALL: [Violation; Violation::StringOrListField as usize + 1] = [
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
        Violation::DuplicateElement,
        Violation::DefinitionCycle,
        Violation::StaleDefinition,
        Violation::CallArity,
        Violation::StringOrListBody,
        Violation::StringOrListField,
    ];
    /// Invariant count.
    pub const COUNT: usize = Self::ALL.len();
}

/// Appends one `X` row per violation found. Appends nothing for a valid design. O(rows). The
/// kernel checks the scope tree, definitions, expressions and the ranges of every handle a row
/// lists in `RowFacts::handles`; each row's own invariants are its `RowFacts::check` (one
/// exhaustive dispatch over `ItemKind`).
pub fn check(design: &Design, diags: &mut Diags) {
    let before = diags.len();
    let mut out = Out { design, diags };
    crate::check_scopes::files_and_scopes(&mut out);
    crate::check_scopes::stale_definitions(&mut out);
    definitions(&mut out);
    let bad = expression_rows(&mut out);
    items(&mut out, &bad);
    // the circuit owner's design-level invariants (A13: element names unique in a scope)
    crate::element::design_check(design, out.diags);
    // the class table reads definitions through their rows, so it runs on a design whose
    // handles hold (A15 K3)
    if out.diags.len() == before {
        // A28: a cycle among definitions, once per definition on it, at its item
        for k in crate::class::cycle_defs(design) {
            out.report(Violation::DefinitionCycle, design.defs[k].item, k as u32);
        }
        probe_classes(&mut out);
    }
}

/// Appends one `UNSUPPORTED` row per (item, feature) for each feature an item uses outside
/// `legal` (span: the item; `arg`: `Feature::index`), and one at `Span::NONE` per feature of the
/// whole design outside it (the owners' `design_features`); a case clash at the item that brings
/// the second spelling. It appends nothing
/// exactly when `features_used(design).minus(legal)` is empty: these rows are `legalize`'s
/// leftovers (R10). Assumes `check` passed. O(rows).
pub fn check_legal(design: &Design, legal: &FeatureSet, diags: &mut Diags) {
    let mut walk = ExprWalk::new(design);
    let clashes = crate::case::case_clashes(design);
    for (item, scope) in design.reachable() {
        let mut used = FeatureSet::EMPTY;
        item_features(design, item, scope, &mut walk, &mut used);
        if clashes.contains(&item) {
            used = used.with(crate::feature::Feature::Extra(
                crate::feature::Extra::CaseSensitiveNames,
            ));
        }
        for f in used.minus(*legal).iter() {
            diags.push_at(UNSUPPORTED, design, item, f.index() as u32);
        }
    }
    let mut whole = FeatureSet::EMPTY;
    design_level(design, &mut whole);
    for f in whole.minus(*legal).iter() {
        diags.push(UNSUPPORTED, Span::NONE, f.index() as u32);
    }
}

impl Out<'_> {
    /// Appends violation `v` about `at` (`ItemRef::NONE`: no item).
    pub(crate) fn report(&mut self, v: Violation, at: ItemRef, arg: u32) {
        self.diags.push_at(violation_code(v), self.design, at, arg);
    }
}

/// The row count of `kind`'s table.
pub(crate) fn table_len(d: &Design, kind: ItemKind) -> usize {
    for_kind!(kind, R => R::table(d).len())
}

// ---- private ----

/// How many formals the user function `def` has, when `def` is a function this design defines.
fn formal_count(d: &Design, def: DefId) -> Option<usize> {
    let row = d.defs[def.index_checked(d.defs.len())?];
    (row.kind == DefKind::Func
        && row.item.kind == ItemKind::Func
        && (row.item.index as usize) < table_len(d, ItemKind::Func))
    .then(|| {
        d.names(d.row::<crate::item::Func>(row.item.index).args)
            .len()
    })
}

/// Whether the definition index holds `def` under its own key (round 7b: a definition a pass
/// removed through `Editor::remove_item` is not held).
fn in_index(d: &Design, def: DefId) -> bool {
    let row = d.defs[def.index()];
    d.def_index.get(&(row.scope, row.kind, row.name)) == Some(&def)
}

/// `ProbeContext` through reads (A15 K3): a constant-in-time field (`Handle::Expr`, a list or
/// head default) whose expression is of class `Circuit` by a parameter or function it reads.
/// A direct probe in it was reported by `expression`; this pass sees only valid designs.
fn probe_classes(out: &mut Out<'_>) {
    let d = out.design;
    let table = crate::class::def_classes(d);
    let mut handles = Vec::new();
    for (item, _) in d.reachable() {
        handles.clear();
        for_kind!(item.kind, R => d.row::<R>(item.index).handles(d, &mut handles));
        let mut roots = Vec::new();
        for h in &handles {
            match *h {
                Handle::Expr(e) => roots.push(e),
                Handle::Params(l) => roots.extend(d.params(l).iter().map(|p| p.1)),
                Handle::Exprs(l) => roots.extend_from_slice(d.exprs(l)),
                Handle::ExprDyn(_)
                | Handle::Name(_)
                | Handle::Ref(_)
                | Handle::Names(_)
                | Handle::Scope(_)
                | Handle::Text(_)
                | Handle::Branches(_) => {}
            }
        }
        for e in roots {
            let class = crate::class::tree_class(d, e, table);
            if class == ExprClass::Circuit {
                out.report(Violation::ProbeContext, item, e.0);
            }
        }
    }
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
            out.report(Violation::HandleRange, ItemRef::NONE, k as u32);
        } else if !seen.insert((def.scope, def.kind, def.name)) {
            out.report(Violation::DuplicateDef, def.item, k as u32);
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
                    Some(l) if l.form == NumForm::Long => {
                        if let Some(v) = literal::long_violation(&x.long, *l) {
                            out.report(v, ItemRef::NONE, i as u32);
                        }
                        true
                    }
                    Some(l) => {
                        if !l.shape_ok() {
                            out.report(Violation::LiteralShape, ItemRef::NONE, i as u32);
                        } else if !l.finite() {
                            out.report(Violation::NonFinite, ItemRef::NONE, i as u32);
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
            out.report(Violation::HandleRange, ItemRef::NONE, i as u32);
            bad[i] = true;
            continue;
        }
        if !arity_ok {
            out.report(Violation::ExprArity, ItemRef::NONE, i as u32);
            bad[i] = true;
            continue;
        }
        kids.clear();
        d.children(ExprId(i as u32), &mut kids);
        if kids.iter().any(|k| k.index_checked(n).is_none()) {
            out.report(Violation::HandleRange, ItemRef::NONE, i as u32);
            bad[i] = true;
        } else if kids.iter().any(|k| k.0 >= i as u32) {
            out.report(Violation::ExprOrder, ItemRef::NONE, i as u32);
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
    for (item, scope) in d.reachable() {
        if (item.index as usize) >= table_len(d, item.kind) {
            continue; // reported with the scope's list
        }
        handles.clear();
        let mut refs: Vec<(crate::Ref, DefKind)> = Vec::new();
        for_kind!(item.kind, R => {
            let row: R = d.row(item.index);
            row.handles(d, &mut handles);
            row.refs(d, item, &mut refs); // the read-only twin (M5): no copy of the design
            row.check(d, item, scope, out.diags);
        });
        for (r, kind) in refs {
            if !r.is_resolved() {
                continue;
            }
            if !def_ok(d, r.def) {
                out.report(Violation::HandleRange, item, r.def.0);
            } else if d.defs[r.def.index()].kind != kind || d.defs[r.def.index()].name != r.name {
                out.report(Violation::RefTarget, item, r.def.0);
            } else if !in_index(d, r.def) {
                out.report(Violation::StaleDefinition, item, r.def.0);
            }
        }
        for h in handles.clone() {
            handle(out, item, h, bad, &mut walk);
        }
    }
}

/// One handle of a reachable item.
fn handle(out: &mut Out<'_>, item: ItemRef, h: Handle, bad: &[bool], walk: &mut ExprWalk) {
    let d = out.design;
    // a list lies in its pool, and an empty one is `List::EMPTY` (start 0)
    let in_pool = |start: u32, len: u32, pool: usize| {
        start as usize + len as usize <= pool && (len > 0 || start == 0)
    };
    match h {
        Handle::Name(n) => name(out, item, n),
        Handle::Names(l) => {
            if !in_pool(l.start, l.len, d.name_pool.len()) {
                return out.report(Violation::HandleRange, item, l.start);
            }
            for &n in d.names(l) {
                name(out, item, n);
            }
        }
        Handle::Params(l) => {
            if !in_pool(l.start, l.len, d.param_pool.len()) {
                return out.report(Violation::HandleRange, item, l.start);
            }
            for &(n, v) in d.params(l) {
                name(out, item, n);
                expression(out, item, v, false, bad, walk);
            }
        }
        Handle::Exprs(l) => {
            if !in_pool(l.start, l.len, d.expr_pool.len()) {
                return out.report(Violation::HandleRange, item, l.start);
            }
            for &e in d.exprs(l) {
                expression(out, item, e, false, bad, walk);
            }
        }
        Handle::Expr(e) => expression(out, item, e, false, bad, walk),
        Handle::ExprDyn(e) => expression(out, item, e, true, bad, walk),
        Handle::Ref(_) => {} // references are reached through `map_refs`
        Handle::Scope(s) => {
            if !s.is_none() && s.index() >= d.scopes.len() {
                out.report(Violation::HandleRange, item, s.0);
            }
        }
        Handle::Text(t) => {
            if !t.is_none() && t.index() >= d.texts.len() {
                out.report(Violation::HandleRange, item, t.0);
            }
        }
        Handle::Branches(l) => {
            if !in_pool(l.start, l.len, d.branch_pool.len()) {
                out.report(Violation::HandleRange, item, l.start);
            }
        }
    }
}

/// A name handle: in range, and non-empty.
fn name(out: &mut Out<'_>, item: ItemRef, n: NameId) {
    if n.is_none() {
        return;
    }
    if n.index() >= out.design.names.len() {
        out.report(Violation::HandleRange, item, n.0);
    } else if out.design.str(n).is_empty() {
        out.report(Violation::EmptyName, item, n.0);
    }
}

/// An expression handle: in range, its references, and (when constant in time) no circuit read.
fn expression(
    out: &mut Out<'_>,
    item: ItemRef,
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
        return out.report(Violation::HandleRange, item, e.0);
    }
    for &node in walk.tree(d, e) {
        if bad[node.index()] {
            continue;
        }
        match d.expr(node) {
            Expr::V(..) | Expr::I(_) | Expr::Time | Expr::Freq if !dynamic => {
                out.report(Violation::ProbeContext, item, node.0)
            }
            Expr::Call(f, _) if f.reads_circuit() && !dynamic => {
                out.report(Violation::ProbeContext, item, node.0)
            }
            // A28: `idt(x, 0)` is `idt(x)` written twice
            Expr::Call(Func::Idt, args)
                if d.exprs(args).get(1).is_some_and(
                    |&ic| matches!(d.expr(ic), Expr::Num(n) if d.literal_value(d.num(n)) == 0.0),
                ) =>
            {
                out.report(Violation::LiteralShape, item, node.0)
            }
            Expr::Param(r) if r.is_resolved() && !param_target_ok(d, item, r) => {
                out.report(Violation::RefTarget, item, r.def.0)
            }
            Expr::Param(r) | Expr::UserCall(r, _)
                if r.is_resolved() && def_ok(d, r.def) && !in_index(d, r.def) =>
            {
                out.report(Violation::StaleDefinition, item, r.def.0)
            }
            Expr::UserCall(r, _) if r.is_resolved() && !same(d, r, DefKind::Func) => {
                out.report(Violation::RefTarget, item, r.def.0)
            }
            Expr::UserCall(r, args)
                if r.is_resolved() && formal_count(d, r.def).is_some_and(|n| n != args.len()) =>
            {
                out.report(Violation::CallArity, item, node.0)
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
