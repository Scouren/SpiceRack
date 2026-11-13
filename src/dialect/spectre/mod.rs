//! Spectre as a dialect: its text → the IR (`read.rs`), the IR → its text (`write.rs`), the
//! features it spells (`FEATURES`, the union of each area's part) and its code table
//! (`codes.rs`). The reader knows only Spectre, the writer only Spectre; every conversion to or
//! from another dialect's meaning is a pass.
//!
//! **The lowest confidence of all dialects: no Spectre runs on this machine.** Every rule rests
//! on the recorded Spectre 25.1 session of 2026-11-07 (RUN-SP: `tests/spectre/*.scs` and their
//! pins), on a manual page (MAN) or on the old pipeline's code (CODE), and says which; the first
//! Spectre run that disagrees overturns a MAN or CODE rule (`rewrite/DIALECT-SPECTRE.md` §10).
//! The CST is the old pipeline's Spectre lexicon and parser (`syntax::Lang::SPECTRE`).
//!
//! Areas: numbers (`number.rs`), expressions (`expr_read.rs`), structure (`read_structure.rs`),
//! instances and models (`read_elements.rs`), sources (`read_sources.rs`), the run
//! (`read_run.rs`), binding (`bind.rs`); the writer's spine and structure (`write.rs`),
//! expressions (`expr_write.rs`), elements and models (`write_elements.rs`), the run
//! (`write_run.rs`), what it refuses (`admits.rs`).
//!
//! Cites: `rewrite/DIALECT-SPECTRE.md`; `REWRITE-PLAN.md` §4.1, §4.3.
//!
//! Contents: `pub struct Spectre` (`impl Read`, `impl Write`), `pub static SPECTRE: Spectre`,
//! `pub static FEATURES: ir::FeatureSet`, `pub fn info(code: ir::Code) -> Option<ir::Info>`

mod admits;
mod bind;
mod codes;
mod expr_read;
mod expr_write;
mod number;
mod read;
mod read_elements;
mod read_run;
mod read_sources;
mod read_structure;
mod write;
mod write_elements;
mod write_run;

use super::{Read, Reference, Sources, Write};
use crate::ir;

/// The `spectre` row's reader and writer.
#[derive(Clone, Copy, Debug)]
pub struct Spectre;

/// The one value the `spectre` row of `dialect::BUILTIN` points at.
pub static SPECTRE: Spectre = Spectre;

/// What Spectre spells: the union of every area's part, and exactly the reader's image (law L5).
pub static FEATURES: ir::FeatureSet = expr_read::FEATURES
    .union(read_structure::FEATURES)
    .union(read_elements::FEATURES)
    .union(read_sources::FEATURES)
    .union(read_run::FEATURES);

impl Read for Spectre {
    /// The files `text` names: each `include "path"` (`Reference::File`) and each
    /// `ahdl_include` of a Verilog-A source (`Reference::VerilogA`), in text order.
    fn references(&self, text: &str) -> Vec<Reference> {
        read::references(text)
    }

    /// Every file of `src`, in `src`'s order (`read.rs`).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        read::read(src, out, diags);
    }
}

impl Write for Spectre {
    /// [`FEATURES`].
    fn features(&self) -> &'static ir::FeatureSet {
        &FEATURES
    }

    /// What no feature states: an unspellable name, another dialect's option (`admits.rs`).
    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        admits::admits(design, diags);
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

/// The catalogue row of one of Spectre's own codes (`P6xxx`, `W6xxx`), or `None`.
pub fn info(code: ir::Code) -> Option<ir::Info> {
    codes::info(code)
}
