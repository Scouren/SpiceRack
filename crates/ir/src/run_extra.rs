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
    /// A transient with its own step ceiling (`max_step` set).
    TranMaxStep,
    /// A transient with a first-step size (`initial_step` set; Xyce).
    TranInitialStep,
    /// A transient whose default step ceiling is none (`cap` `None`, `max_step` `NONE`).
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
    /// A linear sweep by point count (`points` set).
    SweepLinPoints,
    /// A sweep per decade.
    SweepDec,
    /// A sweep per octave.
    SweepOct,
    /// A log sweep by total point count (Spectre `log=`).
    SweepLog,
    /// A sweep over listed values.
    SweepList,
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
    /// A loop inside a loop.
    LoopNested,
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
    /// An alter of temperature.
    AlterTemp,
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
    /// An alter of an option.
    AlterOption,
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
    /// An initial condition or node set in a subcircuit body (applied per instance).
    NodeSetInSubckt,
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
    /// An analysis, option or `Env` listed inside a subcircuit or section body (R26: it is
    /// still its deck's).
    RunItemNested,
    /// A probe of every element's branch current (`ProbeKind::AllBranchI`).
    ProbeAllBranchI,
    /// A statistic measure that reports where the extremum is (`Stat::MinAt`, `Stat::MaxAt`).
    MeasStatAt,
    /// A measure crossing searched only up to a position (`Event::until`).
    MeasUntil,
    /// A measure crossing picked by ngspice's default, skipping the first interval
    /// (`Event::skip_initial`).
    MeasSkipInitial,
}

impl RunExtra {
    /// Every member, in declaration order (the order of `Feature::Run` indices).
    pub const ALL: [RunExtra; 128] = [
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
        RunExtra::SweepLinPoints,
        RunExtra::SweepDec,
        RunExtra::SweepOct,
        RunExtra::SweepLog,
        RunExtra::SweepList,
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
        RunExtra::LoopNested,
        RunExtra::LoopScoped,
        RunExtra::LoopTemp,
        RunExtra::LoopParam,
        RunExtra::LoopSubcktParam,
        RunExtra::LoopSourceDc,
        RunExtra::LoopElementValue,
        RunExtra::LoopDeviceParam,
        RunExtra::LoopModelParam,
        RunExtra::LoopOption,
        RunExtra::AlterTemp,
        RunExtra::AlterParam,
        RunExtra::AlterSubcktParam,
        RunExtra::AlterSourceDc,
        RunExtra::AlterElementValue,
        RunExtra::AlterDeviceParam,
        RunExtra::AlterModelParam,
        RunExtra::AlterOption,
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
        RunExtra::NodeSetInSubckt,
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
        RunExtra::RunItemNested,
        RunExtra::ProbeAllBranchI,
        RunExtra::MeasStatAt,
        RunExtra::MeasUntil,
        RunExtra::MeasSkipInitial,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}
