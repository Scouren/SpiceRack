//! The live rows: a design in, the expression rows and definitions a rule may decide from out.
//! The expression table and the definition table are append-only: a rewrite leaves the row it
//! replaced in place, and `Editor::remove_item` leaves a removed item's definition in place (out
//! of the index). A walk over `0..expr_count_all()` or `definitions_all()` sees those dead rows;
//! these walks never do. The names say which is which (the `passes` agent's finding, supported
//! by the lead: two rules counted a replaced row as a reader, three walks saw a removed item's
//! definition).
//!
//! Cites: `rewrite/IR-KERNEL.md` §4 (append-only tables), `rewrite/AMEND-A12.md` A28 (a dead
//! body is not wrong), round 7b (`Editor::remove_item` takes a definition out of the index).
//!
//! Contents: `Design::live_exprs(&self) -> Vec<ExprId>`,
//! `Design::live_definitions(&self) -> Vec<DefId>`, `Design::def_is_live(&self, DefId) -> bool`

use crate::design::Design;
#[cfg(doc)]
use crate::edit::Editor;
use crate::facts::Handle;
use crate::id::{DefId, ExprId};

impl Design {
    /// Every expression row reachable from a root of a reachable item (a field, a list
    /// element, a parameter value, a branch condition), each once, roots first in `reachable`
    /// order. A row a rewrite replaced, and the rows only it reads, are not listed. Size a table
    /// by `expr_count_all()`; decide from these. O(live rows).
    pub fn live_exprs(&self) -> Vec<ExprId> {
        let n = self.expr_count_all();
        let mut seen = vec![false; n];
        let (mut out, mut handles, mut stack) = (Vec::new(), Vec::new(), Vec::new());
        for (item, _) in self.reachable() {
            handles.clear();
            self.item_handles(item, &mut handles);
            for h in &handles {
                match *h {
                    Handle::Expr(e) | Handle::ExprDyn(e) => stack.push(e),
                    Handle::Exprs(l) => stack.extend_from_slice(self.exprs(l)),
                    Handle::Params(l) => stack.extend(self.params(l).iter().map(|p| p.1)),
                    Handle::Branches(l) => stack.extend(self.branches(l).iter().map(|b| b.cond)),
                    Handle::Name(_)
                    | Handle::Ref(..)
                    | Handle::Names(_)
                    | Handle::Scope(_)
                    | Handle::Text(_) => {}
                }
            }
            // each row enters once: bounded by the table
            while let Some(e) = stack.pop() {
                if e.is_none() || e.index() >= n || seen[e.index()] {
                    continue;
                }
                seen[e.index()] = true;
                out.push(e);
                self.children(e, &mut stack);
            }
        }
        assert!(out.len() <= n, "each row once");
        out
    }

    /// The definitions the index holds, in definition order: a removed item's definition
    /// (`Editor::remove_item`) is not listed. Index a table by `DefId` sized by
    /// `definitions_all().len()`; decide from these. O(definitions).
    pub fn live_definitions(&self) -> Vec<DefId> {
        let out: Vec<DefId> = (0..self.definitions_all().len() as u32)
            .map(DefId)
            .filter(|&d| self.def_is_live(d))
            .collect();
        assert!(out.len() <= self.definitions_all().len(), "a subset");
        out
    }

    /// Whether the index holds `def` under its own key (scope, kind, name). False for an id out
    /// of range.
    pub fn def_is_live(&self, def: DefId) -> bool {
        let Some(k) = def.index_checked(self.defs.len()) else {
            return false;
        };
        let row = self.defs[k];
        self.def_index.get(&(row.scope, row.kind, row.name)) == Some(&def)
    }
}
