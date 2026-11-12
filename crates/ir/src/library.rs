//! Device libraries: where the simulator's own implementation of an equation family is made
//! available to the design (AMEND-A12 C5, ruling R17). A reader of a simulator that loads its
//! devices from files (VACASK `load "spice/resistor.osdi"`) pushes one row per load it reads;
//! a neutral pass adds or removes rows by family; a writer spells each row as its target's own
//! load, or nothing. Builder push in, rows out to writers.
//!
//! IR text: `device_library <family> [path="<path>"]`, `<family>` the `ModelFamily` word of
//! `circuit_text.rs`; `path` absent is the target's default file.
//!
//! Cites: `rewrite/AMEND-A12.md` C5; `rewrite/RULINGS.md` R17; DIALECT-VACASK run n2.
//!
//! Contents: `pub struct DeviceLibrary`, `Builder::device_library(&mut self, ScopeId,
//! DeviceLibrary, Span) -> ItemRef`, `RowFacts` for `DeviceLibrary`

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ItemRef, NameId, Ref, ScopeId};
use crate::item::DefKind;
use crate::model::ModelFamily;
use crate::span::Span;
use crate::text::{Ctx, Words};

/// The simulator's implementation of `family` is available from here on. Defines nothing:
/// models of the family name the family, not the library.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeviceLibrary {
    /// `NONE`: the dialect's default file for the family, the one proven over the family's whole
    /// typed parameter set; set: another proven file, kept as the source spelled it (such as
    /// `spice/resistor.osdi`). Spelling in both cases, never a different meaning: no check or
    /// feature reads it, and a pass that drops libraries drops the row whatever its path (R45).
    pub path: NameId,
    /// The equation family the library implements.
    pub family: ModelFamily,
}

const _: () = assert!(size_of::<DeviceLibrary>() == 8);

impl Builder<'_> {
    /// Appends a device library to `scope`.
    pub fn device_library(&mut self, scope: ScopeId, row: DeviceLibrary, span: Span) -> ItemRef {
        self.push(scope, row, span)
    }
}

impl RowFacts for DeviceLibrary {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.path));
    }

    fn map_refs(_design: &mut Design, _at: ItemRef, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // holds no reference
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {
        // `Feature::Item(DeviceLibrary)` is the kernel's; the row states nothing else
    }

    fn check(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _out: &mut Diags) {
        // every family and every path (or none) is in the domain
    }

    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        crate::circuit_text::dump_library(self, design, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        crate::circuit_parse::parse_library(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        _features: &FeatureSet,
    ) -> ItemRef {
        crate::circuit_gen::generate_library(g, scope, b)
    }
}
