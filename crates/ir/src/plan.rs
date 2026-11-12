//! The run plan as items: loops (a sweep, a table of states or a Monte Carlo; each opens a
//! `ScopeKind::Loop` scope of run items) and alters (the circuit state changes from here on), and
//! the typed target both act on. A reader's `.step`/`sweep`/`.alter`/`alterparam`/`.DATA`/
//! `montecarlo` in, rows out; there is no side table for the plan: order is meaning.
//!
//! Run items are ordinary items of a scope, in order (IR-RUN §1). An `Alter` changes the circuit
//! for every run item after it, in its scope and in every scope nested after it, until another
//! `Alter` of the same target, or a `Reset` of its class (`reset.rs`: ngspice's `reset` is a
//! `Reset`, never restoring alters that copy the cards' values; A27, A28). A `Loop` runs its body once per value (or row, or sample); the
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
//!   `pub struct Alter`, `pub enum AlterEffect`, `pub struct Elaborate`, `pub type RefList`, `pub struct Target`, `pub enum TargetKind`, `pub type TargetList`,
//!   `pub const LOOP_DEPTH_MAX`; on each enum: `ALL`, `COUNT`
//! - `Target::TEMP`, `TargetKind::def_kind(self) -> Option<DefKind>`,
//!   `TargetKind::is_parameter(self) -> bool`,
//!   `Design::targets(&self, TargetList) -> &[Target]`, `Design::refs(&self, RefList) -> &[Ref]`
//! - the run rows' target facts: `pub(crate) TargetKind::reads_b`,
//!   `pub(crate) TargetKind::has_path`, `pub(crate) enum TargetUse`,
//!   `pub(crate) fn target_handles`, `pub(crate) fn target_refs`, `pub(crate) fn target_valid`,
//!   `pub(crate) fn target_features`, `pub(crate) fn dump_target`, `pub(crate) fn parse_target`,
//!   `pub(crate) fn gen_target`, `pub(crate) fn target_kinds`, `pub(crate) fn gen_ref`
//! - (`RowFacts` for `Loop` and `Alter`, and the loop details' facts: `plan_facts.rs`)
//!
//! IR text of a target: `(kind [option] [a] [b] [path=(…)])`: `(temp)`, `(param r)`,
//! `(subckt_param div r)`, `(source_dc v1)`, `(element_value r1)`,
//! `(device_param r1 r path=(x1))`, `(model_param nch vth0)`, `(option rel_tol)`,
//! `(option private tran_rmax vacask)`.

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{DefId, ExprId, ExprList, List, NameId, NameList, Ref, ScopeId};
use crate::item::DefKind;
use crate::option_key::OptionKey;
use crate::run_extra::RunExtra;
use crate::run_words::{
    close, get, open, put_name, put_names, put_ref, put_word, take_enum, take_names, word,
};
use crate::sweep::SweepSpec;
use crate::text::Words;
use crate::text_expr::Ctx;

/// A loop: its body runs once per value of a sweep, row of a table, or Monte Carlo sample.
/// Opens a `ScopeKind::Loop` scope holding run items only (analyses, alters, loops, state and
/// output items). The kind's detail row is `detail` in that kind's table (`Design::detail`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loop {
    /// What the loop runs over; names the detail table.
    pub kind: LoopKind,
    /// What the swept targets hold after the last pass (`INV-semantics.md` 13.03).
    pub after: LoopEnd,
    /// The loop's own name in its dialect (A27): Spectre's and VACASK's statement name
    /// (`sw1 sweep`, `mc1 montecarlo`, `sweep name`), ngspice's `foreach` variable (so a command
    /// in the body that reads `$v` stays valid text where a writer puts it); `NONE` when it has
    /// none (SPICE `.step`).
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
/// `altergroup`, which first defaults every parameter of the device, is not an `Alter`. Never
/// of `TargetKind::Temp` or `TargetKind::Option` (checked; A28): "from here the temperature, the
/// nominal temperature or an option is v" is one row, a `FromHere` `Env` or `OptionRow`; those
/// target kinds are for loops and DC axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alter {
    /// What changes.
    pub target: Target,
    /// Its new value; never `NONE`. A string value (VACASK `type="dc"`) is an `Expr::Str`.
    pub value: ExprId,
    /// When an alter of a parameter (`TargetKind::Param`, `SubcktParam`) takes effect for the
    /// analyses after it (R70/A43); `AtOnce` for every other target (checked).
    pub effect: AlterEffect,
}

