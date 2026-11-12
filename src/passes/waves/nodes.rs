//! What several wave passes read of a design: the independent sources and their waves, a
//! field's literal value, and the notes at items left carrying a feature.
//!
//! Cites: `crates/ir/src/source.rs`, `wave.rs`; `rewrite/SPINE.md` §4 (P2 by `Diag::item`).
//!
//! Contents: `pub(super) fn sources(&ir::Design) -> Vec<(ir::ItemRef, ir::source::Source)>`,
//! `pub(super) fn literal(&ir::Design, ir::ExprId) -> Option<f64>`,
//! `pub(super) fn note_left(&ir::Design, &mut ir::Diags, ir::Feature, ir::Code)`

use crate::ir;

/// Every reachable element with a source row, with that row, in listed order.
pub(super) fn sources(design: &ir::Design) -> Vec<(ir::ItemRef, ir::source::Source)> {
    let items = design.reachable().into_iter().map(|(i, _)| i);
    let elements = items.filter(|i| i.kind == ir::ItemKind::Element);
    elements
        .filter_map(|item| {
            let el: ir::Element = design.row(item.index);
            (el.kind.detail() == ir::DetailKind::Source).then(|| (item, design.source(&el)))
        })
        .collect()
}

/// The value of a literal, or of a literal under negations; `None` for any other expression
/// and for `NONE`.
pub(super) fn literal(design: &ir::Design, e: ir::ExprId) -> Option<f64> {
    let (mut sign, mut at) = (1.0, e);
    for _ in 0..=design.expr_count() {
        if at.is_none() {
            return None;
        }
        match design.expr(at) {
            ir::Expr::Num(n) => return Some(sign * design.literal_value(design.num(n))),
            ir::Expr::Unary(ir::UnOp::Neg, a) => (sign, at) = (-sign, a),
            _ => return None,
        }
    }
    unreachable!("a chain of negations is shorter than the table")
}

/// Puts `code` at every item that still carries `feature` (`ir::check_legal`'s items, the ones
/// `legalize` will refuse), with the feature's index as its argument.
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
            diags.push_at(code, design, d.item, index);
        }
    }
}
