//! The Spectre dialect's diagnostic codes: `P6xxx` reading, `W6xxx` writing. Each row: code,
//! title, cite, explanation, kind (`ir::Info`). Tests pin codes, never titles. **No Spectre runs
//! on this machine**: a cite marked MAN or CODE rests on a manual page or the old pipeline
//! alone, RUN-SP on the recorded Spectre 25.1 session (`rewrite/DIALECT-SPECTRE.md`).
//!
//! Contents: the codes as constants (`UNREAD`, `EXPR_NO_MEMBER`, `READS_CIRCUIT`,
//! `NOT_A_NUMBER`, `LONG_LITERAL`, `CALL_ARITY`, `ROUND_IN_CIRCUIT`, `SIMULATOR_SPECTRE`,
//! `SPICE_REGION`, `MALFORMED`, `REDEFINED`, `PLACEMENT`, `FUNC_FORM`, `GLOBAL_GROUND`,
//! `UPPER_CASE`, `TERMINALS`, `NO_VALUE`, `SOURCE_DEFAULT`, `RECURSION`, `UNSPELLED`), `info`
//! (crate-private)

use crate::ir::{Code, Info, Kind};

/// A statement no rule of this reader reads: `Unread`.
pub(super) const UNREAD: Code = Code::new(b'P', 6001);
/// An expression using a function or form this reader has no member or no evidence for.
pub(super) const EXPR_NO_MEMBER: Code = Code::new(b'P', 6002);
/// A circuit read (`v()`, `$time`, `$freq`) where the value reads none.
pub(super) const READS_CIRCUIT: Code = Code::new(b'P', 6003);
/// A number token Spectre does not read.
pub(super) const NOT_A_NUMBER: Code = Code::new(b'P', 6004);
/// A literal with more significant digits than a double.
pub(super) const LONG_LITERAL: Code = Code::new(b'P', 6005);
/// A function called with another number of arguments than it takes.
pub(super) const CALL_ARITY: Code = Code::new(b'P', 6006);
/// `round` where the circuit is read.
pub(super) const ROUND_IN_CIRCUIT: Code = Code::new(b'P', 6007);
/// `simulator lang=spectre` in a Spectre file.
pub(super) const SIMULATOR_SPECTRE: Code = Code::new(b'P', 6008);
/// A `simulator lang=spice` region, kept whole.
pub(super) const SPICE_REGION: Code = Code::new(b'P', 6009);
/// A statement missing its name, parentheses or `key=value` form.
pub(super) const MALFORMED: Code = Code::new(b'P', 6010);
/// A name defined twice in one scope.
pub(super) const REDEFINED: Code = Code::new(b'P', 6011);
/// A statement where this reader does not take it (a function or `statistics` below the top,
/// a subcircuit in a branch).
pub(super) const PLACEMENT: Code = Code::new(b'P', 6012);
/// A function not of the form `real f(real a, …) { return e; }`.
pub(super) const FUNC_FORM: Code = Code::new(b'P', 6013);
/// A `global` naming only ground.
pub(super) const GLOBAL_GROUND: Code = Code::new(b'P', 6014);
/// A parameter name with an upper-case letter on a primitive or a model.
pub(super) const UPPER_CASE: Code = Code::new(b'P', 6015);
/// An instance with another terminal count than its kind takes.
pub(super) const TERMINALS: Code = Code::new(b'P', 6016);
/// A resistor, capacitor or inductor with no value and no model.
pub(super) const NO_VALUE: Code = Code::new(b'P', 6017);
/// A source wave missing a field whose Spectre default no run settled.
pub(super) const SOURCE_DEFAULT: Code = Code::new(b'P', 6018);
/// A subcircuit instantiated inside its own body.
pub(super) const RECURSION: Code = Code::new(b'P', 6019);
/// A name the writer cannot spell in Spectre text.
pub(super) const UNSPELLED: Code = Code::new(b'W', 6001);