/// When a parameter alter takes effect (R70/A43). R58's evidence: VACASK re-elaborates at the
/// alter (`alter instance("__topinst__") a=2` between two operating points gives the new value
/// at the second: the pair deck `alter_toplevel.sim`, `pair-test` b213ada2); ngspice's
/// `alterparam` takes effect only at the next `reset`, which re-reads the deck with it
/// (ngspice-45 settle runs, `step2-ngspice` d8221c3e, `tests/fixtures/dialect/ngspice/settle/`:
/// `alter_param_noreset.sp`, `.param a=1`, `V1 n 0 {a}`, op, `alterparam a=2`, op, prints
/// v(n) = 1.00000000e+00 both times, with no message and exit 0; `alter_param_reset.sp`, the
/// same with `reset` before the second op, prints 1.00000000e+00 then 2.00000000e+00).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AlterEffect {
    /// Where it stands: every later analysis sees the new value, every other state kept
    /// (VACASK; `RunExtra::AlterParamAtOnce`).
    AtOnce,
    /// From the first full `Reset` after it in run order (one of every class: ngspice's
    /// `reset` is all or nothing, its writer refuses a partial one, W1406), never if none
    /// follows (ngspice `alterparam`; `RunExtra::AlterParamAtReset`). That reset undoes the
    /// device and model alters before it (it re-reads the deck) and applies this one instead;
    /// no reset undoes a parameter alter. A partial reset does not apply it: a definition, no
    /// dialect writes one (`state_order::in_force`, `Setting::Param`).
    AtReset,
}

/// From here on, the top circuit is the named subcircuits' bodies, together (VACASK repeated
/// `elaborate circuit(…)`, A12 R1), until the next `Elaborate`. Before the first, the top circuit
/// is the deck's top scope as written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elaborate {
    /// The subcircuits, at least one, each a `DefKind::Subckt` reference the linker binds; a
    /// range into the run owner's reference pool.
    pub circuits: RefList,
}

/// An ordered list of references (an `Elaborate`'s circuits), a range into the run owner's
/// reference pool.
pub type RefList = List<Ref>;

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

/// Which item a target belongs to: it picks the `RunExtra` family of the target's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetUse {
    /// A DC sweep axis (`DcAxis*`; a source's DC value is the plain DC sweep).
    Dc,
    /// A loop (`Loop*`).
    Loop,
    /// An alter (`Alter*`).
    Alter,
}

/// Deepest loop nest a design may hold (checked); the corpus nests at most 3
/// (`DESIGN-control-epochs.md` §2).
pub const LOOP_DEPTH_MAX: usize = 8;

const _: () = assert!(size_of::<Loop>() == 16);
const _: () = assert!(size_of::<SweepLoop>() == 52);
const _: () = assert!(size_of::<TableLoop>() == 16);
const _: () = assert!(size_of::<MonteCarlo>() == 20);
const _: () = assert!(size_of::<Alter>() == 32);
const _: () = assert!(size_of::<Elaborate>() == 8);
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
        match self {
            TargetKind::Param => Some(DefKind::Param),
            TargetKind::SubcktParam => Some(DefKind::Subckt),
            TargetKind::ModelParam => Some(DefKind::Model),
            TargetKind::Temp
            | TargetKind::SourceDc
            | TargetKind::ElementValue
            | TargetKind::DeviceParam
            | TargetKind::Option => None,
        }
    }

    /// Whether this kind is a parameter of the design (`Param`, `SubcktParam`): the targets an
    /// `AlterEffect` applies to (R70).
    pub const fn is_parameter(self) -> bool {
        matches!(self, TargetKind::Param | TargetKind::SubcktParam)
    }

    /// Whether `Target::b` names a parameter for this kind.
    pub(crate) const fn reads_b(self) -> bool {
        matches!(
            self,
            TargetKind::SubcktParam | TargetKind::DeviceParam | TargetKind::ModelParam
        )
    }

    /// Whether `Target::path` may be non-empty for this kind.
    pub(crate) const fn has_path(self) -> bool {
        matches!(
            self,
            TargetKind::SourceDc | TargetKind::ElementValue | TargetKind::DeviceParam
        )
    }
}

impl LoopKind {
    /// Every kind, in declaration order.
    pub const ALL: [LoopKind; 3] = [LoopKind::Sweep, LoopKind::Table, LoopKind::MonteCarlo];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl AlterEffect {
    /// Every effect, in declaration order.
    pub const ALL: [AlterEffect; 2] = [AlterEffect::AtOnce, AlterEffect::AtReset];
    /// Effect count.
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
        get(&self.run.targets, list).expect("a target list inside the target pool")
    }

    /// The references of a list. Asserts the range is inside the reference pool.
    pub fn refs(&self, list: RefList) -> &[Ref] {
        get(&self.run.refs, list).expect("a reference list inside the reference pool")
    }
}

