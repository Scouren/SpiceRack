//! Sweeps: one pinned reading of "a set of swept values", shared by loops, DC axes and every
//! frequency sweep (AC, noise, S-parameters, the periodic small-signal analyses). A reader's
//! sweep spelling in, one `SweepSpec` row out; a writer spells it back in its own words.
//!
//! The readings that differ between simulators are fields, never a reader's arithmetic:
//! `points` is always the number of values (Spectre `lin=4` and VACASK `points=4` are five
//! values, so their readers add one and their writers subtract one: spelling); `Dec`/`Oct` count
//! per decade or octave, `Log` counts in all (Spectre `log=`, the Python API's `log`); `end`
//! states whether a log or stepped range ends on its grid or on the stop value (LTspice);
//! `bounds` keeps Spectre's `center=`/`span=` as written.
//!
//! Cites: `rewrite/IR-RUN.md` §1 (`SweepSpec`); `rewrite/INV-semantics.md` 8.06, 8.07, 8.08,
//! 8.22, 13.05; `rewrite/INV-constructs.md` G2, G3, G5, G24, G25, G41, G49, G65, G66.
//!
//! Contents:
//! - `pub struct SweepSpec`, `pub enum SweepKind`, `pub enum SweepEnd`, `pub enum SweepBounds`;
//!   on each enum: `ALL`, `COUNT`
//! - `SweepSpec::lin_step(ExprId, ExprId, ExprId) -> SweepSpec`,
//!   `SweepSpec::lin_points(ExprId, ExprId, ExprId) -> SweepSpec`,
//!   `SweepSpec::dec(ExprId, ExprId, ExprId) -> SweepSpec`,
//!   `SweepSpec::oct(ExprId, ExprId, ExprId) -> SweepSpec`,
//!   `SweepSpec::log(ExprId, ExprId, ExprId) -> SweepSpec`, `SweepSpec::list(ExprList) -> SweepSpec`,
//!   `SweepSpec::ending(self, SweepEnd) -> SweepSpec`
//! - the run rows' sweep facts: `pub(crate) fn sweep_handles`, `pub(crate) fn sweep_valid`,
//!   `pub(crate) fn sweep_features`, `pub(crate) fn dump_sweep`, `pub(crate) fn parse_sweep`,
//!   `pub(crate) fn gen_sweep`
//!
//! IR text: `(kind [on_stop] [center_span] [start=v] [stop=v] [step=v] [points=v]
//! [values=[v, …]])`, e.g. `(lin start=0 stop=5 step=0.1)`, `(dec on_stop start=1 stop=1g
//! points=10)`, `(list values=[1, 2])`.

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, ExprList, List, ScopeId};
use crate::run_extra::RunExtra;
use crate::run_words::{
    close, open, put_expr, put_exprs, put_flag, take_enum, take_expr, take_exprs, take_word, word,
};
use crate::text::Words;
use crate::text_expr::Ctx;

/// A set of swept values with one reading (IR-RUN §1). Which fields are read is fixed by `kind`
/// and `bounds`; every unread value field is `NONE` and `values` is empty unless `kind` is
/// `List` (checked). The values may be expressions over outer loops' targets (VACASK allows it,
/// Xyce refuses it: `INV-semantics.md` 8.22; a writer's feature, not this row's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepSpec {
    /// How the values are spaced.
    pub kind: SweepKind,
    /// Where a range ends when its grid misses the stop value.
    pub end: SweepEnd,
    /// Whether `start`/`stop` hold the bounds or Spectre's centre and span.
    pub bounds: SweepBounds,
    /// The first value (`StartStop`) or the centre (`CenterSpan`); `NONE` for `List`.
    pub start: ExprId,
    /// The last value of the range (`StartStop`) or its whole width (`CenterSpan`); `NONE` for
    /// `List`. May be below `start`: the sweep then runs downward.
    pub stop: ExprId,
    /// `Lin` by step: the increment. `NONE` when `points` gives a `Lin` sweep, and for every
    /// other kind.
    pub step: ExprId,
    /// `Lin` by count: the number of values, ends included. `Dec`/`Oct`: values per decade or
    /// octave. `Log`: the number of values in all. `NONE` for a `Lin` by step and for `List`.
    pub points: ExprId,
    /// `List`: the values in run order, repeats kept (a reader that sorts or drops repeats
    /// does so because its simulator does: LTspice, `INV-semantics.md` 13.05). Empty otherwise.
    pub values: ExprList,
}

/// How a sweep's values are spaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SweepKind {
    /// Equally spaced, by `step` or by `points` (exactly one of the two is set).
    Lin,
    /// Logarithmic, `points` values per decade (ngspice/Xyce/LTspice `dec`, Spectre `dec=`,
    /// VACASK `mode="dec"`).
    Dec,
    /// Logarithmic, `points` values per octave (`oct`; Spectre has none).
    Oct,
    /// Logarithmic, `points` values in all (Spectre `log=`, the Python API's `log`).
    Log,
    /// The explicit `values` (Xyce/LTspice `list`, HSPICE `POI`, Spectre and VACASK `values=`).
    List,
}

