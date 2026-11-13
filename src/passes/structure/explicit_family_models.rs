//! Pass `explicit_family_models`: a design in, the same design out with every element that can
//! name a model and names none bound to a parameterless model of its kind's family. It removes
//! `CircuitExtra::ElementWithoutModel` (VACASK instantiates every device through a model, run
//! n2). One model per (file or section, family): at the top of the element's file, or in its
//! innermost library section when the design already places models there (a section is read
//! alone; a model elsewhere in a section, a subcircuit or a branch would be a placement,
//! `Feature::Placed`, the design may not have had), first after a leading title, named by the
//! family's IR-text word (`resistor`, `source_v`, …) or that word and `_<n>` when the design
//! already holds the name. A family that needs a library is left without one
//! (`ModelWithoutLibrary`, which `device_libraries` removes: A32, `legalize` runs this pass
//! when that one makes the feature legal).
//!
//! Why the file's top serves every scope of the file: the IR's lookup is lexical (a scope, then
//! its parents, up to the file's top), and a target that looks in the element's own subcircuit
//! and then the top (VACASK's rule) finds it the same way, nested subcircuits included. No
//! scope between can shadow it: its name is one the design holds nowhere, in any case, so no
//! definition of that name stands between the element and the top (A32).
//!
//! Requires `Item(Model)`, `Circuit(ModelWithoutLibrary)`, the families it names, and
//! `Circuit(BuiltinMaster)` (a source or coupling now naming its builtin). A kind that names no
//! master (`ElemKind::master_kind` is `None`: behavioural, table, Laplace, port, …) keeps the
//! feature with a `NO_FAMILY` note; an element in a section the design places no model in, with
//! a `NO_PLACE` note.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5; R17; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub fn explicit_family_models(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const REQUIRES: ir::FeatureSet`, `pub const NO_FAMILY: ir::Code`,
//! `pub const NO_PLACE: ir::Code`

use crate::ir;

/// What the output may add: models, models without a library, the families of [`FAMILIES`], builtin masters.
pub const REQUIRES: ir::FeatureSet = {
    let mut set = ir::FeatureSet::EMPTY
        .with(ir::Feature::Item(ir::ItemKind::Model))
        .with(ir::Feature::Circuit(ir::CircuitExtra::ModelWithoutLibrary))
        .with(ir::Feature::Circuit(ir::CircuitExtra::BuiltinMaster));
    let mut k = 0;
    while k < FAMILIES.len() {
        set = set.with(ir::Feature::Family(FAMILIES[k].1));
        k += 1;
    }
    set
};

/// The note at an element whose kind names no master, so no model can stand for its absence.
pub const NO_FAMILY: ir::Code = ir::Code::new(b'U', 300);

/// The note at an element in a library section the design places no model in: a model there
/// would be a placement the target may lack, and one outside is not read with the section.
pub const NO_PLACE: ir::Code = ir::Code::new(b'U', 301);

/// Each kind that may name a model and need not, and its family.
const FAMILIES: [(ir::ElemKind, ir::ModelFamily); 11] = [
    (ir::ElemKind::Resistor, ir::ModelFamily::Resistor),
    (ir::ElemKind::Capacitor, ir::ModelFamily::Capacitor),
    (ir::ElemKind::Inductor, ir::ModelFamily::Inductor),
    (ir::ElemKind::Coupling, ir::ModelFamily::Mutual),
    (ir::ElemKind::VSource, ir::ModelFamily::SourceV),
    (ir::ElemKind::ISource, ir::ModelFamily::SourceI),
    (ir::ElemKind::Vcvs, ir::ModelFamily::Vcvs),
    (ir::ElemKind::Vccs, ir::ModelFamily::Vccs),
    (ir::ElemKind::Ccvs, ir::ModelFamily::Ccvs),
    (ir::ElemKind::Cccs, ir::ModelFamily::Cccs),
    (ir::ElemKind::LosslessLine, ir::ModelFamily::TlineIdeal),
];

