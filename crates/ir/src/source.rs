//! Independent sources' typed fields (voltage source, current source, RF port): the three
//! independent slots a source drives (the DC value, the AC stimulus, the transient waveform),
//! their temperature factor, and the cold side rows for port, distortion and parasitic data.
//! Builder pushes in, rows out through `design.source(&e)`.
//!
//! The three slots are independent (INV-semantics 5.13): `.op`/`.dc` read `dc`, `.ac` reads
//! `ac_mag`/`ac_phase_deg`, the transient reads `wave` (or `dc` when there is none). `dc` is
//! always present: each reader writes its own simulator's rule for a missing DC (the wave's
//! t = 0 value with phase included, 5.32; 0 for ngspice SFFM, AM, TRNOISE, TRRANDOM; VACASK,
//! which ignores `dc=` on a waveform type, writes the waveform's t = 0 value), never a rule of
//! another simulator.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §1, §2 (sources); `rewrite/INV-constructs.md` D12–D15, D58,
//! D97, D109–D111, D127, D129, E1, E2, E24, E26, E37; `rewrite/INV-semantics.md` 5.13, 5.14,
//! 5.16–5.21, 5.32.
//!
//! Contents:
//! - `pub struct Source`, `pub struct PortSpec`, `pub struct Distof`
//! - `Design::source(&self, &Element) -> Source`

use crate::design::Design;
use crate::element::{DetailKind, Element};
use crate::id::{ExprId, ExprList};
use crate::wave::WaveRef;

/// The detail row of an independent source (`ElemKind::VSource`, `ISource`, `Port`). Values
/// are volts for a voltage source and a port (the EMF, see [`PortSpec`]), amperes for a current
/// source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {
    /// The value `.op` and `.dc` use (V or A). Never `NONE`: the reader writes its simulator's
    /// value for a card that gives none (Xyce bare V is 0, 5.14). Native: all. Evidence: E1,
    /// D12, 5.13, 5.14, 5.32.
    pub dc: ExprId,
    /// AC stimulus magnitude (V or A). `NONE`: no AC stimulus. `ac` alone is magnitude 1,
    /// written by the reader (5.14). May be an expression (lt, ng, xy; E26). Native: all.
    pub ac_mag: ExprId,
    /// AC stimulus phase (degrees). Present exactly when `ac_mag` is (the reader writes 0 for
    /// a card without one). May be an expression (xy, E37). Native: all.
    pub ac_phase_deg: ExprId,
    /// Temperature coefficient 1 (1/K) of a factor 1 + tc1·ΔT + tc2·ΔT², ΔT = T − tnom, that
    /// multiplies every slot, the waveform included (sp `tc1`). `NONE`: 0. Native: sp; va has
    /// none (an expression of temperature instead). Evidence: D129, 5.19.
    pub tc1: ExprId,
    /// Temperature coefficient 2 (1/K²) of `tc1`'s factor. `NONE`: 0. Evidence: 5.19.
    pub tc2: ExprId,
    /// The magnitude a periodic AC analysis drives (sp `pacmag`, V or A). `NONE`: no PAC
    /// stimulus. Native: sp. Evidence: D129.
    pub pac_mag: ExprId,
    /// The phase of `pac_mag` (degrees, sp `pacphase`). Present exactly when `pac_mag` is.
    /// Native: sp. Evidence: D129.
    pub pac_phase_deg: ExprId,
    /// The transient waveform, or `WaveRef::NONE`: the transient holds `dc`.
    pub wave: WaveRef,
    /// Small-signal noise of the source as (frequency Hz, PSD V²/Hz or A²/Hz) pairs (sp
    /// `noisevec`); `List::EMPTY` for none. Distinct from the time-domain `TrNoise` wave: one
    /// is never converted into the other here (5.18). Native: sp. Evidence: D129, 5.18.
    pub noise_psd: ExprList,
    /// Row in the parasitics table (`rser`, `cpar` only; lt), or `NO_ROW`. Evidence: D8.
    pub parasitics: u32,
    /// Row in the port table: present exactly on a `Port`, `NO_ROW` on a V or I source.
    pub port: u32,
    /// Row in the distortion table (ng `distof1`/`distof2`), or `NO_ROW`. Evidence: D58, 5.21.
    pub distof: u32,
}

/// The port data of an RF port. The source slots of a port hold the EMF: the open-circuit
/// value behind `z0` (INV-semantics 5.17). Spectre's user value is half of it (its internal
/// source is double), so its reader doubles and its writer halves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortSpec {
    /// The port number (ng `portnum`, xy/hs `port=`, sp `num=`), a positive integer. Never
    /// `NONE`. Evidence: D15, D97, D111.
    pub index: ExprId,
    /// Reference impedance (Ω). Never `NONE`: the reader writes 50 when the card gives none.
    /// Evidence: D15, 5.17.
    pub z0: ExprId,
    /// Noise temperature of the port's resistance (°C; sp `noisetemp`). `NONE`: the circuit
    /// temperature. Native: sp. Evidence: 5.17.
    pub noise_temp: ExprId,
    /// Available power for a large-signal drive (ng `pwr`, W). `NONE`: none. Native: ng.
    /// Evidence: E24.
    pub power: ExprId,
    /// Frequency of that drive (ng `freq`, Hz). Present exactly when `power` is. Evidence: E24.
    pub freq: ExprId,
}

/// The distortion inputs of a source, read only by a distortion analysis (ng `.disto`); a
/// source with none contributes 0 there (5.21). `NONE` in a pair: that tone is absent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distof {
    /// `distof1` magnitude (V or A).
    pub f1_mag: ExprId,
    /// `distof1` phase (degrees); present exactly when `f1_mag` is.
    pub f1_phase_deg: ExprId,
    /// `distof2` magnitude (V or A).
    pub f2_mag: ExprId,
    /// `distof2` phase (degrees); present exactly when `f2_mag` is.
    pub f2_phase_deg: ExprId,
}

const _: () = assert!(size_of::<Source>() == 56);
const _: () = assert!(size_of::<PortSpec>() == 20);
const _: () = assert!(size_of::<Distof>() == 16);

impl Design {
    /// The source row of a voltage source, current source or port. Asserts the kind.
    pub fn source(&self, element: &Element) -> Source {
        assert!(
            element.kind.detail() == DetailKind::Source,
            "{:?} has no Source row",
            element.kind
        );
        self.detail(element.detail)
    }
}
