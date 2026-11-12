//! Option keys: the closed list of simulator options that mean one quantity on every simulator
//! that has them, each with its meaning, unit and spellings, and `Private` for the rest. A
//! reader's option word in, a key out; a writer spells the key in its own words or lacks it.
//!
//! A key is one quantity, never one word: SPICE `reltol` (Newton and truncation error at once)
//! is `RelTol`, while Xyce's `timeint reltol` (truncation error only) and `nonlin reltol` (Newton
//! only) are other quantities and stay `Private` (`INV-semantics.md` 10.01); HSPICE `gmin` and
//! `gmindc` are `GminTran` and `GminDc`, ngspice `gmin` is `Gmin` (both at once, 10.05); Xyce
//! `nonlin maxstep` is `Itl1`, not `MaxStep` (10.06); `rshunt` and `gshunt` are two keys whose
//! values are reciprocals (converting one to the other is a pass). Temperature and nominal
//! temperature are not options: they are the `Env` item. Options a simulator alone has, or whose
//! meaning is not established (ngspice `xmu`, `itl3`, `itl5`, `convstep`, `convabsstep`,
//! `noopac`, `noopalter`, `maxopalter`, `maxevtiter`, `ramptime`, `diode_cj0`, `diode_rser`,
//! `badmos3`, `trytocompact`, `seedinfo`; every Xyce package option without a key here; VACASK
//! `nr_*`, `op_*`, `homotopy_*`, `smsig_*`, `tran_rmax`, `tolscale`; LTspice `plotwinsize`,
//! `numdgt`, `measdgt`, `gfloat`, `cshuntintern`), are `Private`. ngspice `savecurrents` and
//! HSPICE `post`/`probe` are no option here: they say what is saved, which the `Save` items
//! state (`INV-semantics.md` 9.03, 9.05).
//!
//! Cites: `rewrite/IR-RUN.md` §3; `rewrite/INV-constructs.md` I1–I6, I12–I14, I16–I18, I21, I22,
//! I24, L21, L33; `rewrite/INV-semantics.md` 10.01–10.09.
//!
//! Contents: `pub enum OptionKey`, `pub enum OptionClass`; on each: `ALL`, `COUNT`;
//! `OptionKey::class(self) -> OptionClass`, `OptionKey::is_flag(self) -> bool`

