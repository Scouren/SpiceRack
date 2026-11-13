//! The ngspice dialect (ngspice-45, the pinned simulator): ngspice text in → the IR (`Read`),
//! the IR → ngspice text (`Write`). The reader never learns a target and the writer never learns
//! a source; neither takes a dialect parameter. Every rule is a fact of ngspice-45 with its
//! evidence (an `INV-semantics.md` or `INV-constructs.md` row, a manual clause, the ngspice-45
//! source, or a run), cited where the rule is.
//!
//! The caller's code (`dialect::transpile`, ngspice → ngspice):
//!
//! ```
//! use spicerack::dialect::{self, Policy, Sources};
//!
//! let deck = "rc divider\n.param rval=1k\n.subckt div in out params: r=rval\n* the body\n.ends\n.end\n";
//! let ng = dialect::by_name("ngspice").expect("the ngspice row");
//! let out = dialect::transpile(&Sources::one("rc.sp", deck), ng, ng, Policy::Lenient)
//!     .expect("no refusal");
//! assert_eq!(out.text, deck);
//! ```
//!
//! Layout (`rewrite/DIALECT-NGSPICE.md` §2): the two spines (`read.rs`, `write.rs`), numbers,
//! names and expressions (`number.rs`, `name.rs`, `expr_read.rs`, `expr_write.rs`, the shared
//! rows in `tables.rs`), the structure area (`read_structure.rs`, `read_include.rs`, `write_structure.rs`), the
//! code table (`codes.rs`), and the areas of the later sub-slices as stubs (elements and
//! sources S3.1, models S3.2, run and outputs S3.3, control S3.4; `rewrite/S3-AREAS.md`).
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md`; `REWRITE-PLAN.md` §2–§4; `rewrite/SPINE.md`.
//!
//! Contents: `pub struct Ngspice`, `pub struct Flavour`, `pub static NGSPICE: Ngspice`,
//! `pub const FEATURES: ir::FeatureSet`, `pub use codes::info`; `impl Read for Ngspice`,
//! `impl Write for Ngspice`

mod bind;
mod codes;
mod cst;
mod declare;
mod expr_lex;
mod expr_read;
mod expr_write;
mod families;
mod name;
mod number;
mod outputs;
mod read;
mod read_control;
mod read_elements;
mod read_include;
mod read_models;
mod read_outputs;
mod read_run;
mod read_sources;
mod read_structure;
mod tables;
mod value;
mod visible;
mod write;
mod write_control;
mod write_elements;
mod write_models;
mod write_outputs;
mod write_run;
mod write_sources;
mod write_structure;

pub use codes::info;

use super::{Read, Reference, Sources, Write};
use crate::ir;

/// The ngspice reader and writer, for one flavour.
#[derive(Clone, Copy, Debug)]
pub struct Ngspice {
    /// The flavour's facts (ESPice and RSpice are flavours: a later slice).
    pub flavour: Flavour,
}

/// What a flavour of ngspice changes in reading and writing. No field yet: nothing in the
/// reader or writer branches on a flavour today; the flavour slice adds the fields it needs.
#[derive(Clone, Copy, Debug)]
pub struct Flavour {}

/// ngspice-45 itself: the `ngspice` row of `dialect::BUILTIN`.
pub static NGSPICE: Ngspice = Ngspice {
    flavour: Flavour {},
};

/// What ngspice-45 spells natively: the union of the areas' sets, each the image of its reader
/// (law L5) and nothing more.
pub const FEATURES: ir::FeatureSet = write_structure::FEATURES
    .union(expr_write::FEATURES)
    .union(write_elements::FEATURES)
    .union(write_sources::FEATURES)
    .union(write_models::FEATURES)
    .union(write_run::FEATURES)
    .union(write_outputs::FEATURES)
    .union(write_control::FEATURES)
    .union(read_elements::FEATURES)
    .union(read_sources::FEATURES)
    .union(read_models::FEATURES)
    .union(read_run::FEATURES)
    .union(read_outputs::FEATURES)
    .union(read_control::FEATURES);

impl Read for Ngspice {
    /// Each `.include` path and `.lib` call's file, in text order, as written.
    fn references(&self, text: &str) -> Vec<Reference> {
        read::references(text)
    }

    /// Every file of `src`, in its order (entries in `src.form()`, the rest libraries).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        read::read(src, out, diags);
    }
}

impl Write for Ngspice {
    /// [`FEATURES`].
    fn features(&self) -> &'static ir::FeatureSet {
        &FEATURES
    }

    /// What ngspice cannot spell for a reason no feature states (R21): `write::admits`.
    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        write::admits(design, diags);
    }

    /// One file as ngspice text.
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

#[cfg(test)]
mod expr_tests;
#[cfg(test)]
mod read_tests;
