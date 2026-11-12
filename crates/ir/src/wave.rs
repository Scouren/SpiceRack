//! Source waveforms: the closed list of waveform kinds, one row type and one table each, the
//! typed reference a source holds, and the decoded view writers read. Builder pushes in
//! (`Builder::wave`), `design.wave(r)` out.
//!
//! Every field is an expression; times are seconds, frequencies hertz, phases degrees. Fields
//! are named, never positional (SFFM's two argument orders are two readers' tables, not two
//! rows, INV-semantics 5.01). A default a simulator fills in is written by its reader as the
//! value that simulator means, and a default that depends on the analysis is the leaf `TStep`
//! or `TStop` (ngspice: a given-zero PULSE rise is TSTEP, a defaulted SFFM carrier 500/TSTOP;
//! 5.02, 5.05; Addendum A1). Where two simulators give one formula two meanings, a law or
//! mode field states which (`PreDelay`, `SffmLaw`, `PwlJoin`, `AmModTime`).
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §1, §3; `rewrite/INV-constructs.md` E3–E38;
//! `rewrite/INV-semantics.md` 5.01–5.12, 5.18, 5.22, 5.25, 5.33.
//!
//! Contents:
//! - `pub enum WaveKind`, `pub enum PreDelay`, `pub enum GivenZero`, `pub enum PulseLaw`,
//!   `pub enum TdFrom`,
//!   `pub enum SffmLaw`, `pub enum PwlRepeat`,
//!   `pub enum PwlJoin`, `pub enum AmModTime`, `pub enum TrRandomKind`; on each: `ALL`, `COUNT`
//! - `pub struct WaveRef`, `WaveRef::NONE`, `WaveRef::is_none(self) -> bool`
//! - `pub struct Pulse`, `pub struct Sin`, `pub struct Exp`, `pub struct Pwl`,
//!   `pub struct Sffm`, `pub struct Am`, `pub struct TrNoise`, `pub struct TrRandom`,
//!   `pub struct Pattern`
//! - `pub trait WaveRow` (`WaveRow::KIND`), `pub enum Wave` (the decoded view)
//! - `Design::wave(&self, WaveRef) -> Wave`

use crate::design::Design;
use crate::design::DetailRow;
use crate::id::{ExprId, ExprList, NameId};

/// A waveform kind (IR-CIRCUIT §3): which table a [`WaveRef`] indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WaveKind {
    /// Trapezoidal pulse train ([`Pulse`]). E3.
    Pulse,
    /// Damped sine ([`Sin`]). E4.
    Sin,
    /// Two-exponential edge ([`Exp`]). E5.
    Exp,
    /// Piecewise linear ([`Pwl`]). E6–E9, E22, E28–E31, E35.
    Pwl,
    /// Single-frequency FM ([`Sffm`]). E10, E21.
    Sffm,
    /// Amplitude modulation ([`Am`]). E11.
    Am,
    /// Transient noise ([`TrNoise`]). E12.
    TrNoise,
    /// Transient random value ([`TrRandom`]). E13.
    TrRandom,
    /// Bit pattern ([`Pattern`]). E14, E32.
    Pattern,
}

/// What a delayed waveform is before its delay (IR-CIRCUIT §1; INV-semantics 5.03, 5.06,
/// 5.25).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PreDelay {
    /// The offset alone (xy, hs SIN: VO before TD; their slices confirm).
    Offset,
    /// The waveform's value at t = delay, held (ng SIN, `vsrcload.c`, AMEND-A12 A17; lt SIN:
    /// VO + VA·sin(φ); sp, va: the value at t = delay, also at the operating point).
    HoldDelayValue,
    /// Zero for t ≤ delay, the delay included, and at the operating point (ng AM, SFFM).
    Zero,
}

/// What a zero or an omitted field means (AMEND-A12 A17, R35), on the waves with defaulted
/// fields (`Pulse`, `Sin`, `Exp`, `Sffm`, `Am`, `TrRandom`; IR-CIRCUIT §3 lists the fields,
/// their defaults, and which of them also replace a given zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GivenZero {
    /// A field means what it holds: a zero `rise` is an ideal edge. Every field is present.
    AsGiven,
    /// ngspice's rule (`vsrcload.c`, `vsrcpar.c`): a listed field the card omits (`NONE`)
    /// means its default, and the "both" fields (PULSE rise, fall, width, period; SIN freq;
    /// EXP `td1`, `tau1`, `td2`, `tau2`; SFFM carrier) also when they evaluate to zero after
    /// parameter substitution. A field holds what the card holds; no reader copies a default
    /// in, and a pass that converts to `AsGiven` writes the `TStep`/`TStop` leaves.
    SpiceDefault,
}

