//! The reach law's KNOWN list (`tests/dialect_laws.rs`, R55), apart from the laws for the
//! 500-line cap: each dialect's kinds that no admitted generated design reaches.
//!
//! Contents: `pub const KNOWN_REACH`.

/// The kinds a row claims and no admitted generated design reaches (R55): (row, feature as
/// `Debug` prints it, the covering property, why). The covering property is a seeded test in
/// the owning area's test file that builds admissible designs of the kind through the builder
/// and checks write, read back and `eq_meaning`, with floors on designs run and designs
/// holding the kind; its function name must be found as `fn <name>` in a file under `tests/`.
/// A row whose reason starts with `interim` names a fixture fixpoint in its place: accepted and
/// printed on every run until the dialect's gate (S3.5 ngspice, S4.5 VACASK), which fails on it.
/// Strict both ways (`reach_every_kind_the_row_claims`).
#[rustfmt::skip] // a table: one row per line
pub const KNOWN_REACH: &[(&str, &str, &str, &str)] = &[
    ("ngspice", "Elem(Inductor)", FIXPOINT, ELEMENTS),
    ("ngspice", "Elem(Port)", FIXPOINT, SOURCES),
    ("ngspice", "Elem(Table)", FIXPOINT, ELEMENTS),
    ("ngspice", "Wave(Pulse)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Sin)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Exp)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Pwl)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Sffm)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Am)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(TrNoise)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(TrRandom)", FIXPOINT, SOURCES),
    ("ngspice", "Family(Mesa)", FIXPOINT, MODELS),
    ("ngspice", "Family(CodeModel)", "code_models_of_the_build_round_trip_by_meaning", CODE_MODELS),
    ("vacask", "Item(Save)", "generated_save_plans_read_back_to_their_meaning", "W2303: VACASK spells two save shapes only (R29/S12)"),
    ("vacask", "Elem(Capacitor)", "proven_device_kinds_round_trip", "VACASK takes an instance's kind from its master: admits wants a visible model of the element's own proven family (W2104, W2108), which W2104 still finds missing on 333 of 341 generated capacitors and 327 of 331 diodes over 512 seeds"),
    ("vacask", "Elem(VSource)", "property_am_and_mutual_round_trip", VC_SOURCES),
    ("vacask", "Elem(Ccvs)", VC_FIXPOINT, VC_SOURCES_INTERIM),
    ("vacask", "Elem(Diode)", "proven_device_kinds_round_trip", "as Elem(Capacitor)"),
    ("vacask", "Wave(Pulse)", VC_FIXPOINT, VC_SOURCES_INTERIM),
    ("vacask", "Wave(Sin)", VC_FIXPOINT, VC_SOURCES_INTERIM),
    ("vacask", "Wave(Pwl)", VC_FIXPOINT, VC_SOURCES_INTERIM),
    ("vacask", "Wave(Sffm)", VC_FIXPOINT, VC_SOURCES_INTERIM),
    ("vacask", "Wave(Am)", "property_am_and_mutual_round_trip", VC_SOURCES),
];

/// ngspice's interim covering property (until round 7): each area's fixture fixpoint.
const FIXPOINT: &str = "each_read_fixture_is_a_fixpoint_of_read_write_read";
// Why, per area: every admitted generated design drops the kind by that area's refusal.
const ELEMENTS: &str =
    "interim: dialect_ngspice_elements' fixtures (resistor, capacitor_inductor, controlled)";
const SOURCES: &str = "interim: dialect_ngspice_sources' fixtures (pulse, pwl, sin, exp, sffm, am, noise, ac_forms, distof_port)";
const MODELS: &str = "interim: dialect_ngspice_models' fixture jfet_mes_hfet";
const VC_FIXPOINT: &str = "every_reader_fixture_is_a_fixpoint_of_write_and_read";
const VC_SOURCES: &str = "W2104 for sources (law L5 seed 1): VACASK takes a source's kind from its master, and the generator names a source's model with a fresh name no model statement defines, so every generated source is refused";
const VC_SOURCES_INTERIM: &str = "interim: dialect_vacask_sources' fixtures (sources, controlled, pulse, sine, exp, pwl, am_fm); W2104 for sources refuses every generated one (its model is a fresh name), until the generator gives a source a model of its builtin family (R48 d) or the sources property draws every wave";
const CODE_MODELS: &str = "the generator's code-model words name none of the build's (W1206); a seeded property of the build's code models covers it (tests/dialect_ngspice_models.rs)";
