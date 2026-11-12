//! VACASK's own code table: `P2xxx` reading, `R2xxx` binding, `W2xxx` writing, each row with its
//! title, cite, explanation and kind. Every area appends its rows in its own file (`CODES`);
//! this file holds the spine's rows and the one lookup over all of them, so no two areas edit
//! one table. Tests pin codes, never messages.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §7; `REWRITE-PLAN.md` §4.5.
//!
//! Contents: `CODES`, `info` (crate-private), and the spine's codes `UNREAD`, `PRIVATE`

use crate::ir;

use super::{
    expr_read, read_elements, read_models, read_outputs, read_run, read_sources, read_structure,
    write_structure,
};

/// A statement the reader has no rule for yet (an area not written): kept as an `Unread` opaque.
pub(super) const UNREAD: ir::Code = ir::Code::new(b'P', 2004);
/// A construct only VACASK has and no pass reads: kept as a `Private` opaque.
pub(super) const PRIVATE: ir::Code = ir::Code::new(b'P', 2003);

/// The spine's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        PRIVATE,
        ir::Info {
            title: "a construct only VACASK has, kept as its text",
            cite: "rewrite/DIALECT-VACASK.md §3, §5 (G5, G13–G16)",
            explain: "The statement means something only VACASK runs (vector values, a solution slot, a custom behavioural body, …); VACASK's own writer copies it, any other dialect refuses it.",
            kind: ir::Kind::Note,
        },
    ),
    (
        UNREAD,
        ir::Info {
            title: "a VACASK statement the reader has no rule for yet",
            cite: "rewrite/DIALECT-VACASK.md §2 (the areas of S4.1–S4.3)",
            explain: "The statement is kept as an `Unread` opaque: VACASK's own writer copies it, another dialect refuses it, and a strict read refuses the deck.",
            kind: ir::Kind::Note,
        },
    ),
];

/// The row of `code` in any area's table, or `None`.
pub(super) fn info(code: ir::Code) -> Option<ir::Info> {
    let tables: [&[(ir::Code, ir::Info)]; 9] = [
        CODES,
        expr_read::CODES,
        read_structure::CODES,
        write_structure::CODES,
        read_elements::CODES,
        read_models::CODES,
        read_sources::CODES,
        read_run::CODES,
        read_outputs::CODES,
    ];
    tables
        .iter()
        .flat_map(|t| t.iter())
        .find(|(c, _)| *c == code)
        .map(|(_, i)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: the range rule of §7 (each code `P`, `R` or `W`, 2000–2999) and uniqueness.
    #[test]
    fn every_row_is_in_vacasks_range_and_unique() {
        let tables: [&[(ir::Code, ir::Info)]; 9] = [
            CODES,
            expr_read::CODES,
            read_structure::CODES,
            write_structure::CODES,
            read_elements::CODES,
            read_models::CODES,
            read_sources::CODES,
            read_run::CODES,
            read_outputs::CODES,
        ];
        let mut seen = Vec::new();
        for (code, info) in tables.iter().flat_map(|t| t.iter()) {
            assert!(matches!(code.letter, b'P' | b'R' | b'W'), "{code:?}");
            assert!((2000..3000).contains(&code.number), "{code:?}");
            assert!(!info.title.is_empty() && !info.cite.is_empty(), "{code:?}");
            assert!(!seen.contains(code), "{code:?} twice");
            seen.push(*code);
        }
    }
}
