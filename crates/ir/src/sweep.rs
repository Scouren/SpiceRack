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
//!   `SweepSpec::ending(self, SweepEnd) -> SweepSpec`, `pub enum SweepCount`,
//!   `pub fn on_grid(&Design, &SweepSpec) -> bool` (R99)
//! - the run rows' sweep facts: `pub(crate) enum SweepUse`, `pub(crate) fn sweep_handles`,
//!   `pub(crate) fn sweep_valid`, `pub(crate) fn sweep_features`, `pub(crate) fn dump_sweep`,
//!   `pub(crate) fn parse_sweep`, `pub(crate) fn sweep_possible` (gen), `pub(crate) fn gen_sweep`
//!
//! A sweep's form is a feature per use (A24): `form_feature` is the table of
//! `Sweep{Dc,Loop,Freq}{LinStep,LinPoints,Dec,Oct,Log,List}`.
//!
//! IR text: `(kind [on_stop | stretch | ng_decade] [ng_lin] [center_span] [start=v] [stop=v] [step=v] [points=v]
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
    /// How a linear frequency sweep's count is read (R99: ngspice's own law, or plain).
    pub count: SweepCount,
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

/// Where a range ends when stepping from `start` misses `stop` (R99, F33). A sweep whose stop
/// is provably on its grid ([`on_grid`]) ends there under every rule and states `Grid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SweepEnd {
    /// On the last value of the exact grid at or before `stop` (ngspice `oct`, Xyce, Spectre).
    Grid,
    /// On `stop` itself, appended after the last grid value that falls short of it (LTspice:
    /// `.step 1k 10k 4k` is 1k 5k 9k 10k; a log AC sweep ends on its stop frequency,
    /// `INV-semantics.md` 8.07).
    OnStop,
    /// A `Dec` or `Oct` range whose interval count (the count per decade or octave times the
    /// decades or octaves) is rounded up and the values spaced evenly in the logarithm so the
    /// last is `stop` (VACASK's `dec` and `oct`, AC and DC: `dec 10 10 20k` is 35 values,
    /// 12.5052, …, 20000; R99 law C).
    Stretch,
    /// ngspice's `dec`, one rule: over one decade or more the interval count rounded down, the
    /// values spaced evenly in the logarithm so the last is `stop` (`dec 10 10 20k` is 34
    /// values, 12.5902, …, 20000; R99 law F); under one decade the exact grid, `stop` not
    /// reached (`dec 2 1k 5k`: 1000, 3162.28; law G). Only on a `Dec` range.
    NgDecade,
}

/// How a linear frequency sweep's count of values is read (R97a, R99). A literal count is read
/// by each reader into its plain meaning (ngspice's `lin 2` is one value at the start), so only
/// a count that is not a literal states `NgLin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SweepCount {
    /// The count is the number of values, ends included.
    Plain,
    /// ngspice-45's AC and S-parameter `lin`: a count of 2 runs one value (the start), 0 runs
    /// ten, any other count that many (runs: `runrules`, R97a). Only on a `Lin` by count in a
    /// frequency sweep whose count is not a literal.
    NgLin,
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

/// Where a sweep is used: its form's feature is per use (A24; ngspice's DC axis is lin by step
/// only, its frequency sweeps have no step).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SweepUse {
    /// A DC analysis's axis.
    Dc,
    /// A sweep loop.
    Loop,
    /// A frequency sweep (AC, noise, transfer, S-parameter, distortion, periodic small-signal).
    Freq,
}

/// A sweep's form: its kind, and for `Lin` whether by step or by count.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    LinStep,
    LinPoints,
    Dec,
    Oct,
    Log,
    List,
}

