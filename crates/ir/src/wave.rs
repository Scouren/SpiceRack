//! Source waveforms: the closed list of waveform kinds and one row table each.
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-CIRCUIT.md`.
//!
//! Contents: `pub enum WaveKind` (`ALL`, `COUNT`), `pub struct Wave`

/// A waveform kind (`rewrite/IR-CIRCUIT.md` §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WaveKind {
    /// circuit: to define (Pulse, Sin, Exp, Pwl, Sffm, Am, TrNoise, TrRandom, Pattern).
    Placeholder,
}

/// A waveform row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    /// circuit: to define.
    pub kind: WaveKind,
}

const _: () = assert!(size_of::<Wave>() == 1);

impl WaveKind {
    /// Every kind, in declaration order.
    pub const ALL: [WaveKind; 1] = [WaveKind::Placeholder];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}
