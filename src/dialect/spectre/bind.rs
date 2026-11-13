//! The read spine's bind step: every row reference and every parameter read or user call the
//! areas queued, bound once every file is lowered, by Spectre's scoping as this reader takes
//! it: the nearest definition outward (a subcircuit's own parameters, then the enclosing
//! scopes', the file's top last), an `include` read as its text in place (MAN: UG p.50
//! "include" inserts the file; no run settled scoping, `rewrite/DIALECT-SPECTRE.md` §4). A
//! name with no definition stays unresolved (R4).
//!
//! Contents: `RowRef`, `bind` (crate-private); the table of what each scope sees,
//! `Visible` (`Visible::new`, `Visible::lookup`), private.

use std::collections::{HashMap, HashSet};

use crate::ir;

/// A row's reference waiting for the bind step (`Lower::refer`): its item, its slot in
/// `RowFacts::map_refs` order, the scope it is read from, the kind the slot expects, the name.
#[derive(Clone, Copy, Debug)]
pub(super) struct RowRef {
    /// The pushed item.
    pub(super) item: ir::ItemRef,
    /// Its reference's slot.
    pub(super) k: u32,
    /// Where the name is read.
    pub(super) scope: ir::ScopeId,
    /// The definition kind the slot expects.
    pub(super) kind: ir::DefKind,
    /// The name, as pushed.
    pub(super) name: ir::NameId,
}

/// Each scope's winning definition of each (kind, name), includes expanded.
struct Visible {
    /// `(scope, kind, name)` → the definition that scope sees without looking outward.
    seen: HashMap<(ir::ScopeId, ir::DefKind, ir::NameId), ir::DefId>,
    /// An included file's top scope → the scope of the include that reads it first.
    caller: HashMap<ir::ScopeId, ir::ScopeId>,
    /// The scopes of the design: a bound on a lookup's steps.
    scopes: usize,
}

/// Binds each queued row reference, then each parameter read and user call still unresolved,
/// to the definition visible from its scope (`Visible::lookup`); none leaves it unresolved.
pub(super) fn bind(b: &mut ir::Builder<'_>, reads: &[(ir::ExprId, ir::ScopeId)], refs: &[RowRef]) {
    let seen = Visible::new(b.design());
    for r in refs {
        let def = seen.lookup(b.design(), r.scope, r.kind, r.name);
        if !def.is_none() {
            b.bind_ref(r.item, r.k, def);
        }
    }
    for &(e, scope) in reads {
        let (kind, r) = match b.design().expr(e) {
            ir::Expr::Param(r) => (ir::DefKind::Param, r),
            ir::Expr::UserCall(r, _) => (ir::DefKind::Func, r),
            other => unreachable!("only reads and calls are queued: {other:?}"),
        };
        if !r.def.is_none() {
            continue;
        }
        let def = seen.lookup(b.design(), scope, kind, r.name);
        if !def.is_none() {
            b.resolve(e, def);
        }
    }
}

impl Visible {
    /// The table of `d` (its definitions and include items as read).
    pub(super) fn new(d: &ir::Design) -> Visible {
        let mut defs_of: HashMap<ir::ItemRef, Vec<ir::DefId>> = HashMap::new();
        for (k, def) in d.definitions_all().iter().enumerate() {
            let id = ir::DefId(u32::try_from(k).expect("definitions fit a u32"));
            defs_of.entry(def.item).or_default().push(id);
        }
        let reachable = d.reachable();
        let scopes: HashSet<ir::ScopeId> = reachable.iter().map(|&(_, s)| s).collect();
        let holder: HashMap<ir::ItemRef, ir::ScopeId> = reachable.into_iter().collect();
        let mut seen = HashMap::new();
        // a definition its item does not stand beside (a subcircuit's formal: its scope is the
        // body, its item the `subckt` in the parent) is seen from its own scope
        for (k, def) in d.definitions_all().iter().enumerate() {
            if holder.get(&def.item).is_some_and(|&h| h != def.scope) {
                let id = ir::DefId(u32::try_from(k).expect("definitions fit a u32"));
                seen.entry((def.scope, def.kind, def.name)).or_insert(id);
            }
        }
        for &scope in &scopes {
            for def in expanded(d, scope, &defs_of) {
                let row = d.def(def);
                seen.entry((scope, row.kind, row.name)).or_insert(def);
            }
        }
        let mut caller = HashMap::new();
        let mut todo: Vec<ir::ScopeId> = d
            .files()
            .map(|f| d.file(f))
            .filter(|f| f.role == ir::Role::Deck)
            .map(|f| f.top)
            .collect();
        // Bounded: a scope is pushed when it first gets its caller.
        while let Some(at) = todo.pop() {
            for &item in d.items(at) {
                if item.kind == ir::ItemKind::Include
                    && let Some(t) = target(d, item)
                    && t != at
                    && !caller.contains_key(&t)
                {
                    caller.insert(t, at);
                    todo.push(t);
                }
            }
        }
        Visible {
            seen,
            caller,
            scopes: scopes.len(),
        }
    }

    /// The definition of `(kind, name)` visible from `scope`: the nearest scope outward (an
    /// included file's top goes on at its include) that sees one; `NONE` when none does.
    pub(super) fn lookup(
        &self,
        d: &ir::Design,
        scope: ir::ScopeId,
        kind: ir::DefKind,
        name: ir::NameId,
    ) -> ir::DefId {
        let mut at = scope;
        // Bounded: each step moves to an enclosing scope or to a caller reached earlier.
        for _ in 0..2 * self.scopes + 2 {
            if at.is_none() {
                break;
            }
            if let Some(&def) = self.seen.get(&(at, kind, name)) {
                return def;
            }
            at = self.caller.get(&at).copied().unwrap_or(d.scope(at).parent);
        }
        ir::DefId::NONE
    }
}

// ---- private ----

/// The definitions `scope` holds in the expanded text's order: its items' own, and each
/// include's target's in its place (a scope already expanded is skipped, so a cycle ends).
fn expanded(
    d: &ir::Design,
    scope: ir::ScopeId,
    defs_of: &HashMap<ir::ItemRef, Vec<ir::DefId>>,
) -> Vec<ir::DefId> {
    let mut out = Vec::new();
    let mut done: HashSet<ir::ScopeId> = HashSet::from([scope]);
    // frames: (scope, next item); bounded: each scope is pushed once
    let mut stack = vec![(scope, 0usize)];
    while let Some((at, k)) = stack.pop() {
        let Some(&item) = d.items(at).get(k) else {
            continue;
        };
        stack.push((at, k + 1));
        out.extend(
            defs_of
                .get(&item)
                .into_iter()
                .flatten()
                .filter(|&&def| d.def(def).scope == at),
        );
        if item.kind == ir::ItemKind::Include
            && let Some(target) = target(d, item)
            && done.insert(target)
        {
            stack.push((target, 0));
        }
    }
    out
}

/// The scope an include reads: the file's top scope; `None` when the file is not in the design
/// or the include names a section (this reader keeps no section).
fn target(d: &ir::Design, item: ir::ItemRef) -> Option<ir::ScopeId> {
    let inc: ir::item::Include = d.row(item.index);
    let file = d.find_file(d.str(inc.path));
    (!file.is_none() && inc.section.is_none()).then(|| d.file(file).top)
}