/// Calls `f` on the target's reference when its kind names a definition (`a`), else not at
/// all: an element's name is no reference.
pub(crate) fn target_refs(t: &mut Target, f: &mut dyn FnMut(&mut Ref, DefKind)) {
    if let Some(kind) = t.kind.def_kind() {
        f(&mut t.a, kind);
    }
}

/// Appends a target's handles: `a` as a reference when its kind names a definition, else as a
/// name; `b`; `path`.
pub(crate) fn target_handles(t: &Target, out: &mut Vec<Handle>) {
    match t.kind.def_kind() {
        Some(kind) => out.push(Handle::Ref(t.a, kind)),
        None => out.push(Handle::Name(t.a.name)),
    }
    out.extend([Handle::Name(t.b), Handle::Names(t.path)]);
}

/// Whether the target holds exactly the fields its kind reads (the `Target` doc; `option` is
/// `Private` for every kind but `Option`).
pub(crate) fn target_valid(t: &Target) -> bool {
    let a = !t.a.name.is_none();
    let b = !t.b.is_none();
    let path_ok = t.path.len == 0 || t.kind.has_path();
    let element_ref = t.kind.def_kind().is_some() || t.a.def.is_none();
    let fields = match t.kind {
        TargetKind::Temp => !a && !b,
        TargetKind::Param | TargetKind::SourceDc | TargetKind::ElementValue => a && !b,
        TargetKind::SubcktParam | TargetKind::DeviceParam | TargetKind::ModelParam => a && b,
        TargetKind::Option => {
            if t.option == OptionKey::Private {
                a && b
            } else {
                !a && !b
            }
        }
    };
    let option_ok = t.kind == TargetKind::Option || t.option == OptionKey::Private;
    fields && path_ok && element_ref && option_ok
}

/// Adds the target's features for its use: the use's family member of its kind, an option
/// target's key, `TargetInInstance`.
pub(crate) fn target_features(t: &Target, used: TargetUse, set: &mut FeatureSet) {
    let x = target_extra(used, t.kind);
    if let Some(x) = x {
        *set = set.with(Feature::Run(x));
    }
    if t.kind == TargetKind::Option {
        *set = set.with(Feature::Option(t.option));
    }
    if t.path.len > 0 {
        *set = set.with(Feature::Run(RunExtra::TargetInInstance));
    }
}

/// Appends ` (kind …)`.
pub(crate) fn dump_target(design: &Design, ctx: &Ctx, t: &Target, out: &mut String) {
    out.push_str(" (");
    out.push_str(&word(t.kind));
    if t.kind == TargetKind::Option {
        put_word(out, &word(t.option));
    }
    if !t.a.name.is_none() {
        match t.kind.def_kind() {
            Some(kind) => put_ref(out, design, ctx, t.a, kind),
            None => put_name(out, design, "", t.a.name),
        }
    }
    put_name(out, design, "", t.b);
    put_names(out, design, "path", t.path);
    out.push(')');
}

/// Reads `(kind …)`; `None` after a report when it is not one.
pub(crate) fn parse_target(
    words: &mut Words<'_>,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<Target> {
    if !open(words, diags) {
        return None;
    }
    let kind = take_enum(words, &TargetKind::ALL, diags)?;
    let mut t = Target {
        kind,
        ..Target::TEMP
    };
    let mut reads_a = !matches!(kind, TargetKind::Temp | TargetKind::Option);
    let mut reads_b = kind.reads_b();
    if kind == TargetKind::Option {
        t.option = take_enum(words, &OptionKey::ALL, diags)?;
        reads_a = t.option == OptionKey::Private;
        reads_b = reads_a;
    }
    if reads_a {
        t.a = if kind.def_kind().is_some() {
            words.reference(b, diags)
        } else {
            Ref::unresolved(words.name(b, diags))
        };
    }
    if reads_b {
        t.b = words.name(b, diags);
    }
    t.path = take_names(words, "path", b, diags);
    close(words, diags).then_some(t)
}

/// A random valid target for `used` whose features are all in `features`, or `None` when the
/// set allows none. Names are fresh; references are left unresolved.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn gen_target(
    g: &mut Gen,
    b: &mut Builder<'_>,
    features: &FeatureSet,
    used: TargetUse,
) -> Option<Target> {
    let keys: Vec<OptionKey> = OptionKey::ALL
        .into_iter()
        .filter(|&k| features.contains(Feature::Option(k)))
        .collect();
    let allowed = target_kinds(features, used);
    if allowed.is_empty() {
        return None;
    }
    let kind = allowed[g.below(allowed.len() as u32) as usize];
    let mut t = Target {
        kind,
        ..Target::TEMP
    };
    if kind == TargetKind::Option {
        t.option = keys[g.below(keys.len() as u32) as usize];
        if t.option == OptionKey::Private {
            t.a = Ref::unresolved(g.fresh_name(b));
            t.b = g.fresh_name(b);
        }
    } else if kind != TargetKind::Temp {
        t.a = gen_ref(g, b, kind.def_kind());
        if kind.reads_b() {
            t.b = g.fresh_name(b);
        }
    }
    if kind.has_path() && features.contains(Feature::Run(RunExtra::TargetInInstance)) && g.chance(3)
    {
        let x = g.fresh_name(b);
        t.path = b.name_ids(&[x]);
    }
    debug_assert!(target_valid(&t));
    Some(t)
}

