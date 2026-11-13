//! VACASK 36be560 as a dialect: its text → the IR (`read.rs`), the IR → its text (`write.rs`),
//! the features it spells (`FEATURES`, the union of each area's part) and its code table
//! (`codes.rs`). The reader knows only VACASK, the writer only VACASK; every conversion to or
//! from another dialect's meaning is a pass (`rewrite/DIALECT-VACASK.md` §6).
//!
//! Areas (one owner per file, `rewrite/S4-AREAS.md`): numbers (`number.rs`), names
//! (`name.rs`), expressions (`expr_read.rs`, `expr_write.rs`), structure (`read_structure.rs`,
//! `write_structure.rs`), master binding (`bind.rs`), the dc-plus-`type`-alter fold shared by
//! sources and run (`fold.rs`, R42), devices and models (S4.1), sources (S4.2), the control
//! block (S4.3).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §1–§4, §7; `REWRITE-PLAN.md` §4.1, §4.3.
//!
//! Contents: `pub struct Vacask` (`impl Read`, `impl Write`), `pub static VACASK: Vacask`,
//! `pub const LAWS: ir::Laws`, `pub static FEATURES: ir::FeatureSet`,
//! `pub fn info(code: ir::Code) -> Option<ir::Info>`

mod atan2;
mod bind;
mod codes;
mod diode_law;
mod expr_read;
mod expr_write;
mod families;
mod fold;
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
mod visible;
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

/// The laws the VACASK modules the flake pins compute by: one variant of each of the IR's law
/// points (devices R90–R96, functions R93a, defaults R102), each variant's doc in
/// `crates/ir/src/laws.rs` naming the run; R102's 27 °C by
/// `crates/ir/tests/fixtures/default_temp/`, BSIM4 4.5's exact small-signal (VACASK runs 4.8, F85)
/// by `families/gf180_n1/`. Every read states them (`Builder::set_laws`), so
/// [`FEATURES`] holds their members.
pub const LAWS: ir::Laws = ir::Laws::of(&[
    ir::Law::BcSatAreabOnly,
    ir::Law::SubSatEmissionNs,
    ir::Law::DiodeWlMultOnce,
    ir::Law::DiodeSidewallOwnF1,
    ir::Law::DiodeXomAbsent1e14,
    ir::Law::VdmosKpAbsent20,
    ir::Law::VdmosTcvthRaises,
    ir::Law::VdmosClampByFc,
    ir::Law::MosSmallSignalExact,
    ir::Law::Mos6SmallSignalExact,
    ir::Law::BjtIrbSmallSignalExact,
    ir::Law::VdmosMultChargesCopies,
    ir::Law::DiodeIsrReversePresent,
    ir::Law::DiodeIsrSmallSignalExact,
    ir::Law::DiodeTtFactorClamped,
    ir::Law::TrigExact,
    ir::Law::ExpExact,
    ir::Law::LogZeroError,
    ir::Law::DivExact,
    ir::Law::AsinEdgeFails,
    ir::Law::DefaultTemp27,
    ir::Law::DefaultTnom27,
    ir::Law::Bsim4v45SmallSignalExact,
]);

/// What VACASK spells: the union of every area's part, and exactly the reader's image (law L5).
pub static FEATURES: ir::FeatureSet = expr_read::FEATURES
    .union(read_structure::FEATURES)
    .union(read_elements::FEATURES)
    .union(read_models::FEATURES)
    .union(read_sources::FEATURES)
    .union(read_run::FEATURES)
    .union(read_outputs::FEATURES)
    .union(LAWS.features());

impl Read for Vacask {
    /// The files `text` names: each `include "path"` (`Reference::File`) and each `load` of a
    /// Verilog-A source (`Reference::VerilogA`), in text order. A compiled `.osdi` and a shipped
    /// library file are not followed.
    fn references(&self, text: &str) -> Vec<Reference> {
        read::references(text)
    }

    /// Every file of `src`, in `src`'s order (`read.rs`).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        out.set_laws(LAWS); // once per read: the build's laws (R90)
        read::read(src, out, diags);
    }
}

impl Write for Vacask {
    /// [`FEATURES`].
    fn features(&self) -> &'static ir::FeatureSet {
        &FEATURES
    }

    /// What no feature states, each area's own check: one walk over the reachable items, each
    /// judged by the areas that spell its kind (one exhaustive match), the expression check on
    /// every item, and the R42 fold judged once every analysis was seen.
    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        use ir::ItemKind as K;
        let mut handles = Vec::new();
        let mut folds = fold::Admits::default();
        let index = write_elements::Index::of(design);
        for (at, scope) in design.reachable() {
            expr_write::admit(design, at, &mut handles, diags);
            folds.see(design, at, scope);
            match at.kind {
                K::Load | K::DeviceLibrary | K::Include | K::Section => {
                    write_structure::admit(design, at, diags);
                }
                K::Model => write_models::admit(design, &index.words, at, diags),
                K::Element => {
                    write_elements::admit(design, &index, at, scope, diags);
                    diode_law::admit(design, at, diags);
                    write_sources::admit(design, at, diags);
                }
                K::Analysis | K::Loop | K::Alter | K::NodeSet | K::Option | K::Reset => {
                    write_run::admit(design, at, scope, diags);
                }
                K::Save => write_outputs::admit(design, at, diags),
                K::Title
                | K::Comment
                | K::Global
                | K::Opaque
                | K::Subckt
                | K::If
                | K::Param
                | K::Func
                | K::Statistics
                | K::Env
                | K::Print
                | K::Measure
                | K::Elaborate => {}
            }
        }
        folds.finish(design, diags);
    }

    /// One file of a legal design (`write.rs`). It appends no note: every name it writes is
    /// the design's (R59, A35).
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
