//! Outputs as items: probes, save, print and measure rows.
//!
//! Owner: run (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-RUN.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-RUN.md`.
//!
//! Contents: `pub struct Save`, `pub struct Print`, `pub struct Measure`

/// What to save.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Save {} // run: to define

/// What to print.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Print {} // run: to define

/// A measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {} // run: to define

const _: () = assert!(size_of::<Save>() == 0);
const _: () = assert!(size_of::<Print>() == 0);
const _: () = assert!(size_of::<Measure>() == 0);

// run: to define — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Save);
todo_facts!(Print);
todo_facts!(Measure);
