//! The ngspice dialect's diagnostic codes: `P1xxx` reading, `R1xxx` binding, `W1xxx` writing,
//! each area in its own hundred so the sub-slices append rows without touching another's:
//!
//! | area | files | codes |
//! |---|---|---|
//! | core and structure (S3.0) | `read.rs`, `expr_read.rs`, `number.rs`, `read_structure.rs`, `write_structure.rs` | `P1001–P1099`, `R1001–R1099`, `W1001–W1099` |
//! | elements and sources (S3.1) | `read_elements.rs`, `read_sources.rs`, `write_elements.rs`, `write_sources.rs` | `P1100–P1199`, `R1100–R1199`, `W1100–W1199` |
//! | models (S3.2) | `read_models.rs`, `write_models.rs`, `families.rs` | `P1200–P1299`, `R1200–R1299`, `W1200–W1299` |
//! | run and outputs (S3.3) | `read_run.rs`, `write_run.rs`, `read_outputs.rs`, `write_outputs.rs` | `P1300–P1399`, `R1300–R1399`, `W1300–W1399` |
//! | control (S3.4) | `read_control.rs`, `write_control.rs` | `P1400–P1499`, `R1400–R1499`, `W1400–W1499` |
//!
//! Each row: code, title, cite, explanation, kind (`ir::Info`). Tests pin codes, never titles.
//! `info` is the chain of the areas' tables; `dialect::info` reaches it.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §5; `REWRITE-PLAN.md` §4.5; `rewrite/S3-AREAS.md`.
//!
//! Contents: `pub(super) type Row`, `pub(super) const CODES: &[Row]` (this area's),
//! `pub(super) const RANGES: &[(&str, u16, &[Row])]`, the S3.0 codes as constants
//! (`pub(super) const UNREAD`, `EXPR_INVALID`, `EXPR_NO_MEMBER`, `TERNARY_NEST`,
//! `PARAM_READS_CIRCUIT`, `MALFORMED`, `UNCLOSED`, `BRANCH_DEF`, `GLOBAL_GROUND`, `NEGATION`, `FIRST_LAST`, `CALL_ARITY`, `DUPLICATE_PARAM`, `DUPLICATE_DEF`,
//! `MISSING_FILE`, `DUPLICATE_ELEMENT`, `DEFAULT_CYCLE`, `TEXT_VALUE`, `UNSPELLED_VALUE`), `pub fn info(ir::Code) -> Option<ir::Info>`

use super::{
    read_control, read_elements, read_models, read_outputs, read_run, read_sources, write_control,
    write_elements, write_models, write_outputs, write_run, write_sources,
};
use crate::ir::{Code, Info, Kind};

/// One catalogue row.
pub(super) type Row = (Code, Info);

/// A statement no area of the reader has a rule for (a stub area, an unknown dot card, a
/// statement the CST could not read): `Unread`.
pub(super) const UNREAD: Code = Code::new(b'P', 1001);
/// An expression ngspice's own evaluator rejects (a syntax error, a wrong argument count, a
/// numparam `-` before a non-number): `InvalidInSource`.
pub(super) const EXPR_INVALID: Code = Code::new(b'P', 1002);
/// An expression using a function or form the IR has no member for (B `u`, `u2`, `pwr`,
/// numparam `sqr`, a string): `Unread`.
pub(super) const EXPR_NO_MEMBER: Code = Code::new(b'P', 1003);
/// A numparam then-branch ternary nested past `a?b?c:d:e`: `InvalidInSource`.
pub(super) const TERNARY_NEST: Code = Code::new(b'P', 1004);
/// A `.param` value or `.if` condition that reads the circuit: `InvalidInSource`.
pub(super) const PARAM_READS_CIRCUIT: Code = Code::new(b'P', 1006);
/// A structure card missing its name, path or parentheses, or a closer with no block:
/// `InvalidInSource`.
pub(super) const MALFORMED: Code = Code::new(b'P', 1007);
/// A block the file ends before closing; it is read to the end of the file.
pub(super) const UNCLOSED: Code = Code::new(b'P', 1008);
/// A `.param`, `.func`, `.subckt`, `.include` or `.lib` inside an `.if` branch (ngspice manual
/// §2.15 allows instances and models only): `InvalidInSource`.
pub(super) const BRANCH_DEF: Code = Code::new(b'P', 1009);
/// A `.global` naming only ground (`0`, `gnd`): it changes nothing; `Inert`.
pub(super) const GLOBAL_GROUND: Code = Code::new(b'P', 1010);
/// A numparam `-` after a binary operator before anything but a number (`{2*-q}`: ngspice's
/// "wrongly determined negation", or a later number negated instead): `InvalidInSource`.
pub(super) const NEGATION: Code = Code::new(b'P', 1011);
/// A numparam call of a two-argument function with three arguments (`limit(a, b, c)`): ngspice
/// evaluates it on the first and the last (`xpressn.c` `formula`), and so does the IR.
pub(super) const FIRST_LAST: Code = Code::new(b'P', 1012);
/// A call of a `.func` in scope with another argument count than its formals: ngspice stops
/// ("parameter mismatch for function call"): `InvalidInSource`.
pub(super) const CALL_ARITY: Code = Code::new(b'P', 1013);
/// A `.param` the same scope defines again later: ngspice keeps the last (INV 3.01); this one
/// is `Inert`.
pub(super) const DUPLICATE_PARAM: Code = Code::new(b'R', 1001);
/// A `.subckt`, `.func` or `.lib` section defined twice in one scope, or a `.param` naming a
/// subcircuit parameter: ngspice's winner is not established; `Unread`.
pub(super) const DUPLICATE_DEF: Code = Code::new(b'R', 1002);
/// An include or library call whose file is not among the sources.
pub(super) const MISSING_FILE: Code = Code::new(b'R', 1003);
/// An element card whose name an earlier card of the same scope has: ngspice bails out
/// ("device already exists"): `InvalidInSource`.
pub(super) const DUPLICATE_ELEMENT: Code = Code::new(b'R', 1004);
/// A `.subckt` whose head defaults read each other in a cycle: ngspice stops ("a level depth
/// greater 1000 for dependent parameters"): `InvalidInSource`.
pub(super) const DEFAULT_CYCLE: Code = Code::new(b'R', 1005);

