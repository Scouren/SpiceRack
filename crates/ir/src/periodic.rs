//! Periodic and stability analysis rows: harmonic balance, periodic steady state, the periodic
//! small-signal analyses that follow them, and loop-gain stability. A reader's `.HB`/`hb`/
//! `.pss`/`pss`/`.SN`/`pac`/`pnoise`/`pxf`/`pstb`/`stb`/`.LSTB`/`acstb` in, detail rows out.
//!
//! A periodic small-signal analysis (`Pac`, `Pnoise`, `Pxf`, `Pstb`) linearises around the
//! nearest `Hb` or `Pss` analysis before it in run order (Spectre and HSPICE's own rule: each
//! follows its large-signal analysis), so each large-signal analysis is written once and no row
//! holds a handle to another (checked: such a predecessor exists).
//!
//! Cites: `rewrite/IR-RUN.md` §2; `rewrite/INV-constructs.md` G16, G23, G33, G44, G51, G59, G61,
//! G67–G70; `rewrite/INV-semantics.md` 5.20, 8.15–8.18, 8.23; `DESIGN-analysis-kinds.md` §2–§3.
//!
//! Contents: `pub struct Hb`, `pub enum HbTruncate`, `pub struct Pss`, `pub enum PssMode`,
//! `pub struct Pac`, `pub struct Pnoise`, `pub struct Pxf`, `pub struct Pstb`,
//! `pub struct Stb`, `pub enum FreqAxis`; on each enum: `ALL`, `COUNT`

use crate::id::{ExprId, ExprList, NameId};
use crate::probe::Probe;
use crate::sweep::SweepSpec;

/// Harmonic balance over one or more tones (several from the start: a one-tone writer lacks
/// `Extra::HbMultiTone`). The spectral index set is explicit: per-tone harmonics, the
/// intermodulation order and the truncation shape (`INV-semantics.md` 8.15).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hb {
    /// The tones' frequencies, Hz, at least one, each positive.
    pub tones: ExprList,
    /// The highest harmonic of each tone, one per tone (Xyce `numfreq`, HSPICE `NHARMS`,
    /// VACASK `nharm`; a reader fills its default, Xyce 10).
    pub harmonics: ExprList,
    /// The highest intermodulation order (HSPICE `INTMODMAX`, Spectre `maximorder`, VACASK
    /// `immax`), or `NONE` for the shape's own bound.
    pub intermod: ExprId,
    /// Which mixing products are kept.
    pub truncate: HbTruncate,
}

/// Which mixing products of the tones harmonic balance keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HbTruncate {
    /// Every product with each tone's index within its harmonic bound (VACASK
    /// `truncate="box"`, Xyce `BOX`).
    Box,
    /// Products whose summed absolute indices are within the bound (VACASK `"diamond"`).
    Diamond,
    /// Xyce's default `HYBRID` set.
    Hybrid,
}

/// Periodic steady state, driven or autonomous (`INV-semantics.md` 8.16).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pss {
    /// Driven by a known period, or an oscillator.
    pub mode: PssMode,
    /// Start from the initial conditions instead of an operating point (ngspice `uic`).
    pub uic: bool,
    /// The fundamental frequency, Hz (driven), or its guess (autonomous); never `NONE`.
    pub fundamental: ExprId,
    /// The stabilisation time before the steady state is sought, s (ESPice counts periods:
    /// its reader multiplies, 8.23); never `NONE`.
    pub tstab: ExprId,
    /// The harmonics reported (an output request, not an accuracy knob), or `NONE`.
    pub harmonics: ExprId,
    /// Time points per period (ngspice `psspoints`), or `NONE` for the simulator's.
    pub points: ExprId,
    /// `Autonomous`: the oscillator's positive node; `NONE` when driven.
    pub osc_p: NameId,
    /// `Autonomous`: its negative node (ground: `NameId::GROUND`); `NONE` when driven.
    pub osc_n: NameId,
}

/// Whether a PSS is driven or free-running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PssMode {
    /// The period is known (Spectre `pss fund=`, HSPICE `.SN`, VACASK `tper=`).
    Driven,
    /// An oscillator; the frequency is found (ngspice `.pss`, Spectre `(p n) pss`, VACASK
    /// `oscillator=1`, HSPICE `.HBOSC`).
    Autonomous,
}