/// Which pulse law (AMEND-A12 C1; DIALECT-VACASK runs p1, p2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PulseLaw {
    /// SPICE's trapezoidal pulse train (ng, hs, xy, lt, sp).
    Spice,
    /// VACASK's: `rise > 0` is required; `fall <= 0` is no falling edge (the value holds `v2`
    /// to the end of the period and is `v1` again at the start of the next); `period <= 0` is
    /// one pulse.
    Vacask,
}

/// Where an exponential's second delay counts from (AMEND-A12 C2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TdFrom {
    /// From t = 0 (SPICE).
    Zero,
    /// From `td1` (va).
    Td1,
}

/// Which SFFM formula (INV-semantics 5.01, 5.02).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SffmLaw {
    /// SPICE3: offset + amplitude·sin(2π·fc·t + mdi·sin(2π·fs·t)) from t = 0, unclamped,
    /// with the phases and delay as [`Sffm`] states them (hs, xy, lt, sp, va).
    Spice3,
    /// ngspice: the same formula with the index limited to fc/fs and ngspice's defaults
    /// (what the source is before the delay is `Sffm::pre`, `Zero` on ngspice; AMEND-A12 C3).
    Ngspice,
}

/// Whether and how a PWL repeats (INV-semantics 5.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PwlRepeat {
    /// No repeat: the last value holds.
    Never,
    /// After the last point, return to the point at time `repeat_at` and replay from there;
    /// period = t_last − `repeat_at` (ng `r=`, which must be a listed point; hs `R`; xy `R=`,
    /// which may fall between points; lt `REPEAT` blocks).
    From,
    /// The value at t is the pattern at (t − delay) mod `repeat_at` (va `period=`, sp
    /// `pwlperiod`).
    Period,
}

/// How a repeating PWL joins its last point to the replayed one (INV-semantics 5.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PwlJoin {
    /// A jump: the value steps at the wrap instant (ng, hs, xy). Also the value when the PWL
    /// does not repeat.
    Jump,
    /// A ramp across the gap to the next pass's first point (lt `REPEAT`).
    Ramp,
    /// A ramp over `join_width` seconds when the first and last values differ (sp
    /// `pwlperiod` with `twidth`, default period/1000 written by the reader).
    RampWidth,
    /// VACASK's periodic PWL (`period=` > 0, A22, R42): on [xN, period) and on [0, x1) of the
    /// wrapped time τ, the value is the line through (xN, yN) and (period + x1, y1), evaluated
    /// at τ itself; between the points it is the plain PWL. So the value jumps at each period
    /// start and at x1. VACASK 36be560 (`src_pwl.sim`, branch `vc-sources`): points
    /// (1u, 1) (2u, 3), period 4u give 4.333 at 0, 3.833 at 0.75u, 1 at 1u, 2.667 at 2.5u,
    /// 1.733 at 3.9u, 4.267 at 4.1u; points (0, 0) (1u, 1), period 2u (x1 = 0) give 0.5 at
    /// 1.5u and 3.5u. A `delay` shifts τ: before it, and at the operating point, the value
    /// is the line at τ = 0. VACASK itself refuses x1 = 0 with xN = period and y1 ≠ yN (it
    /// drops the last point when y1 = yN), and xN > period: the writer's refusals, not
    /// invariants of this row.
    Extrapolate,
}

/// The time the modulating sine of [`Am`] runs on (INV-semantics 5.03).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AmModTime {
    /// t − delay (ng, hs, va).
    Delay,
    /// t − delay + phase_c_deg/(360·carrier_freq) (sp).
    CarrierPhase,
}

/// The distribution of a [`TrRandom`] (ng `TRRANDOM` type 1–4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TrRandomKind {
    /// Type 1: uniform in [−p1, p1] + p2.
    Uniform,
    /// Type 2: gaussian, standard deviation p1, mean p2.
    Gauss,
    /// Type 3: exponential, mean p1, offset p2.
    Exponential,
    /// Type 4: Poisson, λ p1, offset p2.
    Poisson,
}

/// A waveform: a row of `kind`'s table. `WaveRef::NONE` (index `u32::MAX`) is "no waveform".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaveRef {
    /// The table.
    pub kind: WaveKind,
    /// The row in that table.
    pub index: u32,
}

