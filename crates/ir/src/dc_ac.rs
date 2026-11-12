//! DC and small-signal analysis rows: DC sweeps and their axes, AC, noise, the transfer
//! functions, sensitivities, pole-zero, S-parameters, two-port networks, distortion and the
//! timed operating-point report. A reader's analysis statement in, one detail row out.
//!
//! An AC or DC analysis over something other than frequency or a source (Spectre `ac dev=
//! param= freq=`) is a `Loop` over that target around the analysis (IR-RUN §2). Sweeps are
//! [`SweepSpec`]s. Sources and outputs are named, never text: a source is a top-level element's
//! name, an output a [`Probe`] (`INV-semantics.md` 8.10).
//!
//! Cites: `rewrite/IR-RUN.md` §2; `rewrite/INV-constructs.md` G1–G5, G8–G11, G13, G15, G34, G35,
//! G37, G38, G41, G42, G43, G49, G60, G61, G65, G66, G71; `rewrite/INV-semantics.md` 8.04–8.14,
//! 8.19, 8.20, 8.24, 8.26; `DESIGN-analysis-kinds.md` §2.
//!
//! Contents: `pub struct Dc`, `pub struct DcAxis`, `pub type DcAxisList`, `pub struct Ac`,
//! `pub struct Noise`, `pub enum NoiseRef`, `pub struct Tf`, `pub struct DcXf`, `pub struct Xf`,
//! `pub struct Sens`, `pub struct SensAc`, `pub struct Pz`, `pub enum PzInput`,
//! `pub enum PzTransfer`, `pub enum PzFind`, `pub struct Sp`, `pub struct Net`,
//! `pub struct Disto`, `pub struct TimedOp`; on each enum: `ALL`, `COUNT`;
//! `Design::dc_axes(&self, DcAxisList) -> &[DcAxis]`

use crate::design::Design;
use crate::id::{ExprId, ExprList, List, NameId, NameList};
use crate::plan::Target;
use crate::probe::Probe;
use crate::sweep::SweepSpec;

/// A DC sweep: nested axes, the first innermost (the SPICE order), at least one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dc {
    /// The axes, innermost first.
    pub axes: DcAxisList,
    /// Each point starts from the initial guess instead of the previous point's solution
    /// (ngspice in HSPICE compatibility mode; a Schmitt trigger then switches elsewhere,
    /// `INV-semantics.md` 8.09).
    pub restart: bool,
}

/// One axis of a DC sweep: a target (a source's DC value, temperature, a parameter, an element
/// value, a device or model parameter) over a sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DcAxis {
    /// What is swept.
    pub target: Target,
    /// Its values.
    pub sweep: SweepSpec,
}

/// An ordered list of DC axes, a range into the run owner's axis pool.
pub type DcAxisList = List<DcAxis>;

/// A small-signal AC analysis over frequency.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ac {
    /// The frequencies, Hz (a one-point sweep is still an AC analysis, 8.26).
    pub sweep: SweepSpec,
}

/// A small-signal noise analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// The output: a `V`/`Vdiff` (reference node defaults to ground) or an `I` probe.
    pub output: Probe,
    /// The input source the noise is referred to (a top-level element).
    pub source: NameId,
    /// The frequencies, Hz; unread when `ac_sweep`.
    pub sweep: SweepSpec,
    /// Every this many points a summary is printed (ngspice `pts_per_summary`), or `NONE`.
    pub points_per_summary: ExprId,
    /// The frequencies are the deck's AC analysis's (HSPICE `.noise out src [interval]`).
    pub ac_sweep: bool,
    /// How the input-referred noise is scaled.
    pub input_ref: NoiseRef,
}

/// What the input-referred noise is divided by (`INV-semantics.md` 8.10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NoiseRef {
    /// The gain to a unit input (ngspice).
    Unit,
    /// The gain to the input source's own AC magnitude (Xyce: an AC 0.1 source gives 10×).
    SourceAc,
}

/// A DC transfer function from one source to an output, with input and output resistance
/// (`.tf`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tf {
    /// The output: a `V`/`Vdiff` or an `I` probe.
    pub output: Probe,
    /// The input source (a top-level element).
    pub source: NameId,
}

/// The DC transfer function from every source to an output (VACASK `dcxf`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DcXf {
    /// The output.
    pub output: Probe,
}

/// The small-signal transfer function from every source to an output, over frequency
/// (Spectre `xf`, VACASK `acxf`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    /// The output.
    pub output: Probe,
    /// The frequencies, Hz.
    pub sweep: SweepSpec,
}

/// DC sensitivity of an output to every device parameter (ngspice, HSPICE `.sens out`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sens {
    /// The output.
    pub output: Probe,
}

/// AC sensitivity of an output to every device parameter (ngspice `.sens out ac …`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SensAc {
    /// The output.
    pub output: Probe,
    /// The frequencies, Hz.
    pub sweep: SweepSpec,
}

