//! Controlled sources' typed fields (VCVS, VCCS, CCCS, CCVS, polynomial, table, behavioural, Laplace).
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-CIRCUIT.md`.
//!
//! Contents: `pub struct Controlled`

/// The detail row of a linear controlled source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controlled {} // circuit: to define

const _: () = assert!(size_of::<Controlled>() == 0);
