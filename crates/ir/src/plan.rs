//! The run plan as items: loops (a sweep, a table of states or a Monte Carlo; each opens a
//! `ScopeKind::Loop` scope of run items) and alters (the circuit state changes from here on), and
//! the typed target both act on. A reader's `.step`/`sweep`/`.alter`/`alterparam`/`.DATA`/
//! `montecarlo` in, rows out; there is no side table for the plan: order is meaning.
//!
//! Run items are ordinary items of a scope, in order (IR-RUN §1). An `Alter` changes the circuit
//! for every run item after it, in its scope and in every scope nested after it, until another
//! `Alter` of the same target; a reader whose simulator discards alters (ngspice `reset`) emits
//! the restoring alters itself. A `Loop` runs its body once per value (or row, or sample); the
//! body holds exactly the run items the loop wraps, so "which analyses a `.step` wraps" is the
//! body (Xyce, LTspice: every analysis of the deck; HSPICE `SWEEP`: one), and the first
//! `.step` line is the innermost loop (Xyce, LTspice; ESPice: the last). What the target holds
//! after the loop is the loop's `after` field.
//!
//! A Monte Carlo is a loop (`LoopKind::MonteCarlo`), not an analysis kind: Spectre's
//! `montecarlo { }`, VACASK's `mc … endmc`, HSPICE `SWEEP MONTE=` and an ngspice `.control`
//! loop all run their body once per sample, and only a `Loop` owns a `ScopeKind::Loop` scope.
//!
//! Cites: `rewrite/IR-RUN.md` §1; `rewrite/INV-constructs.md` G24–G26, G29, G40, G45, G53, G56,
//! G62, G64, G72, G73, K4, K11, K13–K16, K19, K20; `rewrite/INV-semantics.md` 4.04, 8.05,
//! 12.04, 12.05, 13.01–13.07; `DESIGN-control-epochs.md` §2–§3; `DESIGN-mc-lowering.md` §2.
//!
//! Contents:
//! - `pub struct Loop`, `pub enum LoopKind`, `pub enum LoopEnd`, `pub struct SweepLoop`,
//!   `pub struct TableLoop`, `pub struct MonteCarlo`, `pub enum Variations`, `pub enum Sampler`,
//!   `pub struct Alter`, `pub struct Target`, `pub enum TargetKind`, `pub type TargetList`,
//!   `pub const LOOP_DEPTH_MAX`; on each enum: `ALL`, `COUNT`
//! - `Target::TEMP`, `TargetKind::def_kind(self) -> Option<DefKind>`,
//!   `Design::targets(&self, TargetList) -> &[Target]`
//! - `RowFacts` for `Loop` and `Alter` (`todo_facts!` placeholders, A2)

use crate::design::Design;
use crate::id::{DefId, ExprId, ExprList, List, NameId, NameList, Ref, ScopeId};
use crate::item::DefKind;
use crate::option_key::OptionKey;
use crate::sweep::SweepSpec;

/// A loop: its body runs once per value of a sweep, row of a table, or Monte Carlo sample.
/// Opens a `ScopeKind::Loop` scope holding run items only (analyses, alters, loops, state and
/// output items). The kind's detail row is `detail` in that kind's table (`Design::detail`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loop {
    /// What the loop runs over; names the detail table.
    pub kind: LoopKind,
    /// What the swept targets hold after the last pass (`INV-semantics.md` 13.03).
    pub after: LoopEnd,
    /// The statement's name (Spectre `sw1 sweep`, `mc1 montecarlo`; VACASK `sweep name`), or
    /// `NONE` (SPICE `.step`).
    pub name: NameId,
    /// Row in the kind's detail table; always present.
    pub detail: u32,
    /// Its body, a `ScopeKind::Loop` scope.
    pub body: ScopeId,
}

