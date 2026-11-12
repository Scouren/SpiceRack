//! The run owner's field-level features: what a run row states that some target cannot
//! spell and that is a field, not a kind. A design in (through each run row's
//! `RowFacts::features`), members of `Feature::Run` out. Split from the kernel's `Extra` (A7)
//! so the run owner adds a feature without touching a kernel file; members, names and docs
//! are as the run owner wrote them in `feature.rs`.
//!
//! Owner: run (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-DECISIONS.md` A7).
//!
//! Cites: `rewrite/IR-RUN.md`; `rewrite/COVERAGE-run.md`; `rewrite/INV-semantics.md` categories 4, 8–10, 12, 13.
//!
//! Contents: `pub enum RunExtra` (`ALL`, `COUNT`)

/// The run owner's field-level features. `#[repr(u8)]`: up to 256 members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RunExtra {
    /// More than one analysis in one run (Xyce runs only the last sweeping one; LTspice one per
    /// deck).
    SeveralAnalyses,
    /// A transient with no print step: the simulator chooses (`print_step` `NONE`; ngspice lacks
    /// it).
    TranSimulatorStep,
    /// A transient that records from a later start (`record_start` set).
    TranRecordStart,
    /// A transient with its own step ceiling (`StepCeiling::Given`).
    TranMaxStep,
    /// A transient with a first-step size (`initial_step` set; Xyce).
    TranInitialStep,
    /// A transient whose default step ceiling is none (`StepCeiling::Rule(StepCap::None)`).
    TranCapNone,
    /// A transient whose default step ceiling is ngspice's `min(print_step, span/50)`.
    TranCapNgspice,
    /// A transient whose default step ceiling is `span/50` (Spectre, VACASK).
    TranCapSpan50,
    /// A transient whose default step ceiling is `span/10` (Xyce).
    TranCapSpan10,
    /// A transient that skips the operating point (`uic`).
    TranUic,
    /// A transient that ramps the DC sources (LTspice `startup`).
    TranStartup,
    /// Device noise in a transient (`noise_fmax` set).
    TranNoise,
    /// HSPICE's further transient segments (`segments` non-empty).
    TranSegments,
    /// A DC sweep of two or more axes.
    DcNested,
    /// A DC sweep whose points restart from the initial guess.
    DcRestart,
    /// A DC axis over temperature.
    DcAxisTemp,
    /// A DC axis over a parameter.
    DcAxisParam,
    /// A DC axis over a subcircuit's parameter default.
    DcAxisSubcktParam,
    /// A DC axis over an element's value (a resistor).
    DcAxisElementValue,
    /// A DC axis over a device parameter.
    DcAxisDeviceParam,
    /// A DC axis over a model parameter.
    DcAxisModelParam,
    /// A DC axis over an option.
    DcAxisOption,
    /// The sweep of a DC axis, linear by step (`step` set) (A24: one member per use and form).
    SweepDcLinStep,
    /// The sweep of a DC axis, linear by count (`points` set) (A24: one member per use and form).
    SweepDcLinPoints,
    /// The sweep of a DC axis, per decade (A24: one member per use and form).
    SweepDcDec,
    /// The sweep of a DC axis, per octave (A24: one member per use and form).
    SweepDcOct,
    /// The sweep of a DC axis, logarithmic by total count (Spectre `log=`) (A24: one member per use and form).
    SweepDcLog,
    /// The sweep of a DC axis, over listed values (A24: one member per use and form).
    SweepDcList,
    /// The sweep of a sweep loop, linear by step (`step` set) (A24: one member per use and form).
    SweepLoopLinStep,
    /// The sweep of a sweep loop, linear by count (`points` set) (A24: one member per use and form).
    SweepLoopLinPoints,
    /// The sweep of a sweep loop, per decade (A24: one member per use and form).
    SweepLoopDec,
    /// The sweep of a sweep loop, per octave (A24: one member per use and form).
    SweepLoopOct,
    /// The sweep of a sweep loop, logarithmic by total count (Spectre `log=`) (A24: one member per use and form).
    SweepLoopLog,
    /// The sweep of a sweep loop, over listed values (A24: one member per use and form).
    SweepLoopList,
    /// The sweep of a frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal), linear by step (`step` set) (A24: one member per use and form).
    SweepFreqLinStep,
    /// The sweep of a frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal), linear by count (`points` set) (A24: one member per use and form).
    SweepFreqLinPoints,
    /// The sweep of a frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal), per decade (A24: one member per use and form).
    SweepFreqDec,
    /// The sweep of a frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal), per octave (A24: one member per use and form).
    SweepFreqOct,
    /// The sweep of a frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal), logarithmic by total count (Spectre `log=`) (A24: one member per use and form).
    SweepFreqLog,
    /// The sweep of a frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal), over listed values (A24: one member per use and form).
    SweepFreqList,
    /// A sweep that ends on its stop value off the grid (LTspice).
    SweepEndOnStop,
    /// A sweep written as centre and span (Spectre).
    SweepCenterSpan,
    /// A noise analysis with a summary interval.
    NoiseSummary,
    /// A noise analysis over the deck's AC frequencies (HSPICE).
    NoiseAcSweep,
    /// Input noise referred to the source's AC magnitude (Xyce).
    NoiseRefSourceAc,
    /// A noise analysis whose output is a current.
    NoiseCurrentOutput,
    /// A pole-zero analysis whose input is a source.
    PzSourceInput,
    /// An S-parameter analysis over listed ports.
    SpPorts,
    /// An S-parameter analysis with noise parameters.
    SpNoise,
    /// A Fourier analysis over more than one period (LTspice `Np`), stated as `periods` other than
    /// the literal 1.
    FourPeriods,
    /// A Fourier analysis with its own grid size.
    FourGrid,
    /// Harmonic balance over two or more tones.
    HbMultiTone,
    /// Harmonic balance with an intermodulation order.
    HbIntermod,
    /// Harmonic balance with diamond truncation.
    HbDiamond,
    /// Harmonic balance with Xyce's hybrid truncation.
    HbHybrid,
    /// An autonomous (oscillator) PSS.
    PssAutonomous,
    /// A driven PSS.
    PssDriven,
    /// A PSS that starts from the initial conditions.
    PssUic,
    /// A periodic small-signal sweep read relative to a harmonic.
    PeriodicRelative,
    /// A loop over a table of states.
    LoopTable,
    /// A Monte Carlo loop.
    LoopMonteCarlo,
    /// A loop whose targets keep their last value after it (ngspice `foreach`).
    LoopKeepLast,
    /// A loop whose targets are restored after it (Spectre, VACASK).
    LoopRestore,
    /// A loop whose body does not hold every analysis of the deck.
    LoopScoped,
    /// A loop over temperature.
    LoopTemp,
    /// A loop over a parameter.
    LoopParam,
    /// A loop over a subcircuit's parameter default.
    LoopSubcktParam,
    /// A loop over a source's DC value.
    LoopSourceDc,
    /// A loop over an element's value.
    LoopElementValue,
    /// A loop over a device parameter.
    LoopDeviceParam,
    /// A loop over a model parameter.
    LoopModelParam,
    /// A loop over an option.
    LoopOption,
    /// An alter of a parameter.
    AlterParam,
    /// An alter of a subcircuit's parameter default.
    AlterSubcktParam,
    /// An alter of a source's DC value.
    AlterSourceDc,
    /// An alter of an element's value.
    AlterElementValue,
    /// An alter of a device parameter.
    AlterDeviceParam,
    /// An alter of a model parameter.
    AlterModelParam,
    /// An alter to a string value (VACASK `type="dc"`).
    AlterString,
    /// A loop, alter or DC target inside a subcircuit instance (`path` non-empty).
    TargetInInstance,
    /// A Monte Carlo loop with a seed.
    McSeed,
    /// A Monte Carlo loop over listed sample indices (HSPICE).
    McSampleList,
    /// A Latin-hypercube Monte Carlo loop.
    McLatinHypercube,
    /// A Monte Carlo loop with a nominal pass first.
    McNominal,
    /// A Monte Carlo loop that varies mismatch (`variations` other than `Process`).
    McMismatch,
    /// A `FromHere` option after a run item (A14: exactly this; a `WholeRun` one there is
    /// `WholeRunMidRun`).
    OptionMidRun,
    /// An initial condition held in every operating point (Xyce).
    IcEveryOp,
    /// An initial condition or node set for the next analysis only (VACASK).
    IcNextOnly,
    /// An initial condition across two nodes (VACASK).
    IcDifferential,
    /// A node set on every node (ngspice `.nodeset all=`).
    NodesetAll,
    /// An initial condition or node set on a node inside an instance.
    NodeSetHierarchical,
    /// An initial condition on a device's own state (`NodeSetKind::Device`).
    IcDevice,
    /// A probe of the voltage between two nodes.
    ProbeVdiff,
    /// A probe of an element's branch current.
    ProbeI,
    /// A probe of one terminal's current.
    ProbeITerminal,
    /// A probe of a device's operating-point quantity.
    ProbeParam,
    /// A probe of an element's power.
    ProbePower,
    /// A probe of an expression.
    ProbeExpr,
    /// Every node voltage, at every depth.
    ProbeAllV,
    /// Every top-level node voltage only.
    ProbeAllVTop,
    /// Every voltage source's current.
    ProbeAllSourceI,
    /// Every device terminal current.
    ProbeAllI,
    /// Every subcircuit pin current.
    ProbeAllPinI,
    /// A probe of the output or input noise.
    ProbeNoise,
    /// A probe of a node or element inside an instance.
    ProbeHierarchical,
    /// A probe of a real part.
    PartRe,
    /// A probe of an imaginary part.
    PartIm,
    /// A probe of a magnitude.
    PartMag,
    /// A probe of a phase in degrees.
    PartPhaseDeg,
    /// A probe of a phase in radians.
    PartPhaseRad,
    /// A probe of a magnitude in decibels.
    PartDb,
    /// A save for one analysis kind only.
    SavePerAnalysis,
    /// A print of harmonic balance's time-domain waveform.
    PrintHbTime,
    /// A trigger/target measure.
    MeasTrigTarg,
    /// A when measure.
    MeasWhen,
    /// A find measure.
    MeasFind,
    /// A derivative measure.
    MeasDeriv,
    /// A window statistic measure.
    MeasStat,
    /// A parameter measure.
    MeasParam,
    /// A measure event on the last crossing.
    MeasLast,
    /// A measure event with a delay before counting.
    MeasDelay,
    /// A measure threshold that reads the circuit (`WHEN v(a)=v(b)`).
    MeasCircuitThreshold,
    /// A statistics block with mismatch variations.
    StatsMismatch,
    /// A statistics block with correlations.
    StatsCorrelation,
    /// A statistics block with truncation.
    StatsTruncate,
    /// A log-normal variation.
    VaryLogNormal,
    /// A variation given in percent.
    VaryPercent,
    /// A variation around another centre than the parameter's value.
    VaryMean,
    /// A sweep loop whose body is exactly one operating point (VACASK's spelling of a DC
    /// sweep; R20: a `Dc` is another meaning, one result over the grid, each point continued).
    LoopOverOp,
    /// A `FromHere` `Env` after a run item (A14).
    EnvMidRun,
    /// A `FromHere` `NodeSet` after a run item (A14).
    NodeSetMidRun,
    /// A `WholeRun` state item (option, `Env`, `NodeSet`) after a run item in listed order: a
    /// target whose state is positional cannot spell it in place (A14; a pass moves it before
    /// the first run item when both are in one file).
    WholeRunMidRun,
    /// A probe of every element's branch current (`ProbeKind::AllBranchI`).
    ProbeAllBranchI,
    /// A statistic measure that reports where the extremum is (`Stat::MinAt`, `Stat::MaxAt`).
    MeasStatAt,
    /// A measure crossing searched only up to a position (`Event::until`).
    MeasUntil,
    /// A measure crossing picked by ngspice's default, skipping the first interval
    /// (`Event::skip_initial`).
    MeasSkipInitial,
    /// An analysis with a statement name (ngspice cannot spell one; a pass drops it, A16).
    AnalysisNamed,
    /// A PSS with ngspice's shooting iteration limit (`Pss::sc_iter`).
    PssScIter,
    /// A PSS with ngspice's steady-state coefficient (`Pss::steady_coeff`).
    PssSteadyCoeff,
    /// A Fourier analysis whose harmonic count comes from the deck's `nfreqs` option
    /// (`Four::harmonics` absent).
    FourHarmonicsOption,
    /// A state item (option, `Env`, `NodeSet`) that holds `FromHere`, wherever it stands.
    StateFromHere,
    /// A state item that holds `WholeRun`, wherever it stands.
    StateWholeRun,
    /// A DC axis with a name (`DcAxis::name`; VACASK's `sweep <name>` names the swept vector
    /// in the results; A22).
    DcAxisNamed,
    /// A loop with a statement name (`Loop::name`: VACASK `sweep <name>`, Spectre `sw1 sweep`;
    /// SPICE `.step` has none; A22).
    LoopNamed,
    /// A sweep loop whose body, comments aside, is not exactly one analysis or one sweep loop
    /// (VACASK binds one analysis to each sweep; A22).
    SweepBodyNotOne,
    /// An item of a file's top scope that is no run item, comment or opaque, listed between two
    /// run items there (VACASK has one `control` block per file; A22). A run item here is any
    /// run owner's statement kind: analysis, loop, alter, option, env, node set, save, print,
    /// measure, elaborate.
    RunInterleaved,
    /// An initial condition or node set that holds beyond the next analysis (`next_only`
    /// false; VACASK's are an analysis's own list; A22). `IcNextOnly`'s twin.
    IcBeyondNext,
    /// A save for two or more analysis kinds but not every one (VACASK's `save` limits the
    /// operating point, its sweeps and the transient, not AC; A22). One kind is
    /// `SavePerAnalysis`; every kind neither.
    SaveAnalysisSet,
    /// An autonomous PSS whose oscillator reference node is not ground (`Pss::osc_n`; ngspice's
    /// `.pss` names one node only; A24).
    PssOscDiff,
    /// A reset of the values class (`Restorable::Values`; ngspice `reset`; A27).
    ResetValues,
    /// A reset of the options class (`Restorable::Options`).
    ResetOptions,
    /// A reset of the temperature (`Restorable::Temp`).
    ResetTemp,
    /// A reset of the nominal temperature (`Restorable::Tnom`).
    ResetTnom,
    /// A reset of the saves (`Restorable::Saves`; VACASK `clear saves`).
    ResetSaves,
    /// A save that holds from where it stands (`Save::holds` `FromHere`: VACASK `save`, an
    /// ngspice script `save`; A27).
    SaveFromHere,
    /// A save that holds for the whole run (`Save::holds` `WholeRun`: a `.save` card).
    SaveWholeRun,
    /// A `FromHere` save after a run item in listed order (a `WholeRun` one there is
    /// `WholeRunMidRun`).
    SaveMidRun,
}

