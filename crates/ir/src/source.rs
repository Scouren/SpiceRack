//! Independent sources' typed fields (voltage, current, RF port): DC, AC, waveform, port fields.
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-CIRCUIT.md`.
//!
//! Contents: `pub struct Source`

/// The detail row of an independent source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Source {} // circuit: to define

const _: () = assert!(size_of::<Source>() == 0);
