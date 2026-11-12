//! The neutral linker: a design with unresolved references in, the same design with each bound
//! to the definition its exact name finds from the item's scope outward (Addendum A4). Every
//! `Ref` a row holds is reached through `RowFacts::map_refs` (one exhaustive dispatch over
//! `ItemKind`), and every `Param` and `UserCall` expression through the expression rows.
//!
//! Who calls it: the Python and Rust builders when they are done, and `text::parse` after the
//! whole text (so a hand-written fixture may name a later definition). A dialect reader never
//! calls it: it binds by its own simulator's rule (`Builder::resolve`, a resolved `Ref` in the
//! row). An already-bound reference is never changed; one no definition matches stays
//! unresolved, which is legal (the kit may be absent).
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (A4); `REWRITE-PLAN.md` §3.1 principle 7;
//! `rewrite/AMEND-A12.md` A38, A43 (`rewrite/RULINGS.md` R63, R70).
//!
//! Contents: `pub fn link(&mut Design, &mut Diags)`, `pub struct UnboundRead`,
//! `pub fn unbound_reads(&Design) -> Vec<UnboundRead>`, `pub struct UnboundTarget`,
//! `pub fn unbound_targets(&Design) -> Vec<UnboundTarget>`

use crate::design::Design;
use crate::diag::Diags;
#[cfg(doc)]
use crate::edit::Editor;
use crate::element::{ElemKind, Element};
use crate::expr::Expr;
use crate::facts::RowFacts;
use crate::feature::{ExprWalk, expr_roots};
use crate::id::{DefId, ExprId, ItemRef, NameId, ScopeId};
use crate::item::{DefKind, Include, ItemKind, Role, ScopeKind, Subckt};

/// A parameter read no definition is bound to (A38, R63), at the item whose expressions hold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnboundRead {
    /// The item that holds the read.
    pub item: ItemRef,
    /// The `Expr::Param` row.
    pub expr: ExprId,
    /// The name it reads.
    pub name: NameId,
    /// Whether it is held on purpose: a `Subckt` head default's read, looked up from each
    /// instance's scope (A29; IR text `name?`). A strict read refuses the others only.
    pub held: bool,
}

/// A name a run item reads that provably names nothing (A43, R70), at the item that holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnboundTarget {
    /// The alter, loop, analysis or elaborate that reads the name.
    pub item: ItemRef,
    /// The name that names nothing: the reference's, or the first segment of an element path
    /// (the element's own name last) that is no element where it is looked up.
    pub name: NameId,
}

/// Where an element target is looked up: the deck's top (every file's top, and what it nests
/// outside a subcircuit) is `ScopeId::NONE`, a subcircuit's body is its scope. Per lookup scope:
/// its elements by name, and whether it is open (an `Opaque` item, or an include of a file the
/// design lacks or holds as `Missing`: an element may hide there).
type Lookup = std::collections::HashMap<ScopeId, (Vec<(NameId, ItemRef)>, bool)>;

