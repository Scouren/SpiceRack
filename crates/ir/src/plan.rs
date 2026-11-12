//! The run plan as items: loops (sweeps over a range, a list or a table of states; each opens a scope of run items) and alters (the circuit changes from here on).
//!
//! Owner: run (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-RUN.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-RUN.md`.
//!
//! Contents: `pub struct Loop`, `pub struct Alter`

use crate::id::ScopeId;

/// A loop; opens a `ScopeKind::Loop` scope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loop {
    /// Its body, a `ScopeKind::Loop` scope. run: to define the rest (target, sweep).
    pub body: ScopeId,
}

/// A change of the circuit state for every run item after it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alter {} // run: to define

const _: () = assert!(size_of::<Loop>() == 4);
const _: () = assert!(size_of::<Alter>() == 0);
