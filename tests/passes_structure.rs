//! The meaning of the structure passes (`src/passes/structure/`), on the IR alone, until the
//! trunk's pair decks prove them on the simulators (`PENDING_PAIRS`).
//!
//! Oracles, never the passes' own arithmetic: the IR's own doors over generated designs
//! (`ir::r#gen::generate`): every reference an item held before the pass binds the same
//! definition after it (`Design::item_refs`); an element bound to a model keeps it; an element
//! a pass binds names a parameterless model and `ir::check` accepts the binding (it refuses a
//! source naming anything but its builtin); the IR's `ModelWithoutLibrary` decides which
//! families need a library (`features_used`), and `legalize` for a target without the features
//! leaves nothing to refuse.
//!
//! Contents: tests only.

use std::path::Path;

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes;

/// The structure passes whose simulator proof (a pair deck naming it in `proves =`,
/// `rewrite/SPINE.md` §8a) is still to come: compared by name with the decks, both ways.
const PENDING_PAIRS: &[&str] = &[
    "deck_title",
    "explicit_family_models",
    "device_libraries",
    "drop_builtin_models",
    "drop_device_libraries",
    "hoist_nested_subckts",
    "hoist_enclosing_refs",
    "formal_defaults_to_instances",
];

const SEEDS: std::ops::Range<u64> = 0..96;

const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

// ---- private ----

#[test]
fn explicit_family_models_keeps_every_reference_and_binds_only_the_bare() {
    let without = feature(ir::CircuitExtra::ElementWithoutModel);
    let mut ran = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(without) {
            continue;
        }
        ran += 1;
        let before = refs(&design);
        let bare: Vec<ir::ItemRef> = elements(&design)
            .filter(|&i| design.row::<ir::Element>(i.index).model.name == ir::NameId::NONE)
            .collect();
        let had_library_gap =
            ir::features_used(&design).contains(feature(ir::CircuitExtra::ModelWithoutLibrary));
        let mut diags = ir::Diags::new();
        passes::structure::explicit_family_models::explicit_family_models(&mut design, &mut diags);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "seed {seed}: {:?}", found.rows());
        for (item, refs_before) in &before {
            let mut after = Vec::new();
            design.item_refs(*item, &mut after);
            if !bare.contains(item) {
                assert_eq!(&after, refs_before, "seed {seed}: {item:?} rebound");
                continue;
            }
            let e = design.row::<ir::Element>(item.index);
            if e.model.name == ir::NameId::NONE {
                let noted = diags.rows().iter().any(|d| d.item == *item);
                assert!(noted, "seed {seed}: {item:?} left bare with no note");
                continue;
            }
            let def = design.def(e.model.def);
            assert_eq!(def.kind, ir::DefKind::Model, "seed {seed}");
            let m = design.row::<ir::Model>(def.item.index);
            assert!(m.params.is_empty(), "seed {seed}: a parameterless model");
            assert_eq!(m.polarity, ir::Polarity::None, "seed {seed}");
            assert_eq!(
                design.lookup(def.scope, ir::DefKind::Model, e.model.name),
                e.model.def,
                "seed {seed}: the binding is the lexical one"
            );
        }
        let gap =
            ir::features_used(&design).contains(feature(ir::CircuitExtra::ModelWithoutLibrary));
        assert!(
            !gap || had_library_gap,
            "seed {seed}: it left a model without a library"
        );
    }
    assert!(ran > 0, "no generated design had a model-less element");
    eprintln!("explicit_family_models: {ran} generated designs");
}

#[test]
fn device_libraries_keeps_every_item_and_reference() {
    let gap = feature(ir::CircuitExtra::ModelWithoutLibrary);
    let mut ran = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(gap) {
            continue;
        }
        ran += 1;
        let before = refs(&design);
        let mut diags = ir::Diags::new();
        passes::structure::device_libraries::device_libraries(&mut design, &mut diags);
        assert!(diags.is_empty(), "seed {seed}: no note");
        assert!(!ir::features_used(&design).contains(gap), "seed {seed}");
        let after: Vec<(ir::ItemRef, Vec<(ir::Ref, ir::DefKind)>)> = refs(&design)
            .into_iter()
            .filter(|(i, _)| !added_library(&before, *i))
            .collect();
        assert_eq!(after, before, "seed {seed}: an item or reference moved");
    }
    assert!(ran > 0, "no generated design had a model without a library");
    eprintln!("device_libraries: {ran} generated designs");
}

