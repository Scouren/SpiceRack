//! The VACASK dialect (slice S4, `rewrite/DIALECT-VACASK.md`): the facts its rules stand on, the
//! front end it reads through, and its reader and writer.
//!
//! Unsupported fails, never skips: without `vacask`, `ngspice` or `$VACASK_CORPUS` the tests
//! that need them fail; run them in `nix develop .#testing`.
//!
//! Cites: VACASK 36be560 (the binary is the oracle of its own dialect, R5); DIALECT-VACASK.md
//! Appendix A (each settle deck and what VACASK printed).
//!
//! Contents: tests only.

#[path = "../common/memcap.rs"]
mod memcap;

mod corpus;
mod grid;
mod includes;
mod probe;
mod read;
mod recursion;
mod settle;
mod settle_syntax;
mod write;