/// Binds every reachable model-less element of a [`FAMILIES`] kind to its scope's model of its
/// family, adding the model where none was added yet; notes the rest.
pub fn explicit_family_models(design: &mut ir::Design, diags: &mut ir::Diags) {
    // the elements to bind: (element, its home scope, its row in FAMILIES)
    let mut bare: Vec<(ir::ItemRef, ir::ScopeId, usize)> = Vec::new();
    let used = ir::features_used(design);
    for (item, scope) in design.reachable() {
        if item.kind != ir::ItemKind::Element {
            continue;
        }
        let e = design.row::<ir::Element>(item.index);
        if e.model.name != ir::NameId::NONE {
            continue;
        }
        match FAMILIES.iter().position(|f| f.0 == e.kind) {
            Some(k) => match home(design, scope, used) {
                Some(at) => bare.push((item, at, k)),
                None => diags.push_at(NO_PLACE, design, item, 0),
            },
            None => {
                assert!(e.kind.master_kind().is_none(), "{:?} has a family", e.kind);
                diags.push_at(NO_FAMILY, design, item, 0);
            }
        }
    }
    if bare.is_empty() {
        return;
    }
    let names: Vec<String> = FAMILIES.iter().map(|f| fresh(design, f.1.word())).collect();
    // one model per (home, family), in first-need order, then each element bound to its own
    let mut models: Vec<(ir::ScopeId, usize, ir::DefId)> = Vec::new();
    for &(item, scope, k) in &bare {
        let found = models
            .iter()
            .find(|m| m.0 == scope && m.1 == k)
            .map(|m| m.2);
        let def = match found {
            Some(def) => def,
            None => {
                let before = models.iter().filter(|m| m.0 == scope).count();
                let def = add_model(design, scope, before, FAMILIES[k].1, &names[k]);
                models.push((scope, k, def));
                def
            }
        };
        let mut e = design.row::<ir::Element>(item.index);
        e.model = ir::Ref {
            name: design.def(def).name,
            def,
        };
        design.editor().set(item.index, e);
    }
}

// ---- private ----

/// The scope a model for an element listed in `scope` goes in: the innermost section on its
/// chain (a section is included alone: the file's top is not read with it), when a model there
/// is placed as the design already places models (`used`, A15 K1); else its file's top, where
/// every target lets a model stand. `None` for a section the design places no model in.
fn home(design: &ir::Design, scope: ir::ScopeId, used: ir::FeatureSet) -> Option<ir::ScopeId> {
    let mut section = ir::ScopeId::NONE;
    let mut placed = true;
    let mut at = scope;
    // bounded by the scope tree's depth: each step moves to a parent, and a file scope stops it
    while !at.is_none() {
        let s = design.scope(at);
        if s.kind == ir::ScopeKind::Section && section.is_none() {
            section = at;
        }
        if !section.is_none() && s.kind != ir::ScopeKind::File {
            placed &= used.contains(ir::Feature::Placed(s.kind, ir::ItemKind::Model));
        }
        at = s.parent;
    }
    match (section.is_none(), placed) {
        (true, _) => Some(design.file(design.scope(scope).file).top),
        (false, true) => Some(section),
        (false, false) => None,
    }
}

/// `word`, or `word_<n>` for the least `n` from 1, such that the design holds no such name and
/// no definition of it in any ASCII case (a SPICE target folds case).
fn fresh(design: &ir::Design, word: &str) -> String {
    let taken = |n: &str| {
        !design.find_name(n).is_none()
            || design
                .definitions_all()
                .iter()
                .any(|d| design.str(d.name).eq_ignore_ascii_case(n))
    };
    if !taken(word) {
        return word.to_string();
    }
    let bound = design.definitions_all().len() + 1;
    (1..=bound)
        .map(|n| format!("{word}_{n}"))
        .find(|n| !taken(n))
        .expect("fewer definitions than candidates")
}

/// Adds a parameterless model of `family` named `name` to `scope`, after a leading title and
/// the `before` models this pass added there, and returns its definition.
fn add_model(
    design: &mut ir::Design,
    scope: ir::ScopeId,
    before: usize,
    family: ir::ModelFamily,
    name: &str,
) -> ir::DefId {
    let title = design.items(scope).first().map(|i| i.kind) == Some(ir::ItemKind::Title);
    let at = usize::from(title) + before;
    let mut editor = design.editor();
    let mut b = editor.builder();
    let row = ir::Model {
        name: b.name(name),
        params: ir::ParamList::EMPTY,
        bin_of: ir::NameId::NONE,
        ako: ir::Ref::unresolved(ir::NameId::NONE),
        module: ir::Ref::unresolved(ir::NameId::NONE),
        version: ir::ExprId::NONE,
        word: ir::NameId::NONE,
        family,
        polarity: ir::Polarity::None,
    };
    let item = b.model(scope, row, ir::Span::NONE);
    let items = editor.items_mut(scope);
    assert!(items.pop() == Some(item), "the builder appends last");
    items.insert(at, item);
    let def = design.lookup(scope, ir::DefKind::Model, row.name);
    assert!(design.def(def).item == item, "the model defines its name");
    def
}