/// `PULSE(v1 v2 delay rise fall width period [cycles])` under `law`. Before `delay` the value
/// is `v1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pulse {
    /// Initial value. Never `NONE`.
    pub v1: ExprId,
    /// Pulsed value. Never `NONE`.
    pub v2: ExprId,
    /// Delay to the first rising edge (s); `NONE` only under `SpiceDefault` (omitted: 0).
    pub delay: ExprId,
    /// Rise time (s); a zero under `AsGiven` is an ideal edge, under `SpiceDefault` TSTEP
    /// (5.05), and `NONE` only under `SpiceDefault` (omitted). va refuses 0, its reader never
    /// writes one.
    pub rise: ExprId,
    /// Fall time (s), as `rise`.
    pub fall: ExprId,
    /// Pulse width (s); under `SpiceDefault` a zero or `NONE` is TSTOP.
    pub width: ExprId,
    /// Period (s); under `SpiceDefault` a zero or `NONE` is TSTOP.
    pub period: ExprId,
    /// Number of pulses, after which the value holds `v1` (ng NP, lt Ncycles, 5.04): a value
    /// above 0 is a count, any other value unlimited (A17); `NONE`: unlimited. Xyce ignores a
    /// count, so its reader writes `NONE`.
    pub cycles: ExprId,
    /// Which pulse law (AMEND-A12 C1).
    pub law: PulseLaw,
    /// What a zero in `rise`, `fall`, `width`, `period` means (A17).
    pub given_zero: GivenZero,
}

/// `SIN(offset amplitude freq delay damping phase [cycles])`: for t ≥ delay,
/// offset + amplitude·e^(−damping·(t − delay))·sin(2π·freq·(t − delay) + phase), plus
/// amplitude2·sin(2π·freq2·(t − delay)) when the second tone is present.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sin {
    /// Offset. Never `NONE`.
    pub offset: ExprId,
    /// Amplitude. Never `NONE`.
    pub amplitude: ExprId,
    /// Frequency (Hz); under `SpiceDefault` a zero or `NONE` is 1/TSTOP (A17).
    pub freq: ExprId,
    /// Delay (s); may be negative (ng = xy, 5.06); `NONE` only under `SpiceDefault` (omitted: 0).
    pub delay: ExprId,
    /// Damping factor θ (1/s); sp `damp`, va `theta` (5.07); `NONE` only under `SpiceDefault`
    /// (omitted: 0).
    pub damping: ExprId,
    /// Phase at t = delay (degrees); sp `sinephase`, va `tdphase` (5.06); `NONE` only under
    /// `SpiceDefault` (omitted: 0).
    pub phase_deg: ExprId,
    /// Periods after which the value holds (lt Ncycles, 5.06). `NONE`: unlimited.
    pub cycles: ExprId,
    /// Second tone's amplitude (sp `ampl2`, 5.07). `NONE`: no second tone.
    pub amplitude2: ExprId,
    /// Second tone's frequency (Hz, sp `freq2`). Present exactly when `amplitude2` is.
    pub freq2: ExprId,
    /// What the source is before `delay` (ngspice: `HoldDelayValue`, A17).
    pub pre: PreDelay,
    /// What a zero in `freq` means (A17).
    pub given_zero: GivenZero,
}

/// `EXP(v1 v2 td1 tau1 td2 tau2)`: v1 before td1; rising towards v2 with tau1 from td1;
/// falling back with tau2 from td2 (counted as `td2_from` says). Every field is required (never
/// `NONE`) under `GivenZero::AsGiven`; under `SpiceDefault` `td1`, `tau1`, `td2`, `tau2` may be
/// `NONE` and a zero or `NONE` means its default (A17, 5.05).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exp {
    /// Initial value.
    pub v1: ExprId,
    /// Pulsed value.
    pub v2: ExprId,
    /// Rise delay (s).
    pub td1: ExprId,
    /// Rise time constant (s).
    pub tau1: ExprId,
    /// Fall delay (s), counted from where `td2_from` says (5.08; AMEND-A12 C2 withdrew "its
    /// reader adds": the sum would hold the delay in two fields).
    pub td2: ExprId,
    /// Fall time constant (s).
    pub tau2: ExprId,
    /// Where `td2` counts from.
    pub td2_from: TdFrom,
    /// What a zero in `td1`, `tau1`, `td2`, `tau2` means (A17).
    pub given_zero: GivenZero,
}

