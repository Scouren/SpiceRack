//! Pass `drop_builtin_models`: a design in, the same design out with every source, linear
//! controlled source and lossless line that names a model, and every coupling bound to the
//! builtin mutual inductance, naming none, and each builtin model so unbound that nothing else
//! reads dropped from its scope. It removes `CircuitExtra::BuiltinMaster` (ngspice has no model
//! for these; its cards name none) and requires `Circuit(ElementWithoutModel)`, the bare
//! elements it leaves. Exact: `check` lets such an element name only its kind's builtin family,
//! a model with no parameters, the device the bare card already means.
//!
//! A source naming a model no definition binds (the kit is absent) keeps the feature with an
//! `UNBOUND_MASTER` note: nothing says the master is the builtin.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5, A20; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub fn drop_builtin_models(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const UNBOUND_MASTER: ir::Code`

use crate::ir;

/// The note at a source or linear controlled source whose model name binds no definition.
pub const UNBOUND_MASTER: ir::Code = ir::Code::new(b'U', 302);

/// Unbinds the elements that name a builtin master and drops the models left unread.
pub fn drop_builtin_models(design: &mut ir::Design, diags: &mut ir::Diags) {
    let reachable = design.reachable();
    // the elements to unbind, and the models they named
    let mut unbound: Vec<ir::DefId> = Vec::new();
    for &(item, _) in &reachable {
        if item.kind != ir::ItemKind::Element {
            continue;
        }
        let mut e = design.row::<ir::Element>(item.index);
        if e.model.name == ir::NameId::NONE || !builtin_master(design, e) {
            continue;
        }
        if !e.model.is_resolved() {
            diags.push_at(UNBOUND_MASTER, design, item, 0);
            continue;
        }
        unbound.push(e.model.def);
        e.model = ir::Ref::unresolved(ir::NameId::NONE);
        design.editor().set(item.index, e);
    }
    if unbound.is_empty() {
        return;
    }
    // a model goes when nothing reachable still binds it
    let mut read = Vec::new();
    for &(item, _) in &reachable {
        design.item_refs(item, &mut read);
    }
    let drop: Vec<(ir::ItemRef, ir::ScopeId)> = reachable
        .iter()
        .copied()
        .filter(|&(item, _)| {
            item.kind == ir::ItemKind::Model
                && unbound.iter().any(|&d| design.def(d).item == item)
                && !read
                    .iter()
                    .any(|(r, _)| r.is_resolved() && design.def(r.def).item == item)
        })
        .collect();
    let mut editor = design.editor();
    for (item, scope) in drop {
        editor.remove_item(scope, item); // its definition leaves the index with it
    }
}

// ---- private ----

/// Whether `e` (which names a model) is `BuiltinMaster`: a source, linear controlled source or
/// lossless line naming any, or a coupling bound to a `Mutual` model.
fn builtin_master(design: &ir::Design, e: ir::Element) -> bool {
    use ir::ElemKind as K;
    match e.kind {
        K::VSource | K::ISource | K::Vcvs | K::Vccs | K::Cccs | K::Ccvs | K::LosslessLine => true,
        K::Coupling => {
            e.model.is_resolved() && {
                let def = design.def(e.model.def);
                def.kind == ir::DefKind::Model
                    && design.row::<ir::Model>(def.item.index).family == ir::ModelFamily::Mutual
            }
        }
        _ => false,
    }
}
