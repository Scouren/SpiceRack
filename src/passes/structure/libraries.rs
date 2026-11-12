//! Device libraries for the structure group: a design and the families its models use in, one
//! `DeviceLibrary { family, path absent }` per family that needs one and has none out, at the
//! top of the entry file (after its title). Shared by `device_libraries` (every model's family)
//! and `explicit_family_models` (the families of the models it adds), so neither leaves
//! `ModelWithoutLibrary` behind.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5; R17, R44; `crates/ir/src/circuit_tables.rs`
//! (`library_free`, which this file's table must equal: `tests/passes_structure.rs` checks it
//! against the IR's own `ModelWithoutLibrary`).
//!
//! Contents: `pub(super) fn library_free(ir::ModelFamily) -> bool`,
//! `pub(super) fn add_libraries(&mut ir::Design, &[ir::ModelFamily])`

use crate::ir;

/// Whether a model of `f` needs no `DeviceLibrary` on any target: the builtins of A12 C5 but
/// the ideal line, and the families whose code is a `Load` or unbound (R44).
// ponytail: a copy of the IR's crate-private `circuit_tables::library_free`; the test pins it
// to the IR's feature, and it goes when the IR exports it.
pub(super) fn library_free(f: ir::ModelFamily) -> bool {
    use ir::ModelFamily as F;
    matches!(
        f,
        F::Module
            | F::CodeModel
            | F::Other
            | F::SourceV
            | F::SourceI
            | F::Vcvs
            | F::Vccs
            | F::Ccvs
            | F::Cccs
            | F::Mutual
    )
}

/// Adds a path-less `DeviceLibrary` for each of `families` (first appearance order) that is not
/// library-free and has no reachable library yet, after the entry file's leading title.
pub(super) fn add_libraries(design: &mut ir::Design, families: &[ir::ModelFamily]) {
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
        if library_free(family) || std::mem::replace(&mut has[family as usize], true) {
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