/// Every row, in code order.
const CODES: &[(Code, Info)] = &[
    row(
        UNREAD,
        Kind::Note,
        "a statement the Spectre reader has no rule for",
        "rewrite/DIALECT-SPECTRE.md (the slice's scope)",
        "The statement is kept as Spectre text (`Unread`): a Spectre writer copies it, any other refuses it, and a strict read refuses the deck.",
    ),
    row(
        EXPR_NO_MEMBER,
        Kind::Note,
        "an expression form with no IR member or no evidence",
        "rewrite/DIALECT-SPECTRE.md §3, §10",
        "A function such as `atan2`, `ln`, `ddt`, `idt` or `i()`, an operator such as `%`, a `$` variable, a vector or a string in a value: no recorded run settled its meaning, or the IR has none. The statement is kept as Spectre text.",
    ),
    row(
        READS_CIRCUIT,
        Kind::Refusal,
        "a circuit read where the value reads none",
        "rewrite/DIALECT-SPECTRE.md §3 (MAN: SR p.870; `v()`, `$time` only in a behavioural value)",
        "`v()`, `$time` and `$freq` read the circuit; a parameter, a model or instance parameter, a condition or a function body cannot. The statement is kept as text and the deck refused.",
    ),
    row(
        NOT_A_NUMBER,
        Kind::Refusal,
        "a number Spectre does not read",
        "rewrite/DIALECT-SPECTRE.md §2 (MAN: UG §2.4; CODE: the lexicon's tests)",
        "Spectre reads digits, a point, an `e` exponent and one scale letter with unit letters after it; `1k5` and `1d3` are no numbers. The statement is kept as text and the deck refused.",
    ),
    row(
        LONG_LITERAL,
        Kind::Note,
        "a literal with more digits than a double holds",
        "rewrite/AMEND-A12.md A18; no Spectre run",
        "No run settled how Spectre rounds a literal of more than 18 significant digits; the statement is kept as Spectre text.",
    ),
    row(
        CALL_ARITY,
        Kind::Refusal,
        "a function called with another number of arguments than it takes",
        "rewrite/DIALECT-SPECTRE.md §3 (MAN)",
        "The call does not match the function's argument count; the statement is kept as text and the deck refused.",
    ),
    row(
        ROUND_IN_CIRCUIT,
        Kind::Refusal,
        "round where the circuit is read",
        "rewrite/DIALECT-SPECTRE.md §3 (RUN-SP: SFE-2319 \"Compiled netlist functions are not supported for bsource\")",
        "Spectre refuses `round` in a behavioural source; the statement is kept as text and the deck refused.",
    ),
    row(
        SIMULATOR_SPECTRE,
        Kind::Note,
        "simulator lang=spectre in a Spectre file",
        "rewrite/DIALECT-SPECTRE.md §2 (MAN: UG p.53)",
        "The file is already read as Spectre; the statement changes nothing (`Inert`). Another dialect's writer leaves it out.",
    ),
    row(
        SPICE_REGION,
        Kind::Note,
        "a simulator lang=spice region, kept as text",
        "rewrite/DIALECT-SPECTRE.md §2 (MAN: UG p.53; RUN-SP: its `.measure` runs)",
        "The region is SPICE text Spectre reads with its own SPICE reader; it is kept whole as Spectre text (`Private`), copied by the Spectre writer and refused by any other.",
    ),
    row(
        MALFORMED,
        Kind::Refusal,
        "a statement missing its name, parentheses or key=value form",
        "rewrite/DIALECT-SPECTRE.md §5 (MAN: `name (nodes) master k=v`)",
        "The statement does not have the form Spectre reads, or gives a parameter twice; it is kept as text and the deck refused.",
    ),
    row(
        REDEFINED,
        Kind::Refusal,
        "a name defined twice in one scope",
        "rewrite/DIALECT-SPECTRE.md §4 (MAN: SR19 `redefinedparams`; not run)",
        "Spectre refuses a second definition of a parameter, function, subcircuit or analysis name in one scope. The statement is kept as text and the deck refused.",
    ),
    row(
        PLACEMENT,
        Kind::Note,
        "a statement where this reader does not take it",
        "rewrite/DIALECT-SPECTRE.md §3, §4 (MAN: functions and statistics at the top level)",
        "A user function or a statistics block below the top level, or a subcircuit inside a branch: kept as Spectre text.",
    ),
    row(
        FUNC_FORM,
        Kind::Note,
        "a function not of the form real f(real a) { return e; }",
        "rewrite/DIALECT-SPECTRE.md §3 (MAN: SR p.870)",
        "Only real functions of real arguments whose body is one `return` are read; the statement is kept as Spectre text.",
    ),
    row(
        GLOBAL_GROUND,
        Kind::Note,
        "a global naming only ground",
        "rewrite/DIALECT-SPECTRE.md §7 (CODE)",
        "Ground is global already; the statement changes nothing (`Inert`).",
    ),
    row(
        UPPER_CASE,
        Kind::Refusal,
        "a parameter name with an upper-case letter on a primitive or a model",
        "rewrite/DIALECT-SPECTRE.md §2 (RUN-SP: `W=`/`L=` on a MOS ignored with SFE-102, the default size used: Id 4.225e-5 for 4.225e-4)",
        "Spectre's primitive and model parameters are lower case; it ignores another spelling with a warning and runs the default, a wrong number. The statement is kept as text and the deck refused.",
    ),
    row(
        TERMINALS,
        Kind::Refusal,
        "an instance with another terminal count than its kind",
        "rewrite/DIALECT-SPECTRE.md §5, §6 (MAN)",
        "The instance connects another number of nodes than its master has terminals; it is kept as text and the deck refused.",
    ),
    row(
        NO_VALUE,
        Kind::Note,
        "a resistor, capacitor or inductor with no value and no model",
        "rewrite/DIALECT-SPECTRE.md §5 (MAN: the defaults are not settled)",
        "The element gives no value and names no model; Spectre's default is not settled, so the statement is kept as Spectre text.",
    ),
    row(
        SOURCE_DEFAULT,
        Kind::Note,
        "a source wave missing a field whose default no run settled",
        "rewrite/DIALECT-SPECTRE.md §6, §10",
        "A sine without `ampl` or `freq`, a pulse without one of `val0 val1 rise fall width period`, a PWL without `wave`: Spectre's default is not settled, so the statement is kept as Spectre text.",
    ),
    row(
        RECURSION,
        Kind::Refusal,
        "a subcircuit instantiated inside itself",
        "rewrite/DIALECT-SPECTRE.md §5 (MAN: a hierarchy has no cycle)",
        "The instance names the subcircuit whose body holds it; the hierarchy would not end. The statement is kept as text and the deck refused.",
    ),
    row(
        UNSPELLED,
        Kind::Refusal,
        "a name the Spectre writer cannot spell",
        "rewrite/DIALECT-SPECTRE.md §2 (MAN: names)",
        "The name holds a byte Spectre reads as a separator or an operator, or is empty; the writer refuses the design rather than write another name.",
    ),
];

/// The catalogue row of one of Spectre's own codes, or `None`.
pub(super) fn info(code: Code) -> Option<Info> {
    CODES.iter().find(|(c, _)| *c == code).map(|(_, i)| *i)
}

// ---- private ----

/// One row.
const fn row(
    code: Code,
    kind: Kind,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> (Code, Info) {
    (
        code,
        Info {
            title,
            cite,
            explain,
            kind,
        },
    )
}
