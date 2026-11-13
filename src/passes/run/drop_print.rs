//! Rule `drop_print`: a `.print` card dropped, one note naming it (`PRINT_DROPPED`). It removes
//! `Item(Print)` and requires nothing (R83).
//!
//! Exact for what a run writes (R49: an ngspice deck means `-b -r`): ngspice-45 writes the same
//! raw file with and without `.print` cards, in every combination run
//! (`tests/passes_run/decks/options_probe.sp`): one `.print tran v(out)`; three cards (`tran
//! v(out) v(mid)`, `ac vdb(out)`, `dc i(vdd)`); and `.save v(out)` with `.print tran v(out)`
//! against the `.save` alone (2, 2, 1, 2 vectors in ac, dc, op, tran either way). A print
//! writes the log, never the saved set: it is no save in disguise.
//! `tests/passes_run/options.rs` reruns the four.
//!
//! A print in a loop's body or a branch is left with `PRINT_KEPT`: dropping it changes the
//! body's shape (a loop of one op, `LoopOverOp`), a feature of its own.
//!
//! Cites: R49, R67, R83.
//!
//! Contents: `pub fn drop_print(&mut ir::Design, &mut ir::Diags)`,
//! `pub const PRINT_DROPPED: ir::Code`, `pub const PRINT_KEPT: ir::Code`

use crate::ir;

/// The note at a dropped print's place.
pub const PRINT_DROPPED: ir::Code = ir::Code::new(b'U', 422);
/// The note at a print in a body or branch, left.
pub const PRINT_KEPT: ir::Code = ir::Code::new(b'U', 423);

/// Drops every reachable print listed in a file's top scope; notes the rest.
pub fn drop_print(design: &mut ir::Design, diags: &mut ir::Diags) {
    let prints: Vec<(ir::ItemRef, ir::ScopeId)> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Print)
        .collect();
    let mut drop = Vec::with_capacity(prints.len());
    for (item, scope) in prints {
        if design.scope(scope).kind == ir::ScopeKind::File {
            diags.push_at(PRINT_DROPPED, design, item, 0);
            drop.push((item, scope));
        } else {
            diags.push_at(PRINT_KEPT, design, item, 0);
        }
    }
    let mut editor = design.editor();
    for (item, scope) in drop {
        editor.remove_item(scope, item);
    }
}
