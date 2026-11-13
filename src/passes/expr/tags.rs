//! The static tag table (A12 K1): a design in, for every expression the set of tags (Int,
//! Real) its value may carry over every evaluation the design allows out. A parameter's set is
//! the union over everything that gives it a value (its own value, a head default, every
//! instance's actual for that formal); a function's formal argument's, the union over every
//! call's argument; a parameter some other item sets (a loop, an alter, a sweep) may be either.
//! The typed passes decide by it: a set of one tag is that tag on every evaluation; a set of
//! two is a note, never a guess.
//!
//! The rules are the evaluator's tag table, by node, on sets: `Add`, `Sub`, `Mul`, `DivTyped`,
//! `Min`, `Max` are Int when every operand is; `Neg`, `SgnZeroPos` keep the operand's; a ternary
//! is either branch's; comparisons, logic, bit operators and `ToInt` are Int; a string or a list
//! has no numeric tag (`TEXT`); everything else is Real. A fixpoint over the definitions (the sets only grow), bounded by their count.
//!
//! Cites: `rewrite/AMEND-A12.md` K1, A21; `crates/ir/src/eval.rs` (`apply`'s tags).
//!
//! Contents: `pub(super) const INT`, `pub(super) const REAL`, `pub(super) const BOTH`,
//! `pub(super) const TEXT`,
//! `pub(super) struct Tags` (`Tags::new`, `Tags::of`, `Tags::set`)

use std::collections::HashMap;

use super::nodes::{def_values, live};
use crate::ir;

/// The set holding Int.
pub(super) const INT: u8 = 1;
/// The set holding Real.
pub(super) const REAL: u8 = 2;
/// Either tag.
pub(super) const BOTH: u8 = INT | REAL;
/// No numeric tag: the value may be a string or a list (no rule compares it equal to `INT`,
/// `REAL` or `BOTH`, so a typed rule leaves it).
pub(super) const TEXT: u8 = 4;

/// The tag sets of the design as it stood when built, by `ExprId`, and of the rows a pass
/// appends after (`Tags::of` computes a new row from its operands; `Tags::set` states the tags a
/// rewritten node keeps).
pub(super) struct Tags {
    exprs: Vec<u8>,
    defs: Vec<u8>,
    /// per function definition, per formal argument (the `Func` row's order)
    formals: Vec<Vec<u8>>,
    /// rows appended after the table was built
    new: HashMap<ir::ExprId, u8>,
}

impl Tags {
    /// The table of `design`. O(expressions × rounds), rounds bounded by the definitions.
    pub(super) fn new(design: &ir::Design) -> Tags {
        let defs = design.definitions();
        let mut t = Tags {
            exprs: vec![0; design.expr_count()],
            defs: vec![0; defs.len()],
            formals: defs
                .iter()
                .map(|d| vec![0; formal_count(design, *d)])
                .collect(),
            new: HashMap::new(),
        };
        // a parameter another item sets (a loop, an alter, a sweep) takes values we do not see
        let mut pinned = vec![false; defs.len()];
        let mut refs = Vec::new();
        for (item, _) in design.reachable() {
            refs.clear();
            design.item_refs(item, &mut refs);
            for &(r, kind) in &refs {
                if kind == ir::DefKind::Param && r.is_resolved() {
                    pinned[r.def.index()] = true;
                }
            }
        }
        for (d, def) in defs.iter().enumerate() {
            if pinned[d] || def.kind == ir::DefKind::Measure {
                t.defs[d] = if pinned[d] { BOTH } else { REAL };
            }
        }
        let values = def_values(design);
        // the live calls only: a rewritten call stays in the table (rows are appended)
        let calls: Vec<(ir::Ref, ir::ExprList)> = live(design)
            .into_iter()
            .filter_map(|e| match design.expr(e) {
                ir::Expr::UserCall(r, args) if r.is_resolved() => Some((r, args)),
                _ => None,
            })
            .collect();
        let bound = 2 * (defs.len() + t.formals.iter().map(Vec::len).sum::<usize>()) + 2;
        for _ in 0..bound {
            for i in 0..t.exprs.len() {
                let e = ir::ExprId(i as u32);
                t.exprs[i] = t.node(design, e);
            }
            let mut changed = false;
            for (d, roots) in values.iter().enumerate() {
                let joined = roots.iter().fold(t.defs[d], |s, &e| s | t.exprs[e.index()]);
                changed |= joined != t.defs[d];
                t.defs[d] = joined;
            }
            for &(r, args) in &calls {
                let d = r.def.index();
                for (k, &a) in design.exprs(args).iter().enumerate() {
                    if let Some(slot) = t.formals[d].get_mut(k) {
                        let joined = *slot | t.exprs[a.index()];
                        changed |= joined != *slot;
                        *slot = joined;
                    }
                }
            }
            if !changed {
                return t;
            }
        }
        unreachable!("the sets only grow, and each round grows one")
    }

