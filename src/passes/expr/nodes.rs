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
//! `pub(super) fn reads_tags(&ir::Design) -> bool`,
//! `pub(super) fn note_left(&ir::Design, &mut ir::Diags, ir::Feature, ir::Code)`,
//! `pub(super) fn both_forms(&ir::Design, ir::ItemRef) -> bool`,
//! `pub(super) fn carrying(&ir::Design, ir::Feature) -> Vec<ir::ItemRef>`,
//! `pub(super) fn samples(&ir::Design) -> bool`,
//! `pub(super) fn leaf_literal(&ir::Design, ir::ExprId) -> Option<ir::Literal>`,
//! `pub(super) fn binary_form(ir::BinOp, bool) -> ir::Feature`, `pub(super) fn func_form(ir::Func, bool) -> ir::Feature`,
//! `pub(super) fn literal(&ir::Design, ir::ExprId) -> Option<f64>`,
//! `pub(super) fn never_negative(&ir::Design, ir::ExprId) -> bool`,
//! `pub(super) fn form(bool) -> ir::Form`, `pub(super) const ROOT_PATHS_MAX`,
//! `pub(super) fn keep_bounded(&mut ir::Design, &mut dyn FnMut(&mut ir::Design)) -> Vec<ir::ItemRef>`,
//! `pub(super) fn paths(&ir::Design, ir::ExprId) -> u32`,
//! `pub(super) fn note_left_copying(&ir::Design, &mut ir::Diags, ir::Feature, ir::Code, &[ir::ItemRef])`

use super::{BOTH_FORMS, OPERAND_TOO_LARGE};
use crate::ir;