/// `PWL(t1 v1 t2 v2 …)`: absolute (time, value) points, linear between them. Before the first
/// point the first value holds, after the last the last value, unless `repeat` says otherwise
/// (A22, R42). No reader writes a point the deck lacks: a simulator with another rule before
/// the first point gets a mode member when its slice shows the rule (none today). Relative
/// times (lt `+t`), scale, offset and stretch (lt, sp, va) are accumulated by the reader into
/// the absolute points as expressions (5.09, 5.33).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pwl {
    /// Flat (time, value) pairs, times non-decreasing; `List::EMPTY` only when `file` is set.
    pub points: ExprList,
    /// A shift of the whole wave (s); `NONE`: 0. Native: ng `td=`, hs `TD=`, xy `TD=`, sp
    /// `delay=`. Evidence: E7, E29, 5.11.
    pub delay: ExprId,
    /// The time `repeat` refers to (s): the replay point for `From`, the period for `Period`.
    /// `NONE` exactly when `repeat` is `Never`.
    pub repeat_at: ExprId,
    /// How many passes after the first (lt `REPEAT FOR n`); `NONE`: forever.
    pub repeat_count: ExprId,
    /// The ramp width of `PwlJoin::RampWidth` (s); `NONE` for every other join.
    pub join_width: ExprId,
    /// A file holding the points (lt, hs, xy, sp, va, ng `file=`), as the path is spelled;
    /// `NONE` for inline points. Evidence: E9, E22, 5.12.
    pub file: NameId,
    /// Whether and how the points repeat.
    pub repeat: PwlRepeat,
    /// How a repeat joins its passes; `Jump` when `repeat` is `Never`.
    pub join: PwlJoin,
}

/// Single-frequency FM, by name (5.01): offset + amplitude·sin(2π·carrier_freq·t' +
/// phase_c + mod_index·sin(2π·signal_freq·t' + phase_m)), t' = t − delay, under `law`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sffm {
    /// Offset VO. Never `NONE`.
    pub offset: ExprId,
    /// Amplitude VA. Never `NONE`.
    pub amplitude: ExprId,
    /// Carrier frequency (Hz); under `SpiceDefault` a zero or `NONE` is 500/TSTOP (A17).
    pub carrier_freq: ExprId,
    /// Modulation index; `NONE` only under `SpiceDefault` (omitted: 90, clamped to fc/fm).
    pub mod_index: ExprId,
    /// Modulating (signal) frequency (Hz); `NONE` only under `SpiceDefault` (omitted: 5/TSTOP;
    /// a given 0 stays 0).
    pub signal_freq: ExprId,
    /// Delay (s) (ng TD, E21); `NONE` only under `SpiceDefault` (omitted: 0).
    pub delay: ExprId,
    /// Carrier phase (degrees) (ng PHC); `NONE` only under `SpiceDefault` (omitted: 0).
    pub phase_c_deg: ExprId,
    /// Modulation phase (degrees) (ng PHM); `NONE` only under `SpiceDefault` (omitted: 0).
    pub phase_m_deg: ExprId,
    /// Which formula.
    pub law: SffmLaw,
    /// What the source is before `delay` (AMEND-A12 C3; va holds its value at `delay`, ng is 0
    /// for t ≤ delay).
    pub pre: PreDelay,
    /// What a zero in `carrier_freq` means (A17).
    pub given_zero: GivenZero,
}

/// Amplitude modulation, the general formula every dialect's AM restricts (5.03): for
/// t ≥ delay, offset + amplitude·(mod_offset + mod_index·sin(2π·mod_freq·τ + phase_m))·
/// sin(2π·carrier_freq·(t − delay) + phase_c), τ as `mod_time`. ng: amplitude 1,
/// mod_offset VMO, mod_index VMA; hs: amplitude SA, mod_offset OC, mod_index 1; sp:
/// mod_offset 1. Every field is required under `GivenZero::AsGiven`; under `SpiceDefault` the
/// fields ngspice defaults when omitted (`mod_index`, `mod_freq`, `carrier_freq`, `delay`,
/// `phase_m_deg`, `phase_c_deg`) may be `NONE` (IR-CIRCUIT §3; AM replaces no given zero).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Am {
    /// Offset (ng VO; es has none: 0).
    pub offset: ExprId,
    /// Amplitude.
    pub amplitude: ExprId,
    /// Offset inside the modulating factor.
    pub mod_offset: ExprId,
    /// Modulation index.
    pub mod_index: ExprId,
    /// Modulating frequency (Hz).
    pub mod_freq: ExprId,
    /// Carrier frequency (Hz).
    pub carrier_freq: ExprId,
    /// Delay (s).
    pub delay: ExprId,
    /// Modulation phase (degrees).
    pub phase_m_deg: ExprId,
    /// Carrier phase (degrees).
    pub phase_c_deg: ExprId,
    /// The modulating sine's time.
    pub mod_time: AmModTime,
    /// What the source is before `delay`.
    pub pre: PreDelay,
    /// Whether omitted fields take ngspice's defaults (A17).
    pub given_zero: GivenZero,
}