/// Where a range ends when stepping from `start` misses `stop`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SweepEnd {
    /// On the last grid value at or before `stop` (ngspice `oct`, Xyce, Spectre, VACASK).
    Grid,
    /// On `stop` itself, appended after the last grid value that falls short of it (LTspice:
    /// `.step 1k 10k 4k` is 1k 5k 9k 10k; a log AC sweep ends on its stop frequency,
    /// `INV-semantics.md` 8.07).
    OnStop,
}

/// How a range's two bounds are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SweepBounds {
    /// `start` and `stop`.
    StartStop,
    /// `start` is the centre and `stop` the span: the range is `centre ± span/2` (Spectre
    /// `center=`/`span=`, `INV-semantics.md` 8.08).
    CenterSpan,
}

const _: () = assert!(size_of::<SweepSpec>() == 28);
const _: () = assert!(size_of::<SweepKind>() == 1);

impl SweepSpec {
    /// A linear sweep by increment, ending on its grid.
    pub const fn lin_step(start: ExprId, stop: ExprId, step: ExprId) -> SweepSpec {
        SweepSpec {
            kind: SweepKind::Lin,
            step,
            points: ExprId::NONE,
            ..range(start, stop)
        }
    }

    /// A linear sweep of `points` values, ends included.
    pub const fn lin_points(start: ExprId, stop: ExprId, points: ExprId) -> SweepSpec {
        SweepSpec {
            kind: SweepKind::Lin,
            points,
            ..range(start, stop)
        }
    }

    /// A log sweep of `per_decade` values per decade, ending on its grid.
    pub const fn dec(start: ExprId, stop: ExprId, per_decade: ExprId) -> SweepSpec {
        SweepSpec {
            kind: SweepKind::Dec,
            points: per_decade,
            ..range(start, stop)
        }
    }

    /// A log sweep of `per_octave` values per octave, ending on its grid.
    pub const fn oct(start: ExprId, stop: ExprId, per_octave: ExprId) -> SweepSpec {
        SweepSpec {
            kind: SweepKind::Oct,
            points: per_octave,
            ..range(start, stop)
        }
    }

    /// A log sweep of `points` values in all, ends included.
    pub const fn log(start: ExprId, stop: ExprId, points: ExprId) -> SweepSpec {
        SweepSpec {
            kind: SweepKind::Log,
            points,
            ..range(start, stop)
        }
    }

    /// The listed values, in order.
    pub const fn list(values: ExprList) -> SweepSpec {
        SweepSpec {
            kind: SweepKind::List,
            values,
            ..range(ExprId::NONE, ExprId::NONE)
        }
    }

    /// The same sweep with another end rule (an LTspice reader: `.ending(SweepEnd::OnStop)`).
    pub const fn ending(self, end: SweepEnd) -> SweepSpec {
        SweepSpec { end, ..self }
    }
}

