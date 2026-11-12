//! What several expression passes read of a design: a node's children (the `Expr` view's
//! operands), the values each definition takes, the live rows, whether a value may hold a draw, whether a tag is observable, the
//! notes at items left carrying a feature, and the plain decimal literals the passes build.
//!
//! Cites: `crates/ir/src/expr.rs` (`Expr`, `Literal`); the brief's operand rule (a pass never
//! duplicates an operand holding a `Draw`).
//!
//! Contents: `pub(super) struct Draws` (`Draws::new`, `Draws::holds`),
//! `pub(super) fn kids(&ir::Design, ir::ExprId, &mut Vec<ir::ExprId>)`,
//! `pub(super) fn plain(&mut ir::Builder<'_>, i64, u8) -> ir::ExprId`,
//! `pub(super) fn def_values(&ir::Design) -> Vec<Vec<ir::ExprId>>`,
//! `pub(super) fn instance(&ir::Design, ir::ItemRef) -> Option<(ir::ItemRef, ir::ParamList)>`,
//! `pub(super) fn live(&ir::Design) -> Vec<ir::ExprId>`, `pub(super) fn reads_tags(&ir::Design) -> bool`,
//! `pub(super) fn note_left(&ir::Design, &mut ir::Diags, ir::Feature, ir::Code)`,
//! `pub(super) fn binary_form(ir::BinOp, bool) -> ir::Feature`, `pub(super) fn func_form(ir::Func, bool) -> ir::Feature`,
//! `pub(super) fn literal(&ir::Design, ir::ExprId) -> Option<f64>`

use crate::ir;

/// Which definitions' values may hold a draw (a parameter whose value, or a function whose
/// body, reads one, transitively), for the design as it stood when built.
pub(super) struct Draws {
    defs: Vec<bool>,
}

/// Appends the operands of `e` in evaluation order (a leaf has none).
pub(super) fn kids(design: &ir::Design, e: ir::ExprId, out: &mut Vec<ir::ExprId>) {
    match design.expr(e) {
        ir::Expr::Call(_, l)
        | ir::Expr::UserCall(_, l)
        | ir::Expr::Draw(_, _, _, l)
        | ir::Expr::List(l) => out.extend_from_slice(design.exprs(l)),
        ir::Expr::Unary(_, a) => out.push(a),
        ir::Expr::Binary(_, a, b) => out.extend([a, b]),
        ir::Expr::Ternary(c, a, b) => out.extend([c, a, b]),
        ir::Expr::Num(_)
        | ir::Expr::Str(_)
        | ir::Expr::Param(_)
        | ir::Expr::V(..)
        | ir::Expr::I(_)
        | ir::Expr::Time
        | ir::Expr::Temp
        | ir::Expr::Freq
        | ir::Expr::TStep
        | ir::Expr::TStop
        | ir::Expr::Tnom => {}
    }
}

/// A `Plain` literal `mantissa × 10^−frac` (a Real number however whole: `NumForm::Int` would
/// add `Extra::IntLiteral`).
pub(super) fn plain(b: &mut ir::Builder<'_>, mantissa: i64, frac: u8) -> ir::ExprId {
    b.literal(ir::Literal {
        mantissa,
        frac,
        exp10: 0,
        form: ir::NumForm::Plain,
    })
}

impl Draws {
    /// The table for `design`: a definition holds a draw when its value (a parameter's, a head
    /// default's, an instance's actual for it, a function's body) does; a fixpoint over the
    /// definitions, bounded by their count.
    pub(super) fn new(design: &ir::Design) -> Draws {
        let defs = design.definitions();
        let mut table = Draws {
            defs: vec![false; defs.len()],
        };
        let values = def_values(design);
        for _ in 0..=defs.len() {
            let mut changed = false;
            for (d, roots) in values.iter().enumerate() {
                if !table.defs[d] && roots.iter().any(|&e| table.holds(design, e)) {
                    table.defs[d] = true;
                    changed = true;
                }
            }
            if !changed {
                return table;
            }
        }
        unreachable!("each round marks a definition or ends")
    }

    /// Whether the tree under `e` holds a draw or reads a definition that may. O(its tree).
    pub(super) fn holds(&self, design: &ir::Design, e: ir::ExprId) -> bool {
        let mut stack = vec![e];
        let mut out = Vec::new();
        for _ in 0..=design.expr_count() {
            let Some(e) = stack.pop() else { return false };
            if e.is_none() {
                continue;
            }
            match design.expr(e) {
                ir::Expr::Draw(..) => return true,
                ir::Expr::Param(r) | ir::Expr::UserCall(r, _)
                    if r.is_resolved() && self.defs[r.def.index()] =>
                {
                    return true;
                }
                _ => {}
            }
            out.clear();
            kids(design, e, &mut out);
            stack.extend_from_slice(&out);
        }
        unreachable!("a tree has fewer nodes than the table")
    }
}

