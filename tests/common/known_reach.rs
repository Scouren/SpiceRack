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
    ("vacask", "Item(Save)", "generated_save_plans_read_back_to_their_meaning", "W2303: VACASK spells two save shapes only (R29/S12)"),
    ("vacask", "Elem(Capacitor)", "proven_device_kinds_round_trip", "VACASK takes an instance's kind from its master: admits wants a visible model of the element's own proven family (W2104, W2108), which the generator rarely draws"),
    ("vacask", "Elem(Inductor)", "proven_device_kinds_round_trip", "as Elem(Capacitor)"),
    ("vacask", "Elem(Coupling)", "property_am_and_mutual_round_trip", "the generator makes no coupling over seeds 0..512 (it needs two inductor names and a Mutual model), so none is generated, let alone refused"),
    ("vacask", "Elem(Diode)", "proven_device_kinds_round_trip", "as Elem(Capacitor)"),
    ("vacask", "Elem(ModuleModel)", "proven_device_kinds_round_trip", "an instance of a user module needs its Load, a model of that module (W2104) and the module's own parameter names (W2102 on Gen's names)"),
];
