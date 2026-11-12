//! Passes `drop_unread_source_v` and `drop_unread_source_i`: a design in, the same design out
//! without every model of the builtin voltage (`SourceV`) or current (`SourceI`) source family
//! that nothing reachable reads, and without every device library of that family. They remove
//! `Family(SourceV)` and `Family(SourceI)` (ngspice has no source model: its cards name none)
//! and require nothing. Exact: a model nothing reads moves no number, a builtin source model
//! has no parameters (`ir::check`), and a builtin family needs no library on any target (the
//! IR's `library_free`), so its library moves no number either.
//!
//! VACASK decks declare `model vsource vsource` and `model isource isource` whether or not an
//! element uses them (21 decks of its corpus; `test/test_inductor.sim` declares both and
//! instantiates only the current source). `drop_builtin_models` runs first in the group and
//! unbinds every source that names one, so what is left is read by nothing else in practice; a
//! model still read keeps the feature with a `SOURCE_MODEL_READ` note.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5.
//!
//! Contents: `pub fn drop_unread_source_v(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub fn drop_unread_source_i(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const SOURCE_MODEL_READ: ir::Code`

use crate::ir;

/// The note at a builtin source model something still reads.
pub const SOURCE_MODEL_READ: ir::Code = ir::Code::new(b'U', 313);

/// Drops every voltage-source model nothing reads.
pub fn drop_unread_source_v(design: &mut ir::Design, diags: &mut ir::Diags) {
    drop_unread(design, diags, ir::ModelFamily::SourceV);
}

/// Drops every current-source model nothing reads.
pub fn drop_unread_source_i(design: &mut ir::Design, diags: &mut ir::Diags) {
    drop_unread(design, diags, ir::ModelFamily::SourceI);
}

// ---- private ----

/// Drops the reachable models of `family` that no reachable reference binds and the family's
/// device libraries; notes the models left.
fn drop_unread(design: &mut ir::Design, diags: &mut ir::Diags, family: ir::ModelFamily) {
    assert!(matches!(
        family,
        ir::ModelFamily::SourceV | ir::ModelFamily::SourceI
    ));
    let reachable = design.reachable();
    let models: Vec<(ir::ItemRef, ir::ScopeId)> = reachable
        .iter()
        .copied()
        .filter(|&(item, _)| match item.kind {
            ir::ItemKind::Model => design.row::<ir::Model>(item.index).family == family,
            ir::ItemKind::DeviceLibrary => {
                design.row::<ir::DeviceLibrary>(item.index).family == family
            }
            _ => false,
        })
        .collect();
    if models.is_empty() {
        return;
    }
    let mut read = Vec::new();
    for &(item, _) in &reachable {
        design.item_refs(item, &mut read);
    }
    let is_read = |item: ir::ItemRef| {
        item.kind == ir::ItemKind::Model
            && read
                .iter()
                .any(|(r, _)| r.is_resolved() && design.def(r.def).item == item)
    };
    let (kept, dropped): (Vec<_>, Vec<_>) = models.into_iter().partition(|&(m, _)| is_read(m));
    for &(m, _) in &kept {
        diags.push_at(SOURCE_MODEL_READ, design, m, 0);
    }
    {
        let mut editor = design.editor();
        for &(item, scope) in &dropped {
            editor.remove_item(scope, item); // its definition leaves the index with it
        }
    }
    debug_assert!(
        dropped.iter().all(|d| !design.reachable().contains(d)),
        "a dropped model is still listed"
    );
}
