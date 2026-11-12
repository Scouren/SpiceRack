//! The writer's spine: one file of a legal design → VACASK text. The title first (line 1 of an
//! entry deck: VACASK reads line 1 as the title whatever it holds, so a deck with none gets an
//! empty line), then the file's items in order, each body below its item and indented two
//! spaces per level (explicit stack, C11), the run items of the top scope in one `control …
//! endc` block at the end (S4.3). One exhaustive `match` over `ir::ItemKind`, no `_` arm: each
//! arm is an area function. An `Opaque` is copied when its dialect is `vacask` (the spine's
//! `legalize` refused every other).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; REWRITE-PLAN §4.3 (one exhaustive match).
//!
//! Contents: `Writer` (and its methods), `write` (crate-private)

use super::{write_elements, write_models, write_run, write_structure};
use crate::ir;

/// The text being written and where it is.
pub(super) struct Writer<'d, 'o> {
    /// The legal design.
    pub(super) design: &'d ir::Design,
    /// The text so far.
    pub(super) out: &'o mut String,
    /// The current depth: two spaces of indent each.
    pub(super) depth: usize,
}

impl Writer<'_, '_> {
    /// Starts a line at the current depth.
    pub(super) fn line(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }
}

/// Appends file `file` of `design` (legal for `FEATURES`) to `out`.
pub(super) fn write(
    design: &ir::Design,
    file: ir::FileId,
    out: &mut String,
    diags: &mut ir::Diags,
) {
    let _ = (design, file, out, diags);
    let _ = (
        write_structure::title,
        write_elements::element,
        write_models::model,
        write_run::analysis,
    );
    todo!("S4.0: write::write")
}
