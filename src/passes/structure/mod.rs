//! Passes group `structure` (subcircuits, models, names, libraries, decks): a design in, the
//! design without one feature out, one pass per file in this folder.
//!
//! Owns the passes over scopes, definitions and names (`deck_title`, `hoist_nested_subckts`,
//! `formal_defaults_to_instances`, `explicit_family_models`, `device_libraries`,
//! `splice_elaborate`, `name_with_letter`, … in `rewrite/DIALECT-VACASK.md` §6). It runs first:
//! every other group walks the scopes and names it settles.
//!
//! Contract (`passes` module header, `rewrite/SPINE.md` §4):
//! - one pass per file in this folder, a helper shared by this group's passes here too;
//! - a pass imports `crate::ir`, `crate::passes` (for `Pass`) and this group only;
//! - `PASSES` lists the passes in the order `legalize` tries them within the group;
//! - `CODES` holds the notes the passes put at items they leave, codes `U0300..=U0399`, each
//!   named in its pass's `Pass::notes`;
//! - each pass has its fixtures in `tests/fixtures/passes/structure/<pass>/<case>.ir` and
//!   `.out.ir`, and meets the laws of `tests/pass_laws.rs` (P1–P5).
//!
//! Cites: `rewrite/SPINE.md` §4; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub mod constant_param_to_formal`, `pub mod deck_title`, `pub mod device_libraries`, `pub mod drop_builtin_models`,
//! `pub mod drop_device_libraries`, `pub mod drop_unread_source_models`, `pub mod explicit_family_models`,
//! `pub mod formal_defaults_to_instances`, `pub mod hoist_nested_subckts`, `pub mod name_with_letter`,
//! `pub mod splice_elaborate`, `pub mod unused`,
//! `pub(super) const PASSES: &[Pass]`, `pub(super) const CODES: &[(ir::Code, ir::Info)]`

pub mod constant_param_to_formal;
pub mod deck_title;
pub mod device_libraries;
pub mod drop_builtin_models;
pub mod drop_device_libraries;
pub mod drop_unread_source_models;
pub mod explicit_family_models;
pub mod formal_defaults_to_instances;
pub mod hoist_nested_subckts;
pub mod name_with_letter;
pub mod splice_elaborate;
pub mod unused;

use super::Pass;
use crate::ir;

