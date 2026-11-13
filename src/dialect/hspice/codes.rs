//! The HSPICE dialect's diagnostic codes: `P1xxx` reading, `R1xxx` binding, `W1xxx` writing,
//! each area in its own hundred so the sub-slices append rows without touching another's:
//!
//! | area | files | codes |
//! |---|---|---|
//! | core and structure (S3.0) | `read.rs`, `expr_read.rs`, `number.rs`, `read_structure.rs`, `read_include.rs`, `write_structure.rs` | `P4001–P4099`, `R4001–R4099`, `W4001–W4099` |
//! | elements and sources (S3.1) | `read_elements.rs`, `read_sources.rs`, `write_elements.rs`, `write_sources.rs` | `P4100–P4199`, `R4100–R4199`, `W4100–W4199` |
//! | models (S3.2) | `read_models.rs`, `write_models.rs`, `families.rs` | `P4200–P4299`, `R4200–R4299`, `W4200–W4299` |
//! | run and outputs (S3.3) | `read_run.rs`, `write_run.rs`, `read_outputs.rs`, `write_outputs.rs` | `P4300–P4399`, `R4300–R4399`, `W4300–W4399` |
//!
//! Each row: code, title, cite, explanation, kind (`ir::Info`). Tests pin codes, never titles.
//! `info` is the chain of the areas' tables; `dialect::info` reaches it.
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §5; `REWRITE-PLAN.md` §4.5; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) type Row`, `pub(super) const CODES: &[Row]` (this area's),
//! `pub(super) const RANGES: &[(&str, u16, &[Row])]`, the S3.0 codes as constants
//! (`pub(super) const UNREAD`, `EXPR_INVALID`, `EXPR_NO_MEMBER`,
//! `PARAM_READS_CIRCUIT`, `MALFORMED`, `UNCLOSED`, `BRANCH_DEF`, `GLOBAL_GROUND`, `CALL_ARITY`, `WRAP`, `INCLUDE_CYCLE`, `PARAM_SELF`, `REDEFINED`, `DUPLICATE_DEF`,
//! `MISSING_FILE`, `DUPLICATE_ELEMENT`, `DEFAULT_CYCLE`, `UNSPELLED_VALUE`), `pub fn info(ir::Code) -> Option<ir::Info>`

use super::{
    read_elements, read_models, read_outputs, read_run, read_sources, write_elements, write_models,
    write_outputs, write_run, write_sources,
};
use crate::ir::{Code, Info, Kind};

/// One catalogue row.
pub(super) type Row = (Code, Info);

/// A statement no area of the reader has a rule for (a stub area, an unknown dot card, a
/// statement the CST could not read): `Unread`.
pub(super) const UNREAD: Code = Code::new(b'P', 4001);
/// An expression ngspice's own evaluator rejects (a syntax error, a wrong argument count, a
/// numparam `-` before a non-number): `InvalidInSource`.
pub(super) const EXPR_INVALID: Code = Code::new(b'P', 4002);
/// An expression using a function or form the IR has no member for (B `u`, `u2`, `pwr`,
/// numparam `sqr`, a string): `Unread`.
pub(super) const EXPR_NO_MEMBER: Code = Code::new(b'P', 4003);
/// A `.param` value or `.if` condition that reads the circuit: `InvalidInSource`.
pub(super) const PARAM_READS_CIRCUIT: Code = Code::new(b'P', 4006);
/// A structure card missing its name, path or parentheses, or a closer with no block:
/// `InvalidInSource`.
pub(super) const MALFORMED: Code = Code::new(b'P', 4007);
/// A block the file ends before closing; it is read to the end of the file.
pub(super) const UNCLOSED: Code = Code::new(b'P', 4008);
/// A `.param`, `.func`, `.subckt`, `.include` or `.lib` inside an `.if` branch (ngspice manual
/// §2.15 allows instances and models only): `InvalidInSource`.
pub(super) const BRANCH_DEF: Code = Code::new(b'P', 4009);
/// A `.global` naming only ground (`0`, `gnd`): it changes nothing; `Inert`.
pub(super) const GLOBAL_GROUND: Code = Code::new(b'P', 4010);
/// A call of a `.func` in scope with another argument count than its formals: ngspice stops
/// ("parameter mismatch for function call"): `InvalidInSource`.
pub(super) const CALL_ARITY: Code = Code::new(b'P', 4013);
/// A value whose `{…}` or `'…'` ngspice rejects where numparam reads it whole (a `.param`, a
/// model parameter): wrappers that wrap parts of it (`{a}+{b}`), or one inside another
/// (`'a+{b}'`; settle `wrap_*.sp`): `InvalidInSource`.
pub(super) const WRAP: Code = Code::new(b'P', 4014);
/// An `.include` or `.lib` call whose file includes, itself or through others, the file that
/// holds it: ngspice-45 crashes (settle `inc_cycle.sp`, exit 139): `InvalidInSource`.
pub(super) const INCLUDE_CYCLE: Code = Code::new(b'P', 4015);
/// A `.param` whose value reads its own name (`.param a='a*3'`): a definition cycle (IR X0017)
/// no simulator evaluates; ESPice's reading is unrun: `InvalidInSource`.
pub(super) const PARAM_SELF: Code = Code::new(b'P', 4016);
/// A definition ngspice drops for another of its name in its scope: a `.param` or `.func`
/// defined again later (it keeps the last: INV 3.01, settle `dup_func.sp`), a `.subckt` or
/// `.lib` section defined again earlier (it keeps the first: settle `dup_subckt.sp`,
/// `dup_section.sp`); the loser is `Inert`.
pub(super) const REDEFINED: Code = Code::new(b'R', 4001);
/// A `.param` in a subcircuit body naming one of its head parameters: ngspice-45 replaces the
/// default with it and an instance's value still wins (settle `dup_formal.sp`); read as that
/// default when its value reads no name (the card then `Inert`, or this note beside the card's
/// other assignments), else `Unread` (a default is read where
/// the instance stands, a body `.param` in the body: settle `dup_formal_reads.sp`). Also a
/// head parameter named twice: `Unread`.
pub(super) const DUPLICATE_DEF: Code = Code::new(b'R', 4002);
/// An include or library call whose file is not among the sources.
pub(super) const MISSING_FILE: Code = Code::new(b'R', 4003);
/// An element card whose name an earlier card of the same scope has: ngspice bails out
/// ("device already exists"): `InvalidInSource`.
pub(super) const DUPLICATE_ELEMENT: Code = Code::new(b'R', 4004);
/// A `.subckt` whose head defaults read each other in a cycle: ngspice stops ("a level depth
/// greater 1000 for dependent parameters"): `InvalidInSource`.
pub(super) const DEFAULT_CYCLE: Code = Code::new(b'R', 4005);

/// An expression holding a node the evaluator its class picks cannot spell: a draw in a value
/// that reads the temperature or the circuit (the B-source parser has no `unif`).
pub(super) const UNSPELLED_VALUE: Code = Code::new(b'W', 4007);

/// The S3.0 rows.
pub(super) const CODES: &[Row] = &[
    row(
        UNREAD,
        "a statement the ngspice reader has no rule for",
        "rewrite/DIALECT-HSPICE.md §3",
        "kept as ngspice text (Unread); a strict read refuses it.",
    ),
    row(
        EXPR_INVALID,
        "an expression ngspice rejects",
        "ngspice-45 xpressn.c formula, inpptree-parser.y",
        "the statement is kept as ngspice text (InvalidInSource): ngspice itself cannot evaluate it.",
    ),
    row(
        EXPR_NO_MEMBER,
        "an expression form the IR has no member for",
        "rewrite/IR-KERNEL.md §7.1",
        "the statement is kept as ngspice text (Unread) until the IR has a member with that meaning.",
    ),
    row(
        PARAM_READS_CIRCUIT,
        "a parameter or condition that reads the circuit",
        "rewrite/INV-semantics.md 2.06",
        "a .param or .if is evaluated once by numparam, which has no v(), i(), time, temper or hertz; kept as text (InvalidInSource).",
    ),
    row(
        MALFORMED,
        "a structure card ngspice cannot read",
        "ngspice manual §2.4–§2.15",
        "a card missing its name, path or parentheses, or a closer with no open block; kept as text (InvalidInSource).",
    ),
    row(
        UNCLOSED,
        "a block the file ends before closing",
        "ngspice manual §2.4.3, §2.7, §2.15",
        "its body runs to the end of the file.",
    ),
    row(
        BRANCH_DEF,
        "a definition or include inside an .if branch",
        "ngspice manual §2.15",
        "a branch holds instances and models only; kept as text (InvalidInSource).",
    ),
    row(
        GLOBAL_GROUND,
        "a .global of ground only",
        "ngspice-45 inpcom.c inp_fix_gnd_name",
        "ground is global already; kept as Inert text.",
    ),
    refusal(
        UNSPELLED_VALUE,
        "an expression its evaluator cannot spell",
        "ngspice-45 xpressn.c, inpptree.c",
        "a value that reads the temperature or the circuit is written for the B-source parser, which has no draw function; refused.",
    ),
    row(
        CALL_ARITY,
        "a .func call with another argument count than its formals",
        "ngspice-45 runs (arity: \"parameter mismatch for function call\")",
        "ngspice stops on the deck; kept as InvalidInSource text.",
    ),
    row(
        WRAP,
        "a value whose braces or quotes ngspice rejects",
        "ngspice-45 runs (settle wrap_*.sp: '; sign expected.', 'Syntax error: letter [{]')",
        "numparam replaces each {...} on its own and reads no wrapper inside another in a .param or a model parameter; kept as InvalidInSource text.",
    ),
    row(
        INCLUDE_CYCLE,
        "an include cycle",
        "ngspice-45 run (settle inc_cycle.sp: a segmentation fault, exit 139)",
        "a file that includes, through others, the file that includes it; kept as InvalidInSource text.",
    ),
    row(
        PARAM_SELF,
        "a .param that reads its own name",
        "crates/ir check.rs (DefinitionCycle); dialect_mutants (param_string.sp seed 13)",
        "the value reads the parameter it defines; kept as InvalidInSource text (ESPice's reading not run).",
    ),
    row(
        REDEFINED,
        "a definition another of its name in its scope replaces",
        "rewrite/INV-semantics.md 3.01; ngspice-45 runs (settle dup_subckt.sp: \"redefinition of .subckt cell, ignored\"; dup_func.sp; dup_section.sp)",
        "ngspice keeps the last .param or .func and the first .subckt or .lib section of a name; the other changes no number and is kept as Inert text.",
    ),
    row(
        DUPLICATE_DEF,
        "a .param naming its subcircuit's head parameter, or a head parameter named twice",
        "ngspice-45 runs (settle dup_formal.sp: the body .param replaces the default, an instance's value still wins; dup_formal_reads.sp)",
        "a value that reads no name is read as the head parameter's default (the card kept as Inert text); any other, and a head parameter named twice, is kept as text (Unread).",
    ),
    row(
        DEFAULT_CYCLE,
        "subcircuit head defaults that read each other in a cycle",
        "ngspice-45 run (mutual.cir: \"A level depth greater 1000 for dependent parameters\")",
        "ngspice stops on the deck; kept as InvalidInSource text.",
    ),
    row(
        DUPLICATE_ELEMENT,
        "an element name given twice in one scope",
        "ngspice-45 run (dup.cir: \"device already exists, bail out\")",
        "ngspice rejects the deck at the second card; kept as InvalidInSource text.",
    ),
    row(
        MISSING_FILE,
        "an include whose file is not among the sources",
        "rewrite/SPINE.md §2",
        "the include keeps the path as written; its definitions are unresolved.",
    ),
];

/// Each area's name, first code number, and table: the chain `info` reads (`P`, `R` and `W`
/// share the hundreds).
pub(super) const RANGES: &[(&str, u16, &[Row])] = &[
    ("core and structure", 4001, CODES),
    ("read_elements", 4100, read_elements::CODES),
    ("read_sources", 4100, read_sources::CODES),
    ("write_elements", 4100, write_elements::CODES),
    ("write_sources", 4100, write_sources::CODES),
    ("read_models", 4200, read_models::CODES),
    ("write_models", 4200, write_models::CODES),
    ("read_run", 4300, read_run::CODES),
    ("read_outputs", 4300, read_outputs::CODES),
    ("write_run", 4300, write_run::CODES),
    ("write_outputs", 4300, write_outputs::CODES),
];

/// The catalogue row of an ngspice dialect code, or `None` for any other code.
pub fn info(code: Code) -> Option<Info> {
    RANGES
        .iter()
        .flat_map(|(_, _, rows)| rows.iter())
        .find(|(c, _)| *c == code)
        .map(|(_, i)| *i)
}

// ---- private ----

/// A refusal row.
const fn refusal(
    code: Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    (
        code,
        Info {
            title,
            cite,
            explain,
            kind: Kind::Refusal,
        },
    )
}

/// A note row.
const fn row(code: Code, title: &'static str, cite: &'static str, explain: &'static str) -> Row {
    (
        code,
        Info {
            title,
            cite,
            explain,
            kind: Kind::Note,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_distinct_and_each_area_stays_in_its_hundred() {
        // Oracle: the range table of this file's header (DIALECT-HSPICE §5).
        let mut seen = std::collections::HashSet::new();
        for (area, first, rows) in RANGES {
            for (code, info) in *rows {
                assert!(seen.insert(*code), "{code} twice");
                let hundred = if *first == 4001 { 4000 } else { *first };
                let ok = code.number >= *first && code.number < hundred + 100;
                assert!(ok, "{area}: {code} outside {first}..{}", hundred + 100);
                assert!(matches!(code.letter, b'P' | b'R' | b'W'), "{code}");
                assert!(!info.title.ends_with('.'), "{code}: a title has no period");
            }
        }
        assert_eq!(info(UNREAD).map(|i| i.kind), Some(Kind::Note));
        assert_eq!(info(Code::new(b'P', 4999)), None);
    }
}
