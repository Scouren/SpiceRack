//! The waves group's diagnostic codes (`U0200..=U0299`) and their catalogue rows: the notes its
//! passes put at an item they leave carrying a feature.
//!
//! Contents: `pub(crate) const CODES: &[(ir::Code, ir::Info)]`, `pub(crate) const PULSE_LAWS_DIFFER`

use crate::ir;

/// A SPICE pulse the VACASK law does not reproduce.
pub(crate) const PULSE_LAWS_DIFFER: ir::Code = ir::Code::new(b'U', 201);

/// The group's catalogue rows (`U0200..=U0299`).
pub(crate) const CODES: &[(ir::Code, ir::Info)] = &[(
    PULSE_LAWS_DIFFER,
    ir::Info {
        title: "a SPICE pulse the VACASK pulse law does not reproduce",
        cite: "rewrite/AMEND-A12.md C1; rewrite/DIALECT-VACASK.md §6.1 (RUN q13)",
        explain: "The two pulse laws give the same value at every instant only when the rise and the fall are literals above zero, the period is a literal longer than rise + width + fall, the width is a literal, and the zero rule is the plain one. VACASK reads a zero fall as no falling edge and a period of zero as one pulse, and at a period of exactly rise + width + fall SPICE repeats the pulse where VACASK starts the next one at the first value. Otherwise the item is refused.",
        kind: ir::Kind::Note,
    },
)];