/// Every expression that gives each definition its value, by `DefId`: a parameter's value, a
/// head default and each instance's actual for that formal, a function's body.
pub(super) fn def_values(design: &ir::Design) -> Vec<Vec<ir::ExprId>> {
    let defs = design.definitions();
    let mut out = vec![Vec::new(); defs.len()];
    for (d, def) in defs.iter().enumerate() {
        match (def.kind, def.item.kind) {
            (ir::DefKind::Param, ir::ItemKind::Param) => {
                out[d].push(design.row::<ir::item::Param>(def.item.index).value);
            }
            (ir::DefKind::Param, ir::ItemKind::Subckt) => {
                let s: ir::item::Subckt = design.row(def.item.index);
                let given = design.params(s.params).iter().filter(|p| p.0 == def.name);
                out[d].extend(given.map(|p| p.1).filter(|e| !e.is_none()));
            }
            (ir::DefKind::Func, ir::ItemKind::Func) => {
                out[d].push(design.row::<ir::item::Func>(def.item.index).body);
            }
            _ => {}
        }
    }
    for (item, _) in design.reachable() {
        let Some((subckt, actuals)) = instance(design, item) else {
            continue;
        };
        for &(name, e) in design.params(actuals) {
            let formal = defs
                .iter()
                .position(|f| f.kind == ir::DefKind::Param && f.item == subckt && f.name == name);
            if let Some(f) = formal {
                out[f].push(e);
            }
        }
    }
    out
}

/// The subcircuit item an instance places and its actuals, when `item` is one bound to a
/// subcircuit.
pub(super) fn instance(
    design: &ir::Design,
    item: ir::ItemRef,
) -> Option<(ir::ItemRef, ir::ParamList)> {
    if item.kind != ir::ItemKind::Element {
        return None;
    }
    let el: ir::Element = design.row(item.index);
    if el.kind != ir::ElemKind::Instance || !el.model.is_resolved() {
        return None;
    }
    let def = design.def(el.model.def);
    (def.kind == ir::DefKind::Subckt).then_some((def.item, el.params))
}

/// Every expression row reachable from a root of a reachable item, each once, roots first.
/// O(live rows).
pub(super) fn live(design: &ir::Design) -> Vec<ir::ExprId> {
    let mut seen = vec![false; design.expr_count()];
    let mut out = Vec::new();
    let mut handles = Vec::new();
    let mut stack = Vec::new();
    for (item, _) in design.reachable() {
        handles.clear();
        design.item_handles(item, &mut handles);
        for h in &handles {
            match *h {
                ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => stack.push(e),
                ir::Handle::Exprs(l) => stack.extend_from_slice(design.exprs(l)),
                ir::Handle::Params(l) => stack.extend(design.params(l).iter().map(|p| p.1)),
                ir::Handle::Branches(l) => stack.extend(design.branches(l).iter().map(|b| b.cond)),
                ir::Handle::Name(_)
                | ir::Handle::Ref(_)
                | ir::Handle::Names(_)
                | ir::Handle::Scope(_)
                | ir::Handle::Text(_) => {}
            }
        }
        while let Some(e) = stack.pop() {
            if e.is_none() || seen[e.index()] {
                continue;
            }
            seen[e.index()] = true;
            out.push(e);
            kids(design, e, &mut stack);
        }
    }
    out
}

/// Whether a typed division stands anywhere in the design: then a value's tag is observable
/// (A12 K1: only `DivTyped` reads a tag), else a rewrite may drop it.
pub(super) fn reads_tags(design: &ir::Design) -> bool {
    live(design)
        .into_iter()
        .any(|e| matches!(design.expr(e), ir::Expr::Binary(ir::BinOp::DivTyped, ..)))
}

/// Puts `code` at every item that still carries `feature` (`ir::check_legal`'s spans, the
/// items `legalize` will refuse), with the feature's index as its argument.
pub(super) fn note_left(
    design: &ir::Design,
    diags: &mut ir::Diags,
    feature: ir::Feature,
    code: ir::Code,
) {
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    for d in rows.rows() {
        if d.code == ir::diag::UNSUPPORTED && d.arg == index {
            diags.push(code, d.span, index);
        }
    }
}

/// `Binary(op)` or, in the dynamic form, `BinaryDyn(op)`.
pub(super) fn binary_form(op: ir::BinOp, dynamic: bool) -> ir::Feature {
    match dynamic {
        true => ir::Feature::BinaryDyn(op),
        false => ir::Feature::Binary(op),
    }
}

/// `Func(f)` or, in the dynamic form, `FuncDyn(f)`.
pub(super) fn func_form(f: ir::Func, dynamic: bool) -> ir::Feature {
    match dynamic {
        true => ir::Feature::FuncDyn(f),
        false => ir::Feature::Func(f),
    }
}

/// The value of a literal, or of a literal under negations (`-1` is `Neg(1)` in some readers).
pub(super) fn literal(design: &ir::Design, e: ir::ExprId) -> Option<f64> {
    let mut sign = 1.0;
    let mut at = e;
    for _ in 0..=design.expr_count() {
        match design.expr(at) {
            ir::Expr::Num(n) => return Some(sign * design.num(n).value()),
            ir::Expr::Unary(ir::UnOp::Neg, a) => (sign, at) = (-sign, a),
            _ => return None,
        }
    }
    unreachable!("a chain of negations is shorter than the table")
}
