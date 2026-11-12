//! VACASK's module words ↔ `ir::ModelFamily` (and polarity), and the library file each
//! family's module ships in: the table the model reader and writer share (S4.1).
//!
//! Sub-slice S4.1 writes this file; S4.0 declares it so that the spines and `tables.rs` name it.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (model); VD dev-spice, cir-loading;
//! `syntax/dialects/vacask.rs:244-297` (the module rows the old writer probed).
//!
//! Contents: `FAMILIES` (crate-private)

use crate::ir;

/// `(module word, family, library file)` rows (S4.1 fills it).
#[allow(dead_code, reason = "read_models and write_models (S4.1) read it")]
pub(super) const FAMILIES: &[(&str, ir::ModelFamily, &str)] = &[];
