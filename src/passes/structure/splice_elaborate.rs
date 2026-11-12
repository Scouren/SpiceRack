//! Pass `splice_elaborate`: a design in, the same design out with a deck's one `Elaborate`,
//! standing before every run item, replaced by its circuits: each named subcircuit's body items
//! moved into the deck's top scope where the subcircuit stood (their definitions with them),
//! its formals made top-level parameters of their defaults, the subcircuit item gone. It
//! removes `Item(Elaborate)` (ngspice's top circuit is its deck's top scope; VACASK's is what
//! `elaborate` names, A12 R1) and requires `Item(Param)` (the formals).
//!
//! Left with a note (`ELABORATE_KEPT`): a second `Elaborate` (ngspice cannot swap its top
//! circuit), one after a run item, a top scope that holds elements of its own, a named
//! subcircuit some element instantiates or with a formal without a default, and a name two
//! spliced bodies (or a body and the top) both define.
//!
//! Cites: `rewrite/AMEND-A12.md` R1; `rewrite/DIALECT-VACASK.md` §6 (VS V1, V2).
//!
//! Contents: `pub fn splice_elaborate(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const ELABORATE_KEPT: ir::Code`

use crate::ir;

/// The note at an `Elaborate` the pass cannot splice exactly.
pub const ELABORATE_KEPT: ir::Code = ir::Code::new(b'U', 309);

/// Splices a deck's one leading `Elaborate` into its top scope.
pub fn splice_elaborate(design: &mut ir::Design, diags: &mut ir::Diags) {
    let reachable = design.reachable();
    let elaborates: Vec<(ir::ItemRef, ir::ScopeId)> = reachable
        .iter()
        .copied()
        .filter(|(i, _)| i.kind == ir::ItemKind::Elaborate)
        .collect();
    let [(e, top)] = elaborates[..] else {
        for &(e, _) in &elaborates {
            diags.push_at(ELABORATE_KEPT, design, e, 0);
        }
        return;
    };
    match spliceable(design, e, top) {
        Some(circuits) => splice(design, e, top, &circuits),
        None => diags.push_at(ELABORATE_KEPT, design, e, 0),
    }
}

// ---- private ----

/// The subcircuits of `e`, in its order, when it can be spliced exactly: listed in a deck's
/// top before every analysis, loop or alter there, a top with no element of its own, each
/// circuit a subcircuit of that top no element instantiates and whose formals all have
/// defaults, and no name two bodies (or a body and the top) define or list.
fn spliceable(design: &ir::Design, e: ir::ItemRef, top: ir::ScopeId) -> Option<Vec<ir::ItemRef>> {
    let file = design.scope(top).file;
    if design.scope(top).kind != ir::ScopeKind::File || design.file(file).role != ir::Role::Deck {
        return None;
    }
    let items = design.items(top);
    let at = items.iter().position(|&i| i == e)?;
    let run = [
        ir::ItemKind::Analysis,
        ir::ItemKind::Loop,
        ir::ItemKind::Alter,
    ];
    if items[..at].iter().any(|i| run.contains(&i.kind)) {
        return None;
    }
    if items.iter().any(|i| i.kind == ir::ItemKind::Element) {
        return None;
    }
    let row = design.row::<ir::plan::Elaborate>(e.index);
    let mut circuits = Vec::new();
    for r in design.refs(row.circuits) {
        if !r.is_resolved() {
            return None;
        }
        let def = design.def(r.def);
        if def.scope != top || circuits.contains(&def.item) {
            return None;
        }
        circuits.push(def.item);
    }
    let reachable = design.reachable();
    let instanced = reachable.iter().any(|&(i, _)| {
        i.kind == ir::ItemKind::Element && {
            let m = design.row::<ir::Element>(i.index).model;
            m.is_resolved() && circuits.contains(&design.def(m.def).item)
        }
    });
    if instanced {
        return None;
    }
    // the names the spliced top would hold: definitions by (kind, name), elements by name
    let mut defs: Vec<(ir::DefKind, String)> = Vec::new();
    let mut elements: Vec<String> = Vec::new();
    let mut scopes = vec![top];
    for &c in &circuits {
        let s = design.row::<ir::item::Subckt>(c.index);
        if design.params(s.params).iter().any(|p| p.1.is_none()) {
            return None;
        }
        scopes.push(s.body);
    }
    for d in design.definitions() {
        let spliced = circuits.contains(&d.item) && d.kind == ir::DefKind::Subckt;
        if scopes.contains(&d.scope) && !spliced {
            let key = (d.kind, design.str(d.name).to_ascii_lowercase());
            if defs.contains(&key) {
                return None;
            }
            defs.push(key);
        }
    }
    for &s in &scopes {
        for &i in design.items(s) {
            if i.kind == ir::ItemKind::Element {
                let n = design
                    .str(design.row::<ir::Element>(i.index).name)
                    .to_ascii_lowercase();
                if elements.contains(&n) {
                    return None;
                }
                elements.push(n);
            }
        }
    }
    Some(circuits)
}

/// Each circuit's formals become top parameters and its body items move into the top where it
/// stood; the circuits and `e` go.
fn splice(design: &mut ir::Design, e: ir::ItemRef, top: ir::ScopeId, circuits: &[ir::ItemRef]) {
    let mut editor = design.editor();
    editor.remove_item(top, e);
    for &c in circuits {
        let s = editor.design().row::<ir::item::Subckt>(c.index);
        // the formals: a parameter item each, defining what the formal defined
        let formals: Vec<(ir::NameId, ir::ExprId)> = editor.design().params(s.params).to_vec();
        let mut moved: Vec<ir::ItemRef> = Vec::new();
        for (name, value) in formals {
            let def = editor.design().lookup(s.body, ir::DefKind::Param, name);
            assert!(editor.design().def(def).item == c, "a head formal");
            let p = editor
                .builder()
                .push(s.body, ir::item::Param { name, value }, ir::Span::NONE);
            editor.redefine(def, p);
            moved.push(p);
        }
        let body: Vec<ir::ItemRef> = editor.design().items(s.body).to_vec();
        let rest: Vec<ir::ItemRef> = body.into_iter().filter(|i| !moved.contains(i)).collect();
        moved.extend(rest);
        let at = editor
            .design()
            .items(top)
            .iter()
            .position(|&i| i == c)
            .expect("listed");
        for (k, item) in moved.into_iter().enumerate() {
            editor.move_item(s.body, item, top, at + k);
        }
        editor.remove_item(top, c);
    }
}