/// What a loop runs over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoopKind {
    /// One target over a sweep. Detail [`SweepLoop`].
    Sweep,
    /// Several targets set together, one row of values per pass (HSPICE `.alter` blocks and
    /// `.DATA`, Xyce `.DATA` + `.STEP DATA=`, Spectre `paramset`, LTspice's index step with
    /// `table()`). Detail [`TableLoop`].
    Table,
    /// One pass per Monte Carlo sample. Detail [`MonteCarlo`].
    MonteCarlo,
}

/// What a loop's targets hold after its last pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoopEnd {
    /// The value in force before the loop (Spectre, VACASK: measured).
    Restore,
    /// The last swept value (an ngspice `foreach` loop).
    KeepLast,
}

/// A sweep loop's detail: one target over one sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepLoop {
    /// What is swept.
    pub target: Target,
    /// Its values, in run order.
    pub sweep: SweepSpec,
}

/// A table loop's detail: `values` holds `values.len / columns.len` rows, row-major, each row
/// setting every column's target at once (checked: an exact multiple, at least one row).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableLoop {
    /// The targets one row sets, in column order.
    pub columns: TargetList,
    /// The rows, row-major, in run order.
    pub values: ExprList,
}

/// A Monte Carlo loop's detail. What varies is stated by the design itself: `Statistics` blocks
/// (the scopes `variations` selects) and `Draw` expressions whose `DrawWhen` is `Sampling` or
/// `Always` (Addendum A2); this row only says how many samples and how they are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MonteCarlo {
    /// The number of samples; never `NONE`.
    pub runs: ExprId,
    /// The seed, or `NONE` for the simulator's own. A seed is data but not portable: no two
    /// simulators share a random stream (`INV-semantics.md` 12.06).
    pub seed: ExprId,
    /// Which sample indices run (HSPICE `monte=list(2 4:5)`), 1-based, in order; empty = every
    /// index `1..=runs`.
    pub samples: ExprList,
    /// Which `Statistics` scopes vary (Spectre `variations=`).
    pub variations: Variations,
    /// How samples are drawn.
    pub sampler: Sampler,
    /// Whether a nominal pass runs before the samples (Spectre `donominal=yes`).
    pub nominal: bool,
}

/// Which `Statistics` scopes a Monte Carlo loop varies; the scope left out stays nominal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Variations {
    /// `process` only (Spectre's default).
    Process,
    /// `mismatch` only.
    Mismatch,
    /// Both.
    All,
}

/// How a Monte Carlo loop draws its samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Sampler {
    /// Independent random samples.
    Random,
    /// Latin hypercube (Xyce `sample_type=LHS`).
    LatinHypercube,
}

/// From here on, `target` holds `value` (IR-RUN §1). Changes only the named target: a Spectre
/// `altergroup`, which first defaults every parameter of the device, is not an `Alter`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alter {
    /// What changes.
    pub target: Target,
    /// Its new value; never `NONE`. A string value (VACASK `type="dc"`) is an `Expr::Str`.
    pub value: ExprId,
}

/// What a loop, an alter or a DC axis acts on (IR-RUN §1). Temperature is its own kind on every
/// simulator, never a parameter named `temp` (`INV-semantics.md` 4.04).
///
/// `path` is the instance path from the top to the subcircuit instance that holds the element
/// (Xyce `X1:R1:R`, Spectre `alter sub=x1`); empty at the top. `a` and `b` are read per
/// [`TargetKind`]; `option` only for `Option`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// What kind of quantity.
    pub kind: TargetKind,
    /// `Option`: the key; `OptionKey::Private` reads its name from `a.name`.
    pub option: OptionKey,
    /// `Param`: the parameter; `SubcktParam`: the subcircuit; `ModelParam`: the model (each a
    /// reference the linker binds, of `TargetKind::def_kind`). `SourceDc`, `ElementValue`,
    /// `DeviceParam`: the element's name (`def` always `NONE`: an element is no definition).
    /// `Option` with a private key: the option's name. `Temp`: `NONE`.
    pub a: Ref,
    /// `SubcktParam`, `DeviceParam`, `ModelParam`: the parameter's name. `Option` with a
    /// private key: the dialect. `NONE` otherwise.
    pub b: NameId,
    /// The instance path to the element's scope; empty at the top and for every kind but
    /// `SourceDc`, `ElementValue`, `DeviceParam`.
    pub path: NameList,
}

