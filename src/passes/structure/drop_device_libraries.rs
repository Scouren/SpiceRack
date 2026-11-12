//! Pass `drop_device_libraries`: a design in, the same design out with every reachable
//! `DeviceLibrary` dropped from its scope, whatever its path (R45: a library exists only for a
//! proven file, so its path is spelling, never meaning). It removes `Item(DeviceLibrary)` and
//! requires `Circuit(ModelWithoutLibrary)`, what the models of a family with no library left
//! use: a target without libraries has its families built in. Nothing else moves; no note.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5; R17, R45.
//!
//! Contents: `pub fn drop_device_libraries(design: &mut ir::Design, diags: &mut ir::Diags)`

use crate::ir;

/// Drops every reachable device library. Adds no diagnostic.
pub fn drop_device_libraries(design: &mut ir::Design, _diags: &mut ir::Diags) {
    let libraries: Vec<(ir::ItemRef, ir::ScopeId)> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::DeviceLibrary)
        .collect();
    let mut editor = design.editor();
    for (item, scope) in libraries {
        editor.remove_item(scope, item);
    }
}
