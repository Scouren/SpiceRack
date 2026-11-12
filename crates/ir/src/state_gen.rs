//! The state items' generator helpers: a design under construction and a feature set in, a
//! state item's mode or an `nfreqs` option out (split from `option.rs`).
//!
//! Cites: `rewrite/AMEND-A12.md` A14, A16; `crates/ir/src/option.rs` docs.
//!
//! Contents: `pub(crate) fn gen_nfreqs`, `pub(crate) fn gen_holds`

use crate::build::Builder;
use crate::feature::{Feature, FeatureSet};
use crate::r#gen::{Gen, Reads};
use crate::id::{NameId, ScopeId};
use crate::option::{Applies, OptionRow, mode_feature};
use crate::option_key::OptionKey;
use crate::run_extra::RunExtra;
use crate::run_words::has;
use crate::span::Span;

/// Pushes an `nfreqs` option into `scope` for a Fourier analysis about to follow it (its count
/// then comes from the option, A16), when the set allows one there. Whether it pushed.
pub(crate) fn gen_nfreqs(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    features: &FeatureSet,
) -> bool {
    let allowed = features.contains(Feature::Item(crate::item::ItemKind::Option))
        && features.contains(Feature::Option(OptionKey::NFreqs))
        && crate::r#gen::placeable(b.design(), scope, crate::item::ItemKind::Option, features)
        && g.items_left > 1;
    let Some(holds) = allowed
        .then(|| gen_holds(g, b, scope, features, RunExtra::OptionMidRun, mode_feature))
        .flatten()
    else {
        return false;
    };
    let value = g.expr(b, features, Reads::Constant);
    let row = OptionRow {
        key: OptionKey::NFreqs,
        private: NameId::NONE,
        dialect: NameId::NONE,
        value,
        holds,
    };
    b.push(scope, row, Span::NONE);
    g.items_left -= 1;
    true
}

/// The mode of a state item generated into `scope`, or `None` when the set allows neither: once
/// any run item exists (conservatively: anywhere, since items are generated in listed order),
/// `FromHere` needs `mid` and `WholeRun` needs `WholeRunMidRun` (placement is `fill`'s, A15 K1);
/// each mode also needs its own feature, `mode(m)` (a state item's or a save's).
pub(crate) fn gen_holds(
    g: &mut Gen,
    b: &Builder<'_>,
    _scope: ScopeId,
    features: &FeatureSet,
    mid: RunExtra,
    mode: fn(Applies) -> RunExtra,
) -> Option<Applies> {
    let d = b.design();
    let after = !d.analyses.is_empty() || !d.loops.is_empty() || !d.alters.is_empty();
    let modes: Vec<Applies> = [
        (Applies::FromHere, mid),
        (Applies::WholeRun, RunExtra::OptionWholeRunMidRun),
    ]
    .into_iter()
    .filter(|&(m, x)| (!after || has(features, x)) && has(features, mode(m)))
    .map(|(m, _)| m)
    .collect();
    (!modes.is_empty()).then(|| modes[g.below(modes.len() as u32) as usize])
}
