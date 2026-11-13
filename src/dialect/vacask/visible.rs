//! What a scope sees through its includes, by VACASK's rule: an `include` is textual (VD
//! input-include), so the definitions of the included file's top, or of the section a
//! `section=` names, and of what those include in turn, stand in the scope that holds the
//! include statement; VACASK resolves a name after the whole netlist is parsed, so a read
//! standing before the include line sees them too (settle b_incsec: `early=pd*10` before
//! `include … section=dfl` reads dfl's `pd`). A name defined both there and in the including
//! scope is VACASK's "Parameter redefinition." (settle b_incsec_top_*), so the order in which
//! the spliced scopes are searched picks no winner VACASK would accept otherwise.
//!
//! Only the definitions a scope holds itself count (not its parents'): a library's text outside
//! every section is skipped when it is read by section (settle star_outside_section).
//!
//! Cites: settle b_incsec*.sim (VACASK 36be560); R3 (the reader binds), R71 (the ngspice reader's
//! table, `ngspice/visible.rs`, whose shape this copies; the order rule is VACASK's).
//!
//! Contents: `Visible`, `Visible::new`, `Visible::at` (crate-private to the dialect)

use std::collections::{HashMap, HashSet};

use crate::ir;

/// Each scope's own definitions and the scopes its includes splice into it.
pub(super) struct Visible {
    /// `(scope, kind, name)` → the first definition the scope itself holds.
    own: HashMap<(ir::ScopeId, ir::DefKind, ir::NameId), ir::DefId>,
    /// A scope that holds an include → every scope spliced into it, transitively, in text order.
    spliced: HashMap<ir::ScopeId, Vec<ir::ScopeId>>,
}

impl Visible {
    /// The table of `d` as read (its definitions and its include items).
    pub(super) fn new(d: &ir::Design) -> Visible {
        let mut own = HashMap::new();
        for (k, def) in d.definitions().iter().enumerate() {
            let id = ir::DefId(u32::try_from(k).expect("definitions fit a u32"));
            own.entry((def.scope, def.kind, def.name)).or_insert(id);
        }
        let mut direct: HashMap<ir::ScopeId, Vec<ir::ScopeId>> = HashMap::new();
        for (item, scope) in d.reachable() {
            if item.kind == ir::ItemKind::Include
                && let Some(target) = target(d, item)
            {
                direct.entry(scope).or_default().push(target);
            }
        }
        let spliced = direct.keys().map(|&s| (s, closure(&direct, s))).collect();
        Visible { own, spliced }
    }

    /// The definition of `(kind, name)` that `scope` holds itself or through its includes.
    pub(super) fn at(&self, scope: ir::ScopeId, kind: ir::DefKind, name: ir::NameId) -> ir::DefId {
        let spliced = self.spliced.get(&scope).map_or(&[][..], Vec::as_slice);
        std::iter::once(&scope)
            .chain(spliced)
            .find_map(|&s| self.own.get(&(s, kind, name)).copied())
            .unwrap_or(ir::DefId::NONE)
    }
}

// ---- private ----

/// Every scope `from`'s includes reach, depth first in text order, each once (a worklist with a
/// visited set: a file that includes itself, as `cmos180n.lib` does, ends).
fn closure(direct: &HashMap<ir::ScopeId, Vec<ir::ScopeId>>, from: ir::ScopeId) -> Vec<ir::ScopeId> {
    let mut out = Vec::new();
    let mut seen = HashSet::from([from]);
    let mut todo: Vec<ir::ScopeId> = direct[&from].iter().rev().copied().collect();
    // Bounded: a scope is pushed to `out` once (`seen`), and each push expands it once.
    while let Some(s) = todo.pop() {
        if !seen.insert(s) {
            continue;
        }
        out.push(s);
        if let Some(next) = direct.get(&s) {
            todo.extend(next.iter().rev());
        }
    }
    out
}

/// The scope an include reads: the file's top, or the body of the section it names (names are
/// exact, RUN n4); `None` when the file is not in the design or has no such section.
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
        (s.name == inc.section).then_some(s.body)
    })
}