/// Periodic AC around the preceding `Hb`/`Pss` (Spectre `pac`, HSPICE `.SNAC`/`.HBAC`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pac {
    /// The swept frequencies, Hz, read per `axis`.
    pub sweep: SweepSpec,
    /// The highest sideband index reported, or `NONE` for the simulator's.
    pub sidebands: ExprId,
    /// `Relative`: the harmonic the sweep is offset from (`relharmnum`); `NONE` when
    /// `Absolute`.
    pub relharm: ExprId,
    /// How the sweep's frequencies are read.
    pub axis: FreqAxis,
}

/// Periodic noise around the preceding `Hb`/`Pss` (Spectre `pnoise`, HSPICE `.HBNOISE`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pnoise {
    /// The output.
    pub output: Probe,
    /// The input source the noise is referred to.
    pub source: NameId,
    /// The swept frequencies, Hz, read per `axis`.
    pub sweep: SweepSpec,
    /// The highest sideband index, or `NONE`.
    pub sidebands: ExprId,
    /// `Relative`: the harmonic offset from; `NONE` when `Absolute`.
    pub relharm: ExprId,
    /// How the sweep's frequencies are read.
    pub axis: FreqAxis,
}

/// Periodic transfer functions from every source to an output, around the preceding
/// `Hb`/`Pss` (Spectre `pxf`, HSPICE `.HBXF`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pxf {
    /// The output.
    pub output: Probe,
    /// The swept frequencies, Hz, read per `axis`.
    pub sweep: SweepSpec,
    /// The highest sideband index, or `NONE`.
    pub sidebands: ExprId,
    /// `Relative`: the harmonic offset from; `NONE` when `Absolute`.
    pub relharm: ExprId,
    /// How the sweep's frequencies are read.
    pub axis: FreqAxis,
}

/// Periodic loop-gain stability around the preceding `Hb`/`Pss` (Spectre `pstb`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pstb {
    /// The probe element in the loop (a 0 V source).
    pub probe: NameId,
    /// The swept frequencies, Hz, read per `axis`.
    pub sweep: SweepSpec,
    /// The highest sideband index, or `NONE`.
    pub sidebands: ExprId,
    /// `Relative`: the harmonic offset from; `NONE` when `Absolute`.
    pub relharm: ExprId,
    /// How the sweep's frequencies are read.
    pub axis: FreqAxis,
}

/// Loop-gain stability over frequency at a probe in the loop (Spectre `stb`, HSPICE and ESPice
/// `.LSTB`, VACASK `acstb`). The result is the return ratio; its sign convention is each
/// simulator's (Spectre and HSPICE report −T, VACASK and ngspice +T: `INV-semantics.md` 8.18), a
/// result-side rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stb {
    /// The probe element in the loop (a 0 V series source, or VACASK's probe instance).
    pub probe: NameId,
    /// The frequencies, Hz.
    pub sweep: SweepSpec,
}

/// How a periodic small-signal sweep's frequencies are read (`INV-semantics.md` 8.17).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FreqAxis {
    /// The input frequencies themselves (ESPice `.pac`; Spectre `sweeptype=absolute`).
    Absolute,
    /// Offsets from `relharm` times the fundamental (Spectre `sweeptype=relative`, VACASK
    /// `hbac`; a reader resolves Spectre's `unspecified` by its rule: relative when
    /// autonomous).
    Relative,
}

const _: () = assert!(size_of::<Hb>() == 24);
const _: () = assert!(size_of::<Pss>() == 28);
const _: () = assert!(size_of::<Pac>() == 40);
const _: () = assert!(size_of::<Pnoise>() == 68);
const _: () = assert!(size_of::<Pxf>() == 64);
const _: () = assert!(size_of::<Pstb>() == 44);
const _: () = assert!(size_of::<Stb>() == 32);

impl HbTruncate {
    /// Every shape, in declaration order.
    pub const ALL: [HbTruncate; 3] = [HbTruncate::Box, HbTruncate::Diamond, HbTruncate::Hybrid];
    /// Shape count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PssMode {
    /// Every mode, in declaration order.
    pub const ALL: [PssMode; 2] = [PssMode::Driven, PssMode::Autonomous];
    /// Mode count.
    pub const COUNT: usize = Self::ALL.len();
}

impl FreqAxis {
    /// Every reading, in declaration order.
    pub const ALL: [FreqAxis; 2] = [FreqAxis::Absolute, FreqAxis::Relative];
    /// Reading count.
    pub const COUNT: usize = Self::ALL.len();
}
