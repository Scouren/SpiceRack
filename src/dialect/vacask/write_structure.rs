//! The structure rows → VACASK statements: `Title` (line 1), `Comment` (`//text`, or `/*text*/`
//! for a text with a newline), `Include` (`include "path" [section=name]`), `Section`,
//! `Load` (`load "path"`), `Subckt` (`subckt name (ports)`, its head as one `parameters` line,
//! then its body, `ends`), `Param` (`parameters name=value`), `Global`, `If` (`@if c`,
//! `@elseif c`, `@else`, `@end`), and an `Opaque` of VACASK's text family copied as written.
//! Each function writes its statement's first line and leaves the body to the spine.
//!
//! Cites: VD input-include, cir-subckt, cir-nodes, cir-conditional, cir-loading.
//!
//! Contents: `CODES`, `title`, `comment`, `include`, `section`, `section_end`, `load`,
//! `subckt`, `subckt_end`, `param`, `global`, `branch`, `if_end`, `opaque` (crate-private)

use super::write::Writer;
use crate::ir;

/// This area's rows (none yet).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[];

/// Line 1 of an entry deck: the title's text, or nothing.
pub(super) fn title(w: &mut Writer<'_, '_>, row: Option<ir::item::Title>) {
    let _ = (w, row);
    todo!("S4.0: write_structure::title")
}

/// A comment line.
pub(super) fn comment(w: &mut Writer<'_, '_>, row: ir::item::Comment) {
    let _ = (w, row);
    todo!("S4.0: write_structure::comment")
}

/// An `include` line.
pub(super) fn include(w: &mut Writer<'_, '_>, row: ir::item::Include) {
    let _ = (w, row);
    todo!("S4.0: write_structure::include")
}

/// A `section` line (its body follows).
pub(super) fn section(w: &mut Writer<'_, '_>, row: ir::item::Section) {
    let _ = (w, row);
    todo!("S4.0: write_structure::section")
}

/// `endsection`.
pub(super) fn section_end(w: &mut Writer<'_, '_>) {
    let _ = w;
    todo!("S4.0: write_structure::section_end")
}

/// A `load` line.
pub(super) fn load(w: &mut Writer<'_, '_>, row: ir::item::Load) {
    let _ = (w, row);
    todo!("S4.0: write_structure::load")
}

/// The `subckt` line and its head's `parameters` line (its body follows).
pub(super) fn subckt(w: &mut Writer<'_, '_>, row: ir::item::Subckt) {
    let _ = (w, row);
    todo!("S4.0: write_structure::subckt")
}

/// `ends`.
pub(super) fn subckt_end(w: &mut Writer<'_, '_>) {
    let _ = w;
    todo!("S4.0: write_structure::subckt_end")
}

/// A `parameters` line of one parameter.
pub(super) fn param(w: &mut Writer<'_, '_>, row: ir::item::Param) {
    let _ = (w, row);
    todo!("S4.0: write_structure::param")
}

/// A `global` line.
pub(super) fn global(w: &mut Writer<'_, '_>, row: ir::item::Global) {
    let _ = (w, row);
    todo!("S4.0: write_structure::global")
}

/// Branch `k` of an `If`: `@if c` (k = 0), `@elseif c`, or `@else` (condition `NONE`).
pub(super) fn branch(w: &mut Writer<'_, '_>, k: usize, branch: ir::item::Branch) {
    let _ = (w, k, branch);
    todo!("S4.0: write_structure::branch")
}

/// `@end`.
pub(super) fn if_end(w: &mut Writer<'_, '_>) {
    let _ = w;
    todo!("S4.0: write_structure::if_end")
}

/// An `Opaque` of VACASK's family, as written.
pub(super) fn opaque(w: &mut Writer<'_, '_>, row: ir::item::Opaque) {
    let _ = (w, row);
    todo!("S4.0: write_structure::opaque")
}