/// ngspice `TRNOISE(na nt nalpha namp rtsam rtscapt rtsemit)`: noise superposed on a given DC value in
/// the transient only, silent in `.noise` (5.18). `NONE` in an optional field: that term is
/// absent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrNoise {
    /// White noise RMS (V or A). Never `NONE`.
    pub na: ExprId,
    /// Sample interval (s). Never `NONE`.
    pub nt: ExprId,
    /// 1/f exponent, 0 < nalpha < 2.
    pub nalpha: ExprId,
    /// 1/f amplitude.
    pub namp: ExprId,
    /// Random telegraph amplitude.
    pub rts_amp: ExprId,
    /// Mean capture time (s).
    pub rts_capture: ExprId,
    /// Mean emission time (s).
    pub rts_emission: ExprId,
}

/// ngspice `TRRANDOM(type ts [td [p1 [p2]]])`: a random value drawn every `ts`, superposed on a
/// given DC value, transient only; before its first draw and at the operating point it is the
/// offset of its distribution, `p2` (A17; `1-f-code.c:239`). Every field required under
/// `GivenZero::AsGiven`; under `SpiceDefault` an omitted `td`, `p1`, `p2` is `NONE` and means
/// ngspice's default (0, 1, 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrRandom {
    /// Sample interval (s).
    pub ts: ExprId,
    /// Delay (s).
    pub td: ExprId,
    /// First distribution parameter (see `TrRandomKind`).
    pub p1: ExprId,
    /// Second distribution parameter.
    pub p2: ExprId,
    /// The distribution.
    pub kind: TrRandomKind,
    /// Whether omitted fields take ngspice's defaults (A17).
    pub given_zero: GivenZero,
}

/// A bit-pattern source (hs `PAT`, xy `PAT`; 5.22): each bit holds `high` or `low` (`m` the
/// midpoint) for `sample` seconds, edges of `rise`/`fall`. Every field required but
/// `repeats`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pattern {
    /// The high level.
    pub high: ExprId,
    /// The low level.
    pub low: ExprId,
    /// Delay to the first bit (s).
    pub delay: ExprId,
    /// Rise time (s).
    pub rise: ExprId,
    /// Fall time (s).
    pub fall: ExprId,
    /// Bit duration (s).
    pub sample: ExprId,
    /// The bits, as one string of `0`, `1` and `m` (several `b…` words concatenated).
    pub bits: NameId,
    /// Passes after the first (hs `R=2` is three copies); `NONE`: forever (hs `R=-1`). The
    /// last bit holds after the last pass.
    pub repeats: ExprId,
    /// The 1-based bit each repeat starts from (hs/xy `RB`, at least 1).
    pub repeat_start: ExprId,
}

/// A row type of one waveform table: names its kind.
pub trait WaveRow: DetailRow {
    /// The kind whose table holds rows of this type.
    const KIND: WaveKind;
}

/// One waveform, decoded (what `design.wave(r)` returns): the row of the kind's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wave {
    /// A pulse.
    Pulse(Pulse),
    /// A sine.
    Sin(Sin),
    /// An exponential.
    Exp(Exp),
    /// A PWL.
    Pwl(Pwl),
    /// An SFFM.
    Sffm(Sffm),
    /// An AM.
    Am(Am),
    /// Transient noise.
    TrNoise(TrNoise),
    /// Transient random value.
    TrRandom(TrRandom),
    /// A bit pattern.
    Pattern(Pattern),
}

