//! The waves group's diagnostic codes (`U0200..=U0299`) and their catalogue rows: the notes its
//! passes put at an item they leave carrying a feature.
//!
//! Contents: `pub(crate) const CODES: &[(ir::Code, ir::Info)]`, `pub(crate) const PULSE_LAWS_DIFFER`,
//! `pub(crate) const GIVEN_ZERO_LEFT`, `pub(crate) const EXP_TD2_LEFT`

use crate::ir;

/// A SPICE pulse the VACASK law does not reproduce.
pub(crate) const PULSE_LAWS_DIFFER: ir::Code = ir::Code::new(b'U', 201);

/// A wave left under ngspice's zero rule.
pub(crate) const GIVEN_ZERO_LEFT: ir::Code = ir::Code::new(b'U', 202);

/// An exponential whose second delay cannot be recounted exactly.
pub(crate) const EXP_TD2_LEFT: ir::Code = ir::Code::new(b'U', 203);

/// The group's catalogue rows (`U0200..=U0299`).
pub(crate) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        PULSE_LAWS_DIFFER,
        ir::Info {
            title: "a SPICE pulse the VACASK pulse law does not reproduce",
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
];
