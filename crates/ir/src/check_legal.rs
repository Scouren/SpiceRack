//! The target's side of validity: a checked design and a target's feature set in, one `U`
//! diagnostic per (item, feature) the design uses outside the set out. Split from `check.rs` at
//! its size limit; `check::check_legal` re-exports it.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9; RULINGS R10 (these rows are `legalize`'s leftovers).
//!
//! Contents: `pub fn check_legal(&Design, &FeatureSet, &mut Diags)`

use crate::design::Design;
use crate::diag::{Diags, UNSUPPORTED};
use crate::feature::{ExprWalk, FeatureSet, item_features, unplaced_level};
use crate::span::Span;

/// Appends one `UNSUPPORTED` row per (item, feature) for each feature an item uses outside
/// `legal` (span: the item; `arg`: `Feature::index`), and one at `Span::NONE` per feature of the
/// whole design outside it (the owners' `design_features`); a case clash at the item that brings
/// the second spelling. It appends nothing
/// exactly when `features_used(design).minus(legal)` is empty: these rows are `legalize`'s
/// leftovers (R10). Assumes `check` passed. O(rows).
pub fn check_legal(design: &Design, legal: &FeatureSet, diags: &mut Diags) {
    let mut walk = ExprWalk::new(design);
    let clashes = crate::case::case_clashes(design);
    for (item, scope) in design.reachable() {
        let mut used = FeatureSet::EMPTY;
        item_features(design, item, scope, &mut walk, &mut used);
        if clashes.contains(&item) {
            used = used.with(crate::feature::Feature::Extra(
                crate::feature::Extra::CaseSensitiveNames,
            ));
        }
        for f in used.minus(*legal).iter() {
            diags.push_at(UNSUPPORTED, design, item, f.index() as u32);
        }
    }
    // the run owner's design-level features name their item (`analysis::feature_items`)
    let mut placed = Vec::new();
    crate::analysis::feature_items(design, &mut placed);
    for (f, item) in placed {
        if !legal.contains(f) {
            diags.push_at(UNSUPPORTED, design, item, f.index() as u32);
        }
    }
    let mut whole = FeatureSet::EMPTY;
    unplaced_level(design, &mut whole);
    for f in whole.minus(*legal).iter() {
        diags.push(UNSUPPORTED, Span::NONE, f.index() as u32);
    }
}