/// The target kinds `gen_target` may draw for `used` under `features` (empty: it returns
/// `None`, having changed nothing).
#[cfg(any(test, feature = "gen"))]
pub(crate) fn target_kinds(features: &FeatureSet, used: TargetUse) -> Vec<TargetKind> {
    let any_key = OptionKey::ALL
        .into_iter()
        .any(|k| features.contains(Feature::Option(k)));
    TargetKind::ALL
        .into_iter()
        .filter(|&k| {
            let ok = target_extra(used, k).is_none_or(|x| features.contains(Feature::Run(x)));
            // A28: an alter of the temperature or an option is an `Env` or option row
            let state =
                used == TargetUse::Alter && matches!(k, TargetKind::Temp | TargetKind::Option);
            // R70/A43: a parameter alter states its effect, so one of the twins is needed
            let effect = [RunExtra::AlterParamAtOnce, RunExtra::AlterParamAtReset]
                .iter()
                .any(|&x| features.contains(Feature::Run(x)));
            let effect = used != TargetUse::Alter || !k.is_parameter() || effect;
            ok && !state && effect && (k != TargetKind::Option || any_key)
        })
        .collect()
}

/// A reference for a generated target or variation: a visible definition of `kind` when there
/// is one (resolved, A9: the linker reaches every run reference), else a fresh unresolved name.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn gen_ref(g: &mut Gen, b: &mut Builder<'_>, kind: Option<DefKind>) -> Ref {
    if let Some(kind) = kind
        && g.chance(2)
    {
        let scope = g.scope;
        let def = g.pick_def(b.design(), scope, kind);
        if !def.is_none() {
            return Ref {
                name: b.design().def(def).name,
                def,
            };
        }
    }
    Ref::unresolved(g.fresh_name(b))
}

// ---- private ----

/// The `RunExtra` a target kind uses for `used`, if any (a DC sweep of a source's DC value is
/// every target's).
const fn target_extra(used: TargetUse, kind: TargetKind) -> Option<RunExtra> {
    use RunExtra as X;
    use TargetKind as K;
    match (used, kind) {
        (TargetUse::Dc, K::SourceDc) => None,
        (TargetUse::Dc, K::Temp) => Some(X::DcAxisTemp),
        (TargetUse::Dc, K::Param) => Some(X::DcAxisParam),
        (TargetUse::Dc, K::SubcktParam) => Some(X::DcAxisSubcktParam),
        (TargetUse::Dc, K::ElementValue) => Some(X::DcAxisElementValue),
        (TargetUse::Dc, K::DeviceParam) => Some(X::DcAxisDeviceParam),
        (TargetUse::Dc, K::ModelParam) => Some(X::DcAxisModelParam),
        (TargetUse::Dc, K::Option) => Some(X::DcAxisOption),
        (TargetUse::Loop, K::Temp) => Some(X::LoopTemp),
        (TargetUse::Loop, K::Param) => Some(X::LoopParam),
        (TargetUse::Loop, K::SubcktParam) => Some(X::LoopSubcktParam),
        (TargetUse::Loop, K::SourceDc) => Some(X::LoopSourceDc),
        (TargetUse::Loop, K::ElementValue) => Some(X::LoopElementValue),
        (TargetUse::Loop, K::DeviceParam) => Some(X::LoopDeviceParam),
        (TargetUse::Loop, K::ModelParam) => Some(X::LoopModelParam),
        (TargetUse::Loop, K::Option) => Some(X::LoopOption),
        (TargetUse::Alter, K::Temp) => None, // refused by `check` (A28)
        (TargetUse::Alter, K::Param) => Some(X::AlterParam),
        (TargetUse::Alter, K::SubcktParam) => Some(X::AlterSubcktParam),
        (TargetUse::Alter, K::SourceDc) => Some(X::AlterSourceDc),
        (TargetUse::Alter, K::ElementValue) => Some(X::AlterElementValue),
        (TargetUse::Alter, K::DeviceParam) => Some(X::AlterDeviceParam),
        (TargetUse::Alter, K::ModelParam) => Some(X::AlterModelParam),
        (TargetUse::Alter, K::Option) => None, // refused by `check` (A28)
    }
}