/// The group's passes, in the order `legalize` tries them.
pub(super) const PASSES: &[Pass] = &[
    Pass {
        name: "deck_title",
        removes: ir::Feature::Extra(ir::Extra::DeckWithoutTitle),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Title)),
        notes: &[],
        run: deck_title::deck_title,
    },
    // first: the passes after it place models and libraries by the scopes it settles
    Pass {
        name: "hoist_nested_subckts",
        removes: ir::Feature::Placed(ir::ScopeKind::Subckt, ir::ItemKind::Subckt),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Extra(ir::Extra::FormalWithoutDefault)),
        notes: &[
            hoist_nested_subckts::READS_ENCLOSING,
            hoist_nested_subckts::NOT_IN_BODY,
        ],
        run: hoist_nested_subckts::hoist_nested_subckts,
    },
    Pass {
        name: "hoist_nested_subckts_default",
        removes: ir::Feature::Placed(ir::ScopeKind::Subckt, ir::ItemKind::Subckt),
        requires: ir::FeatureSet::EMPTY,
        notes: &[
            hoist_nested_subckts::READS_ENCLOSING,
            hoist_nested_subckts::NOT_IN_BODY,
            hoist_nested_subckts::HOIST_DEFAULT,
        ],
        run: hoist_nested_subckts::hoist_nested_subckts_default,
    },
    Pass {
        name: "hoist_enclosing_refs",
        removes: ir::Feature::Extra(ir::Extra::EnclosingRef),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Extra(ir::Extra::FormalWithoutDefault)),
        notes: &[
            hoist_nested_subckts::READS_ENCLOSING,
            hoist_nested_subckts::NOT_IN_BODY,
        ],
        run: hoist_nested_subckts::hoist_enclosing_refs,
    },
    Pass {
        name: "hoist_enclosing_refs_default",
        removes: ir::Feature::Extra(ir::Extra::EnclosingRef),
        requires: ir::FeatureSet::EMPTY,
        notes: &[
            hoist_nested_subckts::READS_ENCLOSING,
            hoist_nested_subckts::NOT_IN_BODY,
            hoist_nested_subckts::HOIST_DEFAULT,
        ],
        run: hoist_nested_subckts::hoist_enclosing_refs_default,
    },
    Pass {
        name: "constant_param_to_formal",
        removes: ir::Feature::Extra(ir::Extra::SubcktConstantParam),
        requires: ir::FeatureSet::EMPTY,
        notes: &[constant_param_to_formal::PASSED_LOCAL],
        run: constant_param_to_formal::constant_param_to_formal,
    },
    Pass {
        name: "splice_elaborate",
        removes: ir::Feature::Item(ir::ItemKind::Elaborate),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Param)),
        notes: &[splice_elaborate::ELABORATE_KEPT],
        run: splice_elaborate::splice_elaborate,
    },
    Pass {
        name: "formal_defaults_to_instances",
        removes: ir::Feature::Extra(ir::Extra::FormalNotConstant),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Extra(ir::Extra::FormalWithoutDefault))
            .with(ir::Feature::Extra(ir::Extra::EnclosingRef)),
        notes: &[
            formal_defaults_to_instances::DRAW_IN_DEFAULT,
            formal_defaults_to_instances::UNKNOWN_INSTANCES,
        ],
        run: formal_defaults_to_instances::formal_defaults_to_instances,
    },
    Pass {
        name: "formal_defaults_to_instances_default",
        removes: ir::Feature::Extra(ir::Extra::FormalNotConstant),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Extra(ir::Extra::EnclosingRef)),
        notes: &[
            formal_defaults_to_instances::DRAW_IN_DEFAULT,
            formal_defaults_to_instances::UNKNOWN_INSTANCES,
            formal_defaults_to_instances::PASS_DEFAULT,
        ],
        run: formal_defaults_to_instances::formal_defaults_to_instances_default,
    },
    Pass {
        name: "formal_defaults_to_instances_outer",
        removes: ir::Feature::Extra(ir::Extra::FormalOuterRead),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Extra(ir::Extra::FormalWithoutDefault))
            .with(ir::Feature::Extra(ir::Extra::EnclosingRef)),
        notes: &[
            formal_defaults_to_instances::DRAW_IN_DEFAULT,
            formal_defaults_to_instances::UNKNOWN_INSTANCES,
        ],
        run: formal_defaults_to_instances::formal_defaults_to_instances_outer,
    },
    Pass {
        name: "formal_defaults_to_instances_unbound",
        removes: ir::Feature::Extra(ir::Extra::FormalUnboundRead),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Extra(ir::Extra::FormalWithoutDefault))
            .with(ir::Feature::Extra(ir::Extra::EnclosingRef)),
        notes: &[
            formal_defaults_to_instances::DRAW_IN_DEFAULT,
            formal_defaults_to_instances::UNKNOWN_INSTANCES,
        ],
        run: formal_defaults_to_instances::formal_defaults_to_instances_unbound,
    },
    Pass {
        name: "name_with_letter",
        removes: ir::Feature::Circuit(ir::CircuitExtra::NameWithoutLetter),
        requires: ir::FeatureSet::EMPTY,
        notes: &[name_with_letter::UNRESOLVED_USE],
        run: name_with_letter::name_with_letter,
    },
    // before `device_libraries`: the models it adds may need libraries (it adds them itself)
    Pass {
        name: "explicit_family_models",
        removes: ir::Feature::Circuit(ir::CircuitExtra::ElementWithoutModel),
        requires: explicit_family_models::REQUIRES,
        notes: &[
            explicit_family_models::NO_FAMILY,
            explicit_family_models::NO_PLACE,
        ],
        run: explicit_family_models::explicit_family_models,
    },
    Pass {
        name: "device_libraries",
        removes: ir::Feature::Circuit(ir::CircuitExtra::ModelWithoutLibrary),
        requires: ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::DeviceLibrary)),
        notes: &[],
        run: device_libraries::device_libraries,
    },
    Pass {
        name: "drop_builtin_models",
        removes: ir::Feature::Circuit(ir::CircuitExtra::BuiltinMaster),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Circuit(ir::CircuitExtra::ElementWithoutModel)),
        notes: &[drop_builtin_models::UNBOUND_MASTER],
        run: drop_builtin_models::drop_builtin_models,
    },
    // after `drop_builtin_models`: the models it unbinds are unread here
    Pass {
        name: "drop_unread_source_v",
        removes: ir::Feature::Family(ir::ModelFamily::SourceV),
        requires: ir::FeatureSet::EMPTY,
        notes: &[drop_unread_source_models::SOURCE_MODEL_READ],
        run: drop_unread_source_models::drop_unread_source_v,
    },
    Pass {
        name: "drop_unread_source_i",
        removes: ir::Feature::Family(ir::ModelFamily::SourceI),
        requires: ir::FeatureSet::EMPTY,
        notes: &[drop_unread_source_models::SOURCE_MODEL_READ],
        run: drop_unread_source_models::drop_unread_source_i,
    },
    Pass {
        name: "drop_device_libraries",
        removes: ir::Feature::Item(ir::ItemKind::DeviceLibrary),
        requires: ir::FeatureSet::EMPTY
            .with(ir::Feature::Circuit(ir::CircuitExtra::ModelWithoutLibrary)),
        notes: &[],
        run: drop_device_libraries::drop_device_libraries,
    },
];