/// The group's copy of the IR's library-free table, pinned to the IR's own feature: for one
/// model of each family, the pass adds a library exactly when the IR says one is missing.
#[test]
fn device_libraries_adds_a_library_exactly_for_the_families_the_ir_says_lack_one() {
    for family in ir::ModelFamily::ALL {
        let mut design = ir::Design::new();
        {
            let mut b = design.builder();
            let (_, top) = b.file("a.sp", ir::Role::Deck);
            let name = b.name("m");
            let other = b.name("w");
            let open = matches!(family, ir::ModelFamily::CodeModel | ir::ModelFamily::Other);
            let module = match family {
                ir::ModelFamily::Module => ir::Ref::unresolved(other),
                _ => ir::Ref::unresolved(ir::NameId::NONE),
            };
            let polarity = [ir::Polarity::None, ir::Polarity::N]
                .into_iter()
                .find(|&p| {
                    let row = model_row(name, family, p, module, open.then_some(other));
                    let mut d = ir::Design::new();
                    let mut bb = d.builder();
                    let (_, t) = bb.file("p.sp", ir::Role::Deck);
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        bb.model(t, row, ir::Span::NONE)
                    }))
                    .is_ok()
                })
                .expect("one polarity is in the family's domain");
            let row = model_row(name, family, polarity, module, open.then_some(other));
            b.model(top, row, ir::Span::NONE);
        }
        let gap = feature(ir::CircuitExtra::ModelWithoutLibrary);
        let missing = ir::features_used(&design).contains(gap);
        let mut diags = ir::Diags::new();
        passes::structure::device_libraries::device_libraries(&mut design, &mut diags);
        let top = design.file(design.entry()).top;
        let libraries = design
            .items(top)
            .iter()
            .filter(|i| i.kind == ir::ItemKind::DeviceLibrary)
            .count();
        assert_eq!(libraries, usize::from(missing), "{family:?}");
        assert!(!ir::features_used(&design).contains(gap), "{family:?}");
    }
}

#[test]
fn drop_builtin_models_unbinds_only_parameterless_builtins_and_keeps_every_other_reference() {
    let mut ran = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(feature(ir::CircuitExtra::BuiltinMaster)) {
            continue;
        }
        ran += 1;
        let before = refs(&design);
        let mut diags = ir::Diags::new();
        passes::structure::drop_builtin_models::drop_builtin_models(&mut design, &mut diags);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "seed {seed}: {:?}", found.rows());
        let after = refs(&design);
        for (item, refs_before) in &before {
            let Some((_, refs_after)) = after.iter().find(|(i, _)| i == item) else {
                // a dropped item: a parameterless builtin model
                assert_eq!(
                    item.kind,
                    ir::ItemKind::Model,
                    "seed {seed}: {item:?} dropped"
                );
                let m = design.row::<ir::Model>(item.index);
                assert!(m.params.is_empty(), "seed {seed}: a parameterless model");
                continue;
            };
            if refs_after == refs_before {
                continue;
            }
            // an unbound element: its old master was a parameterless model, now it names none
            assert_eq!(
                item.kind,
                ir::ItemKind::Element,
                "seed {seed}: {item:?} rebound"
            );
            let e = design.row::<ir::Element>(item.index);
            assert_eq!(e.model.name, ir::NameId::NONE, "seed {seed}");
            let old = refs_before[0].0;
            let m = design.row::<ir::Model>(design.def(old.def).item.index);
            assert!(
                m.params.is_empty(),
                "seed {seed}: a builtin has no parameters"
            );
        }
    }
    assert!(ran > 0, "no generated design had a builtin master");
    eprintln!("drop_builtin_models: {ran} generated designs");
}

/// A metamorphic pair: dropping the builtin masters after adding them gives each source,
/// coupling and line back its bare form.
#[test]
fn drop_builtin_models_undoes_explicit_family_models_on_sources() {
    let text = "file \"a.sp\" deck\n  element vsource v1 (a 0) dc=1\n  element inductor l1 (a 0) value=1u\n  element inductor l2 (b 0) value=1u\n  element coupling k1 () inductors=(l1 l2) k=0.5\n  element vcvs e1 (o 0 a 0) gain=2\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    passes::structure::explicit_family_models::explicit_family_models(&mut design, &mut diags);
    passes::structure::drop_builtin_models::drop_builtin_models(&mut design, &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let mut out = String::new();
    ir::dump(&design, design.entry(), &mut out);
    let expected = "file \"a.sp\" deck\n  device_library inductor\n  model inductor inductor\n  element vsource v1 (a 0) dc=1\n  element inductor l1 (a 0) model=inductor value=1u\n  element inductor l2 (b 0) model=inductor value=1u\n  element coupling k1 () inductors=(l1 l2) k=0.5\n  element vcvs e1 (o 0 a 0) gain=2\n";
    assert_eq!(out, expected);
}

#[test]
fn drop_device_libraries_drops_only_libraries() {
    let mut ran = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        let library = ir::Feature::Item(ir::ItemKind::DeviceLibrary);
        if !ir::features_used(&design).contains(library) {
            continue;
        }
        ran += 1;
        let before: Vec<_> = refs(&design)
            .into_iter()
            .filter(|(i, _)| i.kind != ir::ItemKind::DeviceLibrary)
            .collect();
        let mut diags = ir::Diags::new();
        passes::structure::drop_device_libraries::drop_device_libraries(&mut design, &mut diags);
        assert!(diags.is_empty(), "seed {seed}: no note");
        assert!(!ir::features_used(&design).contains(library), "seed {seed}");
        assert_eq!(refs(&design), before, "seed {seed}: only libraries go");
    }
    assert!(ran > 0, "no generated design had a library");
    eprintln!("drop_device_libraries: {ran} generated designs");
}