const _: () = assert!(size_of::<WaveKind>() == 1);
const _: () = assert!(size_of::<WaveRef>() == 8);
const _: () = assert!(size_of::<Pulse>() == 36);
const _: () = assert!(size_of::<Sin>() == 40);
const _: () = assert!(size_of::<Exp>() == 28);
const _: () = assert!(size_of::<Pwl>() == 32);
const _: () = assert!(size_of::<Sffm>() == 36);
const _: () = assert!(size_of::<Am>() == 40);
const _: () = assert!(size_of::<TrNoise>() == 28);
const _: () = assert!(size_of::<TrRandom>() == 20);
const _: () = assert!(size_of::<Pattern>() == 36);

macro_rules! wave_rows {
    ($($row:ty => $kind:ident;)*) => {
        $(
            impl WaveRow for $row {
                const KIND: WaveKind = WaveKind::$kind;
            }
        )*
    };
}

wave_rows! {
    Pulse => Pulse;
    Sin => Sin;
    Exp => Exp;
    Pwl => Pwl;
    Sffm => Sffm;
    Am => Am;
    TrNoise => TrNoise;
    TrRandom => TrRandom;
    Pattern => Pattern;
}

impl WaveKind {
    /// Every kind, in declaration order.
    pub const ALL: [WaveKind; 9] = [
        WaveKind::Pulse,
        WaveKind::Sin,
        WaveKind::Exp,
        WaveKind::Pwl,
        WaveKind::Sffm,
        WaveKind::Am,
        WaveKind::TrNoise,
        WaveKind::TrRandom,
        WaveKind::Pattern,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PreDelay {
    /// Every member, in declaration order.
    pub const ALL: [PreDelay; 3] = [PreDelay::Offset, PreDelay::HoldDelayValue, PreDelay::Zero];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl GivenZero {
    /// Every member, in declaration order.
    pub const ALL: [GivenZero; 2] = [GivenZero::AsGiven, GivenZero::SpiceDefault];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PulseLaw {
    /// Every member, in declaration order.
    pub const ALL: [PulseLaw; 2] = [PulseLaw::Spice, PulseLaw::Vacask];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl TdFrom {
    /// Every member, in declaration order.
    pub const ALL: [TdFrom; 2] = [TdFrom::Zero, TdFrom::Td1];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl SffmLaw {
    /// Every member, in declaration order.
    pub const ALL: [SffmLaw; 2] = [SffmLaw::Spice3, SffmLaw::Ngspice];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PwlRepeat {
    /// Every member, in declaration order.
    pub const ALL: [PwlRepeat; 3] = [PwlRepeat::Never, PwlRepeat::From, PwlRepeat::Period];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PwlJoin {
    /// Every member, in declaration order.
    pub const ALL: [PwlJoin; 4] = [
        PwlJoin::Jump,
        PwlJoin::Ramp,
        PwlJoin::RampWidth,
        PwlJoin::Extrapolate,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl AmModTime {
    /// Every member, in declaration order.
    pub const ALL: [AmModTime; 2] = [AmModTime::Delay, AmModTime::CarrierPhase];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl TrRandomKind {
    /// Every member, in declaration order.
    pub const ALL: [TrRandomKind; 4] = [
        TrRandomKind::Uniform,
        TrRandomKind::Gauss,
        TrRandomKind::Exponential,
        TrRandomKind::Poisson,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl WaveRef {
    /// No waveform.
    pub const NONE: WaveRef = WaveRef {
        kind: WaveKind::Pulse,
        index: u32::MAX,
    };

    /// Whether this is [`WaveRef::NONE`].
    pub const fn is_none(self) -> bool {
        self.index == u32::MAX
    }
}

impl Design {
    /// The waveform `wave` refers to, decoded. Asserts it is not `NONE` and is in range.
    pub fn wave(&self, wave: WaveRef) -> Wave {
        assert!(!wave.is_none(), "no waveform");
        match wave.kind {
            WaveKind::Pulse => Wave::Pulse(self.detail(wave.index)),
            WaveKind::Sin => Wave::Sin(self.detail(wave.index)),
            WaveKind::Exp => Wave::Exp(self.detail(wave.index)),
            WaveKind::Pwl => Wave::Pwl(self.detail(wave.index)),
            WaveKind::Sffm => Wave::Sffm(self.detail(wave.index)),
            WaveKind::Am => Wave::Am(self.detail(wave.index)),
            WaveKind::TrNoise => Wave::TrNoise(self.detail(wave.index)),
            WaveKind::TrRandom => Wave::TrRandom(self.detail(wave.index)),
            WaveKind::Pattern => Wave::Pattern(self.detail(wave.index)),
        }
    }
}
