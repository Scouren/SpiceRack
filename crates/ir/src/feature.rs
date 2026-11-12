//! Features: everything a target may be unable to spell, as one closed enum, and the fixed bitset
//! each writer declares (`const FEATURES: FeatureSet`). A design in, the set of features it uses
//! out (`features_used`, one scan); legalization compares that set with the target's.
//!
//! Every kind is a feature (`Item`, `Elem`, `Wave`, `Analysis`, `Expr`, `Func`, `Binary`,
//! `Unary`, `Dist`, and a draw's `DrawWhen` and `DrawPer`); a function or operator used inside an expression that holds a `V`, `I`,
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
use crate::expr::{BinOp, Dist, DrawPer, DrawWhen, ExprTag, Func, UnOp};
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
    /// When a draw samples (Addendum A2).
    DrawWhen(DrawWhen),
    /// How often a draw samples (Addendum A2).
    DrawPer(DrawPer),
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
    // multiplicity (`Element::mult` not `NONE`; INV-semantics 6.01–6.03, 5.16)
    /// `mult` on a resistor.
    MultResistor,
    /// `mult` on a capacitor (ESPice ignores it, 6.02).
    MultCapacitor,
    /// `mult` on an inductor (ESPice ignores it, 6.02).
    MultInductor,
    /// `mult` on a voltage source (Xyce ignores it; ngspice refuses `V … M=`, 5.16).
    MultVSource,
    /// `mult` on a current source (Xyce ignores it, 5.16).
    MultISource,
    /// `mult` on a controlled source (linear, `Poly`, `Table`, `Behav`, `Laplace`, `FreqTable`, `Delay`).
    MultControlled,
    /// `mult` on a diode, BJT, MOSFET, JFET, MESFET or IGBT.
    MultSemi,
    /// `mult` on a subcircuit instance (ESPice ignores it, B10, 6.01).
    MultInstance,
    /// `mult` on a code model or module instance (VACASK `$mfactor`, B24).
    MultModule,
    /// `mult` on any other kind (port, switch, line, charge or flux element).
    MultOther,
    // initial conditions (IR-CIRCUIT §1, D6)
    /// A device `ic` applied only under `uic` (`IcMode::UicOnly`).
    IcUicOnly,
    /// A device `ic` held in the operating point (`IcMode::InOp`).
    IcInOp,
    // passive and instance fields
    /// An R, C or L whose `value` reads the circuit (ngspice runs it as a B source, 6.14, 6.16).
    PassiveDynamicValue,
    /// An R, C or L with `noisy == false` (D82, 6.17).
    PassiveNoiseless,
    /// A resistor's AC resistance (`Passive::ac`, ng only, 6.17).
    PassiveAc,
    /// The ngspice instance scale (`Passive::scale`, 6.07).
    PassiveScale,
    /// Instance `tc1`/`tc2` on an R, C or L (6.12).
    PassiveTc,
    /// The exponential coefficient `tce` (6.13).
    PassiveTce,
    /// An instance's absolute temperature (`temp_abs` on `Passive`, `Semi`, `Behav`).
    InstanceTemp,
    /// An instance's temperature offset (`dtemp` on `Passive`, `Semi`, `Behav`).
    InstanceDtemp,
    /// LTspice parasitics on a capacitor or inductor (`Passive::parasitics`, D8).
    ElementParasitics,
    /// LTspice parasitics on a voltage source (`Source::parasitics`, D8).
    SourceParasitics,
    /// A behavioural source's parallel resistance (`Behav::rpar`, lt, D101).
    BehavRpar,
    // couplings
    /// A coupling of three or more inductors on one row (xy, lt; ngspice expands, D56).
    CouplingMany,
    /// A coupling naming a nonlinear core model (xy, D11, D114).
    CouplingCore,
    // sources
    /// A source temperature factor (`Source::tc1`/`tc2`, sp, 5.19).
    SourceTc,
    /// A periodic-AC stimulus (`Source::pac_mag`, sp, D129).
    SourcePac,
    /// A small-signal noise PSD on a source (`Source::noise_psd`, sp, 5.18).
    SourceNoisePsd,
    /// Distortion inputs (`Source::distof`, ng, 5.21).
    SourceDistof,
    /// A port noise temperature (`PortSpec::noise_temp`, sp, 5.17).
    PortNoiseTemp,
    /// A port large-signal drive (`PortSpec::power`, ng, E24).
    PortPower,
    // waveform fields
    /// A pulse count (`Pulse::cycles`, ng NP, lt Ncycles, 5.04).
    PulseCycles,
    /// A sine cycle count (`Sin::cycles`, lt, 5.06).
    SinCycles,
    /// A sine that is its offset before the delay (`PreDelay::Offset`).
    SinPreOffset,
    /// A sine that holds its t = delay value before the delay (`PreDelay::HoldDelayValue`).
    SinPreHold,
    /// A sine that is 0 before the delay (`PreDelay::Zero`).
    SinPreZero,
    /// A sine's second tone (`Sin::amplitude2`, sp, 5.07).
    SinSecondTone,
    /// An SFFM under `SffmLaw::Spice3`.
    SffmSpice3,
    /// An SFFM under `SffmLaw::Ngspice`.
    SffmNgspice,
    /// An AM that is its offset before the delay.
    AmPreOffset,
    /// An AM that holds its t = delay value before the delay (va).
    AmPreHold,
    /// An AM that is 0 before the delay (ng).
    AmPreZero,
    /// An AM whose modulation time is shifted by the carrier phase (`AmModTime::CarrierPhase`, sp).
    AmModCarrierPhase,
    /// A PWL delay (`Pwl::delay`, E7).
    PwlDelay,
    /// A PWL replaying from a time (`PwlRepeat::From`, 5.11).
    PwlRepeatFrom,
    /// A PWL repeating with a period (`PwlRepeat::Period`, va, sp).
    PwlRepeatPeriod,
    /// A finite PWL repeat count (`Pwl::repeat_count`, lt).
    PwlRepeatCount,
    /// A PWL repeat that ramps across the gap (`PwlJoin::Ramp`, lt).
    PwlJoinRamp,
    /// A PWL repeat that ramps over a width (`PwlJoin::RampWidth`, sp).
    PwlJoinRampWidth,
    /// A PWL repeat whose wrap extrapolates the first segment (`PwlJoin::Extrapolate`, va).
    PwlJoinExtrapolate,
    /// PWL points in a file (`Pwl::file`, 5.12).
    PwlFile,
    // controlled sources
    /// A table under `TableLaw::Exact`.
    TableExact,
    /// A table under `TableLaw::NgspiceRounded`.
    TableNgspiceRounded,
    /// A table under `TableLaw::Smoothed`.
    TableSmoothed,
    /// Temperature terms on a behavioural source (`Behav::tc1`, `tc2`, ng, D63).
    BehavTc,
    /// A behavioural `OutKind::Resistance`.
    BehavResistance,
    /// A behavioural `OutKind::Conductance`.
    BehavConductance,
    /// A behavioural `OutKind::Capacitance`.
    BehavCapacitance,
    /// A behavioural `OutKind::CapacitanceCharge`.
    BehavCapacitanceCharge,
    /// A behavioural `OutKind::Inductance`.
    BehavInductance,
    /// A behavioural `OutKind::Power`.
    BehavPower,
    /// A Laplace source in `LaplaceForm::PoleZero`.
    LaplacePoleZero,
    /// A Laplace or frequency-table source whose input is an expression.
    LaplaceExprInput,
    /// A frequency table under `FreqLaw::ComplexLinear`.
    FreqComplexLinear,
    /// A frequency table under `FreqLaw::DbLogFreq`.
    FreqDbLogFreq,
    /// A delay under `DelayLaw::Sampled`.
    DelaySampled,
    // semiconductors and switches
    /// A semiconductor area factor (`Semi::area`).
    SemiArea,
    /// A semiconductor first guess (`Semi::initial` not `Unset`).
    SemiInitial,
    /// A diode with a thermal terminal (3 terminals, ng, D65).
    DiodeThermal,
    /// A switch first guess (`Switch::initial` not `Unset`, D57).
    SwitchInitial,
    /// A switch controlled by an expression (`Switch::control`, xy, D118).
    SwitchControlExpr,
    // lines
    /// A lossless line given by `freq` and `nl` (D31).
    LineFreqNl,
    /// A lossless line given by `length` and `vel` (sp, D31).
    LineLengthVel,
    /// An explicit URC lump count (`Line::lumps`).
    LineLumps,
    // models
    /// A model that is a bin (`Model::bin_of`, F7).
    ModelBin,
    /// A model derived by `ako:` (`Model::ako`, F8).
    ModelAko,
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
        + DrawWhen::COUNT
        + DrawPer::COUNT
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
        Extra::MultResistor,
        Extra::MultCapacitor,
        Extra::MultInductor,
        Extra::MultVSource,
        Extra::MultISource,
        Extra::MultControlled,
        Extra::MultSemi,
        Extra::MultInstance,
        Extra::MultModule,
        Extra::MultOther,
        Extra::IcUicOnly,
        Extra::IcInOp,
        Extra::PassiveDynamicValue,
        Extra::PassiveNoiseless,
        Extra::PassiveAc,
        Extra::PassiveScale,
        Extra::PassiveTc,
        Extra::PassiveTce,
        Extra::InstanceTemp,
        Extra::InstanceDtemp,
        Extra::ElementParasitics,
        Extra::SourceParasitics,
        Extra::BehavRpar,
        Extra::CouplingMany,
        Extra::CouplingCore,
        Extra::SourceTc,
        Extra::SourcePac,
        Extra::SourceNoisePsd,
        Extra::SourceDistof,
        Extra::PortNoiseTemp,
        Extra::PortPower,
        Extra::PulseCycles,
        Extra::SinCycles,
        Extra::SinPreOffset,
        Extra::SinPreHold,
        Extra::SinPreZero,
        Extra::SinSecondTone,
        Extra::SffmSpice3,
        Extra::SffmNgspice,
        Extra::AmPreOffset,
        Extra::AmPreHold,
        Extra::AmPreZero,
        Extra::AmModCarrierPhase,
        Extra::PwlDelay,
        Extra::PwlRepeatFrom,
        Extra::PwlRepeatPeriod,
        Extra::PwlRepeatCount,
        Extra::PwlJoinRamp,
        Extra::PwlJoinRampWidth,
        Extra::PwlJoinExtrapolate,
        Extra::PwlFile,
        Extra::TableExact,
        Extra::TableNgspiceRounded,
        Extra::TableSmoothed,
        Extra::BehavTc,
        Extra::BehavResistance,
        Extra::BehavConductance,
        Extra::BehavCapacitance,
        Extra::BehavCapacitanceCharge,
        Extra::BehavInductance,
        Extra::BehavPower,
        Extra::LaplacePoleZero,
        Extra::LaplaceExprInput,
        Extra::FreqComplexLinear,
        Extra::FreqDbLogFreq,
        Extra::DelaySampled,
        Extra::SemiArea,
        Extra::SemiInitial,
        Extra::DiodeThermal,
        Extra::SwitchInitial,
        Extra::SwitchControlExpr,
        Extra::LineFreqNl,
        Extra::LineLengthVel,
        Extra::LineLumps,
        Extra::ModelBin,
        Extra::ModelAko,
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

/// Every feature the design uses, in one scan over its reachable items: per item, its
/// `Feature::Item`, its row's `RowFacts::features` (one exhaustive dispatch over `ItemKind`), and
/// the expression features of the expressions in its `RowFacts::handles`.
/// An expression's functions and operators count as `*Dyn` when the expression holds a `V`,
/// `I`, `Time` or `Freq` leaf.
pub fn features_used(design: &Design) -> FeatureSet {
    todo!()
}