/// The hoist keeps what every read binds: a row reference binds the same definition after it,
/// and a parameter read either binds the same definition or a formal of the same name in the
/// head of the subcircuit that now holds the read, which every instance of that subcircuit
/// passes as a read of the old definition.
#[test]
fn hoist_nested_subckts_keeps_what_every_read_binds() {
    let nested = ir::Feature::Placed(ir::ScopeKind::Subckt, ir::ItemKind::Subckt);
    let mut ran = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(nested) {
            continue;
        }
        ran += 1;
        let refs_before = refs(&design);
        let reads_before = param_reads(&design);
        let mut diags = ir::Diags::new();
        passes::structure::hoist_nested_subckts::hoist_nested_subckts(&mut design, &mut diags);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "seed {seed}: {:?}", found.rows());
        let refs_after = refs(&design);
        for (item, before) in &refs_before {
            let after = refs_after.iter().find(|(i, _)| i == item).map(|(_, r)| r);
            assert_eq!(
                after,
                Some(before),
                "seed {seed}: {item:?} rebound or dropped"
            );
        }
        let reads_after = param_reads(&design);
        for ((item, old), new) in reads_before.iter().zip(&reads_after) {
            assert_eq!(item, &new.0, "seed {seed}: the reads keep their order");
            if old == &new.1 {
                continue;
            }
            let (o, n) = (design.def(*old), design.def(new.1));
            assert_eq!(o.name, n.name, "seed {seed}: a formal of the same name");
            assert_eq!(
                n.item.kind,
                ir::ItemKind::Subckt,
                "seed {seed}: a head formal"
            );
            assert!(
                instances_pass(&design, n.item, n.name, *old),
                "seed {seed}: every instance passes the old definition"
            );
        }
        let still = ir::features_used(&design).contains(nested);
        assert!(
            !still || !diags.is_empty(),
            "seed {seed}: left nested with no note"
        );
    }
    assert!(ran > 0, "no generated design had a nested subcircuit");
    eprintln!("hoist_nested_subckts: {ran} generated designs");
}

#[test]
fn legalize_gives_every_model_less_element_a_model_and_a_library() {
    let text = "file \"a.sp\" deck\n  element resistor r1 (a 0) value=1k\n  element vsource v1 (a 0) dc=1\n  element lossless_line t1 (a 0 b 0) z0=50 td=1n\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let lacks = ir::FeatureSet::EMPTY
        .with(feature(ir::CircuitExtra::ElementWithoutModel))
        .with(feature(ir::CircuitExtra::ModelWithoutLibrary));
    passes::legalize(&mut design, &ir::FeatureSet::ALL.minus(lacks), &mut diags);
    assert!(diags.is_empty(), "nothing to refuse: {:?}", diags.rows());
    assert!(
        ir::features_used(&design)
            .minus(ir::FeatureSet::ALL.minus(lacks))
            .is_empty()
    );
}

#[test]
fn every_structure_pass_has_its_pair_deck_or_a_pending_marker_and_not_both() {
    let proven = proven_by_pair_decks();
    let group = passes::GROUPS
        .iter()
        .find(|g| g.name == "structure")
        .expect("the group");
    let mut wrong = Vec::new();
    for pass in group.passes {
        let pending = PENDING_PAIRS.contains(&pass.name);
        if pending == proven.iter().any(|p| p == pass.name) {
            wrong.push(pass.name);
        }
    }
    for &p in PENDING_PAIRS {
        if !group.passes.iter().any(|q| q.name == p) {
            wrong.push(p);
        }
    }
    assert!(
        wrong.is_empty(),
        "pair deck against PENDING_PAIRS: {wrong:?}"
    );
}

