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

use crate::id::{ExprId, ExprList};

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
        todo!()
    }

    /// A linear sweep of `points` values, ends included.
    pub const fn lin_points(start: ExprId, stop: ExprId, points: ExprId) -> SweepSpec {
        todo!()
    }

    /// A log sweep of `per_decade` values per decade, ending on its grid.
    pub const fn dec(start: ExprId, stop: ExprId, per_decade: ExprId) -> SweepSpec {
        todo!()
    }

    /// A log sweep of `per_octave` values per octave, ending on its grid.
    pub const fn oct(start: ExprId, stop: ExprId, per_octave: ExprId) -> SweepSpec {
        todo!()
    }

    /// A log sweep of `points` values in all, ends included.
    pub const fn log(start: ExprId, stop: ExprId, points: ExprId) -> SweepSpec {
        todo!()
    }

    /// The listed values, in order.
    pub const fn list(values: ExprList) -> SweepSpec {
        todo!()
    }

    /// The same sweep with another end rule (an LTspice reader: `.ending(SweepEnd::OnStop)`).
    pub const fn ending(self, end: SweepEnd) -> SweepSpec {
        todo!()
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
