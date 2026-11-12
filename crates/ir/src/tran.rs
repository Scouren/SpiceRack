//! Time-domain analysis rows: the transient and the Fourier analysis that reads it. A reader's
//! `.tran`/`tran`/`.four` in, detail rows out.
//!
//! Every simulator reads the positional `.tran` fields differently (Xyce's first is an
//! initial-step guess, ngspice's a print step that also caps the step, HSPICE's further fields
//! are segments, Spectre's `step` is for output only), so the row names every quantity and the
//! reader puts each number in the field its simulator means (IR-RUN §2, `INV-semantics.md`
//! 8.01–8.03). The step ceiling a simulator applies by default differs too: that is `cap`, a law
//! field, used when `max_step` is `NONE`; no number is computed by a reader.
//!
//! Cites: `rewrite/IR-RUN.md` §2; `rewrite/INV-constructs.md` G6, G7, G14, G39, G42, G48, G50,
//! G54, H29, I11, I19; `rewrite/INV-semantics.md` 8.01–8.03, 8.19, 8.20, 8.21, 8.25.
//!
//! Contents: `pub struct Tran`, `pub enum StepCap`, `pub struct Four`; `StepCap::ALL`,
//! `StepCap::COUNT`

use crate::id::{ExprId, ExprList};
use crate::probe::ProbeList;

/// A transient analysis's detail. Every field is named and none is inferred from another;
/// `NONE` is "not given", never 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tran {
    /// The end time, s; never `NONE`.
    pub stop: ExprId,
    /// When results start being recorded, s (SPICE `tstart`, HSPICE `START=`, VACASK
    /// `start=`); `NONE` = from 0.
    pub record_start: ExprId,
    /// The output interval, s (ngspice `tstep`, Spectre `step`); `NONE` = the simulator
    /// chooses (Xyce and LTspice step 0; ngspice has no such thing: its writer lacks
    /// `RunExtra::TranSimulatorStep`).
    pub print_step: ExprId,
    /// The time-step ceiling, s (SPICE `tmax`, Spectre and VACASK `maxstep`, Xyce's fourth
    /// field; a Xyce `schedule()` is its smallest nonzero step); `NONE` = `cap` decides.
    pub max_step: ExprId,
    /// The first step's size, s (Xyce's first field); `NONE` = the simulator's guess.
    pub initial_step: ExprId,
    /// Device noise in the transient: the highest frequency, Hz (VACASK `noisefmax=`); `NONE`
    /// = no device noise (HSPICE `.trannoise` is this, `INV-semantics.md` 8.20).
    pub noise_fmax: ExprId,
    /// Device noise: the lowest frequency, Hz (VACASK default `noise_fmax/1e3`, Spectre's
    /// `noise_fmax`: a reader fills its default). `NONE` exactly when `noise_fmax` is.
    pub noise_fmin: ExprId,
    /// Device noise: the seed, or `NONE` for the simulator's own.
    pub noise_seed: ExprId,
    /// HSPICE's further segments: `(print_step, stop)` pairs after the first, in order; empty
    /// elsewhere.
    pub segments: ExprList,
    /// The step ceiling when `max_step` is `NONE`.
    pub cap: StepCap,
    /// Skip the operating point and start from the initial conditions (`uic`, Xyce `NOOP`,
    /// Spectre `skipdc=yes`, VACASK `icmode="uic"`).
    pub uic: bool,
    /// Ramp the DC-only top-level sources over the first 20 µs (LTspice `startup`); lowering
    /// it to PWL sources is a pass.
    pub startup: bool,
}

/// The ceiling a simulator puts on the time step when the deck gives none. The span is
/// `stop − record_start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StepCap {
    /// No ceiling beyond the span.
    None,
    /// `min(print_step, span/50)` (ngspice).
    NgspiceRule,
    /// `span/50` (Spectre, VACASK).
    SpanOver50,
    /// `span/10` (Xyce).
    SpanOver10,
}

/// A Fourier analysis of the preceding transient's last periods (`.four`, Spectre `fourier`).
/// Its result is a table, not a waveform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Four {
    /// The fundamental frequency, Hz; never `NONE`.
    pub fundamental: ExprId,
    /// The number of harmonics, the fundamental included (ngspice `.options nfreqs=`, LTspice
    /// `Nh`, default 9); never `NONE`: a reader fills its default.
    pub harmonics: ExprId,
    /// How many periods at the end are analysed (LTspice `Np`, default 1); never `NONE`.
    pub periods: ExprId,
    /// The interpolation grid's size (ngspice `fourgridsize`), or `NONE` for the simulator's.
    pub grid: ExprId,
    /// The outputs analysed, at least one.
    pub outputs: ProbeList,
}

const _: () = assert!(size_of::<Tran>() == 44);
const _: () = assert!(size_of::<Four>() == 24);

impl StepCap {
    /// Every rule, in declaration order.
    pub const ALL: [StepCap; 4] = [
        StepCap::None,
        StepCap::NgspiceRule,
        StepCap::SpanOver50,
        StepCap::SpanOver10,
    ];
    /// Rule count.
    pub const COUNT: usize = Self::ALL.len();
}
