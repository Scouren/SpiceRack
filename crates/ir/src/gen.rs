//! The design generator `Gen(F)`: a seed, a feature set and a size in, a random design out that
//! passes `check` and uses only features in the set (`features_used(d) ⊆ F`). The input of the
//! dialect laws L4 (writer totality) and L5 (round trip), and of the IR text law
//! `parse(dump(x)) == x`. Built only for tests and under the `gen` feature.
//!
//! Deterministic: the same seed, set and size give the same design (its own PRNG, no
//! dependency). Shrinking is by size: a failing seed is replayed with smaller sizes.
//!
//! Cites: `REWRITE-PLAN.md` §4.2 (`Gen(F)`); `rewrite/IR-KERNEL.md` §9.
//!
//! Contents: `pub struct Size`, `pub fn generate(u64, &FeatureSet, Size) -> Design`

use crate::design::Design;
use crate::feature::FeatureSet;

/// How big a generated design is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Most items over all scopes.
    pub items: u32,
    /// Deepest scope nesting below a file's top (subcircuits, sections, branches, loops).
    pub scope_depth: u8,
    /// Deepest expression tree.
    pub expr_depth: u8,
    /// Most files.
    pub files: u16,
}

/// A random design under `features`, at most `size`. A feature outside the set never appears;
/// a feature in it may not (the set is an upper bound).
pub fn generate(seed: u64, features: &FeatureSet, size: Size) -> Design {
    todo!()
}