/// The most rows an expression a copying rule writes may spell, counted by paths (a row two
/// parents share counts at each use: what a writer writes). The longest expression of the two
/// corpora is 132 tokens (ngspice: spice-bench
/// `kicad-test-circuits/GaN_Test/GaN_Pspice_GS66508T_L2V2.lib`, a B source; VACASK: 33,
/// `test/test_viwfm.sim`); through the most copying rule (`round_half_away_to_floor` spells its
/// operand 9 times) and one more (4) that is under 4800; rounded up to a power of two.
///
/// ponytail: no let-binding to a new parameter (a copy bound once and read twice) in this
/// version; it lifts the bound, and it changes the design's definitions (a structure-level
/// decision): the upgrade path.
pub(super) const ROOT_PATHS_MAX: u32 = 8192;

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
        let defs = design.definitions_all();
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

    /// Whether the tree under `e` holds a draw or reads a definition that may. O(its rows):
    /// a row two parents share is visited once (a pass's output is a DAG).
    pub(super) fn holds(&self, design: &ir::Design, e: ir::ExprId) -> bool {
        let mut stack = vec![e];
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for _ in 0..=2 * design.expr_count_all() + 1 {
            let Some(e) = stack.pop() else { return false };
            if e.is_none() || !seen.insert(e) {
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
    let defs = design.definitions_all();
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

/// Whether a typed division stands anywhere in the design: then a value's tag is observable
/// (A12 K1: only `DivTyped` reads a tag), else a rewrite may drop it.
pub(super) fn reads_tags(design: &ir::Design) -> bool {
    design
        .live_exprs()
        .into_iter()
        .any(|e| matches!(design.expr(e), ir::Expr::Binary(ir::BinOp::DivTyped, ..)))
}

/// Runs `rewrite` (a copying rule's rewrite) and points back at its old tree every root it made
/// spell more than both [`ROOT_PATHS_MAX`] rows and what it spelled before (counted by paths);
/// returns the items whose root went back, which then keep the rule's feature. Bounds every
/// root a copying rule writes, whatever rule wrote it first: a copy made early grows with every
/// later rewrite under it, so the bound is on the root, not the operand.
pub(super) fn keep_bounded(
    design: &mut ir::Design,
    rewrite: &mut dyn FnMut(&mut ir::Design),
) -> Vec<ir::ItemRef> {
    let items: Vec<ir::ItemRef> = design.reachable().into_iter().map(|(i, _)| i).collect();
    let mut before: Vec<ir::ExprId> = Vec::new();
    {
        let mut editor = design.editor();
        for &item in &items {
            editor.map_exprs(item, &mut |e, _| before.push(*e));
        }
    }
    rewrite(design);
    let mut after: Vec<ir::ExprId> = Vec::new();
    {
        let mut editor = design.editor();
        for &item in &items {
            editor.map_exprs(item, &mut |e, _| after.push(*e));
        }
    }
    assert!(after.len() == before.len(), "a rewrite keeps every root");
    let back: Vec<bool> = before
        .iter()
        .zip(&after)
        .map(|(&old, &new)| {
            old != new
                && !new.is_none()
                && paths(design, new) > ROOT_PATHS_MAX.max(paths(design, old))
        })
        .collect();
    let mut reverted = Vec::new();
    let mut k = 0;
    let mut editor = design.editor();
    for &item in &items {
        let mut hit = false;
        editor.map_exprs(item, &mut |e, _| {
            if back[k] {
                *e = before[k];
                hit = true;
            }
            k += 1;
        });
        if hit {
            reverted.push(item);
        }
    }
    assert!(k == before.len(), "map_exprs visits the same roots");
    reverted
}

/// The number of rows under `e` counted by paths (what a writer spells), saturating at
/// `u32::MAX`. Visits each row once: children before parents, by id.
pub(super) fn paths(design: &ir::Design, e: ir::ExprId) -> u32 {
    let mut rows = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![e];
    let mut out = Vec::new();
    while let Some(r) = stack.pop() {
        if r.is_none() || !seen.insert(r) {
            continue;
        }
        rows.push(r);
        out.clear();
        kids(design, r, &mut out);
        stack.extend_from_slice(&out);
    }
    assert!(rows.len() <= design.expr_count_all(), "each row once");
    rows.sort_unstable();
    let mut size: std::collections::HashMap<ir::ExprId, u32> = std::collections::HashMap::new();
    for &r in &rows {
        out.clear();
        kids(design, r, &mut out);
        let below = out.iter().fold(1u32, |n, k| n.saturating_add(size[k]));
        size.insert(r, below);
    }
    size.get(&e).copied().unwrap_or(0)
}

/// [`note_left`] for a copying rule: an item [`keep_bounded`] put back gets `U0116`.
pub(super) fn note_left_copying(
    design: &ir::Design,
    diags: &mut ir::Diags,
    feature: ir::Feature,
    code: ir::Code,
    reverted: &[ir::ItemRef],
) {
    note_items(design, diags, feature, code, reverted);
}

/// Puts `code` (`U0112` at a function body read in both forms) at every item that still
/// carries `feature` (`ir::check_legal`'s items, the
/// ones `legalize` will refuse), with the feature's index as its argument.
pub(super) fn note_left(
    design: &ir::Design,
    diags: &mut ir::Diags,
    feature: ir::Feature,
    code: ir::Code,
) {
    note_items(design, diags, feature, code, &[]);
}

/// Whether `item` is a function whose body its callers read in both forms (A28): no one-form
/// row rewrites it, whatever the rule.
pub(super) fn both_forms(design: &ir::Design, item: ir::ItemRef) -> bool {
    if item.kind != ir::ItemKind::Func {
        return false;
    }
    let body = design.row::<ir::item::Func>(item.index).body;
    ir::root_form(design, body, ir::ExprUse::Circuit) == ir::Form::Both
}

/// The items that carry `feature` (`ir::check_legal` against every feature but it), in its
/// order.
pub(super) fn carrying(design: &ir::Design, feature: ir::Feature) -> Vec<ir::ItemRef> {
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let rows = rows.rows().iter();
    rows.filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index)
        .map(|d| d.item)
        .collect()
}

/// Whether the design runs a sampling loop (a reachable `Loop` of kind `MonteCarlo`): only
/// there does a `DrawWhen::Sampling` draw yield a sample.
pub(super) fn samples(design: &ir::Design) -> bool {
    design.reachable().into_iter().any(|(item, _)| {
        item.kind == ir::ItemKind::Loop
            && design.row::<ir::Loop>(item.index).kind == ir::LoopKind::MonteCarlo
    })
}

/// The literal a leaf may be replaced by: `e`'s own when it is one, Real (an `Int` literal is
/// respelled `Plain`, its value unchanged: the leaf it replaces is Real).
pub(super) fn leaf_literal(design: &ir::Design, e: ir::ExprId) -> Option<ir::Literal> {
    if e.is_none() {
        return None;
    }
    let ir::Expr::Num(n) = design.expr(e) else {
        return None;
    };
    let l = design.num(n);
    Some(match l.form {
        ir::NumForm::Int => ir::Literal {
            form: ir::NumForm::Plain,
            ..l
        },
        _ => l,
    })
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

/// Whether `x` is `>= +0` on every evaluation (never negative, never `-0`).
pub(super) fn never_negative(d: &ir::Design, x: ir::ExprId) -> bool {
    match d.expr(x) {
        ir::Expr::Num(n) => d.literal_value(d.num(n)).is_sign_positive(),
        ir::Expr::Call(f, _) => matches!(
            f,
            ir::Func::Abs | ir::Func::SqrtAbs | ir::Func::Exp | ir::Func::Sqr | ir::Func::PowAbs
        ),
        _ => false,
    }
}

/// The value of a literal, or of a literal under negations (`-1` is `Neg(1)` in some readers).
pub(super) fn literal(design: &ir::Design, e: ir::ExprId) -> Option<f64> {
    let mut sign = 1.0;
    let mut at = e;
    for _ in 0..=design.expr_count_all() {
        match design.expr(at) {
            ir::Expr::Num(n) => return Some(sign * design.literal_value(design.num(n))),
            ir::Expr::Unary(ir::UnOp::Neg, a) => (sign, at) = (-sign, a),
            _ => return None,
        }
    }
    unreachable!("a chain of negations is shorter than the table")
}

/// The expression form a row of a rule pair rewrites: `Dynamic` for its `_dyn` row.
pub(super) fn form(dynamic: bool) -> ir::Form {
    match dynamic {
        true => ir::Form::Dynamic,
        false => ir::Form::Static,
    }
}

// ---- private ----

/// The notes of [`note_left`] and [`note_left_copying`].
fn note_items(
    design: &ir::Design,
    diags: &mut ir::Diags,
    feature: ir::Feature,
    code: ir::Code,
    reverted: &[ir::ItemRef],
) {
    let index = u32::from(feature.index());
    // only a function's or operator's feature has a form (A26)
    let split = matches!(
        feature,
        ir::Feature::Func(_)
            | ir::Feature::FuncDyn(_)
            | ir::Feature::Binary(_)
            | ir::Feature::BinaryDyn(_)
            | ir::Feature::Unary(_)
            | ir::Feature::UnaryDyn(_)
    );
    for item in carrying(design, feature) {
        let code = if split && both_forms(design, item) {
            BOTH_FORMS
        } else if reverted.contains(&item) {
            OPERAND_TOO_LARGE
        } else {
            code
        };
        diags.push_at(code, design, item, index);
    }
}
