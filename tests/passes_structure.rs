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

#[path = "common/memcap.rs"]
mod memcap;

#[path = "passes_structure/bindings.rs"]
mod bindings;
#[path = "passes_structure/r53.rs"]
mod r53;
#[path = "passes_structure/reads.rs"]
mod reads;

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes;

use reads::{
    Run, added_library, elements, feature, instance_actual, instances_pass, model_row, param_reads,
    proven_by_pair_decks, refs,
};

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
    "hoist_nested_subckts_default",
    "hoist_enclosing_refs_default",
    "formal_defaults_to_instances",
    "formal_defaults_to_instances_default",
    "formal_defaults_to_instances_outer",
    "formal_defaults_to_instances_unbound",
    "splice_elaborate",
    "constant_param_to_formal",
    "name_with_letter",
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
    let expected = "file \"a.sp\" deck\n  model inductor inductor\n  element vsource v1 (a 0) dc=1\n  element inductor l1 (a 0) model=inductor value=1u\n  element inductor l2 (b 0) model=inductor value=1u\n  element coupling k1 () inductors=(l1 l2) k=0.5\n  element vcvs e1 (o 0 a 0) gain=2\n";
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
    let mut refused = 0;
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
        refused += diags
            .rows()
            .iter()
            .filter(|d| d.code == ir::Code::new(b'U', 303))
            .count();
        let still = ir::features_used(&design).contains(nested);
        assert!(
            !still || !diags.is_empty(),
            "seed {seed}: left nested with no note"
        );
    }
    assert!(ran > 0, "no generated design had a nested subcircuit");
    eprintln!("hoist_nested_subckts: {ran} generated designs, {refused} left with U0303");
}

/// The runs of each rule a formal's default states (`tests/fixtures/passes/structure/<row>/
/// <case>.ir`): after the row, the actual the instance passes, evaluated by the reference
/// evaluator with every read bound by its definition, is the value the simulator printed.
/// R53 (ng-scaffold, ngspice-45, v(n1) at 1 mA): which1 3k, which2 3k, which3 6k (`l`); R62's
/// rows (A37): `outer/which1` 2000 (VACASK run which1, `settle/which1.sim`: the read bound
/// where the definition stands, the wrapper's 8k not seen), `unbound/which2` 3k (ngspice-45
/// run which2: the instance's scope).
#[test]
fn formal_default_rows_give_the_values_each_simulator_printed() {
    use passes::structure::formal_defaults_to_instances as f;
    let base: Run = f::formal_defaults_to_instances;
    let rows: [(&str, Run, &str, &str, f64); 5] = [
        ("formal_defaults_to_instances/which1", base, "xs", "t", 3e3),
        ("formal_defaults_to_instances/which2", base, "xs", "t", 3e3),
        ("formal_defaults_to_instances/which3", base, "xs", "l", 6e3),
        (
            "formal_defaults_to_instances_outer/which1",
            f::formal_defaults_to_instances_outer,
            "x1",
            "r",
            2e3,
        ),
        (
            "formal_defaults_to_instances_unbound/which2",
            f::formal_defaults_to_instances_unbound,
            "xs",
            "t",
            3e3,
        ),
    ];
    for (case, run, inst, formal, value) in rows {
        let got = instance_actual(case, run, inst, formal);
        assert_eq!(got, Ok(value), "{case}: {inst}'s {formal}");
    }
}

/// A rename keeps what every by-name reference names: each use (`Design::element_uses`, in
/// the same order before and after) names, in its scope's list, the same element; every
/// renamed element begins with one of its letters; each element left unrenamed with its name
/// wrong has a note; the notes' old names are the elements' names before.
#[test]
fn name_with_letter_keeps_what_every_reference_names() {
    let feature = ir::Feature::Circuit(ir::CircuitExtra::NameWithoutLetter);
    let named = |d: &ir::Design, u: ir::ElementUse| -> Option<ir::ItemRef> {
        (!u.scope.is_none()).then_some(())?;
        d.items(u.scope).iter().copied().find(|i| {
            i.kind == ir::ItemKind::Element && d.row::<ir::Element>(i.index).name == u.name
        })
    };
    let mut ran = 0;
    let mut renamed = 0;
    for seed in SEEDS {
        let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        if !ir::features_used(&design).contains(feature) {
            continue;
        }
        ran += 1;
        let before: Vec<Option<ir::ItemRef>> = design
            .element_uses()
            .into_iter()
            .map(|(_, u)| named(&design, u))
            .collect();
        let names: Vec<(ir::ItemRef, ir::NameId)> = elements(&design)
            .map(|i| (i, design.row::<ir::Element>(i.index).name))
            .collect();
        let mut diags = ir::Diags::new();
        passes::structure::name_with_letter::name_with_letter(&mut design, &mut diags);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "seed {seed}: {:?}", found.rows());
        let after: Vec<Option<ir::ItemRef>> = design
            .element_uses()
            .into_iter()
            .map(|(_, u)| named(&design, u))
            .collect();
        assert_eq!(
            after, before,
            "seed {seed}: a reference names another element"
        );
        for d in diags.rows() {
            if d.code == ir::Code::new(b'U', 311) {
                renamed += 1;
                let old = names.iter().find(|n| n.0 == d.item).map(|n| n.1.0);
                assert_eq!(old, Some(d.arg), "seed {seed}: the note's old name");
            }
        }
    }
    assert!(ran > 0 && renamed > 0, "no generated element to rename");
    eprintln!("name_with_letter: {ran} generated designs, {renamed} renames");
}

