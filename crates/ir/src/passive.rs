//! Passive elements' typed fields (resistor, capacitor, inductor): value, initial condition, temperature coefficients, parasitics.
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-CIRCUIT.md`.
//!
//! Contents: `pub struct Passive`

/// The detail row of a resistor, capacitor or inductor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Passive {} // circuit: to define

const _: () = assert!(size_of::<Passive>() == 0);
