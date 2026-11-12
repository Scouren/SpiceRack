//! Measures, typed: the kinds of a `Measure` item and one detail row per kind, built from one
//! event row (a crossing, a time, or an end of the analysis). A reader's `.meas` form in
//! (`TRIG v(1) VAL=0.5 RISE=1`, Xyce `TRIG v(1)=0.5`, `FIND … WHEN …`, `AVG … FROM= TO=`), rows
//! out: thresholds, edges and counts are fields, never words.
//!
//! The reference semantics are ngspice's (linear interpolation between points,
//! `DESIGN-vacask-postprocess.md` §3); every default that differs between simulators is filled
//! by the reader (`count` is always present). A measure is evaluated once per pass of every loop
//! around it: one result row per step (`INV-semantics.md` 9.08). A measure reads earlier measures
//! by name in its expressions. Forms this file does not type (Xyce's FFT measures `ENOB SFDR
//! SNDR SNR THD`, `ERROR`, the `*_CONT` modes, HSPICE `ERR1`) are `Opaque` of class `Private`.
//!
//! Cites: `rewrite/IR-RUN.md` §4; `rewrite/INV-constructs.md` H4, H5, H14, H18, H21, H28, G58;
//! `rewrite/INV-semantics.md` 9.08; `DESIGN-vacask-postprocess.md` §3.
//!
//! Contents: `pub enum MeasureKind`, `pub struct Event`, `pub enum EventKind`, `pub enum Edge`,
//! `pub enum Stat`, `pub struct MeasureTrigTarg`, `pub struct MeasureWhen`,
//! `pub struct MeasureFind`, `pub struct MeasureDeriv`, `pub struct MeasureStat`,
//! `pub struct MeasureParam`; on each enum: `ALL`, `COUNT`; `Event::BOUND`,
//! `Event::at(ExprId) -> Event`, `Event::cross(Probe, Edge, ExprId, ExprId) -> Event`

use crate::id::{ExprId, List};
use crate::probe::{AcPart, Probe, ProbeKind};

/// What a measure computes; names its detail table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MeasureKind {
    /// The distance on the sweep axis from one event to another (`TRIG … TARG …`). Detail
    /// [`MeasureTrigTarg`].
    TrigTarg,
    /// Where on the sweep axis an event happens (`WHEN v(x)=0.5`). Detail [`MeasureWhen`].
    When,
    /// A probe's value at an event (`FIND v(y) WHEN v(x)=0.5`, `FIND v(y) AT=1u`). Detail
    /// [`MeasureFind`].
    Find,
    /// A probe's derivative along the sweep axis at an event (`DERIV v(y) AT=1u`). Detail
    /// [`MeasureDeriv`].
    Deriv,
    /// A statistic of a probe over a window (`AVG MIN MAX PP RMS INTEG`). Detail
    /// [`MeasureStat`].
    Stat,
    /// An expression of parameters and earlier measures (`PARAM='…'`). Detail
    /// [`MeasureParam`].
    Param,
}

/// One point on the sweep axis (time, frequency or the DC sweep value): a crossing of a
/// threshold, a given position, or an end of the analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// How the point is found.
    pub kind: EventKind,
    /// `Cross`: which crossings count.
    pub edge: Edge,
    /// `Cross`: the last crossing instead of the `count`-th (`LAST`).
    pub last: bool,
    /// `Cross`: the probe that crosses. Unread otherwise (an `Expr` probe with no expression).
    pub signal: Probe,
    /// `Cross`: the threshold (it may read the circuit: `WHEN v(a)=v(b)`). `At`: the position
    /// on the sweep axis. `NONE` for `Bound`.
    pub value: ExprId,
    /// `Cross`: which crossing, from 1; never `NONE` (a reader fills its simulator's default)
    /// unless `last`. `NONE` otherwise.
    pub count: ExprId,
    /// `Cross`: crossings before this position are not counted (`TD=`), or `NONE`.
    pub delay: ExprId,
}

/// How an [`Event`] is found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EventKind {
    /// Where `signal` crosses `value` on `edge`.
    Cross,
    /// At the position `value` (`AT=`, `FROM=`, `TO=`).
    At,
    /// The analysis's own start (in a window's `from`) or end (in its `to`).
    Bound,
}