/// Moving and re-pointing definitions keeps every read: for every item listed before and
/// after `splice_elaborate` or `constant_param_to_formal`, its row references and its
/// expressions' parameter reads bind the same definitions (`ir::check` clean: no stale entry).
#[test]
fn splice_and_constant_params_keep_what_every_read_binds() {
    let rows = [
        (
            "splice_elaborate",
            ir::Feature::Item(ir::ItemKind::Elaborate),
        ),
        (
            "constant_param_to_formal",
            ir::Feature::Extra(ir::Extra::SubcktConstantParam),
        ),
    ];
    for (name, feature) in rows {
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let mut ran = 0;
        for seed in SEEDS {
            let mut design = generate(seed, &ir::FeatureSet::ALL, SIZE);
            if !ir::features_used(&design).contains(feature) {
                continue;
            }
            ran += 1;
            let refs_before = refs(&design);
            let reads_before = param_reads(&design);
            let mut diags = ir::Diags::new();
            (pass.run)(&mut design, &mut diags);
            let mut found = ir::Diags::new();
            ir::check(&design, &mut found);
            assert!(found.is_empty(), "{name} seed {seed}: {:?}", found.rows());
            let refs_after = refs(&design);
            for (item, r) in &refs_after {
                if let Some((_, b)) = refs_before.iter().find(|(i, _)| i == item) {
                    assert_eq!(r, b, "{name} seed {seed}: {item:?} rebound");
                }
            }
            let reads_after = param_reads(&design);
            let kept = |reads: &[(ir::ItemRef, ir::DefId)]| -> Vec<(ir::ItemRef, ir::DefId)> {
                let both = |i: &ir::ItemRef| refs_after.iter().any(|(j, _)| j == i);
                reads.iter().copied().filter(|(i, _)| both(i)).collect()
            };
            assert_eq!(
                kept(&reads_after),
                kept(&reads_before),
                "{name} seed {seed}: a read rebound"
            );
        }
        assert!(ran > 0, "{name}: no generated design");
        eprintln!("{name}: {ran} generated designs");
    }
}

/// R53 over seeded designs of the shape it converts (R55: built for the rule, not drawn from
/// the generator, whose expressions rarely evaluate): top parameters, a wrapper subcircuit
/// shadowing some of them, a subcircuit whose formals default to constants, to reads of those
/// parameters, and to sibling formals, and instances at the top and in the wrapper passing
/// random subsets of actuals. The reads are unbound (R53: from each instance's scope) or bound
/// to the top's definition (R62's lexical rule: the wrapper's shadow is not seen). Every
/// formal's value at every instance (`passes_structure/r53.rs`: an unbound read looked up from
/// the instance's scope, a bound one by its definition) is the same, bit for bit, before and
/// after each row.
#[test]
fn formal_defaults_keep_every_instances_values() {
    use passes::structure::formal_defaults_to_instances as f;
    use r53::Read::{Lexical, Unbound};
    let base: Run = f::formal_defaults_to_instances;
    // every seeded default reads unbound (or a sibling), or bound where the definition stands
    for (name, run, read) in [
        ("formal_defaults_to_instances", base, Unbound),
        (
            "formal_defaults_to_instances_unbound",
            f::formal_defaults_to_instances_unbound,
            Unbound,
        ),
        ("formal_defaults_to_instances", base, Lexical),
        (
            "formal_defaults_to_instances_outer",
            f::formal_defaults_to_instances_outer,
            Lexical,
        ),
    ] {
        r53::keeps_every_instances_values(name, run, read);
    }
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
