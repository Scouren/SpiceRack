//! SpiceRack: a netlist read in any dialect and written in any other
//! (`syntax`, the transpiler; `import` its file-level entry), run on a
//! simulator (`backend`, `simulation`) and read back (`rawfile`, `result`,
//! `measure_parse`). `python` is the PyO3 binding the `spicerack` package
//! ships, which builds its decks as ngspice text for `syntax` to read.
//! The rewrite's pipeline beside it: `ir` (the semantic IR crate), `dialect`
//! (the `Read`/`Write` contract, the spines and the seam the old entry
//! points route through) and `passes` (IR → IR legalization).
//!
//! Cites: none (each module cites its own manuals; `syntax::lexicon` and
//! `syntax::device` are the dialect readers' tables).
//!
//! Contents:
//! - `pub use spicerack_ir as ir`, `pub mod dialect`, `pub mod passes`
//! - `pub mod backend` (feature `backend`), `import`, `lint`,
//!   `measure_parse`, `normalize`, `rawfile`, `result`,
//!   `simulation` (feature `backend`), `syntax` (the transpiler),
//!   `unit`, `veriloga`, `python` (feature `python`)

#![allow(clippy::too_many_arguments)]

pub use spicerack_ir as ir;

#[cfg(feature = "backend")]
pub mod backend;
pub mod dialect;
pub mod import;
pub mod lint;
pub mod measure_parse;
pub mod normalize;
pub mod passes;
pub mod rawfile;
pub mod result;
#[cfg(feature = "backend")]
pub mod simulation;
pub mod syntax;
pub mod unit;
pub mod veriloga;

#[cfg(feature = "python")]
#[allow(non_snake_case)]
pub mod python;
