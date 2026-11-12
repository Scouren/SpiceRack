//! The waves group's diagnostic codes (`U0200..=U0299`) and their catalogue rows: the notes its
//! passes put at an item they leave carrying a feature.
//!
//! Contents: `pub(crate) const CODES: &[(ir::Code, ir::Info)]`, `pub(crate) const PULSE_LAWS_DIFFER`,
//! `pub(crate) const GIVEN_ZERO_LEFT`, `pub(crate) const EXP_TD2_LEFT`,
//! `pub(crate) const SFFM_LAW_LEFT`, `pub(crate) const GIVEN_ZERO_KEPT`

use crate::ir;

/// A pulse the other pulse law does not reproduce.
pub(crate) const PULSE_LAWS_DIFFER: ir::Code = ir::Code::new(b'U', 201);

/// A wave left under ngspice's zero rule.
pub(crate) const GIVEN_ZERO_LEFT: ir::Code = ir::Code::new(b'U', 202);

/// An exponential whose second delay cannot be recounted exactly.
pub(crate) const EXP_TD2_LEFT: ir::Code = ir::Code::new(b'U', 203);

/// An SFFM whose law cannot be changed exactly.
pub(crate) const SFFM_LAW_LEFT: ir::Code = ir::Code::new(b'U', 204);

/// An as-given wave ngspice's zero rule would read otherwise.
pub(crate) const GIVEN_ZERO_KEPT: ir::Code = ir::Code::new(b'U', 205);

/// The group's catalogue rows (`U0200..=U0299`).
pub(crate) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        PULSE_LAWS_DIFFER,
        ir::Info {
            title: "a pulse the other pulse law (SPICE's or VACASK's) does not reproduce",
            cite: "rewrite/AMEND-A12.md C1; rewrite/DIALECT-VACASK.md §6.1 (RUN q13)",
            explain: "The two pulse laws give the same value at every instant only when the rise and the fall are literals above zero, the period is a literal longer than rise + width + fall, the width is a literal, and the zero rule is the plain one. VACASK reads a zero fall as no falling edge and a period of zero as one pulse, and at a period of exactly rise + width + fall SPICE repeats the pulse where VACASK starts the next one at the first value. Otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        GIVEN_ZERO_LEFT,
        ir::Info {
            title: "a wave whose zero rule cannot be written out",
            cite: "rewrite/AMEND-A12.md A17; rewrite/IR-CIRCUIT.md §3",
            explain: "ngspice reads a zero rise, fall, width, period, frequency or time constant as its default (TSTEP, TSTOP or a fraction of it); the target reads it as zero. A field that is an expression becomes `x == 0 ? default : x`, which reads it twice; this field may hold a statistical draw, which two reads would draw twice. The item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        EXP_TD2_LEFT,
        ir::Info {
            title: "an exponential whose second delay cannot be counted from the other origin exactly",
            cite: "rewrite/AMEND-A12.md C2",
            explain: "SPICE counts the exponential's second delay from t = 0, VACASK from the first delay. From 0 to the first delay the new field is the difference, exact only when both delays are literals, the second is later, and the difference added back gives the second delay again; from the first delay to 0 it is the sum, which reads the first delay twice, so it must hold no statistical draw; and ngspice's zero rule must be written out first. Otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        SFFM_LAW_LEFT,
        ir::Info {
            title: "an SFFM whose modulation-index law cannot be changed exactly",
            cite: "rewrite/DIALECT-VACASK.md §6; crates/ir/src/wave.rs (SffmLaw)",
            explain: "ngspice limits the SFFM modulation index to between 0 and carrier/signal; SPICE3 and VACASK keep it as given. The law changes exactly when the index, carrier and signal frequencies are literals (the limit is then applied to the literal, or the index is already inside it) and ngspice's zero rule is written out first. Otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        GIVEN_ZERO_KEPT,
        ir::Info {
            title: "a wave ngspice's zero rule would read otherwise",
            cite: "rewrite/AMEND-A12.md A17; rewrite/IR-CIRCUIT.md §3",
            explain: "ngspice reads a zero rise, fall, width, period, frequency or time constant as its default (TSTEP, TSTOP or a fraction of it), and an omitted field as its default; the source reads such a field as it holds it and has no default. The rule is relabelled only when every such field is a literal other than zero and every other field is given. This wave holds a zero, an expression that may be zero, or an omitted field, so the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
];
