//! Passes `substitute_tstep` and `substitute_tstop`: a design in, the same design out with every
//! transient leaf (`TStep`, ngspice's `TSTEP`, the transient's output interval; `TStop`, its
//! stop time) replaced by that field of the design's one transient analysis, when that is a
//! literal (an `Int` one respelled Real, the leaf being Real). A design with no transient, with
//! two, or with a field that is not given or not a literal keeps its leaves with note `U0108`.
//! One row per leaf: the leaf is no function or operator, so one row covers both forms.
//!
//! `substitute_tstep` removes `Expr(TStep)`, `substitute_tstop` removes `Expr(TStop)`; both
//! require `Expr(Num)`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.1 (RUN p1); `crates/ir/src/tran.rs` (`Tran`).
//!
//! Contents: `pub fn substitute_tstep(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn substitute_tstop(&mut ir::Design, &mut ir::Diags)`

use super::NO_LEAF_VALUE;
use super::nodes::{leaf_literal, note_left};
use crate::ir;

/// Replaces every `TStep` leaf by the one transient's `print_step` literal.
pub fn substitute_tstep(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, ir::ExprTag::TStep);
}

/// Replaces every `TStop` leaf by the one transient's `stop` literal.
pub fn substitute_tstop(design: &mut ir::Design, diags: &mut ir::Diags) {
    run(design, diags, ir::ExprTag::TStop);
}

// ---- private ----

fn run(design: &mut ir::Design, diags: &mut ir::Diags, leaf: ir::ExprTag) {
    let trans: Vec<ir::tran::Tran> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Analysis)
        .map(|(i, _)| design.row::<ir::Analysis>(i.index))
        .filter(|a| a.kind == ir::AnalysisKind::Tran)
        .map(|a| design.detail::<ir::tran::Tran>(a.detail))
        .collect();
    let value = match trans.as_slice() {
        [t] if leaf == ir::ExprTag::TStep => leaf_literal(design, t.print_step),
        [t] => leaf_literal(design, t.stop),
        _ => None,
    };
    if let Some(l) = value {
        ir::rewrite_exprs(design, &mut |b, e| {
            (b.design().expr(e).tag() == leaf).then(|| b.literal(l))
        });
    }
    note_left(design, diags, ir::Feature::Expr(leaf), NO_LEAF_VALUE);
}
