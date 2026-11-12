//! Legalization: a design and a target's feature set in, the same design rewritten to use only
//! what the target has out, or `U0001` refusals (`ir::check_legal`'s, one at each item) for
//! what no pass removes. Passes know features, never dialects: this file (and each pass, one
//! per file under `passes/` once the first lands) imports `crate::ir` only
//! (`tests/layering.rs`).
//!
//! Cites: `REWRITE-PLAN.md` §4.4; `rewrite/SPINE.md` §4.
//!
//! Contents: `pub struct Pass`, `pub static PASSES: &[Pass]`,
//! `pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags)`

use crate::ir;

/// One IR → IR transform that removes a feature.
#[derive(Clone, Copy, Debug)]
pub struct Pass {
    /// Its name, for diagnostics and `--dump-ir=legal`.
    pub name: &'static str,
    /// The feature it removes.
    pub removes: ir::Feature,
    /// What the target must have for the pass's output to be legal.
    pub requires: ir::FeatureSet,
    /// The transform. It leaves a design `ir::check` accepts.
    pub run: fn(&mut ir::Design, &mut ir::Diags),
}

/// The registered passes, in the order `legalize` tries them. Empty in slice S2.
pub static PASSES: &[Pass] = &[];

/// Rounds of: the features `design` uses and `target` lacks; for each, the first pass of
/// [`PASSES`] that removes it and whose `requires` `target` has, run and then `ir::check`ed,
/// until nothing is missing or a round changes nothing. Then `ir::check_legal` reports what is
/// left: one `U0001` row per (item, feature) at the item. A check violation after a pass is
/// that pass's bug (a debug assertion).
///
/// # Panics
/// When the rounds do not settle within `PASSES.len() + 1` (passes that undo each other).
pub fn legalize(design: &mut ir::Design, target: &ir::FeatureSet, diags: &mut ir::Diags) {
    let mut missing = ir::features_used(design).minus(*target);
    let mut rounds = 0;
    while !missing.is_empty() {
        assert!(rounds <= PASSES.len(), "legalize: the passes do not settle");
        rounds += 1;
        for feature in missing.iter() {
            let usable = |p: &&Pass| p.removes == feature && p.requires.minus(*target).is_empty();
            let Some(pass) = PASSES.iter().find(usable) else {
                continue;
            };
            (pass.run)(design, diags);
            let checked = diags.len();
            ir::check(design, diags);
            debug_assert_eq!(
                diags.len(),
                checked,
                "pass {} made IR that fails ir::check",
                pass.name
            );
        }
        let left = ir::features_used(design).minus(*target);
        if left == missing {
            break;
        }
        missing = left;
    }
    ir::check_legal(design, target, diags);
}