/// A portable option: one quantity, whatever each simulator calls it. Each member says its
/// meaning, its unit (or that it is a flag, given with no value) and its spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OptionKey {
    // ---- accuracy ----
    /// Relative tolerance of both the Newton convergence test and the truncation-error test,
    /// dimensionless. ngspice, ESPice, HSPICE, LTspice, Spectre, VACASK `reltol`. Xyce has none.
    RelTol,
    /// Absolute current tolerance, A. ngspice, LTspice, VACASK `abstol`; HSPICE `absi`;
    /// Spectre `iabstol`.
    AbsTol,
    /// Absolute voltage tolerance, V. ngspice, LTspice, VACASK `vntol`; HSPICE `absv`; Spectre
    /// `vabstol`.
    VnTol,
    /// Absolute charge tolerance, C. ngspice, LTspice, VACASK `chgtol`; Spectre `chargeabstol`.
    ChgTol,
    /// Truncation-error overestimation factor, dimensionless (ngspice default 7, LTspice 2,
    /// VACASK 3.5). ngspice, LTspice `trtol`; VACASK `tran_lteratio`.
    TrTol,
    /// Absolute pivot threshold of the sparse factorisation. ngspice `pivtol`; Spectre
    /// `pivabs`.
    PivTol,
    /// Relative pivot ratio of the sparse factorisation. ngspice, HSPICE, Spectre `pivrel`.
    PivRel,
    /// Highest order of the Gear method, 1–6. ngspice `maxord`; Xyce `timeint maxord`; VACASK
    /// `tran_maxord`; HSPICE `maxord` (read only with Gear).
    MaxOrd,
    /// Flag: trapezoidal integration. ngspice, LTspice, HSPICE `method=trap`; Xyce
    /// `timeint method=7`; VACASK `tran_method="trap"` or `"am2"`.
    MethodTrap,
    /// Flag: Gear (BDF) integration up to `MaxOrd`. ngspice, LTspice, HSPICE `method=gear`;
    /// Xyce `timeint method=8`; VACASK `tran_method="gear2"` or `"bdf2"`.
    MethodGear,
    /// Flag: backward Euler integration. VACASK `tran_method="euler"` (ngspice states it as
    /// `MethodGear` with `MaxOrd` 1).
    MethodEuler,
    /// Newton iteration limit of the DC operating point, count. ngspice, HSPICE, LTspice
    /// `itl1`; Spectre `dcmaxiters`; VACASK `op_itl`; Xyce `nonlin maxstep`.
    Itl1,
    /// Newton iteration limit per point of a DC sweep, count. ngspice, HSPICE, LTspice `itl2`.
    Itl2,
    /// Newton iteration limit per transient time point, count. ngspice, HSPICE, LTspice
    /// `itl4`; Spectre `maxiters`; VACASK `tran_itl`.
    Itl4,
    /// Number of gmin-stepping steps, count. ngspice, LTspice `gminsteps`; VACASK
    /// `homotopy_gminsteps`.
    GminSteps,
    /// Number of source-stepping steps, count. ngspice `srcsteps` (alias `itl6`), LTspice
    /// `srcsteps`; VACASK `homotopy_srcsteps`.
    SrcSteps,
    /// Flag: skip the plain Newton attempt of the operating point and start with stepping.
    /// ngspice `noopiter`; VACASK `op_skipinitial=1`.
    NoOpIter,
    /// Deck-wide ceiling on the transient time step, s, applied to every transient. HSPICE
    /// `delmax`; LTspice `maxstep`. A transient's own ceiling is `Tran::max_step`.
    MaxStep,
    // ---- equations ----
    /// Conductance across every junction in every analysis, S. ngspice, LTspice, Spectre,
    /// VACASK `gmin`; Xyce `device gmin`.
    Gmin,
    /// Conductance across every junction in DC analyses only, S. HSPICE `gmindc`.
    GminDc,
    /// Conductance across every junction in the transient only, S. HSPICE `gmin`.
    GminTran,
    /// Resistance from every node to ground, Ω. ngspice `rshunt`.
    Rshunt,
    /// Conductance from every node to ground, S. HSPICE, VACASK `gshunt`.
    Gshunt,
    /// Capacitance from every node to ground, F. ngspice, LTspice `cshunt`.
    Cshunt,
    /// Resistance in series with every inductor, Ω. ngspice `rseries`.
    Rseries,
    /// Geometry scale, dimensionless: every instance length and width (MOSFET `l`, `w`, `pd`,
    /// `ps`; R and C `l`, `w`; the level-3 diode's geometry) is multiplied by it, every area
    /// (`ad`, `as`, diode `area` given in m²) by its square (`INV-semantics.md` 6.06). Folding
    /// it into the instances (LTspice has no such option) is a pass. ngspice, HSPICE, Spectre, VACASK `scale`; Xyce `parser scale`. LTspice
    /// has none.
    Scale,
    /// Default MOSFET channel length, m. ngspice, HSPICE `defl`.
    DefL,
    /// Default MOSFET channel width, m. ngspice, HSPICE `defw`.
    DefW,
    /// Default MOSFET drain area, m². ngspice, HSPICE `defad`.
    DefAd,
    /// Default MOSFET source area, m². ngspice, HSPICE `defas`.
    DefAs,
    // ---- path: changes how a solution is found, not which ----
    /// Flag: the KLU sparse solver. ngspice `klu`.
    Klu,
    /// Flag: the built-in sparse solver. ngspice `sparse`.
    Sparse,
    /// Seed of the random generator, integer. ngspice, Spectre `seed`. Not portable in effect:
    /// no two simulators share a random stream.
    Seed,
    /// Directory searched for included files, a string. HSPICE `search`.
    Search,
    /// Flag: end the transient once every measure has its value. ngspice `autostop`.
    Autostop,
    // ---- output only ----
    /// Flag: interpolate the output onto the transient's print-step grid. ngspice `interp`.
    Interp,
    /// Flag: print run statistics. ngspice, HSPICE `acct`.
    Acct,
    /// Flag: suppress run statistics. ngspice `noacct`.
    NoAcct,
    /// Flag: print the netlist listing. ngspice, HSPICE `list`.
    List,
    /// Flag: suppress the model listing. ngspice, HSPICE `nomod`.
    NoMod,
    /// Flag: suppress page breaks. ngspice, HSPICE `nopage`.
    NoPage,
    /// Flag: print the node table. ngspice, HSPICE `node`.
    Node,
    /// Flag: print the option values. ngspice, HSPICE `opts`.
    Opts,
    /// Flag: print warnings. ngspice `warn`.
    Warn,
    /// Most warnings printed, count. ngspice `maxwarns`.
    MaxWarns,
    /// Flag: suppress the reference value column. ngspice `norefvalue`.
    NoRefValue,
    /// Flag: keep the operating-point information for the following small-signal analyses.
    /// ngspice `keepopinfo`.
    KeepOpInfo,
    /// Flag: suppress the initialisation report. ngspice `noinit`.
    NoInit,
    /// A simulator's own option, kept by name with its dialect (`OptionRow::private`,
    /// `OptionRow::dialect`); only that dialect's writer spells it.
    Private,
}

