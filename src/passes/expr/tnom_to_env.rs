//! Pass `tnom_to_env`: a design in, the same design out with every nominal-temperature leaf
//! (`Tnom`) replaced by the design's one nominal temperature: the `tnom` of its one `Env` that
//! sets it, held for the whole run, when that is a literal (an `Int` one respelled Real, the
//! leaf being Real). A design with no such `Env` (the leaf is then the simulator's own default,
//! which differs between simulators), with two, or with one whose `tnom` is not a literal keeps
//! its leaves with note `U0108`. The leaf is no function or operator, so one row covers both
//! forms.
//!
//! Removes `Expr(Tnom)`, requires `Expr(Num)`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §6.2; `crates/ir/src/option.rs` (`Env`).
//!
//! Contents: `pub fn tnom_to_env(&mut ir::Design, &mut ir::Diags)`

use super::NO_LEAF_VALUE;
use super::nodes::{leaf_literal, note_left};
use crate::ir;

/// Replaces every `Tnom` leaf by the one `Env.tnom` literal, or notes every item holding one.
pub fn tnom_to_env(design: &mut ir::Design, diags: &mut ir::Diags) {
    let set: Vec<ir::Env> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Env)
        .map(|(i, _)| design.row::<ir::Env>(i.index))
        .filter(|env| !env.tnom.is_none())
        .collect();
    let value = match set.as_slice() {
        [env] if env.holds == ir::Applies::WholeRun => leaf_literal(design, env.tnom),
        _ => None,
    };
    if let Some(l) = value {
        ir::rewrite_exprs(design, &mut |b, e| {
            matches!(b.design().expr(e), ir::Expr::Tnom).then(|| b.literal(l))
        });
    }
    note_left(
        design,
        diags,
        ir::Feature::Expr(ir::ExprTag::Tnom),
        NO_LEAF_VALUE,
    );
}