/// A pole-zero analysis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pz {
    /// How the input is given.
    pub input: PzInput,
    /// Voltage or current transfer (`vol`/`cur`).
    pub transfer: PzTransfer,
    /// What is found.
    pub find: PzFind,
    /// `Nodes`: the input's positive node; `NONE` for `Source`.
    pub in_p: NameId,
    /// `Nodes`: the input's negative node; `NONE` for `Source`.
    pub in_n: NameId,
    /// The output's positive node.
    pub out_p: NameId,
    /// The output's negative node (ground: `NameId::GROUND`).
    pub out_n: NameId,
    /// `Source`: the input source (HSPICE `.pz v(out) src`, Spectre `iprobe=`); `NONE` for
    /// `Nodes`.
    pub source: NameId,
}

/// How a pole-zero analysis names its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PzInput {
    /// A node pair (ngspice `.pz in+ in- out+ out- …`).
    Nodes,
    /// An independent source.
    Source,
}

/// The transfer a pole-zero analysis studies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PzTransfer {
    /// Output voltage over input voltage (`vol`).
    Voltage,
    /// Output voltage over input current (`cur`).
    Current,
}

/// What a pole-zero analysis finds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PzFind {
    /// Poles only (`pol`).
    Poles,
    /// Zeros only (`zer`).
    Zeros,
    /// Both (`pz`).
    Both,
}

/// An S-parameter analysis over the deck's RF ports (ngspice `.sp`, Xyce and HSPICE `.LIN`,
/// Spectre `sp`, VACASK `acsp`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sp {
    /// The frequencies, Hz.
    pub sweep: SweepSpec,
    /// The port elements in port order (Spectre `ports=`, VACASK `ports=[…]`); empty = every
    /// RF port element, by its own port index.
    pub ports: NameList,
    /// Also compute the noise parameters (ngspice `donoise`, HSPICE `noisecalc=1`).
    pub noise: bool,
}

/// Two-port network parameters during the AC analysis (LTspice `.net`, HSPICE `.NET`). The
/// terminations are not part of the network (`INV-semantics.md` 8.14).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Net {
    /// The output port: a `V` or `I` probe.
    pub output: Probe,
    /// The input source.
    pub source: NameId,
    /// The input termination, Ω, or `NONE` for the source's own series resistance.
    pub rin: ExprId,
    /// The output termination, Ω, or `NONE` for the output element's resistance.
    pub rout: ExprId,
}

/// Small-signal distortion (ngspice `.disto`): raw harmonic and intermodulation voltages of the
/// sources' `distof1`/`distof2` drives (HSPICE `.DISTO Rload` is another quantity: `Opaque`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Disto {
    /// The first tone's frequencies, Hz.
    pub sweep: SweepSpec,
    /// The second tone as a ratio of the first (`f2overf1`), or `NONE` for harmonics only.
    pub f2_over_f1: ExprId,
}

/// Operating-point reports at given times of the transient (HSPICE `.op t1 t2 …`, Spectre
/// `tran … infotimes=[…]`), `INV-semantics.md` 8.11.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedOp {
    /// The times, s, at least one.
    pub times: ExprList,
}

const _: () = assert!(size_of::<Dc>() == 12);
const _: () = assert!(size_of::<DcAxis>() == 52);
const _: () = assert!(size_of::<Ac>() == 28);
const _: () = assert!(size_of::<Noise>() == 64);
const _: () = assert!(size_of::<Tf>() == 28);
const _: () = assert!(size_of::<DcXf>() == 24);
const _: () = assert!(size_of::<Xf>() == 52);
const _: () = assert!(size_of::<Sens>() == 24);
const _: () = assert!(size_of::<SensAc>() == 52);
const _: () = assert!(size_of::<Pz>() == 24);
const _: () = assert!(size_of::<Sp>() == 40);
const _: () = assert!(size_of::<Net>() == 36);
const _: () = assert!(size_of::<Disto>() == 32);
const _: () = assert!(size_of::<TimedOp>() == 8);

impl NoiseRef {
    /// Every rule, in declaration order.
    pub const ALL: [NoiseRef; 2] = [NoiseRef::Unit, NoiseRef::SourceAc];
    /// Rule count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PzInput {
    /// Every form, in declaration order.
    pub const ALL: [PzInput; 2] = [PzInput::Nodes, PzInput::Source];
    /// Form count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PzTransfer {
    /// Every transfer, in declaration order.
    pub const ALL: [PzTransfer; 2] = [PzTransfer::Voltage, PzTransfer::Current];
    /// Transfer count.
    pub const COUNT: usize = Self::ALL.len();
}

impl PzFind {
    /// Every choice, in declaration order.
    pub const ALL: [PzFind; 3] = [PzFind::Poles, PzFind::Zeros, PzFind::Both];
    /// Choice count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The axes of a list. Asserts the range is inside the axis pool.
    pub fn dc_axes(&self, list: DcAxisList) -> &[DcAxis] {
        todo!()
    }
}
