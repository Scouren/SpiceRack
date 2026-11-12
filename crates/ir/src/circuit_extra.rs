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
    /// `mult` on a subcircuit instance under `MultLaw::Parallel` (ESPice ignores it, B10, 6.01).
    MultInstanceParallel,
    /// `mult` on a subcircuit instance under `MultLaw::NgspiceAppend` (A15 C1, R31).
    MultInstanceNgspiceAppend,
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
    /// A behavioural source whose expression reads a parameter outside `ToReal` and `ToInt`
    /// (A33, R56): VACASK types every identifier of a behavioural expression, ngspice's B
    /// source never does.
    BehavUntypedParam,
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
    /// A PWL repeat joined by VACASK's periodic law (`PwlJoin::Extrapolate`, va; A22).
    PwlJoinExtrapolate,
    /// A periodic PWL joined by a jump (`PwlRepeat::Period` with `PwlJoin::Jump`, ng/sp `pwlperiod`;
    /// VACASK lacks it: its periodic PWL only extrapolates; A24).
    PwlPeriodJump,
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
    /// A semiconductor first guessed off (`InitialState::Off`; A24 ruling).
    SemiInitialOff,
    /// A semiconductor first guessed on (`InitialState::On`: Spectre `region=on`; ngspice lacks
    /// it, its semiconductors take `off` only; A24 ruling).
    SemiInitialOn,
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
    // A12 C1–C4 (VACASK, AMEND-A12)
    /// A pulse under `PulseLaw::Spice` (A12 C1).
    PulseSpice,
    /// A pulse under `PulseLaw::Vacask` (A12 C1).
    PulseVacask,
    /// An exponential whose `td2` counts from t = 0 (`TdFrom::Zero`, SPICE; A12 C2).
    ExpTd2FromZero,
    /// An exponential whose `td2` counts from `td1` (`TdFrom::Td1`, va; A12 C2).
    ExpTd2FromTd1,
    /// An SFFM that is its offset before the delay (A12 C3).
    SffmPreOffset,
    /// An SFFM that holds its t = delay value before the delay (va; A12 C3).
    SffmPreHold,
    /// An SFFM that is 0 before the delay (ng; A12 C3).
    SffmPreZero,
    /// A source with a waveform whose DC is given (`DcRule::Given`): va ignores `dc` on a waveform type (A12 C4).
    DcGivenWithWave,
    /// A pulse source whose DC is the wave at t = 0 (`DcRule::WaveAtZero`; A12 C4).
    DcAtZeroPulse,
    /// A sine source whose DC is the wave at t = 0.
    DcAtZeroSin,
    /// An exponential source whose DC is the wave at t = 0.
    DcAtZeroExp,
    /// A PWL source whose DC is the wave at t = 0.
    DcAtZeroPwl,
    /// An SFFM source whose DC is the wave at t = 0 (ngspice lacks it: its rule is 0).
    DcAtZeroSffm,
    /// An AM source whose DC is the wave at t = 0 (ngspice lacks it).
    DcAtZeroAm,
    /// A transient-noise source whose DC is the wave at t = 0 (ngspice lacks it).
    DcAtZeroTrNoise,
    /// A transient-random source whose DC is the wave at t = 0 (ngspice lacks it).
    DcAtZeroTrRandom,
    /// A pattern source whose DC is the wave at t = 0.
    DcAtZeroPattern,
    // A12 C5 (device libraries, R17)
    /// An element of a kind that can name a master (`ElemKind::master_kind` is `Some`) that names
    /// none (`model.name` `NONE`): VACASK instantiates every device through a model (A12 C5, run
    /// n2). A masterless kind (`Behav`, `ChargeCapacitor`, `FluxInductor`, …) never sets it.
    ElementWithoutModel,
    /// A model whose family has no `DeviceLibrary` of that family in the design (design-level; A12 C5).
    ModelWithoutLibrary,
    /// An independent or linear controlled source or a lossless line that names a model (A20),
    /// or a coupling bound to the builtin `Mutual` model: ngspice has no model for these (AMEND-A12 C5; a pass drops
    /// the model and unbinds the element).
    BuiltinMaster,
    // A13 (R23)
    /// An element whose name begins with none of its SPICE letters (`letter::spice_letters`,
    /// ASCII case ignored): a SPICE-card dialect cannot spell it; a pass renames it and every
    /// reference to it.
    NameWithoutLetter,
    // A15 C2 (R31)
    /// A behavioural temperature coefficient measured from `tnom` (`TcBase::Tnom` on a `Behav`
    /// or a circuit-valued passive with `tc1`/`tc2`): ngspice lacks it.
    TcBaseTnom,
    /// A behavioural temperature coefficient measured from 27 °C (`TcBase::Fixed27`).
    TcBaseFixed27,
    /// A circuit-valued capacitor's temperature factor applied directly (`TcLaw::Direct`).
    TcLawDirect,
    /// A circuit-valued capacitor's temperature factor applied as a divisor
    /// (`TcLaw::Reciprocal`, ngspice).
    TcLawReciprocal,
    // A17 (R35)
    /// A pulse, sine, exponential or SFFM whose zero fields mean what they hold
    /// (`GivenZero::AsGiven`).
    GivenZeroAsGiven,
    /// A pulse, sine, exponential or SFFM under ngspice's zero rule
    /// (`GivenZero::SpiceDefault`).
    GivenZeroSpiceDefault,
}

