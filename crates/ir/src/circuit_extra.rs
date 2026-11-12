//! The circuit owner's field-level features: what a circuit row states that some target cannot
//! spell and that is a field, not a kind. A design in (through each circuit row's
//! `RowFacts::features`), members of `Feature::Circuit` out. Split from the kernel's `Extra` (A7)
//! so the circuit owner adds a feature without touching a kernel file; members, names and docs
//! are as the circuit owner wrote them in `feature.rs`.
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-DECISIONS.md` A7).
//!
//! Cites: `rewrite/IR-CIRCUIT.md`; `rewrite/COVERAGE-circuit.md`; `rewrite/INV-semantics.md` categories 5–7.
//!
//! Contents: `pub enum CircuitExtra` (`ALL`, `COUNT`)

/// The circuit owner's field-level features. `#[repr(u8)]`: up to 256 members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CircuitExtra {
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
}

impl CircuitExtra {
    /// Every member, in declaration order (the order of `Feature::Circuit` indices).
    pub const ALL: [CircuitExtra; 76] = [
        CircuitExtra::MultResistor,
        CircuitExtra::MultCapacitor,
        CircuitExtra::MultInductor,
        CircuitExtra::MultVSource,
        CircuitExtra::MultISource,
        CircuitExtra::MultControlled,
        CircuitExtra::MultSemi,
        CircuitExtra::MultInstance,
        CircuitExtra::MultModule,
        CircuitExtra::MultOther,
        CircuitExtra::IcUicOnly,
        CircuitExtra::IcInOp,
        CircuitExtra::PassiveDynamicValue,
        CircuitExtra::PassiveNoiseless,
        CircuitExtra::PassiveAc,
        CircuitExtra::PassiveScale,
        CircuitExtra::PassiveTc,
        CircuitExtra::PassiveTce,
        CircuitExtra::InstanceTemp,
        CircuitExtra::InstanceDtemp,
        CircuitExtra::ElementParasitics,
        CircuitExtra::SourceParasitics,
        CircuitExtra::BehavRpar,
        CircuitExtra::CouplingMany,
        CircuitExtra::CouplingCore,
        CircuitExtra::SourceTc,
        CircuitExtra::SourcePac,
        CircuitExtra::SourceNoisePsd,
        CircuitExtra::SourceDistof,
        CircuitExtra::PortNoiseTemp,
        CircuitExtra::PortPower,
        CircuitExtra::PulseCycles,
        CircuitExtra::SinCycles,
        CircuitExtra::SinPreOffset,
        CircuitExtra::SinPreHold,
        CircuitExtra::SinPreZero,
        CircuitExtra::SinSecondTone,
        CircuitExtra::SffmSpice3,
        CircuitExtra::SffmNgspice,
        CircuitExtra::AmPreOffset,
        CircuitExtra::AmPreHold,
        CircuitExtra::AmPreZero,
        CircuitExtra::AmModCarrierPhase,
        CircuitExtra::PwlDelay,
        CircuitExtra::PwlRepeatFrom,
        CircuitExtra::PwlRepeatPeriod,
        CircuitExtra::PwlRepeatCount,
        CircuitExtra::PwlJoinRamp,
        CircuitExtra::PwlJoinRampWidth,
        CircuitExtra::PwlJoinExtrapolate,
        CircuitExtra::PwlFile,
        CircuitExtra::TableExact,
        CircuitExtra::TableNgspiceRounded,
        CircuitExtra::TableSmoothed,
        CircuitExtra::BehavTc,
        CircuitExtra::BehavResistance,
        CircuitExtra::BehavConductance,
        CircuitExtra::BehavCapacitance,
        CircuitExtra::BehavCapacitanceCharge,
        CircuitExtra::BehavInductance,
        CircuitExtra::BehavPower,
        CircuitExtra::LaplacePoleZero,
        CircuitExtra::LaplaceExprInput,
        CircuitExtra::FreqComplexLinear,
        CircuitExtra::FreqDbLogFreq,
        CircuitExtra::DelaySampled,
        CircuitExtra::SemiArea,
        CircuitExtra::SemiInitial,
        CircuitExtra::DiodeThermal,
        CircuitExtra::SwitchInitial,
        CircuitExtra::SwitchControlExpr,
        CircuitExtra::LineFreqNl,
        CircuitExtra::LineLengthVel,
        CircuitExtra::LineLumps,
        CircuitExtra::ModelBin,
        CircuitExtra::ModelAko,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}
