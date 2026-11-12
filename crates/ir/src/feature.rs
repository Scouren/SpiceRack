//! Features: everything a target may be unable to spell, as one closed enum, and the fixed bitset
//! each writer declares (`const FEATURES: FeatureSet`). A design in, the set of features it uses
//! out (`features_used`, one scan); legalization compares that set with the target's.
//!
//! Every kind is a feature (`Item`, `Elem`, `Wave`, `Analysis`, `Expr`, `Func`, `Binary`,
//! `Unary`, `Dist`); a function or operator used inside an expression that holds a `V`, `I`,
//! `Time` or `Freq` leaf is its `*Dyn` feature instead (Addendum A1: legality is per context);
//! `Extra` lists the features that are fields, not kinds. Feature indices are dense, in that
//! order, so a set is `Feature::COUNT` bits.
//!
//! `Extra` has three regions. The kernel's is fixed; the circuit and the run owner each append
//! only in their marked region, in the enum and in `Extra::ALL`, so their edits merge.
//!
//! Cites: `rewrite/IR-KERNEL.md` §8, Addendum A1; `REWRITE-PLAN.md` §3.5, §4.4.
//!
//! Contents:
//! - `pub enum Feature`, `Feature::COUNT`, `Feature::index(self) -> u16`,
//!   `Feature::from_index(u16) -> Feature`
//! - `pub enum Extra`, `Extra::ALL`, `Extra::COUNT`
//! - `pub struct FeatureSet`, `FeatureSet::EMPTY`, `FeatureSet::with(self, Feature) -> FeatureSet`,
//!   `FeatureSet::union(self, FeatureSet) -> FeatureSet`,
//!   `FeatureSet::minus(self, FeatureSet) -> FeatureSet`,
//!   `FeatureSet::contains(&self, Feature) -> bool`, `FeatureSet::is_empty(&self) -> bool`,
//!   `FeatureSet::iter(&self) -> impl Iterator<Item = Feature>`
//! - `pub fn features_used(&Design) -> FeatureSet`

use crate::analysis::AnalysisKind;
use crate::design::Design;
use crate::element::ElemKind;
use crate::expr::{BinOp, Dist, ExprTag, Func, UnOp};
use crate::item::ItemKind;
use crate::wave::WaveKind;

/// A feature: one kind, one context-qualified operator, or one field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Feature {
    /// An item kind.
    Item(ItemKind),
    /// An element kind.
    Elem(ElemKind),
    /// A waveform kind.
    Wave(WaveKind),
    /// An analysis kind.
    Analysis(AnalysisKind),
    /// An expression tag (`UserCall`, `Str`, `List`, the `TStep`/`TStop`/`Tnom` leaves, …).
    Expr(ExprTag),
    /// A function, in an expression that does not read the circuit.
    Func(Func),
    /// A function, in an expression that reads the circuit.
    FuncDyn(Func),
    /// A binary operator, in an expression that does not read the circuit.
    Binary(BinOp),
    /// A binary operator, in an expression that reads the circuit.
    BinaryDyn(BinOp),
    /// A unary operator, in an expression that does not read the circuit.
    Unary(UnOp),
    /// A unary operator, in an expression that reads the circuit.
    UnaryDyn(UnOp),
    /// A statistical draw's distribution.
    Dist(Dist),
    /// A field-level feature.
    Extra(Extra),
}

/// The features that are fields or placements, not kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Extra {
    // ---- kernel ----
    /// A `Subckt` inside a subcircuit body.
    NestedSubckt,
    /// A `Model` inside a subcircuit body.
    LocalModel,
    /// A `Func` inside a subcircuit body.
    LocalFunc,
    /// A `Param` inside an `If` branch.
    ParamInBranch,
    /// A `Load` inside an `If` branch.
    LoadInBranch,
    /// An `Include` of one section (`section != NONE`): a library call.
    IncludeSection,
    /// A `Section` wrapped in a library (`library != NONE`).
    SectionLibrary,
    /// Two names of the design that differ only in ASCII case: a case-folding target would
    /// merge them.
    CaseSensitiveNames,
    // ---- circuit owner appends below ----
    //
    //
    //
    //
    //
    //
    // ---- run owner appends below ----
}

/// A fixed set of features: one bit per `Feature::index`. `Copy`, built in `const` context
/// (`FeatureSet::EMPTY.with(f).with(g)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FeatureSet {
    words: [u64; WORDS],
}

const WORDS: usize = (Feature::COUNT as usize).div_ceil(64);

impl Feature {
    /// Feature count; indices are `0..COUNT`.
    pub const COUNT: u16 = (ItemKind::COUNT
        + ElemKind::COUNT
        + WaveKind::COUNT
        + AnalysisKind::COUNT
        + ExprTag::COUNT
        + 2 * Func::COUNT
        + 2 * BinOp::COUNT
        + 2 * UnOp::COUNT
        + Dist::COUNT
        + Extra::COUNT) as u16;

    /// The dense index: the variant's offset (the counts of the variants before it, in
    /// declaration order) plus the member's position in its `ALL`.
    pub const fn index(self) -> u16 {
        todo!()
    }

    /// The feature at `index`. Asserts `index < COUNT`.
    pub const fn from_index(index: u16) -> Feature {
        todo!()
    }
}

impl Extra {
    /// Every member, in declaration order. A slice, so an owner's append changes no length.
    pub const ALL: &[Extra] = &[
        // ---- kernel ----
        Extra::NestedSubckt,
        Extra::LocalModel,
        Extra::LocalFunc,
        Extra::ParamInBranch,
        Extra::LoadInBranch,
        Extra::IncludeSection,
        Extra::SectionLibrary,
        Extra::CaseSensitiveNames,
        // ---- circuit owner appends below ----
        //
        //
        //
        //
        //
        //
        // ---- run owner appends below ----
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl FeatureSet {
    /// The empty set.
    pub const EMPTY: FeatureSet = FeatureSet { words: [0; WORDS] };

    /// This set and `f`.
    pub const fn with(self, f: Feature) -> FeatureSet {
        todo!()
    }

    /// Both sets.
    pub const fn union(self, other: FeatureSet) -> FeatureSet {
        todo!()
    }

    /// This set without `other`'s members (what a design uses that a target lacks:
    /// `used.minus(target)`).
    pub const fn minus(self, other: FeatureSet) -> FeatureSet {
        todo!()
    }

    /// Whether `f` is in the set.
    pub const fn contains(&self, f: Feature) -> bool {
        todo!()
    }

    /// Whether the set has no member.
    pub const fn is_empty(&self) -> bool {
        todo!()
    }

    /// The members, in index order.
    pub fn iter(&self) -> impl Iterator<Item = Feature> + use<> {
        // A real body: `todo!()` does not satisfy an `impl Iterator` return.
        let set = *self;
        (0..Feature::COUNT)
            .filter(move |&i| set.words[i as usize / 64] >> (i % 64) & 1 == 1)
            .map(Feature::from_index)
    }
}

/// Every feature the design uses, in one scan over its reachable items and their expressions.
/// An expression's functions and operators count as `*Dyn` when the expression holds a `V`,
/// `I`, `Time` or `Freq` leaf.
pub fn features_used(design: &Design) -> FeatureSet {
    todo!()
}
