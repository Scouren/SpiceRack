//! VACASK's own code table: `P2xxx` reading, `R2xxx` binding, `W2xxx` writing, each row with its
//! title, cite, explanation and kind. Every area appends its rows in its own file (`CODES`);
//! this file holds the spine's rows and the one lookup over all of them, so no two areas edit
//! one table. Tests pin codes, never messages.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §7; `REWRITE-PLAN.md` §4.5.
//!
//! Contents: `CODES`, `info` (crate-private), and the spine's codes `NO_SYNTAX`, `SKIPPED`, `PRIVATE`, `UNCLOSED`

use crate::ir;

use super::{
    atan2, expr_read, expr_write, fold, read_elements, read_models, read_outputs, read_run,
    read_sources, read_structure, write_structure,
};

/// A statement VACASK has no syntax for (an ngspice card, a `*` line): invalid in source.
pub(super) const NO_SYNTAX: ir::Code = ir::Code::new(b'P', 2013);
/// The same at a library's top outside every section: skipped when included by section.
pub(super) const SKIPPED: ir::Code = ir::Code::new(b'P', 2014);
/// A construct only VACASK has and no pass reads: kept as a `Private` opaque.
pub(super) const PRIVATE: ir::Code = ir::Code::new(b'P', 2003);

/// A statement whose `(`, `[` or `{` never closes, or a string a line break ends.
pub(super) const UNCLOSED: ir::Code = ir::Code::new(b'P', 2012);

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
        UNCLOSED,
        ir::Info {
            title: "a bracket that never closes, or a string a line break ends",
            cite: "settle unclosed_p1, _b1, _c1, _e1, _q1 (VACASK 36be560: \"Parser syntax error\", \"unexpected newline in string literal\"); unclosed_p2, _b2, _c2 (a bracket joins lines)",
            explain: "VACASK reads on across line breaks while a bracket is open and stops with a syntax error when it never closes; a string cannot cross a line break. The statement, to where VACASK's joining takes it, is invalid in source.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        NO_SYNTAX,
        ir::Info {
            title: "a statement VACASK has no syntax for",
            cite: "DIALECT-VACASK run fx; settle star_deck, star_in_section (VACASK 36be560: \"Syntax error, unexpected string.\")",
            explain: "No VACASK statement begins this way (an ngspice card, a `*` comment line, a top-level `save`); VACASK rejects it in a deck and in the section a deck includes.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        SKIPPED,
        ir::Info {
            title: "text outside every section of a library, which VACASK skips",
            cite: "settle star_outside_section, junk_outside_section (VACASK 36be560 runs a deck that includes such a library by section=)",
            explain: "VACASK reads only the chosen section of a library it includes by section; a statement outside every section is never read there, so it moves no number (a plain include of the library would reject it).",
            kind: ir::Kind::Note,
        },
    ),
];

/// The row of `code` in any area's table, or `None`.
pub(super) fn info(code: ir::Code) -> Option<ir::Info> {
    let tables: [&[(ir::Code, ir::Info)]; 12] = [
        CODES,
        atan2::CODES,
        expr_read::CODES,
        expr_write::CODES,
        fold::CODES,
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
        let tables: [&[(ir::Code, ir::Info)]; 12] = [
            CODES,
            atan2::CODES,
            expr_read::CODES,
            expr_write::CODES,
            fold::CODES,
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
