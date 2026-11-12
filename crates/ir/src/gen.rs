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
//! `generate` picks kinds whose `Feature::Item` is in the set and calls each row's
//! `RowFacts::generate` (one exhaustive dispatch over `ItemKind`) with a [`Gen`], the state every
//! row's generator draws from.
//!
//! Contents: `pub struct Size`, `pub fn generate(u64, &FeatureSet, Size) -> Design`,
//! `pub struct Gen`, `Gen::below(&mut self, u32) -> u32`, `Gen::chance(&mut self, u32) -> bool`,
//! `Gen::literal(&mut self) -> Literal`, `Gen::fresh_name(&mut self, &mut Builder<'_>) -> NameId`,
//! `Gen::expr(&mut self, &mut Builder<'_>, &FeatureSet, Reads) -> ExprId`,
//! `Gen::pick_def(&mut self, &Design, ScopeId, DefKind) -> DefId`,
//! `Gen::budget(&self) -> u32`, `pub enum Reads`

use crate::build::Builder;
use crate::design::Design;
use crate::expr::Literal;
use crate::feature::FeatureSet;
use crate::id::{DefId, ExprId, NameId, ScopeId};
use crate::item::DefKind;

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

/// The generator's state: its PRNG, what is left of the size budget, and a counter for fresh
/// names. Deterministic given the seed.
#[derive(Clone, Debug)]
pub struct Gen {
    /// PRNG state (splitmix64).
    pub(crate) state: u64,
    /// The size being generated to.
    pub(crate) size: Size,
    /// Items still allowed.
    pub(crate) items_left: u32,
    /// Fresh names made so far (`g0`, `g1`, …).
    pub(crate) fresh: u32,
}

/// Whether a generated expression may read the circuit (`V`, `I`, `Time`, `Freq`, `Ddt`,
/// `Idt`): the field's context decides, as `check`'s `ProbeContext` does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reads {
    /// A parameter-like field: constants, parameters, `Temp` and the analysis leaves only.
    Constant,
    /// A field that may read the circuit (a behavioural source, a charge).
    Circuit,
}

/// A random design under `features`, at most `size`. A feature outside the set never appears;
/// a feature in it may not (the set is an upper bound).
pub fn generate(seed: u64, features: &FeatureSet, size: Size) -> Design {
    todo!()
}

impl Gen {
    /// A uniform integer in `0..n`. Asserts `n > 0`.
    pub fn below(&mut self, n: u32) -> u32 {
        todo!()
    }

    /// True with probability `1 / n`. Asserts `n > 0`.
    pub fn chance(&mut self, n: u32) -> bool {
        todo!()
    }

    /// A random valid literal: every `NumForm`, magnitudes `1e-15..1e9`, either sign, zero,
    /// trailing zeros in `frac`, so every writer's number spelling and the literal round trip are
    /// exercised.
    pub fn literal(&mut self) -> Literal {
        todo!()
    }

    /// A name never used before in this design (`g` and a counter), interned.
    pub fn fresh_name(&mut self, b: &mut Builder<'_>) -> NameId {
        todo!()
    }

    /// A random expression of depth at most `size.expr_depth`, using only `features` and only
    /// leaves `reads` allows. Parameter reads name definitions visible where it is used, or are
    /// left unresolved.
    pub fn expr(&mut self, b: &mut Builder<'_>, features: &FeatureSet, reads: Reads) -> ExprId {
        todo!()
    }

    /// An existing definition of `kind` visible from `scope`, chosen uniformly, or `NONE` when
    /// there is none.
    pub fn pick_def(&mut self, design: &Design, scope: ScopeId, kind: DefKind) -> DefId {
        todo!()
    }

    /// Items still allowed; a row generator that opens a scope fills its body only while this is
    /// positive.
    pub fn budget(&self) -> u32 {
        todo!()
    }
}
