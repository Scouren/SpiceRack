//! Device models: the model row (family, polarity, parameters, bins).
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-CIRCUIT.md`.
//!
//! Contents: `pub struct Model`

use crate::id::NameId;

/// A device model (one row per bin). Defines `DefKind::Model` `name`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    /// The model's name. circuit: to define the rest.
    pub name: NameId,
}

const _: () = assert!(size_of::<Model>() == 4);

// circuit: to define — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Model);