fn feature(x: ir::CircuitExtra) -> ir::Feature {
    ir::Feature::Circuit(x)
}

/// Every reachable item with the references it holds.
fn refs(design: &ir::Design) -> Vec<(ir::ItemRef, Vec<(ir::Ref, ir::DefKind)>)> {
    let reachable = design.reachable().into_iter();
    reachable
        .map(|(item, _)| {
            let mut out = Vec::new();
            design.item_refs(item, &mut out);
            (item, out)
        })
        .collect()
}

/// Every bound parameter read of every reachable item's expressions, in item and tree order:
/// (item, definition).
fn param_reads(design: &ir::Design) -> Vec<(ir::ItemRef, ir::DefId)> {
    let mut out = Vec::new();
    let mut handles = Vec::new();
    for (item, _) in design.reachable() {
        handles.clear();
        design.item_handles(item, &mut handles);
        let mut stack: Vec<ir::ExprId> = Vec::new();
        for h in &handles {
            match *h {
                ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => stack.push(e),
                ir::Handle::Exprs(l) => stack.extend_from_slice(design.exprs(l)),
                ir::Handle::Params(l) => stack.extend(design.params(l).iter().map(|p| p.1)),
                _ => {}
            }
        }
        stack.retain(|e| !e.is_none());
        stack.reverse();
        // bounded: a tree's rows lie below its root, each visited once per path
        while let Some(e) = stack.pop() {
            match design.expr(e) {
                ir::Expr::Param(r) if r.is_resolved() => out.push((item, r.def)),
                ir::Expr::Call(_, l)
                | ir::Expr::UserCall(_, l)
                | ir::Expr::Draw(_, _, _, l)
                | ir::Expr::List(l) => stack.extend(design.exprs(l).iter().rev()),
                ir::Expr::Unary(_, a) => stack.push(a),
                ir::Expr::Binary(_, a, b) => stack.extend([b, a]),
                ir::Expr::Ternary(c, a, b) => stack.extend([b, a, c]),
                _ => {}
            }
        }
    }
    out
}

/// Whether every reachable instance of the subcircuit `subckt` passes `name` as a read of `old`.
fn instances_pass(
    design: &ir::Design,
    subckt: ir::ItemRef,
    name: ir::NameId,
    old: ir::DefId,
) -> bool {
    elements(design).all(|i| {
        let e = design.row::<ir::Element>(i.index);
        let of = e.kind == ir::ElemKind::Instance
            && e.model.is_resolved()
            && design.def(e.model.def).item == subckt;
        !of || design.params(e.params).iter().any(|&(n, v)| {
            n == name && matches!(design.expr(v), ir::Expr::Param(r) if r.def == old)
        })
    })
}

fn elements(design: &ir::Design) -> impl Iterator<Item = ir::ItemRef> + '_ {
    let reachable = design.reachable().into_iter();
    reachable
        .map(|(i, _)| i)
        .filter(|i| i.kind == ir::ItemKind::Element)
}

/// Whether `item` is a library the pass added (no item of `before`).
fn added_library(before: &[(ir::ItemRef, Vec<(ir::Ref, ir::DefKind)>)], item: ir::ItemRef) -> bool {
    item.kind == ir::ItemKind::DeviceLibrary && !before.iter().any(|(i, _)| *i == item)
}

fn model_row(
    name: ir::NameId,
    family: ir::ModelFamily,
    polarity: ir::Polarity,
    module: ir::Ref,
    word: Option<ir::NameId>,
) -> ir::Model {
    ir::Model {
        name,
        params: ir::ParamList::EMPTY,
        bin_of: ir::NameId::NONE,
        ako: ir::Ref::unresolved(ir::NameId::NONE),
        module,
        version: ir::ExprId::NONE,
        word: word.unwrap_or(ir::NameId::NONE),
        family,
        polarity,
    }
}

/// The passes some pair deck says it proves (`proves = <pass>` in a `.pair` file).
fn proven_by_pair_decks() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pair");
    let mut out = Vec::new();
    let cells = std::fs::read_dir(root).into_iter().flatten().flatten();
    for cell in cells.filter(|c| c.path().is_dir()) {
        for deck in std::fs::read_dir(cell.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            if deck.path().extension().is_some_and(|e| e == "pair") {
                let text = std::fs::read_to_string(deck.path()).unwrap_or_default();
                for line in text.lines() {
                    let line = line.split('#').next().unwrap_or("");
                    if let Some((k, v)) = line.split_once('=')
                        && k.trim() == "proves"
                    {
                        out.push(v.trim().to_string());
                    }
                }
            }
        }
    }
    out
}