impl CircuitExtra {
    /// Every member, in declaration order (the order of `Feature::Circuit` indices).
    pub const ALL: [CircuitExtra; 107] = [
        CircuitExtra::MultResistor,
        CircuitExtra::MultCapacitor,
        CircuitExtra::MultInductor,
        CircuitExtra::MultVSource,
        CircuitExtra::MultISource,
        CircuitExtra::MultControlled,
        CircuitExtra::MultSemi,
        CircuitExtra::MultInstanceParallel,
        CircuitExtra::MultInstanceNgspiceAppend,
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
        CircuitExtra::BehavUntypedParam,
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
        CircuitExtra::PwlPeriodJump,
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
        CircuitExtra::SemiInitialOff,
        CircuitExtra::SemiInitialOn,
        CircuitExtra::DiodeThermal,
        CircuitExtra::SwitchInitial,
        CircuitExtra::SwitchControlExpr,
        CircuitExtra::LineFreqNl,
        CircuitExtra::LineLengthVel,
        CircuitExtra::LineLumps,
        CircuitExtra::ModelBin,
        CircuitExtra::ModelAko,
        CircuitExtra::PulseSpice,
        CircuitExtra::PulseVacask,
        CircuitExtra::ExpTd2FromZero,
        CircuitExtra::ExpTd2FromTd1,
        CircuitExtra::SffmPreOffset,
        CircuitExtra::SffmPreHold,
        CircuitExtra::SffmPreZero,
        CircuitExtra::DcGivenWithWave,
        CircuitExtra::DcAtZeroPulse,
        CircuitExtra::DcAtZeroSin,
        CircuitExtra::DcAtZeroExp,
        CircuitExtra::DcAtZeroPwl,
        CircuitExtra::DcAtZeroSffm,
        CircuitExtra::DcAtZeroAm,
        CircuitExtra::DcAtZeroTrNoise,
        CircuitExtra::DcAtZeroTrRandom,
        CircuitExtra::DcAtZeroPattern,
        CircuitExtra::ElementWithoutModel,
        CircuitExtra::ModelWithoutLibrary,
        CircuitExtra::BuiltinMaster,
        CircuitExtra::NameWithoutLetter,
        CircuitExtra::TcBaseTnom,
        CircuitExtra::TcBaseFixed27,
        CircuitExtra::TcLawDirect,
        CircuitExtra::TcLawReciprocal,
        CircuitExtra::GivenZeroAsGiven,
        CircuitExtra::GivenZeroSpiceDefault,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}