    /// The tag set of `e`: the table's for a row it was built over; for a row appended since,
    /// the set `Tags::set` stated or the one its operands give (computed once).
    pub(super) fn of(&mut self, design: &ir::Design, e: ir::ExprId) -> u8 {
        if e.index() < self.exprs.len() {
            return none_is_both(self.exprs[e.index()]);
        }
        if let Some(&s) = self.new.get(&e) {
            return s;
        }
        // the rewrite meets every appended row after its operands, so they are known here
        let s = none_is_both(self.node(design, e));
        self.new.insert(e, s);
        s
    }

    /// States that the appended row `e` stands for a node whose tag set was `tags` (a rewrite
    /// that drops a tag no reader observes).
    pub(super) fn set(&mut self, e: ir::ExprId, tags: u8) {
        assert!(
            e.index() >= self.exprs.len(),
            "only an appended row is stated"
        );
        self.new.insert(e, tags);
    }
}

// ---- private ----

impl Tags {
    /// One node's set from its operands' current sets (the evaluator's table on sets).
    fn node(&self, design: &ir::Design, e: ir::ExprId) -> u8 {
        let get = |k: ir::ExprId| match k.index() < self.exprs.len() {
            true => self.exprs[k.index()],
            false => self.new.get(&k).copied().unwrap_or(BOTH),
        };
        match design.expr(e) {
            ir::Expr::Num(n) if design.num(n).form == ir::NumForm::Int => INT,
            ir::Expr::Param(r) if !r.is_resolved() => BOTH,
            ir::Expr::Param(r) => {
                let def = design.def(r.def);
                match def.kind {
                    ir::DefKind::Func => {
                        let row: ir::item::Func = design.row(def.item.index);
                        let k = design.names(row.args).iter().position(|n| *n == r.name);
                        k.map_or(BOTH, |k| self.formals[r.def.index()][k])
                    }
                    ir::DefKind::Param | ir::DefKind::Measure => self.defs[r.def.index()],
                    _ => BOTH,
                }
            }
            ir::Expr::UserCall(r, _) if r.is_resolved() => self.defs[r.def.index()],
            ir::Expr::UserCall(..) => BOTH,
            ir::Expr::Unary(ir::UnOp::Neg, a) => get(a),
            ir::Expr::Unary(..) => INT,
            ir::Expr::Binary(op, a, b) => match op {
                ir::BinOp::Add | ir::BinOp::Sub | ir::BinOp::Mul | ir::BinOp::DivTyped => {
                    arith(get(a), get(b))
                }
                ir::BinOp::Div | ir::BinOp::IntDiv => REAL,
                ir::BinOp::Eq
                | ir::BinOp::Ne
                | ir::BinOp::Lt
                | ir::BinOp::Le
                | ir::BinOp::Gt
                | ir::BinOp::Ge
                | ir::BinOp::And
                | ir::BinOp::Or
                | ir::BinOp::BitAnd
                | ir::BinOp::BitOr
                | ir::BinOp::BitXor
                | ir::BinOp::Shl
                | ir::BinOp::Shr => INT,
            },
            ir::Expr::Ternary(_, a, b) => get(a) | get(b),
            ir::Expr::Call(ir::Func::Min | ir::Func::Max, l) => {
                let xs = design.exprs(l);
                xs.iter().skip(1).fold(get(xs[0]), |s, &x| arith(s, get(x)))
            }
            ir::Expr::Call(ir::Func::ToInt, _) => INT,
            ir::Expr::Call(ir::Func::SgnZeroPos, l) => get(design.exprs(l)[0]),
            ir::Expr::Str(_) | ir::Expr::List(_) => TEXT,
            ir::Expr::Num(_)
            | ir::Expr::Call(..)
            | ir::Expr::V(..)
            | ir::Expr::I(_)
            | ir::Expr::Time
            | ir::Expr::Temp
            | ir::Expr::Freq
            | ir::Expr::TStep
            | ir::Expr::TStop
            | ir::Expr::Tnom
            | ir::Expr::Draw(..) => REAL,
        }
    }
}

/// `Add`'s rule on sets: Int when both may be Int, Real when either may be Real; nothing when
/// either has no value yet.
fn arith(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let int = if a & b & INT != 0 { INT } else { 0 };
    let real = if (a | b) & REAL != 0 { REAL } else { 0 };
    int | real
}