/// The group's catalogue rows (`U0300..=U0399`).
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        explicit_family_models::NO_FAMILY,
        ir::Info {
            title: "an element of a kind that names no model",
            cite: "rewrite/IR-CIRCUIT.md §4a; rewrite/AMEND-A12.md C5",
            explain: "The target instantiates every device through a model, and this element's kind (a behavioural, table, Laplace, frequency-table, delay or polynomial source, a port, a charge-defined capacitor or a flux-defined inductor) has no model family a model could name, so no model can stand for it and the element is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        explicit_family_models::NO_PLACE,
        ir::Info {
            title: "an element in a library section the design places no model in",
            cite: "rewrite/IR-CIRCUIT.md §4a; rewrite/AMEND-A12.md A15 K1, C5",
            explain: "The target instantiates every device through a model. A section is read alone, so the element's model must stand in the section; the design places no model in a section, so the target may lack that placement, and the element is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        hoist_nested_subckts::READS_ENCLOSING,
        ir::Info {
            title: "a nested subcircuit that reads an enclosing model, function or subcircuit",
            cite: "rewrite/AMEND-A12.md K2; rewrite/DIALECT-VACASK.md §6",
            explain: "The target resolves a name in the subcircuit itself and then at the top, never in an enclosing subcircuit, so this subcircuit must stand at the top; its body reads a model, function or subcircuit of the one enclosing it, which would not be visible there, and the subcircuit is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        hoist_nested_subckts::NOT_IN_BODY,
        ir::Info {
            title: "a subcircuit defined in a branch, loop or section inside a subcircuit",
            cite: "rewrite/AMEND-A12.md A15 K1; R27",
            explain: "The subcircuit is defined under a condition or inside a section of an enclosing subcircuit; moving it out would change when or where it is defined, so it is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        hoist_nested_subckts::HOIST_DEFAULT,
        ir::Info {
            title: "a hoisted subcircuit's added formal given the default 0",
            cite: "R13; rewrite/AMEND-A12.md K2",
            explain: "Hoisting the subcircuit made a parameter of its enclosing subcircuit a formal, which every instance passes. The target has no formal without a default, so the formal got the default 0: the hoist's value, which no instance reads.",
            kind: ir::Kind::Note,
        },
    ),
    (
        formal_defaults_to_instances::DRAW_IN_DEFAULT,
        ir::Info {
            title: "a subcircuit formal whose non-constant default holds a draw",
            cite: "R53; rewrite/AMEND-A12.md A29",
            explain: "The target lets an instance override only a formal whose default is a constant, so the default would be passed at each instance; it holds a random draw, and a copy per instance would not draw what the default draws, so the subcircuit is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        formal_defaults_to_instances::UNKNOWN_INSTANCES,
        ir::Info {
            title: "a subcircuit with instances the design cannot bind",
            cite: "R53; rewrite/AMEND-A12.md A29",
            explain: "Its non-constant default would be passed at every instance and itself replaced; an element names a subcircuit of this name without a binding (its file is not read), and that instance would read the replaced default, so the subcircuit is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        formal_defaults_to_instances::PASS_DEFAULT,
        ir::Info {
            title: "a formal whose non-constant default the pass replaced with 0",
            cite: "R13, R53; rewrite/AMEND-A12.md A29",
            explain: "Every instance now passes this formal's default as its actual. The target has no formal without a default, so the formal's own default became 0: the pass's value, which no instance reads.",
            kind: ir::Kind::Note,
        },
    ),
    (
        splice_elaborate::ELABORATE_KEPT,
        ir::Info {
            title: "an elaborate statement the target's one top circuit cannot hold",
            cite: "rewrite/AMEND-A12.md R1; rewrite/DIALECT-VACASK.md §6 (VS V1, V2)",
            explain: "The target's top circuit is its deck's top scope, so an elaborate statement is spliced in: the named subcircuits' bodies join the top. This one cannot be: it is not the only one, it follows an analysis, a named subcircuit is also instantiated or has a formal without a default, or two spliced bodies define one name. The statement is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        constant_param_to_formal::PASSED_LOCAL,
        ir::Info {
            title: "a subcircuit's local parameter an instance passes a value for",
            cite: "rewrite/AMEND-A12.md A25; R48",
            explain: "The target reads a constant parameter in a subcircuit's body as a formal an instance may override. An instance passes a value of this parameter's name, which the source ignores and the target would take, so the parameter is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        name_with_letter::RENAMED,
        ir::Info {
            title: "an element renamed to begin with its letter",
            cite: "R23; rewrite/AMEND-A12.md A13, A32",
            explain: "The target spells an element's kind by its name's first letter, and this name began with another, so the element was renamed (the letter before its name) and every reference to it follows. The note's argument is the old name: results are given back under it.",
            kind: ir::Kind::Note,
        },
    ),
    (
        name_with_letter::UNRESOLVED_USE,
        ir::Info {
            title: "an element whose name a reference reads through an unknown subcircuit",
            cite: "R23; rewrite/AMEND-A12.md A32",
            explain: "The element's name does not begin with its letter, but a reference reads a name like it through an instance whose subcircuit no read file defines, so renaming could leave that reference behind; the element is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        drop_builtin_models::UNBOUND_MASTER,
        ir::Info {
            title: "a source whose model name binds no definition",
            cite: "rewrite/IR-CIRCUIT.md §4a; rewrite/AMEND-A12.md C5",
            explain: "The target's source names no model. This one names a model no definition in the design binds (its kit is absent), so nothing says it is the builtin device the bare source means, and the element is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        drop_unread_source_models::SOURCE_MODEL_READ,
        ir::Info {
            title: "a builtin source model something still reads",
            cite: "rewrite/IR-CIRCUIT.md §4a; rewrite/AMEND-A12.md C5",
            explain: "The target has no model for an independent source, so a source model nothing reads is dropped. This one is still read (an element names it, or a run item addresses it), so it cannot go and is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        unused::UNUSED_DROPPED,
        ir::Info {
            title: "an unused definition dropped for a refusal it held",
            cite: "rewrite/RULINGS.md R80",
            explain: "Nothing the deck runs uses this definition (no element, analysis, statement or used definition names it, reads it or calls it, and no unresolved name or opaque statement spells it), and it holds an item the target refuses (a feature the target lacks, or the target writer's own refusal), or it names a definition dropped for one, so it is dropped rather than refused. The note's argument packs the refusal's code and argument; a kit conversion keeps every definition.",
            kind: ir::Kind::Note,
        },
    ),
];