/// The kind of a [`Target`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TargetKind {
    /// The circuit temperature, °C (`.step temp`, `.STEP TEMP`, `option="temp"`).
    Temp,
    /// A parameter definition (`.step param p`, `alterparam p=`, Spectre `param=p`).
    Param,
    /// A subcircuit's default for one of its parameters, for every instance (ngspice
    /// `alterparam sub p=`).
    SubcktParam,
    /// An independent source's DC value (`.dc V1 …`).
    SourceDc,
    /// An element's own value: R, C, L (ngspice `.dc R1 …`; Xyce: a bare device name is its
    /// default parameter, and a parameter of that name wins, which the Xyce reader decides).
    ElementValue,
    /// A named instance parameter (`R1:R`, `dev=r1 param=r`, `instance="r1" parameter="r"`).
    DeviceParam,
    /// A model parameter (`<model>:<PARAM>`, Spectre `mod=`, LTspice `NMOS m(vto)`).
    ModelParam,
    /// A simulator option (VACASK `option="tran_rmax"`, ngspice `option k=v`).
    Option,
}

/// An ordered list of targets (a table loop's columns), a range into the run owner's target
/// pool.
pub type TargetList = List<Target>;

/// Deepest loop nest a design may hold (checked); the corpus nests at most 3
/// (`DESIGN-control-epochs.md` §2).
pub const LOOP_DEPTH_MAX: usize = 8;

const _: () = assert!(size_of::<Loop>() == 16);
const _: () = assert!(size_of::<SweepLoop>() == 52);
const _: () = assert!(size_of::<TableLoop>() == 16);
const _: () = assert!(size_of::<MonteCarlo>() == 20);
const _: () = assert!(size_of::<Alter>() == 28);
const _: () = assert!(size_of::<Target>() == 24);

impl Target {
    /// The circuit temperature.
    pub const TEMP: Target = Target {
        kind: TargetKind::Temp,
        option: OptionKey::Private,
        a: Ref {
            name: NameId::NONE,
            def: DefId::NONE,
        },
        b: NameId::NONE,
        path: List::EMPTY,
    };
}

impl TargetKind {
    /// Every kind, in declaration order.
    pub const ALL: [TargetKind; 8] = [
        TargetKind::Temp,
        TargetKind::Param,
        TargetKind::SubcktParam,
        TargetKind::SourceDc,
        TargetKind::ElementValue,
        TargetKind::DeviceParam,
        TargetKind::ModelParam,
        TargetKind::Option,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();

    /// The definition kind `Target::a` refers to (`Param`, `Subckt`, `Model`), or `None` when
    /// `a` is no reference (the linker leaves it alone).
    pub const fn def_kind(self) -> Option<DefKind> {
        todo!()
    }
}

impl LoopKind {
    /// Every kind, in declaration order.
    pub const ALL: [LoopKind; 3] = [LoopKind::Sweep, LoopKind::Table, LoopKind::MonteCarlo];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl LoopEnd {
    /// Every rule, in declaration order.
    pub const ALL: [LoopEnd; 2] = [LoopEnd::Restore, LoopEnd::KeepLast];
    /// Rule count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Variations {
    /// Every set, in declaration order.
    pub const ALL: [Variations; 3] = [Variations::Process, Variations::Mismatch, Variations::All];
    /// Set count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Sampler {
    /// Every sampler, in declaration order.
    pub const ALL: [Sampler; 2] = [Sampler::Random, Sampler::LatinHypercube];
    /// Sampler count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The targets of a list. Asserts the range is inside the target pool.
    pub fn targets(&self, list: TargetList) -> &[Target] {
        todo!()
    }
}

// run: implementation phase — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Loop);
todo_facts!(Alter);
