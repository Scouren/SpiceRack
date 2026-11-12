//! Pass `device_libraries`: a design in, the same design out with one `DeviceLibrary { family,
//! path absent }` for every family a reachable model uses that needs a library and has no
//! reachable one, at the top of the entry file after its title, in the order the models first
//! appear. It removes `CircuitExtra::ModelWithoutLibrary` (design-level) and requires
//! `Item(DeviceLibrary)`; the family is the model's already (`Family(f)`, A25). Nothing else
//! moves, and it always converts: no note.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5; R17, R44, R45; `crates/ir/src/circuit_tables.rs`
//! (`ModelFamily::library_free`).
//!
//! Contents: `pub fn device_libraries(design: &mut ir::Design, diags: &mut ir::Diags)`

use crate::ir;

/// Adds the libraries the reachable models' families lack. Adds no diagnostic.
pub fn device_libraries(design: &mut ir::Design, _diags: &mut ir::Diags) {
    let families: Vec<ir::ModelFamily> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Model)
        .map(|(i, _)| design.row::<ir::Model>(i.index).family)
        .collect();
    add_libraries(design, &families);
}

// ---- private ----

/// Adds a path-less `DeviceLibrary` for each of `families` (first appearance order) that is not
/// library-free and has no reachable library yet, after the entry file's leading title.
fn add_libraries(design: &mut ir::Design, families: &[ir::ModelFamily]) {
    if design.entry().is_none() {
        assert!(families.is_empty(), "a model lives in a file");
        return;
    }
    let mut has = [false; ir::ModelFamily::COUNT];
    for (item, _) in design.reachable() {
        if item.kind == ir::ItemKind::DeviceLibrary {
            has[design.row::<ir::DeviceLibrary>(item.index).family as usize] = true;
        }
    }
    let top = design.file(design.entry()).top;
    let at = usize::from(design.items(top).first().map(|i| i.kind) == Some(ir::ItemKind::Title));
    let mut added = 0;
    let mut editor = design.editor();
    for &family in families {
        if family.library_free() || std::mem::replace(&mut has[family as usize], true) {
            continue;
        }
        let row = ir::DeviceLibrary {
            path: ir::NameId::NONE,
            family,
        };
        let item = editor.builder().device_library(top, row, ir::Span::NONE);
        let items = editor.items_mut(top);
        assert!(items.pop() == Some(item), "the builder appends last");
        items.insert(at + added, item);
        added += 1;
    }
    assert!(added <= ir::ModelFamily::COUNT, "one library per family");
}