/// Every form, in `RunExtra` order.
#[cfg(any(test, feature = "gen"))]
const FORMS: [Form; 6] = [
    Form::LinStep,
    Form::LinPoints,
    Form::Dec,
    Form::Oct,
    Form::Log,
    Form::List,
];

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
    pub const ALL: [SweepEnd; 4] = [
        SweepEnd::Grid,
        SweepEnd::OnStop,
        SweepEnd::Stretch,
        SweepEnd::NgDecade,
    ];
    /// Rule count.
    pub const COUNT: usize = Self::ALL.len();
}

impl SweepCount {
    /// Every reading, in declaration order.
    pub const ALL: [SweepCount; 2] = [SweepCount::Plain, SweepCount::NgLin];
    /// Reading count.
    pub const COUNT: usize = Self::ALL.len();
}

/// Whether the sweep's stop is provably on its grid, where every end rule gives the same
/// values (R99): a `Dec` or `Oct` range by start and stop, both positive literal numbers whose
/// ratio is an exact integer power of ten or two (either way up). Decided in integer arithmetic
/// on the literals' digits, never in doubles; the count does not enter (base^(k/n) is rational
/// only when n divides k). `false` for any other sweep, a read, a `Long` literal, or digits
/// whose ratio overflows an `i128`. The one function every reader and rule asks.
pub fn on_grid(design: &Design, s: &SweepSpec) -> bool {
    let base: i128 = match s.kind {
        SweepKind::Dec => 10,
        SweepKind::Oct => 2,
        SweepKind::Lin | SweepKind::Log | SweepKind::List => return false,
    };
    if s.bounds != SweepBounds::StartStop {
        return false;
    }
    let (Some((m0, e0)), Some((m1, e1))) = (digits(design, s.start), digits(design, s.stop)) else {
        return false;
    };
    // stop / start = (m1 / m0) * 10^(e1 - e0), as a fraction num / den in lowest terms
    let shift = |m: i128, by: i32| 10i128.checked_pow(by as u32).and_then(|p| m.checked_mul(p));
    let (num, den) = if e1 >= e0 {
        (shift(m1, e1 - e0), Some(m0))
    } else {
        (Some(m1), shift(m0, e0 - e1))
    };
    let (Some(num), Some(den)) = (num, den) else {
        return false;
    };
    let g = gcd(num, den);
    let (num, den) = (num / g, den / g);
    let power = |mut x: i128| {
        // bounded: each pass divides x by base > 1
        while x > 1 && x % base == 0 {
            x /= base;
        }
        x == 1
    };
    (den == 1 && power(num)) || (num == 1 && power(den))
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

/// Whether the sweep reads exactly the fields its kind and bounds name (the struct doc), and
/// its end rule and count reading stand where they mean something (R99: `Stretch` on a `Dec`
/// or `Oct` off its grid, `NgDecade` on a `Dec` off its grid, `NgLin` on a frequency sweep's
/// `Lin` by a count that is not a literal).
pub(crate) fn sweep_valid(design: &Design, s: &SweepSpec, at: SweepUse) -> bool {
    let set = |e: ExprId| !e.is_none();
    let end_ok = match s.end {
        SweepEnd::Grid | SweepEnd::OnStop => true,
        SweepEnd::Stretch => {
            matches!(s.kind, SweepKind::Dec | SweepKind::Oct) && !on_grid(design, s)
        }
        SweepEnd::NgDecade => s.kind == SweepKind::Dec && !on_grid(design, s),
    };
    let count_ok = match s.count {
        SweepCount::Plain => true,
        SweepCount::NgLin => {
            at == SweepUse::Freq
                && s.kind == SweepKind::Lin
                && set(s.points)
                && !matches!(design.expr(s.points), crate::expr::Expr::Num(_))
        }
    };
    end_ok && count_ok && fields_valid(s)
}

/// Adds the sweep's `RunExtra` features.
pub(crate) fn sweep_features(s: &SweepSpec, at: SweepUse, set: &mut FeatureSet) {
    let mut add = |x: RunExtra| *set = set.with(Feature::Run(x));
    add(form_feature(at, form_of(s)));
    match s.end {
        SweepEnd::Grid => {}
        SweepEnd::OnStop => add(RunExtra::SweepEndOnStop),
        SweepEnd::Stretch => add(RunExtra::SweepEndStretch),
        SweepEnd::NgDecade => add(RunExtra::SweepEndNgDecade),
    }
    if s.count == SweepCount::NgLin {
        add(RunExtra::SweepCountNgLin);
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
    put_flag(out, s.end == SweepEnd::Stretch, "stretch");
    put_flag(out, s.end == SweepEnd::NgDecade, "ng_decade");
    put_flag(out, s.count == SweepCount::NgLin, "ng_lin");
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
    } else if take_word(words, "stretch") {
        SweepEnd::Stretch
    } else if take_word(words, "ng_decade") {
        SweepEnd::NgDecade
    } else {
        SweepEnd::Grid
    };
    let count = if take_word(words, "ng_lin") {
        SweepCount::NgLin
    } else {
        SweepCount::Plain
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
        count,
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
pub(crate) fn gen_sweep(
    g: &mut Gen,
    b: &mut Builder<'_>,
    features: &FeatureSet,
    at: SweepUse,
) -> SweepSpec {
    let has = |x: RunExtra| features.contains(Feature::Run(x));
    let forms: Vec<Form> = FORMS
        .into_iter()
        .filter(|&f| has(form_feature(at, f)))
        .collect();
    assert!(
        !forms.is_empty(),
        "a sweep generated only where `sweep_possible`"
    );
    let form = forms[g.below(forms.len() as u32) as usize];
    let kind = match form {
        Form::LinStep | Form::LinPoints => SweepKind::Lin,
        Form::Dec => SweepKind::Dec,
        Form::Oct => SweepKind::Oct,
        Form::Log => SweepKind::Log,
        Form::List => SweepKind::List,
    };
    let value = |g: &mut Gen, b: &mut Builder<'_>| g.expr(b, features, Reads::Constant);
    let s = if kind == SweepKind::List {
        let n = 1 + g.below(3) as usize;
        let v: Vec<ExprId> = (0..n).map(|_| value(g, b)).collect();
        SweepSpec::list(b.exprs(&v))
    } else {
        let (start, stop, n) = (value(g, b), value(g, b), value(g, b));
        match kind {
            SweepKind::Lin if form == Form::LinPoints => SweepSpec::lin_points(start, stop, n),
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
        // R99: an end law only off the grid, a count law only on a count that is not a literal
        let off_grid = !on_grid(b.design(), &s);
        let dec_oct = matches!(kind, SweepKind::Dec | SweepKind::Oct);
        if dec_oct && off_grid && has(RunExtra::SweepEndStretch) && g.chance(3) {
            s.end = SweepEnd::Stretch;
        }
        if kind == SweepKind::Dec && off_grid && has(RunExtra::SweepEndNgDecade) && g.chance(3) {
            s.end = SweepEnd::NgDecade;
        }
        let literal =
            !s.points.is_none() && matches!(b.design().expr(s.points), crate::expr::Expr::Num(_));
        let by_count = form == Form::LinPoints;
        if at == SweepUse::Freq
            && by_count
            && !literal
            && has(RunExtra::SweepCountNgLin)
            && g.chance(3)
        {
            s.count = SweepCount::NgLin;
        }
        if has(RunExtra::SweepCenterSpan) && g.chance(3) {
            s.bounds = SweepBounds::CenterSpan;
        }
    }
    debug_assert!(sweep_valid(b.design(), &s, at));
    s
}

/// Whether `features` hold some form of a sweep used `at` (`gen_sweep`'s precondition).
#[cfg(any(test, feature = "gen"))]
pub(crate) fn sweep_possible(features: &FeatureSet, at: SweepUse) -> bool {
    FORMS
        .into_iter()
        .any(|f| features.contains(Feature::Run(form_feature(at, f))))
}

// ---- private ----

/// Whether the sweep reads exactly the fields its kind and bounds name.
fn fields_valid(s: &SweepSpec) -> bool {
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

/// A positive literal's digits as `(mantissa, power of ten)`, its value `mantissa × 10^power`
/// exactly (`Mil`: the mantissa times 254 and the power less 7, 25.4e-6 = 254e-7); `None` for a
/// read, a `Long` literal or a value not above zero.
fn digits(design: &Design, e: ExprId) -> Option<(i128, i32)> {
    let crate::expr::Expr::Num(k) = design.expr(e) else {
        return None;
    };
    let l = design.num(k);
    let power = i32::from(l.exp10) - i32::from(l.frac);
    match l.form {
        _ if l.mantissa <= 0 => None,
        crate::literal::NumForm::Long => None,
        crate::literal::NumForm::Mil => Some((i128::from(l.mantissa) * 254, power - 7)),
        crate::literal::NumForm::Plain
        | crate::literal::NumForm::Exponent
        | crate::literal::NumForm::Scale
        | crate::literal::NumForm::Int => Some((i128::from(l.mantissa), power)),
    }
}

/// The greatest common divisor of two positive integers. Bounded: Euclid's steps are
/// logarithmic in the smaller.
fn gcd(mut a: i128, mut b: i128) -> i128 {
    assert!(a > 0 && b > 0, "gcd of positive integers");
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A range sweep of no kind yet: the fields every constructor shares.
const fn range(start: ExprId, stop: ExprId) -> SweepSpec {
    SweepSpec {
        kind: SweepKind::Lin,
        end: SweepEnd::Grid,
        count: SweepCount::Plain,
        bounds: SweepBounds::StartStop,
        start,
        stop,
        step: ExprId::NONE,
        points: ExprId::NONE,
        values: List::EMPTY,
    }
}

/// The form of a sweep.
fn form_of(s: &SweepSpec) -> Form {
    match s.kind {
        SweepKind::Lin if s.points.is_none() => Form::LinStep,
        SweepKind::Lin => Form::LinPoints,
        SweepKind::Dec => Form::Dec,
        SweepKind::Oct => Form::Oct,
        SweepKind::Log => Form::Log,
        SweepKind::List => Form::List,
    }
}

/// The feature of each use and form (A24's class).
const fn form_feature(at: SweepUse, form: Form) -> RunExtra {
    use RunExtra as X;
    match (at, form) {
        (SweepUse::Dc, Form::LinStep) => X::SweepDcLinStep,
        (SweepUse::Dc, Form::LinPoints) => X::SweepDcLinPoints,
        (SweepUse::Dc, Form::Dec) => X::SweepDcDec,
        (SweepUse::Dc, Form::Oct) => X::SweepDcOct,
        (SweepUse::Dc, Form::Log) => X::SweepDcLog,
        (SweepUse::Dc, Form::List) => X::SweepDcList,
        (SweepUse::Loop, Form::LinStep) => X::SweepLoopLinStep,
        (SweepUse::Loop, Form::LinPoints) => X::SweepLoopLinPoints,
        (SweepUse::Loop, Form::Dec) => X::SweepLoopDec,
        (SweepUse::Loop, Form::Oct) => X::SweepLoopOct,
        (SweepUse::Loop, Form::Log) => X::SweepLoopLog,
        (SweepUse::Loop, Form::List) => X::SweepLoopList,
        (SweepUse::Freq, Form::LinStep) => X::SweepFreqLinStep,
        (SweepUse::Freq, Form::LinPoints) => X::SweepFreqLinPoints,
        (SweepUse::Freq, Form::Dec) => X::SweepFreqDec,
        (SweepUse::Freq, Form::Oct) => X::SweepFreqOct,
        (SweepUse::Freq, Form::Log) => X::SweepFreqLog,
        (SweepUse::Freq, Form::List) => X::SweepFreqList,
    }
}