/// What an option changes (`INV-semantics.md` 10.09): only `Equations` and `Accuracy` are
/// semantic; a writer may drop the other two with a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OptionClass {
    /// The circuit equations (shunts, gmin, scale, defaults).
    Equations,
    /// The accuracy of their solution (tolerances, method, step ceiling, iteration limits).
    Accuracy,
    /// How a solution is found, not which (solver, seed, search path, early stop).
    Path,
    /// What is printed or listed.
    Output,
}

const _: () = assert!(size_of::<OptionKey>() == 1);

impl OptionKey {
    /// Every key, in declaration order.
    pub const ALL: [OptionKey; 49] = [
        OptionKey::RelTol,
        OptionKey::AbsTol,
        OptionKey::VnTol,
        OptionKey::ChgTol,
        OptionKey::TrTol,
        OptionKey::PivTol,
        OptionKey::PivRel,
        OptionKey::MaxOrd,
        OptionKey::MethodTrap,
        OptionKey::MethodGear,
        OptionKey::MethodEuler,
        OptionKey::Itl1,
        OptionKey::Itl2,
        OptionKey::Itl4,
        OptionKey::GminSteps,
        OptionKey::SrcSteps,
        OptionKey::NoOpIter,
        OptionKey::MaxStep,
        OptionKey::Gmin,
        OptionKey::GminDc,
        OptionKey::GminTran,
        OptionKey::Rshunt,
        OptionKey::Gshunt,
        OptionKey::Cshunt,
        OptionKey::Rseries,
        OptionKey::Scale,
        OptionKey::DefL,
        OptionKey::DefW,
        OptionKey::DefAd,
        OptionKey::DefAs,
        OptionKey::Klu,
        OptionKey::Sparse,
        OptionKey::Seed,
        OptionKey::Search,
        OptionKey::Autostop,
        OptionKey::Interp,
        OptionKey::Acct,
        OptionKey::NoAcct,
        OptionKey::List,
        OptionKey::NoMod,
        OptionKey::NoPage,
        OptionKey::Node,
        OptionKey::Opts,
        OptionKey::Warn,
        OptionKey::MaxWarns,
        OptionKey::NoRefValue,
        OptionKey::KeepOpInfo,
        OptionKey::NoInit,
        OptionKey::Private,
    ];
    /// Key count.
    pub const COUNT: usize = Self::ALL.len();

    /// What the key changes (the grouping of the declaration). `Private` is `Accuracy`: a
    /// writer cannot know it changes nothing.
    pub const fn class(self) -> OptionClass {
        todo!()
    }

    /// Whether the key is given with no value (`OptionRow::value` is `NONE`).
    pub const fn is_flag(self) -> bool {
        todo!()
    }
}

impl OptionClass {
    /// Every class, in declaration order.
    pub const ALL: [OptionClass; 4] = [
        OptionClass::Equations,
        OptionClass::Accuracy,
        OptionClass::Path,
        OptionClass::Output,
    ];
    /// Class count.
    pub const COUNT: usize = Self::ALL.len();
}
