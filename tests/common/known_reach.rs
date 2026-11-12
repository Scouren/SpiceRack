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
pub const KNOWN_REACH: &[(&str, &str, &str, &str)] = &[
    ("ngspice", "Elem(Resistor)", FIXPOINT, ELEMENTS),
    ("ngspice", "Elem(Capacitor)", FIXPOINT, ELEMENTS),
    ("ngspice", "Elem(Inductor)", FIXPOINT, ELEMENTS),
    ("ngspice", "Elem(ISource)", FIXPOINT, SOURCES),
    ("ngspice", "Elem(Port)", FIXPOINT, SOURCES),
    ("ngspice", "Elem(Table)", FIXPOINT, ELEMENTS),
    ("ngspice", "Wave(Pulse)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Sin)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Exp)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Pwl)", FIXPOINT, SOURCES_NONE),
    ("ngspice", "Wave(Sffm)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(Am)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(TrNoise)", FIXPOINT, SOURCES),
    ("ngspice", "Wave(TrRandom)", FIXPOINT, SOURCES),
    ("ngspice", "Analysis(Dc)", FIXPOINT, RUN),
    ("ngspice", "Analysis(Noise)", FIXPOINT, RUN),
    ("ngspice", "Analysis(SensAc)", FIXPOINT, RUN),
    ("ngspice", "Analysis(Disto)", FIXPOINT, RUN),
    ("ngspice", "Analysis(Pss)", FIXPOINT, RUN),
    ("ngspice", "Family(Vbic)", FIXPOINT, VERSIONED),
    ("ngspice", "Family(Hicum2)", FIXPOINT, VERSIONED),
    ("ngspice", "Family(Bsim3)", FIXPOINT, VERSIONED),
    ("ngspice", "Family(Bsim4)", FIXPOINT, VERSIONED),
    ("ngspice", "Family(B3SoiFd)", FIXPOINT, MODELS_NONE),
    ("ngspice", "Family(B3SoiDd)", FIXPOINT, MODELS_NONE),
    ("ngspice", "Family(B4Soi)", FIXPOINT, VERSIONED),
    ("ngspice", "Family(Hisim2)", FIXPOINT, MODELS_NONE),
    ("ngspice", "Family(HisimHv)", FIXPOINT, VERSIONED),
    ("ngspice", "Family(Mesa)", FIXPOINT, MODELS),
];

/// ngspice's interim covering property (until round 7): each area's fixture fixpoint.
const FIXPOINT: &str = "each_read_fixture_is_a_fixpoint_of_read_write_read";
// Why, per area: every admitted generated design drops the kind by that area's refusal.
const ELEMENTS: &str =
    "interim: dialect_ngspice_elements' fixtures (resistor, capacitor_inductor, controlled)";
const SOURCES: &str = "interim: dialect_ngspice_sources' fixtures (pulse, sin, exp, sffm, am, noise, ac_forms, distof_port)";
const SOURCES_NONE: &str =
    "interim: no read fixture of the kind yet in dialect_ngspice_sources (asked)";
const RUN: &str = "interim: dialect_ngspice_run's fixtures (dc, noise, sens, disto, pss)";
const MODELS: &str = "interim: dialect_ngspice_models' fixture jfet_mes_hfet";
const VERSIONED: &str = "interim: dialect_ngspice_models' fixtures (bjt, mos_levels, bins, polarity_default, bsim_version); no valid version drawn before ir-circuit b6888c35";
const MODELS_NONE: &str =
    "interim: no read fixture of the family yet in dialect_ngspice_models (asked)";