/// Which crossings of a threshold count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Edge {
    /// Rising crossings (`RISE=`).
    Rise,
    /// Falling crossings (`FALL=`).
    Fall,
    /// Every crossing (`CROSS=`).
    Cross,
}

/// A statistic over a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Stat {
    /// The mean: the integral divided by the window's width.
    Avg,
    /// The least value.
    Min,
    /// The greatest value.
    Max,
    /// Greatest minus least.
    Pp,
    /// The root of the mean of the square.
    Rms,
    /// The integral along the sweep axis (trapezoidal).
    Integ,
}

/// `TrigTarg` detail: `targ`'s position minus `trig`'s.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureTrigTarg {
    /// The start event.
    pub trig: Event,
    /// The end event.
    pub targ: Event,
}

/// `When` detail: the event's position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureWhen {
    /// The event; never `Bound`.
    pub at: Event,
}

/// `Find` detail: `find`'s value at the event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureFind {
    /// The probe read.
    pub find: Probe,
    /// Where it is read; `Cross` or `At`.
    pub at: Event,
}

/// `Deriv` detail: `signal`'s derivative along the sweep axis at the event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureDeriv {
    /// The probe differentiated.
    pub signal: Probe,
    /// Where; `Cross` or `At`.
    pub at: Event,
}

/// `Stat` detail: `stat` of `signal` between `from` and `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureStat {
    /// Which statistic.
    pub stat: Stat,
    /// The probe.
    pub signal: Probe,
    /// The window's start (`Bound`: the analysis's start).
    pub from: Event,
    /// The window's end (`Bound`: the analysis's end).
    pub to: Event,
}

/// `Param` detail: the value of an expression.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasureParam {
    /// The expression; never `NONE`.
    pub value: ExprId,
}

const _: () = assert!(size_of::<Event>() == 40);
const _: () = assert!(size_of::<MeasureTrigTarg>() == 80);
const _: () = assert!(size_of::<MeasureWhen>() == 40);
const _: () = assert!(size_of::<MeasureFind>() == 64);
const _: () = assert!(size_of::<MeasureDeriv>() == 64);
const _: () = assert!(size_of::<MeasureStat>() == 108);
const _: () = assert!(size_of::<MeasureParam>() == 4);

impl Event {
    /// An end of the analysis: its start as a window's `from`, its end as a window's `to`.
    pub const BOUND: Event = Event {
        kind: EventKind::Bound,
        edge: Edge::Cross,
        last: false,
        signal: Probe {
            kind: ProbeKind::Expr,
            part: AcPart::None,
            terminal: u8::MAX,
            a: List::EMPTY,
            b: List::EMPTY,
            expr: ExprId::NONE,
        },
        value: ExprId::NONE,
        count: ExprId::NONE,
        delay: ExprId::NONE,
    };

    /// The position `value` on the sweep axis.
    pub const fn at(value: ExprId) -> Event {
        todo!()
    }

    /// The `count`-th crossing of `value` by `signal` on `edge`, with no delay (set `last` or
    /// `delay` on the result for the other forms).
    pub const fn cross(signal: Probe, edge: Edge, value: ExprId, count: ExprId) -> Event {
        todo!()
    }
}

impl MeasureKind {
    /// Every kind, in declaration order.
    pub const ALL: [MeasureKind; 6] = [
        MeasureKind::TrigTarg,
        MeasureKind::When,
        MeasureKind::Find,
        MeasureKind::Deriv,
        MeasureKind::Stat,
        MeasureKind::Param,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl EventKind {
    /// Every kind, in declaration order.
    pub const ALL: [EventKind; 3] = [EventKind::Cross, EventKind::At, EventKind::Bound];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Edge {
    /// Every edge, in declaration order.
    pub const ALL: [Edge; 3] = [Edge::Rise, Edge::Fall, Edge::Cross];
    /// Edge count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Stat {
    /// Every statistic, in declaration order.
    pub const ALL: [Stat; 6] = [
        Stat::Avg,
        Stat::Min,
        Stat::Max,
        Stat::Pp,
        Stat::Rms,
        Stat::Integ,
    ];
    /// Statistic count.
    pub const COUNT: usize = Self::ALL.len();
}
