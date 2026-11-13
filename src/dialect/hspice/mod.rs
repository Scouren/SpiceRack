//! The HSPICE dialect: HSPICE text in → the IR (`Read`), the IR → HSPICE text (`Write`). The
//! reader never learns a target and the writer never learns a source; neither takes a dialect
//! parameter.
//!
//! **The oracle is not HSPICE.** No HSPICE binary runs here: the runnable oracle is ESPice 0.3.0
//! reading HSPICE text (`espice-hspice --tokenizer=hspice`, rev 5f0b8fd), beside the recorded
//! HSPICE pins (`tests/hspice/recorded/*.pin`). A rule settled on ESPice alone says
//! "ESPice only" where it is cited (`rewrite/DIALECT-HSPICE.md` on branch `differences`, the
//! settle decks `tests/fixtures/dialect/hspice/settle/`). The areas started as the ngspice
//! dialect's shape (HSPICE is SPICE-family text); a rule whose cite still names ngspice is a
//! lead carried from there, not yet settled for HSPICE.
//!
//! The caller's code (`dialect::transpile`, HSPICE → HSPICE):
//!
//! ```
//! use spicerack::dialect::{self, Policy, Sources};
//!
//! let deck = "rc divider\n.param rval=1k\nr1 in out rval\n.end\n";
//! let hs = dialect::by_name("hspice").expect("the hspice row");
//! let out = dialect::transpile(&Sources::one("rc.sp", deck), hs, hs, Policy::Lenient)
//!     .expect("no refusal");
//! assert_eq!(out.text, deck);
//! ```
//!
//! Layout as the ngspice dialect's, with no control language (HSPICE has none: `.alter`,
//! `.data` and sweeps are cards).
//!
//! Cites: `rewrite/DIALECT-HSPICE.md`; `REWRITE-PLAN.md` §2–§4; `rewrite/SPINE.md`.
//!
//! Contents: `pub struct Hspice`, `pub static HSPICE: Hspice`, `pub const LAWS: ir::Laws`,
//! `pub const FEATURES: ir::FeatureSet`, `pub use codes::info`; `impl Read for Hspice`, `impl Write for Hspice`

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
mod read_elements;
mod read_include;
mod read_models;
mod read_outputs;
mod read_plan;
mod read_run;
mod read_sources;
mod read_structure;
mod tables;
mod value;
mod visible;
mod write;
mod write_elements;
mod write_models;
mod write_outputs;
mod write_plan;
mod write_run;
mod write_sources;
mod write_structure;

pub use codes::info;

use super::{Read, Reference, Sources, Write};
use crate::ir;

/// The HSPICE reader and writer.
#[derive(Clone, Copy, Debug)]
pub struct Hspice;

/// The `hspice` row of `dialect::BUILTIN`.
pub static HSPICE: Hspice = Hspice;

/// The laws HSPICE computes by (R90): none stated yet. HSPICE's device laws are not settled
/// (ESPice is the only oracle here); hspice-2 settles them. A design read here states no law
/// and goes to any target under the target's law.
pub const LAWS: ir::Laws = ir::Laws::EMPTY;

/// What HSPICE spells natively: the union of the areas' sets, each the image of its reader
/// (law L5) and nothing more.
pub const FEATURES: ir::FeatureSet = write_structure::FEATURES
    .union(expr_write::FEATURES)
    .union(write_elements::FEATURES)
    .union(write_sources::FEATURES)
    .union(write_models::FEATURES)
    .union(write_run::FEATURES)
    .union(write_outputs::FEATURES)
    .union(read_elements::FEATURES)
    .union(read_sources::FEATURES)
    .union(read_models::FEATURES)
    .union(read_run::FEATURES)
    .union(read_outputs::FEATURES)
    .union(LAWS.features());

impl Read for Hspice {
    /// Each `.include` path and `.lib` call's file, in text order, as written.
    fn references(&self, text: &str) -> Vec<Reference> {
        read::references(text)
    }

    /// Every file of `src`, in its order (entries in `src.form()`, the rest libraries).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        out.set_laws(LAWS); // once per read: the build's laws (R90)
        read::read(src, out, diags);
    }
}

impl Write for Hspice {
    /// [`FEATURES`].
    fn features(&self) -> &'static ir::FeatureSet {
        &FEATURES
    }

    /// What HSPICE cannot spell for a reason no feature states (R21): `write::admits`.
    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        write::admits(design, diags);
    }

    /// One file as HSPICE text.
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
