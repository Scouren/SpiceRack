//! The value of a wave field that no analysis can see change (R79): a literal, or a constant
//! expression of literals, operators, built-in functions and parameter reads, valued by the
//! reference evaluator. A read is allowed when its definition is a parameter whose value
//! obeys the same rule, that no alter, sweep, loop or Monte Carlo item targets (followed
//! through every parameter it reads), or a subcircuit formal no instance of that subcircuit
//! gives a value (then its default is its value everywhere, and the default obeys the rule).
//! No draw, no temperature, no transient leaf, no user call, no unbound read: those give
//! `None`, and the rules that ask keep the wave.
//!
//! Cites: `rewrite/RULINGS.md` R79; `crates/ir/src/eval.rs`.
//!
//! Contents: `pub(super) struct Constants` (`Constants::new`, `Constants::value`)

use std::cell::Cell;

use crate::ir;

/// What the design pins and overrides, for valuing its fields.
pub(super) struct Constants {
    /// By `DefId`: a parameter an alter, a sweep, a loop or a Monte Carlo item sets.
    pinned: Vec<bool>,
    /// By `DefId`: a subcircuit formal some instance gives a value.
    passed: Vec<bool>,
}

/// The reference evaluator's bindings for an allowed field: a parameter read is its
/// definition's value; nothing else is read (the walk refused it).
struct Values<'d> {
    design: &'d ir::Design,
    depth: Cell<u32>,
}

impl Constants {
    /// The tables of `design`: one walk over its reachable items.
    pub(super) fn new(design: &ir::Design) -> Constants {
        let n = design.definitions().len();
        let (mut pinned, mut passed) = (vec![false; n], vec![false; n]);
        let mut refs = Vec::new();
        for (item, _) in design.reachable() {
            refs.clear();
            design.item_refs(item, &mut refs);
            let instance = item.kind == ir::ItemKind::Element && {
                let el = design.row::<ir::Element>(item.index);
                el.kind == ir::ElemKind::Instance
            };
            for &(r, kind) in &refs {
                if kind == ir::DefKind::Param && r.is_resolved() && !instance {
                    pinned[r.def.index()] = true;
                }
            }
            if instance {
                let el = design.row::<ir::Element>(item.index);
                if !el.model.is_resolved() {
                    continue;
                }
                let subckt = design.def(el.model.def).item;
                for &(name, _) in design.params(el.params) {
                    let formal = design.definitions().iter().position(|d| {
                        d.kind == ir::DefKind::Param && d.item == subckt && d.name == name
                    });
                    if let Some(f) = formal {
                        passed[f] = true;
                    }
                }
            }
        }
        Constants { pinned, passed }
    }

    /// The value of field `e` with the names of the parameters it reads (directly or through
    /// other parameters), when the module header's rule allows it; `None` otherwise.
    pub(super) fn value(
        &self,
        design: &ir::Design,
        e: ir::ExprId,
    ) -> Option<(f64, Vec<ir::NameId>)> {
        if e.is_none() {
            return None;
        }
        let mut names = Vec::new();
        let mut stack = vec![e];
        let mut seen = std::collections::HashSet::new();
        // bounded: each row and each definition's root once
        while let Some(x) = stack.pop() {
            if x.is_none() || !seen.insert(x) {
                continue;
            }
            match design.expr(x) {
                ir::Expr::Num(_) => {}
                ir::Expr::Unary(_, a) => stack.push(a),
                ir::Expr::Binary(_, a, b) => stack.extend([a, b]),
                ir::Expr::Ternary(c, a, b) => stack.extend([c, a, b]),
                ir::Expr::Call(_, l) => stack.extend_from_slice(design.exprs(l)),
                ir::Expr::Param(r) => {
                    let root = self.definition(design, r)?;
                    names.push(r.name);
                    stack.push(root);
                }
                _ => return None,
            }
        }
        let env = Values {
            design,
            depth: Cell::new(0),
        };
        let v = ir::eval(design, e, &env).ok()?;
        names.sort_unstable_by_key(|n| n.0);
        names.dedup();
        v.is_finite().then_some((v, names))
    }
}

impl ir::Bindings for Values<'_> {
    fn param(&self, p: ir::Ref) -> Option<f64> {
        if !p.is_resolved() || self.depth.get() > 64 {
            return None;
        }
        let def = self.design.def(p.def);
        let root = match (def.kind, def.item.kind) {
            (ir::DefKind::Param, ir::ItemKind::Param) => {
                self.design.row::<ir::item::Param>(def.item.index).value
            }
            (ir::DefKind::Param, ir::ItemKind::Subckt) => head_default(self.design, &def),
            _ => return None,
        };
        if root.is_none() {
            return None;
        }
        self.depth.set(self.depth.get() + 1);
        let v = ir::eval(self.design, root, self).ok();
        self.depth.set(self.depth.get() - 1);
        v
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        None
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        None
    }
    fn time(&self) -> Option<f64> {
        None
    }
    fn temp(&self) -> Option<f64> {
        None
    }
    fn freq(&self) -> Option<f64> {
        None
    }
    fn tstep(&self) -> Option<f64> {
        None
    }
    fn tstop(&self) -> Option<f64> {
        None
    }
    fn tnom(&self) -> Option<f64> {
        None
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, _: &[f64]) -> Option<f64> {
        None
    }
}

// ---- private ----

impl Constants {
    /// The expression that gives read `r` its one value, or `None` when it has none or more.
    fn definition(&self, design: &ir::Design, r: ir::Ref) -> Option<ir::ExprId> {
        if !r.is_resolved() || self.pinned[r.def.index()] || self.passed[r.def.index()] {
            return None;
        }
        let def = design.def(r.def);
        let root = match (def.kind, def.item.kind) {
            (ir::DefKind::Param, ir::ItemKind::Param) => {
                design.row::<ir::item::Param>(def.item.index).value
            }
            (ir::DefKind::Param, ir::ItemKind::Subckt) => head_default(design, &def),
            _ => return None,
        };
        (!root.is_none()).then_some(root)
    }
}

/// A formal's default in its subcircuit's head (`NONE` when it has none).
fn head_default(design: &ir::Design, def: &ir::scope::Def) -> ir::ExprId {
    let s: ir::item::Subckt = design.row(def.item.index);
    let given = design.params(s.params).iter().find(|p| p.0 == def.name);
    given.map_or(ir::ExprId::NONE, |p| p.1)
}
