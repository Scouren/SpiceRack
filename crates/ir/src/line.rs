//! Transmission lines' typed fields (lossless, lossy, RC, single lossy, coupled).
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-CIRCUIT.md`.
//!
//! Contents: `pub struct Line`

/// The detail row of a transmission line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {} // circuit: to define

const _: () = assert!(size_of::<Line>() == 0);