impl SweepKind {
    /// Every kind, in declaration order.
    pub const ALL: [SweepKind; 5] = [
        SweepKind::Lin,
        SweepKind::Dec,
        SweepKind::Oct,
        SweepKind::Log,
        SweepKind::List,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl SweepEnd {
    /// Every rule, in declaration order.
    pub const ALL: [SweepEnd; 2] = [SweepEnd::Grid, SweepEnd::OnStop];
    /// Rule count.
    pub const COUNT: usize = Self::ALL.len();
}

impl SweepBounds {
    /// Every form, in declaration order.
    pub const ALL: [SweepBounds; 2] = [SweepBounds::StartStop, SweepBounds::CenterSpan];
    /// Form count.
    pub const COUNT: usize = Self::ALL.len();
}

/// Appends a sweep's handles in field order (all constant in time: a sweep value may read
/// parameters and outer loop targets, never the circuit).
pub(crate) fn sweep_handles(s: &SweepSpec, out: &mut Vec<Handle>) {
    out.extend([
        Handle::Expr(s.start),
        Handle::Expr(s.stop),
        Handle::Expr(s.step),
        Handle::Expr(s.points),
        Handle::Exprs(s.values),
    ]);
}

/// Whether the sweep reads exactly the fields its kind and bounds name (the struct doc).
pub(crate) fn sweep_valid(s: &SweepSpec) -> bool {
    let set = |e: ExprId| !e.is_none();
    match s.kind {
        SweepKind::Lin => {
            set(s.start) && set(s.stop) && (set(s.step) != set(s.points)) && s.values.len == 0
        }
        SweepKind::Dec | SweepKind::Oct | SweepKind::Log => {
            set(s.start) && set(s.stop) && !set(s.step) && set(s.points) && s.values.len == 0
        }
        SweepKind::List => {
            !set(s.start)
                && !set(s.stop)
                && !set(s.step)
                && !set(s.points)
                && s.values.len > 0
                && s.end == SweepEnd::Grid
                && s.bounds == SweepBounds::StartStop
        }
    }
}

/// Adds the sweep's `RunExtra` features.
pub(crate) fn sweep_features(s: &SweepSpec, set: &mut FeatureSet) {
    let mut add = |x: RunExtra| *set = set.with(Feature::Run(x));
    match s.kind {
        SweepKind::Lin => {
            if !s.points.is_none() {
                add(RunExtra::SweepLinPoints);
            }
        }
        SweepKind::Dec => add(RunExtra::SweepDec),
        SweepKind::Oct => add(RunExtra::SweepOct),
        SweepKind::Log => add(RunExtra::SweepLog),
        SweepKind::List => add(RunExtra::SweepList),
    }
    if s.end == SweepEnd::OnStop {
        add(RunExtra::SweepEndOnStop);
    }
    if s.bounds == SweepBounds::CenterSpan {
        add(RunExtra::SweepCenterSpan);
    }
}

/// Appends ` (kind …)`.
pub(crate) fn dump_sweep(design: &Design, ctx: &Ctx, s: &SweepSpec, out: &mut String) {
    out.push_str(" (");
    out.push_str(&word(s.kind));
    put_flag(out, s.end == SweepEnd::OnStop, "on_stop");
    put_flag(out, s.bounds == SweepBounds::CenterSpan, "center_span");
    put_expr(out, design, ctx, "start", s.start);
    put_expr(out, design, ctx, "stop", s.stop);
    put_expr(out, design, ctx, "step", s.step);
    put_expr(out, design, ctx, "points", s.points);
    put_exprs(out, design, ctx, "values", s.values);
    out.push(')');
}

/// Reads `(kind …)`; `None` after a report when it is not one.
pub(crate) fn parse_sweep(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<SweepSpec> {
    if !open(words, diags) {
        return None;
    }
    let kind = take_enum(words, &SweepKind::ALL, diags)?;
    let end = if take_word(words, "on_stop") {
        SweepEnd::OnStop
    } else {
        SweepEnd::Grid
    };
    let bounds = if take_word(words, "center_span") {
        SweepBounds::CenterSpan
    } else {
        SweepBounds::StartStop
    };
    let start = take_expr(words, "start", scope, b, diags);
    let stop = take_expr(words, "stop", scope, b, diags);
    let step = take_expr(words, "step", scope, b, diags);
    let points = take_expr(words, "points", scope, b, diags);
    let values = take_exprs(words, "values", scope, b, diags);
    close(words, diags).then_some(SweepSpec {
        kind,
        end,
        bounds,
        start,
        stop,
        step,
        points,
        values,
    })
}

/// A random valid sweep using only `features` (a linear sweep by step needs none).
#[cfg(any(test, feature = "gen"))]
pub(crate) fn gen_sweep(g: &mut Gen, b: &mut Builder<'_>, features: &FeatureSet) -> SweepSpec {
    let has = |x: RunExtra| features.contains(Feature::Run(x));
    let mut kinds = vec![SweepKind::Lin];
    for (k, x) in [
        (SweepKind::Dec, RunExtra::SweepDec),
        (SweepKind::Oct, RunExtra::SweepOct),
        (SweepKind::Log, RunExtra::SweepLog),
        (SweepKind::List, RunExtra::SweepList),
    ] {
        if has(x) {
            kinds.push(k);
        }
    }
    let kind = kinds[g.below(kinds.len() as u32) as usize];
    let value = |g: &mut Gen, b: &mut Builder<'_>| g.expr(b, features, Reads::Constant);
    let s = if kind == SweepKind::List {
        let n = 1 + g.below(3) as usize;
        let v: Vec<ExprId> = (0..n).map(|_| value(g, b)).collect();
        SweepSpec::list(b.exprs(&v))
    } else {
        let (start, stop, n) = (value(g, b), value(g, b), value(g, b));
        match kind {
            SweepKind::Lin if has(RunExtra::SweepLinPoints) && g.chance(2) => {
                SweepSpec::lin_points(start, stop, n)
            }
            SweepKind::Lin => SweepSpec::lin_step(start, stop, n),
            SweepKind::Dec => SweepSpec::dec(start, stop, n),
            SweepKind::Oct => SweepSpec::oct(start, stop, n),
            SweepKind::Log => SweepSpec::log(start, stop, n),
            SweepKind::List => unreachable!("a list sweep is built above"),
        }
    };
    let mut s = s;
    if kind != SweepKind::List {
        if has(RunExtra::SweepEndOnStop) && g.chance(3) {
            s.end = SweepEnd::OnStop;
        }
        if has(RunExtra::SweepCenterSpan) && g.chance(3) {
            s.bounds = SweepBounds::CenterSpan;
        }
    }
    debug_assert!(sweep_valid(&s));
    s
}

// ---- private ----

/// A range sweep of no kind yet: the fields every constructor shares.
const fn range(start: ExprId, stop: ExprId) -> SweepSpec {
    SweepSpec {
        kind: SweepKind::Lin,
        end: SweepEnd::Grid,
        bounds: SweepBounds::StartStop,
        start,
        stop,
        step: ExprId::NONE,
        points: ExprId::NONE,
        values: List::EMPTY,
    }
}
