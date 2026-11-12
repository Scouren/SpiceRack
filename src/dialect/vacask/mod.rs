//! VACASK 36be560 as a dialect: its text → the IR (`read.rs`), the IR → its text (`write.rs`),
//! the features it spells (`FEATURES`, the union of each area's part) and its code table
//! (`codes.rs`). The reader knows only VACASK, the writer only VACASK; every conversion to or
//! from another dialect's meaning is a pass (`rewrite/DIALECT-VACASK.md` §6).
//!
//! Areas (one owner per file, `rewrite/S4-AREAS.md`): numbers (`number.rs`), names
//! (`name.rs`), expressions (`expr_read.rs`, `expr_write.rs`), structure (`read_structure.rs`,
//! `write_structure.rs`), devices and models (S4.1), sources (S4.2), the control block (S4.3).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §1–§4, §7; `REWRITE-PLAN.md` §4.1, §4.3.
//!
//! Contents: `pub struct Vacask` (`impl Read`, `impl Write`), `pub static VACASK: Vacask`,
//! `pub static FEATURES: ir::FeatureSet`, `pub fn info(code: ir::Code) -> Option<ir::Info>`

mod codes;
mod expr_read;
mod expr_write;
mod families;
mod name;
mod number;
mod read;
mod read_elements;
mod read_models;
mod read_outputs;
mod read_run;
mod read_sources;
mod read_structure;
mod tables;
mod write;
mod write_elements;
mod write_models;
mod write_outputs;
mod write_run;
mod write_sources;
mod write_structure;

use super::{Read, Reference, Sources, Write};
use crate::ir;

/// The `vacask` row's reader and writer.
#[derive(Clone, Copy, Debug)]
pub struct Vacask;

/// The one value the `vacask` row of `dialect::BUILTIN` points at.
pub static VACASK: Vacask = Vacask;

/// What VACASK spells: the union of every area's part, and exactly the reader's image (law L5).
pub static FEATURES: ir::FeatureSet = expr_read::FEATURES
    .union(read_structure::FEATURES)
    .union(read_elements::FEATURES)
    .union(read_models::FEATURES)
    .union(read_sources::FEATURES)
    .union(read_run::FEATURES)
    .union(read_outputs::FEATURES);

impl Read for Vacask {
    /// The files `text` names: each `include "path"` (`Reference::File`) and each `load` of a
    /// Verilog-A source (`Reference::VerilogA`), in text order. A compiled `.osdi` and a shipped
    /// library file are not followed.
    fn references(&self, text: &str) -> Vec<Reference> {
        read::references(text)
    }

    /// Every file of `src`, in `src`'s order (`read.rs`).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        read::read(src, out, diags);
    }
}

impl Write for Vacask {
    /// [`FEATURES`].
    fn features(&self) -> &'static ir::FeatureSet {
        &FEATURES
    }

    /// What no feature states, each area's own check (`write_structure::admits` and each
    /// writer area's `admits`).
    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        write_structure::admits(design, diags);
        write_elements::admits(design, diags);
        write_models::admits(design, diags);
        write_sources::admits(design, diags);
        write_run::admits(design, diags);
        write_outputs::admits(design, diags);
    }

    /// One file of a legal design (`write.rs`).
    fn write(
        &self,
        design: &ir::Design,
        file: ir::FileId,
        out: &mut String,
        diags: &mut ir::Diags,
    ) {
        write::write(design, file, out, diags);
    }
}

/// The catalogue row of one of VACASK's own codes (`P2xxx`, `R2xxx`, `W2xxx`), or `None`.
pub fn info(code: ir::Code) -> Option<ir::Info> {
    codes::info(code)
}