/// A set no value reaches (a formal never passed, an unset parameter) is unknown: either.
fn none_is_both(s: u8) -> u8 {
    if s == 0 { BOTH } else { s }
}

/// The formal argument count of a function definition (0 for any other kind).
fn formal_count(design: &ir::Design, def: ir::scope::Def) -> usize {
    match (def.kind, def.item.kind) {
        (ir::DefKind::Func, ir::ItemKind::Func) => {
            let row: ir::item::Func = design.row(def.item.index);
            design.names(row.args).len()
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::ir::r#gen::{Size, generate};

    /// A parameter read is its definition's value, evaluated (a head default for a formal); a
    /// leaf or an unresolved read a number by name, Int on odd draws when whole.
    struct Follow<'d> {
        design: &'d ir::Design,
        depth: Cell<u32>,
    }

    impl ir::Bindings for Follow<'_> {
        fn param(&self, p: ir::Ref) -> Option<f64> {
            self.param_value(p).map(|v| v.number)
        }
        fn param_value(&self, p: ir::Ref) -> Option<ir::Value> {
            if !p.is_resolved() {
                return Some(by_name(self.design.str(p.name)));
            }
            let def = self.design.def(p.def);
            let value = match (def.kind, def.item.kind) {
                (ir::DefKind::Param, ir::ItemKind::Param) => {
                    self.design.row::<ir::item::Param>(def.item.index).value
                }
                (ir::DefKind::Param, ir::ItemKind::Subckt) => {
                    let s: ir::item::Subckt = self.design.row(def.item.index);
                    let given = self
                        .design
                        .params(s.params)
                        .iter()
                        .find(|q| q.0 == def.name);
                    given.map_or(ir::ExprId::NONE, |q| q.1)
                }
                (ir::DefKind::Measure, _) => return Some(ir::Value::real(0.75)),
                _ => return None,
            };
            if value.is_none() || self.depth.get() > 64 {
                return None;
            }
            self.depth.set(self.depth.get() + 1);
            let v = ir::eval_typed(self.design, value, self).ok();
            self.depth.set(self.depth.get() - 1);
            v
        }
        fn voltage(&self, n: ir::NameId, _: ir::NameId) -> Option<f64> {
            Some(by_name(self.design.str(n)).number)
        }
        fn current(&self, n: ir::NameId) -> Option<f64> {
            Some(by_name(self.design.str(n)).number)
        }
        fn time(&self) -> Option<f64> {
            Some(1e-6)
        }
        fn temp(&self) -> Option<f64> {
            Some(27.0)
        }
        fn freq(&self) -> Option<f64> {
            Some(1e3)
        }
        fn tstep(&self) -> Option<f64> {
            Some(1e-9)
        }
        fn tstop(&self) -> Option<f64> {
            Some(1e-6)
        }
        fn tnom(&self) -> Option<f64> {
            Some(27.0)
        }
        fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, a: &[f64]) -> Option<f64> {
            Some(a[0])
        }
    }

    fn by_name(name: &str) -> ir::Value {
        let h = name
            .bytes()
            .fold(7u64, |h, b| (h ^ u64::from(b)).wrapping_mul(31));
        let n = (h % 9) as f64 - 4.0;
        match h & 1 == 1 {
            true => ir::Value::int(n),
            false => ir::Value::real(n),
        }
    }

    // oracle: the reference evaluator's tags (eval_typed), at every row of generated designs
    #[test]
    fn every_value_the_evaluator_gives_carries_a_tag_the_table_allows() {
        let size = Size {
            items: 48,
            scope_depth: 3,
            expr_depth: 4,
            files: 3,
        };
        let (mut values, mut ints) = (0, 0);
        for seed in 0..200u64 {
            let design = generate(seed, &ir::FeatureSet::ALL, size);
            let mut tags = Tags::new(&design);
            let env = Follow {
                design: &design,
                depth: Cell::new(0),
            };
            for i in 0..design.expr_count() {
                let e = ir::ExprId(i as u32);
                let Ok(v) = ir::eval_typed(&design, e, &env) else {
                    continue;
                };
                let tag = if v.int { INT } else { REAL };
                let allowed = tags.of(&design, e);
                assert!(
                    allowed & tag != 0,
                    "seed {seed} row {i} {:?}: {v:?}, the table says {allowed}",
                    design.expr(e)
                );
                values += 1;
                ints += usize::from(v.int && allowed == INT);
            }
        }
        eprintln!("{values} values, {ints} sure Ints");
        assert!(
            values > 1000 && ints > 50,
            "{values} values, {ints} sure Ints"
        );
    }
}