/// A string or list value where ngspice reads an expression (a `.param`, head default, `.if`
/// or `.func`): numparam has no string or vector values.
pub(super) const TEXT_VALUE: Code = Code::new(b'W', 1006);
/// An expression holding a node the evaluator its class picks cannot spell: a draw in a value
/// that reads the temperature or the circuit (the B-source parser has no `unif`).
pub(super) const UNSPELLED_VALUE: Code = Code::new(b'W', 1007);

/// The S3.0 rows.
pub(super) const CODES: &[Row] = &[
    row(
        UNREAD,
        "a statement the ngspice reader has no rule for",
        "rewrite/DIALECT-NGSPICE.md §3",
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
        TERNARY_NEST,
        "a numparam ternary nested in a then-branch past one level",
        "rewrite/INV-semantics.md 2.13",
        "numparam reads a?b?c:d:e as a?c:e and a deeper nest no single way; kept as text (InvalidInSource).",
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
        TEXT_VALUE,
        "a string or list where ngspice reads an expression",
        "ngspice-45 xpressn.c formula",
        "a .param, head default, .if condition or .func body is a number expression; refused.",
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
        FIRST_LAST,
        "a three-argument call of a two-argument numparam function",
        "ngspice-45 xpressn.c formula",
        "numparam passes such a function its first and last argument and evaluates the middle one only for its side effects; read the same way.",
    ),
    row(
        DUPLICATE_PARAM,
        "a parameter defined again later in its scope",
        "rewrite/INV-semantics.md 3.01",
        "ngspice keeps the last definition; this one changes no number and is kept as Inert text.",
    ),
    row(
        DUPLICATE_DEF,
        "a definition repeated in one scope",
        "rewrite/DIALECT-NGSPICE.md §3",
        "which one ngspice keeps is not established for this kind; the repeat is kept as text (Unread).",
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
    ("core and structure", 1001, CODES),
    ("read_elements", 1100, read_elements::CODES),
    ("read_sources", 1100, read_sources::CODES),
    ("write_elements", 1100, write_elements::CODES),
    ("write_sources", 1100, write_sources::CODES),
    ("read_models", 1200, read_models::CODES),
    ("write_models", 1200, write_models::CODES),
    ("read_run", 1300, read_run::CODES),
    ("read_outputs", 1300, read_outputs::CODES),
    ("write_run", 1300, write_run::CODES),
    ("write_outputs", 1300, write_outputs::CODES),
    ("read_control", 1400, read_control::CODES),
    ("write_control", 1400, write_control::CODES),
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
        // Oracle: the range table of this file's header (DIALECT-NGSPICE §5).
        let mut seen = std::collections::HashSet::new();
        for (area, first, rows) in RANGES {
            for (code, info) in *rows {
                assert!(seen.insert(*code), "{code} twice");
                let hundred = if *first == 1001 { 1000 } else { *first };
                let ok = code.number >= *first && code.number < hundred + 100;
                assert!(ok, "{area}: {code} outside {first}..{}", hundred + 100);
                assert!(matches!(code.letter, b'P' | b'R' | b'W'), "{code}");
                assert!(!info.title.ends_with('.'), "{code}: a title has no period");
            }
        }
        assert_eq!(info(UNREAD).map(|i| i.kind), Some(Kind::Note));
        assert_eq!(info(Code::new(b'P', 1999)), None);
    }
}