/// Binds every still-unresolved reference by `Design::lookup` from the scope of the item (or the
/// expression's item) that holds it, innermost scope first, exact names, with the kind the field
/// expects. A `Param`-tag expression reached from a `Measure` item's handles tries
/// `DefKind::Param` first, then `DefKind::Measure` (A7). A reference it cannot bind stays unresolved and gets no diagnostic: unresolved is
/// legal. A bound reference and a held one (IR text `name?`, held by a crate-private sentinel) are never changed.
/// Works through an [`Editor`] (`Editor::map_refs`, `Builder::resolve`). Deterministic: items in
/// scope order, fields in `map_refs` order. O(references × scope depth). `diags` is not appended
/// to today; it is the spine's uniform signature.
pub fn link(design: &mut Design, diags: &mut Diags) {
    let mut walk = ExprWalk::new(design);
    let mut handles = Vec::new();
    for (item, scope) in design.reachable() {
        // rows: bind on a copy against the design as it is, then store the copy back
        handles.clear();
        for_kind!(item.kind, R => {
            let row: R = design.row(item.index);
            row.handles(design, &mut handles);
        });
        // rows: record every reference in `map_refs` order, look the unresolved ones up through
        // `&Design`, then write the k-th answer into the k-th reference
        let mut slots: Vec<(NameId, DefKind, DefId)> = Vec::new();
        design
            .editor()
            .map_refs(item, &mut |r, kind| slots.push((r.name, kind, r.def)));
        let found: Vec<DefId> = slots
            .iter()
            .map(|&(name, kind, def)| {
                if def.is_none() {
                    design.lookup(scope, kind, name)
                } else {
                    def
                }
            })
            .collect();
        if found.iter().zip(&slots).any(|(f, s)| *f != s.2) {
            let mut k = 0;
            design.editor().map_refs(item, &mut |r, _| {
                r.def = found[k];
                k += 1;
            });
        }
        // expressions: a subcircuit's head defaults are looked up from its body (A10: a default
        // reads its sibling head parameters first); every other row's from its listing scope
        // A29 (R53): a default's read of its own formal is never the formal itself; it is what
        // is visible from where the subcircuit is defined (the listing scope), or unbound
        let listing = scope;
        let (scope, roots): (ScopeId, Vec<(ExprId, NameId)>) = if item.kind == ItemKind::Subckt {
            let row = design.row::<Subckt>(item.index);
            let defaults = design
                .params(row.params)
                .iter()
                .map(|&(n, e)| (e, n))
                .collect();
            (row.body, defaults)
        } else {
            let roots = expr_roots(design, &handles);
            (
                scope,
                roots.into_iter().map(|e| (e, NameId::NONE)).collect(),
            )
        };
        let mut binds = Vec::new();
        for (root, own) in roots {
            for &e in walk.tree(design, root) {
                let def = match design.expr(e) {
                    Expr::Param(r) if r.def.is_none() && r.name == own => {
                        design.lookup(listing, DefKind::Param, r.name)
                    }
                    Expr::Param(r) if r.def.is_none() => {
                        let found = design.lookup(scope, DefKind::Param, r.name);
                        if found.is_none() && item.kind == ItemKind::Measure {
                            design.lookup(scope, DefKind::Measure, r.name)
                        } else {
                            found
                        }
                    }
                    Expr::UserCall(r, _) if r.def.is_none() => {
                        design.lookup(scope, DefKind::Func, r.name)
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
                    | Expr::List(_) => DefId::NONE,
                };
                if !def.is_none() {
                    binds.push((e, def));
                }
            }
        }
        let mut b = design.builder();
        for (e, def) in binds {
            b.resolve(e, def);
        }
    }
    let _ = diags; // unresolved is legal: link appends nothing (A4)
}

/// Every `Expr::Param` row no definition is bound to, item by item in `Design::reachable` order,
/// root by root in `RowFacts::handles` order and left to right within a root (A38: the spine's
/// strict read refuses the ones not `held`). O(expressions).
pub fn unbound_reads(design: &Design) -> Vec<UnboundRead> {
    let mut out = Vec::new();
    let (mut handles, mut nodes) = (Vec::new(), Vec::new());
    for (item, _) in design.reachable() {
        handles.clear();
        design.item_handles(item, &mut handles);
        let mut seen = std::collections::HashSet::new();
        for root in expr_roots(design, &handles) {
            nodes.clear();
            crate::class::tree_nodes(design, root, &mut nodes);
            // children are built before parents, left to right: ascending ids are source order
            nodes.sort_unstable();
            for &e in &nodes {
                if let Expr::Param(r) = design.expr(e)
                    && !r.is_resolved()
                    && seen.insert(e)
                {
                    let held = item.kind == ItemKind::Subckt;
                    out.push(UnboundRead {
                        item,
                        expr: e,
                        name: r.name,
                        held,
                    });
                }
            }
        }
    }
    out
}

/// The names run items read that provably name nothing (A43, R70), item by item in
/// `reachable` order: every unresolved definition reference of an alter, a loop, an analysis
/// or an elaborate (an alter's, a loop's or a DC axis's parameter, subcircuit or model target,
/// an elaborated circuit), and every element such an item reads by name (an element target's
/// path, a noise analysis's `source=`, an `i(…)` in its expressions) whose lookup provably
/// fails. An element is looked up from the deck's top (from a subcircuit's body for an item in
/// one) along its instance path, and is listed only when every instance on the path binds to a
/// subcircuit the design holds and the name that fails is no element of the scope it is looked
/// up in, or follows an element that is no instance. Not listed: a path through an instance
/// whose master is unresolved, or a lookup scope that holds an `Opaque` item or includes a file
/// the design lacks (or holds as `Missing`): the absent kit may hold the element. (The spine's
/// interim walk, `ir-next` 3294c4a9, listed a path through an unresolved master; this one does
/// not, by the lead's ruling.) Listed, for one: VACASK's `alter instance("__topinst__") a=2`
/// read as a device parameter of an element `__topinst__` the deck does not have (the pair
/// test's F7). Unresolved is legal IR (A4): the refusal is the spine's, under its strict
/// policy. O(design) for one copy (the rows' element-name walk takes `&mut`), then O(items ×
/// scope depth + names × path length).
pub fn unbound_targets(design: &Design) -> Vec<UnboundTarget> {
    let lookup = element_scopes(design);
    let mut copy = design.clone();
    let (mut refs, mut handles, mut nodes) = (Vec::new(), Vec::new(), Vec::new());
    let mut out = Vec::new();
    for (item, scope) in design.reachable() {
        let run = matches!(
            item.kind,
            ItemKind::Alter | ItemKind::Loop | ItemKind::Analysis | ItemKind::Elaborate
        );
        if !run {
            continue;
        }
        refs.clear();
        design.item_refs(item, &mut refs);
        let unbound = refs
            .iter()
            .filter(|(r, _)| !r.name.is_none() && !r.is_resolved());
        out.extend(unbound.map(|(r, _)| UnboundTarget { item, name: r.name }));
        // the names the row reads by name (paths joined), then each `i(…)` of its expressions
        let mut paths: Vec<Vec<NameId>> = Vec::new();
        for_kind!(item.kind, R => R::map_element_names(&mut copy, item, &mut |p| paths.push(p.to_vec())));
        handles.clear();
        design.item_handles(item, &mut handles);
        for root in expr_roots(design, &handles) {
            nodes.clear();
            crate::class::tree_nodes(design, root, &mut nodes);
            nodes.sort_unstable();
            for &e in &nodes {
                if let Expr::I(name) = design.expr(e) {
                    paths.push(vec![name]);
                }
            }
        }
        let from = lookup_scope(design, scope);
        for path in paths {
            if let Some(name) = element_unbound(design, &lookup, from, &path) {
                out.push(UnboundTarget { item, name });
            }
        }
    }
    out
}

// ---- private ----

/// Every lookup scope's elements and openness. O(items × scope depth).
fn element_scopes(design: &Design) -> Lookup {
    let mut out: Lookup = Lookup::new();
    out.entry(ScopeId::NONE).or_default();
    for (item, scope) in design.reachable() {
        let entry = out.entry(lookup_scope(design, scope)).or_default();
        match item.kind {
            ItemKind::Element => entry.0.push((design.row::<Element>(item.index).name, item)),
            ItemKind::Opaque => entry.1 = true,
            ItemKind::Include => {
                let path = design.str(design.row::<Include>(item.index).path);
                let file = design.find_file(path);
                entry.1 |= file.is_none() || design.file(file).role == Role::Missing;
            }
            _ => {}
        }
    }
    out
}

/// The lookup scope of an item listed in `scope`: the body of the subcircuit it sits in, or
/// the deck's top (`ScopeId::NONE`).
fn lookup_scope(design: &Design, scope: ScopeId) -> ScopeId {
    let mut at = scope;
    // bounded by the scope count: each step moves to a parent
    for _ in 0..=design.scopes.len() {
        let s = design.scope(at);
        if s.kind == ScopeKind::Subckt {
            return at;
        }
        if s.parent.is_none() {
            break;
        }
        at = s.parent;
    }
    ScopeId::NONE
}

/// The name `path` (instances, then the element) provably fails at from the lookup scope
/// `from` (`unbound_targets`' doc), or `None` when it names an element or the lookup cannot
/// tell.
fn element_unbound(
    design: &Design,
    lookup: &Lookup,
    from: ScopeId,
    path: &[NameId],
) -> Option<NameId> {
    let mut at = from;
    for (k, &name) in path.iter().enumerate() {
        if name.is_none() {
            return None;
        }
        // a body with no item has no entry: empty, and closed
        let (list, open) = lookup
            .get(&at)
            .map_or((&[][..], false), |(l, o)| (&l[..], *o));
        let Some(&(_, item)) = list.iter().find(|(n, _)| *n == name) else {
            return (!open).then_some(name);
        };
        if k + 1 == path.len() {
            return None; // the element itself is there
        }
        let e: Element = design.row(item.index);
        let master = e
            .model
            .def
            .index_checked(design.defs.len())
            .map(|d| design.defs[d].item);
        at = match (e.kind, master) {
            (ElemKind::Instance, Some(m)) if m.kind == ItemKind::Subckt => {
                design.row::<Subckt>(m.index).body
            }
            (ElemKind::Instance, _) => return None, // an unresolved master: the kit may hold it
            // no element lives below an element that is no instance
            _ => return Some(path[k + 1]),
        };
    }
    None
}