impl RunExtra {
    /// Every member, in declaration order (the order of `Feature::Run` indices).
    pub const ALL: [RunExtra; 157] = [
        RunExtra::SeveralAnalyses,
        RunExtra::TranSimulatorStep,
        RunExtra::TranRecordStart,
        RunExtra::TranMaxStep,
        RunExtra::TranInitialStep,
        RunExtra::TranCapNone,
        RunExtra::TranCapNgspice,
        RunExtra::TranCapSpan50,
        RunExtra::TranCapSpan10,
        RunExtra::TranUic,
        RunExtra::TranStartup,
        RunExtra::TranNoise,
        RunExtra::TranSegments,
        RunExtra::DcNested,
        RunExtra::DcRestart,
        RunExtra::DcAxisTemp,
        RunExtra::DcAxisParam,
        RunExtra::DcAxisSubcktParam,
        RunExtra::DcAxisElementValue,
        RunExtra::DcAxisDeviceParam,
        RunExtra::DcAxisModelParam,
        RunExtra::DcAxisOption,
        RunExtra::SweepDcLinStep,
        RunExtra::SweepDcLinPoints,
        RunExtra::SweepDcDec,
        RunExtra::SweepDcOct,
        RunExtra::SweepDcLog,
        RunExtra::SweepDcList,
        RunExtra::SweepLoopLinStep,
        RunExtra::SweepLoopLinPoints,
        RunExtra::SweepLoopDec,
        RunExtra::SweepLoopOct,
        RunExtra::SweepLoopLog,
        RunExtra::SweepLoopList,
        RunExtra::SweepFreqLinStep,
        RunExtra::SweepFreqLinPoints,
        RunExtra::SweepFreqDec,
        RunExtra::SweepFreqOct,
        RunExtra::SweepFreqLog,
        RunExtra::SweepFreqList,
        RunExtra::SweepEndOnStop,
        RunExtra::SweepCenterSpan,
        RunExtra::NoiseSummary,
        RunExtra::NoiseAcSweep,
        RunExtra::NoiseRefSourceAc,
        RunExtra::NoiseCurrentOutput,
        RunExtra::PzSourceInput,
        RunExtra::SpPorts,
        RunExtra::SpNoise,
        RunExtra::FourPeriods,
        RunExtra::FourGrid,
        RunExtra::HbMultiTone,
        RunExtra::HbIntermod,
        RunExtra::HbDiamond,
        RunExtra::HbHybrid,
        RunExtra::PssAutonomous,
        RunExtra::PssDriven,
        RunExtra::PssUic,
        RunExtra::PeriodicRelative,
        RunExtra::LoopTable,
        RunExtra::LoopMonteCarlo,
        RunExtra::LoopKeepLast,
        RunExtra::LoopRestore,
        RunExtra::LoopScoped,
        RunExtra::LoopTemp,
        RunExtra::LoopParam,
        RunExtra::LoopSubcktParam,
        RunExtra::LoopSourceDc,
        RunExtra::LoopElementValue,
        RunExtra::LoopDeviceParam,
        RunExtra::LoopModelParam,
        RunExtra::LoopOption,
        RunExtra::AlterParam,
        RunExtra::AlterSubcktParam,
        RunExtra::AlterSourceDc,
        RunExtra::AlterElementValue,
        RunExtra::AlterDeviceParam,
        RunExtra::AlterModelParam,
        RunExtra::AlterString,
        RunExtra::TargetInInstance,
        RunExtra::McSeed,
        RunExtra::McSampleList,
        RunExtra::McLatinHypercube,
        RunExtra::McNominal,
        RunExtra::McMismatch,
        RunExtra::OptionMidRun,
        RunExtra::IcEveryOp,
        RunExtra::IcNextOnly,
        RunExtra::IcDifferential,
        RunExtra::NodesetAll,
        RunExtra::NodeSetHierarchical,
        RunExtra::IcDevice,
        RunExtra::ProbeVdiff,
        RunExtra::ProbeI,
        RunExtra::ProbeITerminal,
        RunExtra::ProbeParam,
        RunExtra::ProbePower,
        RunExtra::ProbeExpr,
        RunExtra::ProbeAllV,
        RunExtra::ProbeAllVTop,
        RunExtra::ProbeAllSourceI,
        RunExtra::ProbeAllI,
        RunExtra::ProbeAllPinI,
        RunExtra::ProbeNoise,
        RunExtra::ProbeHierarchical,
        RunExtra::PartRe,
        RunExtra::PartIm,
        RunExtra::PartMag,
        RunExtra::PartPhaseDeg,
        RunExtra::PartPhaseRad,
        RunExtra::PartDb,
        RunExtra::SavePerAnalysis,
        RunExtra::PrintHbTime,
        RunExtra::MeasTrigTarg,
        RunExtra::MeasWhen,
        RunExtra::MeasFind,
        RunExtra::MeasDeriv,
        RunExtra::MeasStat,
        RunExtra::MeasParam,
        RunExtra::MeasLast,
        RunExtra::MeasDelay,
        RunExtra::MeasCircuitThreshold,
        RunExtra::StatsMismatch,
        RunExtra::StatsCorrelation,
        RunExtra::StatsTruncate,
        RunExtra::VaryLogNormal,
        RunExtra::VaryPercent,
        RunExtra::VaryMean,
        RunExtra::LoopOverOp,
        RunExtra::EnvMidRun,
        RunExtra::NodeSetMidRun,
        RunExtra::WholeRunMidRun,
        RunExtra::ProbeAllBranchI,
        RunExtra::MeasStatAt,
        RunExtra::MeasUntil,
        RunExtra::MeasSkipInitial,
        RunExtra::AnalysisNamed,
        RunExtra::PssScIter,
        RunExtra::PssSteadyCoeff,
        RunExtra::FourHarmonicsOption,
        RunExtra::StateFromHere,
        RunExtra::StateWholeRun,
        RunExtra::DcAxisNamed,
        RunExtra::LoopNamed,
        RunExtra::SweepBodyNotOne,
        RunExtra::RunInterleaved,
        RunExtra::IcBeyondNext,
        RunExtra::SaveAnalysisSet,
        RunExtra::PssOscDiff,
        RunExtra::ResetValues,
        RunExtra::ResetOptions,
        RunExtra::ResetTemp,
        RunExtra::ResetTnom,
        RunExtra::ResetSaves,
        RunExtra::SaveFromHere,
        RunExtra::SaveWholeRun,
        RunExtra::SaveMidRun,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}
