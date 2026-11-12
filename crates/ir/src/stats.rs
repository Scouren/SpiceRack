//! Statistics blocks: process and mismatch variations, correlations, truncation.
//!
//! Owner: run (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-RUN.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-RUN.md`.
//!
//! Contents: `pub struct Statistics`

/// A statistics block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Statistics {} // run: to define

const _: () = assert!(size_of::<Statistics>() == 0);
