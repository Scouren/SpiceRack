//! The design → what each scope sees by ngspice's rule, which reads the deck after every include
//! is expanded (ruling R71): a scope sees its own definitions and the top-level definitions of
//! each file it includes (`.include`, and the named section of a `.lib file section`), the
//! included file's own includes too, in the expanded text's order; then its enclosing scope's,
//! outward. Where one name is defined twice in that order, a `.param` keeps the last and every
//! other kind the first ("redefinition of .subckt sub, ignored"). The IR's own `Design::lookup`
//! stays neutral; the reader's bind step asks this table.
//!
//! Cites: ngspice-45 runs, settle `include_subckt_ok.sp`, `inc_after.sp` (an X card before or
//! after the include), `inc_local_{in,out,param}.sp` (an include in a `.subckt` body is local
//! to it), `inc_section_{named,other}.sp` (only the named section), `inc_twice_{before,after}.sp`
//! (first subcircuit, last parameter), `inc_model_param.sp`, `inc_nested.sp`,
//! `formal_scope.sp`, `formal_scope_lib.sp` (a subcircuit's formal is seen from its own body
//! only, F34); INV 3.01.
//!
//! Contents: `pub(super) struct Visible`, `Visible::new(&ir::Design) -> Visible`,
//! `Visible::lookup(&self, &ir::Design, ir::ScopeId, ir::DefKind, ir::NameId) -> ir::DefId`

use std::collections::{HashMap, HashSet};

use crate::ir;

/// Each scope's winning definition of each (kind, name), includes expanded.
pub(super) struct Visible {
    /// `(scope, kind, name)` → the definition that scope sees without looking outward.
    seen: HashMap<(ir::ScopeId, ir::DefKind, ir::NameId), ir::DefId>,
    /// An included file's top scope or a called section's body → the scope of the include
    /// statement that reads it, the first one the entries reach (an active call): ngspice reads
    /// the text there, so a lookup that leaves it goes on in that scope (settle
    /// `inc_lib_self.sp`: a called section reads its caller's `.param`).
    caller: HashMap<ir::ScopeId, ir::ScopeId>,
    /// The scopes of the design: a bound on a lookup's steps.
    scopes: usize,
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
        // body, its item the `.subckt` in the parent) is seen from its own scope first (F34)
        for (k, def) in d.definitions().iter().enumerate() {
            if holder.get(&def.item).is_some_and(|&h| h != def.scope) {
                let id = ir::DefId(u32::try_from(k).expect("definitions fit a u32"));
                seen.entry((def.scope, def.kind, def.name)).or_insert(id);
            }
        }
        for &scope in &scopes {
            for def in expanded(d, scope, &defs_of) {
                let row = d.def(def);
                let key = (scope, row.kind, row.name);
                if row.kind == ir::DefKind::Param || !seen.contains_key(&key) {
                    seen.insert(key, def);
                }
            }
        }
        // the active calls: from the entries' top scopes through the includes they reach
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

    /// The definition of `(kind, name)` ngspice binds from `scope`: the nearest scope outward
    /// that sees one; `NONE` when none does.
    pub(super) fn lookup(
        &self,
        d: &ir::Design,
        scope: ir::ScopeId,
        kind: ir::DefKind,
        name: ir::NameId,
    ) -> ir::DefId {
        let mut at = scope;
        // Bounded: each step moves to an enclosing scope or to a caller, and a caller was
        // reached from the entries before the scope it calls, so no step repeats.
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
/// include's target's in its place (a worklist; a scope already expanded is skipped, so an
/// include cycle ends).
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
        // only the definitions this item makes in the scope it stands in (F34: a subckt's
        // formals are its body's, not the parent's)
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

/// The scope an include reads: the file's top scope, or the named section's body in it; `None`
/// when the file is not in the design (a kit may be absent) or has no such section.
fn target(d: &ir::Design, item: ir::ItemRef) -> Option<ir::ScopeId> {
    let inc: ir::item::Include = d.row(item.index);
    let file = d.find_file(d.str(inc.path));
    if file.is_none() {
        return None;
    }
    let top = d.file(file).top;
    if inc.section.is_none() {
        return Some(top);
    }
    d.items(top).iter().find_map(|&i| {
        let s: ir::item::Section = (i.kind == ir::ItemKind::Section).then(|| d.row(i.index))?;
        d.str(s.name)
            .eq_ignore_ascii_case(d.str(inc.section))
            .then_some(s.body)
    })
}
