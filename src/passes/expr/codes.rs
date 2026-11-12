//! The expression group's diagnostic codes (`U0100..=U0199`) and their catalogue rows: the
//! notes its passes put at an item they leave carrying a feature.
//!
//! Contents: `pub(crate) const CODES: &[(ir::Code, ir::Info)]`, `pub(crate) const MIXED_DIVISION`, `pub(crate) const TAG_READ`, `pub(crate) const POW_BASE`, `pub(crate) const DRAW_TWICE`, `pub(crate) const PWL_POINTS`, `pub(crate) const BITOP_NOT_CONSTANT`, `pub(crate) const NO_LEAF_VALUE`, `pub(crate) const PLAIN_RUN_NOMINAL`, `pub(crate) const DRAW_SAMPLES`, `pub(crate) const DIGITS_ROUNDED`, `pub(crate) const CALL_LEFT`, `pub(crate) const FUNC_CALLED`, `pub(crate) const OPERAND_TOO_LARGE`, `pub(crate) const BOTH_FORMS`

use crate::ir;

/// A typed division whose operand types the design does not fix.
pub(crate) const MIXED_DIVISION: ir::Code = ir::Code::new(b'U', 101);
/// A rewrite that drops a tag while a typed division may read it.
pub(crate) const TAG_READ: ir::Code = ir::Code::new(b'U', 103);
/// A signed power whose exponent is no literal and whose base may be negative.
pub(crate) const POW_BASE: ir::Code = ir::Code::new(b'U', 104);
/// A rewrite that would read an operand holding a draw more than once.
pub(crate) const DRAW_TWICE: ir::Code = ir::Code::new(b'U', 105);
/// A `pwl` whose points are not increasing literals, or whose input holds a draw.
pub(crate) const PWL_POINTS: ir::Code = ir::Code::new(b'U', 106);
/// A bit operator whose operands are not integer literals, or whose fold would drop a read tag.
pub(crate) const BITOP_NOT_CONSTANT: ir::Code = ir::Code::new(b'U', 107);
/// A transient or nominal-temperature leaf with no one literal to stand for it.
pub(crate) const NO_LEAF_VALUE: ir::Code = ir::Code::new(b'U', 108);
/// A draw that sampled in every run now samples only in a sampling run.
pub(crate) const PLAIN_RUN_NOMINAL: ir::Code = ir::Code::new(b'U', 109);
/// A draw whose rewrite would change what a sampling run draws.
pub(crate) const DRAW_SAMPLES: ir::Code = ir::Code::new(b'U', 110);
/// A literal's digits past a double's precision were rounded.
pub(crate) const DIGITS_ROUNDED: ir::Code = ir::Code::new(b'U', 111);
/// A call of a user function that cannot be inlined.
pub(crate) const CALL_LEFT: ir::Code = ir::Code::new(b'U', 113);
/// A user function still called after inlining.
pub(crate) const FUNC_CALLED: ir::Code = ir::Code::new(b'U', 114);
/// A rule whose copies would make an expression too large to spell.
pub(crate) const OPERAND_TOO_LARGE: ir::Code = ir::Code::new(b'U', 116);
/// A function body its callers read in both forms.
pub(crate) const BOTH_FORMS: ir::Code = ir::Code::new(b'U', 112);

/// The group's catalogue rows (`U0100..=U0199`).
pub(crate) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        MIXED_DIVISION,
        ir::Info {
            title: "a typed division whose operand types the design does not fix",
            cite: "rewrite/AMEND-A12.md K1; rewrite/DIALECT-VACASK.md §6.2",
            explain: "The division truncates when both operands are integers and divides exactly otherwise, and the design gives an operand both types (two instances pass a parameter an integer and a real, two calls pass a formal differently, or a loop or an alter sets it). Real division cannot say which, so the item is refused. While such a division stands, no integer quotient elsewhere is rewritten either: it could read that quotient's type.",
            kind: ir::Kind::Note,
        },
    ),
    (
        TAG_READ,
        ir::Info {
            title: "a rewrite that drops an integer type while a typed division may read it",
            cite: "rewrite/AMEND-A12.md K1",
            explain: "The target has no spelling for an integer-typed value here; its real spelling has the same value, but a typed division still in the design reads types, so the item is refused until none is left.",
            kind: ir::Kind::Note,
        },
    ),
    (
        POW_BASE,
        ir::Info {
            title: "a signed power whose exponent is not a literal and whose base may be negative",
            cite: "rewrite/DIALECT-VACASK.md §6.2",
            explain: "pow(x, y) of a negative x is (-1)^y |x|^y for a whole y; the target's powers keep the sign only for a known odd exponent (pwr) or drop it (pow_abs). With an exponent known only at run time and a base that may be negative, no spelling is equal, so the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        DRAW_TWICE,
        ir::Info {
            title: "a rewrite that would read an operand holding a draw more than once",
            cite: "the expression group's operand rule; rewrite/RULINGS.md R22",
            explain: "The target spells this function by its definition, which reads its operand more than once. The operand holds a statistical draw (directly or through a parameter or function it reads), and each read may draw anew, so the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        PWL_POINTS,
        ir::Info {
            title: "a pwl whose points are not increasing literals, or whose input holds a draw",
            cite: "crates/ir/src/expr.rs (Func::Pwl)",
            explain: "The target spells pwl as a chain of ternaries, which reads the input several times and each point only on its own segment. That is the same function only when every point is a literal with strictly increasing x values and the input holds no draw; otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        BITOP_NOT_CONSTANT,
        ir::Info {
            title: "a bit operator whose operands are not integer literals",
            cite: "rewrite/DIALECT-VACASK.md §6.2",
            explain: "The target has no bit operators; one is replaced by the literal of its value only when its operands are integer literals, a shift stays within 0 to 63, the value stays within 2^53, and no typed division is left to read its integer type. Otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        NO_LEAF_VALUE,
        ir::Info {
            title: "a transient or nominal-temperature leaf with no single literal value in the design",
            cite: "rewrite/DIALECT-VACASK.md §6",
            explain: "The target has no name for the transient analysis's step or stop time, or for the nominal temperature, inside an expression. The leaf is replaced by that value when the design has exactly one transient analysis (one Env setting tnom for the whole run) and the value is a literal; otherwise the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        PLAIN_RUN_NOMINAL,
        ir::Info {
            title: "a draw that sampled in every run now samples only in a sampling run",
            cite: "rewrite/RULINGS.md R22",
            explain: "The source simulator draws a fresh sample even in a plain run; the target draws only in a Monte Carlo run. In a plain run the draw now reads its nominal value, which is the only value a plain run can be checked against.",
            kind: ir::Kind::Note,
        },
    ),
    (
        DRAW_SAMPLES,
        ir::Info {
            title: "a draw whose rewrite would change what a sampling run draws",
            cite: "rewrite/RULINGS.md R22; rewrite/DIALECT-VACASK.md §6.2",
            explain: "The design runs a Monte Carlo loop, or the draw samples in every run, so a per-reference draw relabelled per call site, or a sampling draw replaced by its nominal, would change the joint distribution of the results. The item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        DIGITS_ROUNDED,
        ir::Info {
            title: "a literal's digits past a double's precision were written as the shortest decimal of its double",
            cite: "rewrite/RULINGS.md R37; rewrite/AMEND-A12.md A18",
            explain: "The source wrote more significant digits than the target keeps; the literal is written as the shortest decimal that reads back as the correctly rounded double of the digits. A simulator whose own conversion of the long digits is not correctly rounded (ngspice's INPevaluate) may have computed a double a few ulp away from it.",
            kind: ir::Kind::Note,
        },
    ),
    (
        CALL_LEFT,
        ir::Info {
            title: "a call of a user function that cannot be inlined",
            cite: "rewrite/RULINGS.md R51; INV-semantics.md 2.26",
            explain: "The target has no user functions, so each call is replaced by the function's body with its arguments substituted. This call stays: the function is recursive (directly or through another), the call is unresolved, its argument count does not match, an argument holds a statistical draw that the body reads more than once (each read would draw anew). The item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        FUNC_CALLED,
        ir::Info {
            title: "a user function still called after inlining",
            cite: "rewrite/RULINGS.md R51",
            explain: "The target has no user functions; a function is removed once nothing calls it, and this one is still called by a call inline_funcs could not replace. The item is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        OPERAND_TOO_LARGE,
        ir::Info {
            title: "a rule whose copies would make an expression too large to spell",
            cite: "the expression group's size bound (ROOT_PATHS_MAX, 8192 rows by paths)",
            explain: "The target spells this function by a definition that reads its operand more than once, and a writer writes each read out in full, so nested copies grow the text exponentially. Converted here, an expression of this item would spell 8192 rows or more (the longest expression of the ngspice and VACASK corpora is 132 tokens), so the rule leaves the item, which is refused.",
            kind: ir::Kind::Note,
        },
    ),
    (
        BOTH_FORMS,
        ir::Info {
            title: "a function body its callers read in both forms",
            cite: "rewrite/AMEND-A12.md A28; rewrite/RULINGS.md R51",
            explain: "The body of a user function counts in the forms of the expressions that call it; this one is called from a constant expression and from one that reads the circuit or the temperature. A rule that converts one form cannot rewrite it without changing the other's spelling. Inlining the function (inline_funcs) gives each caller its own copy; without it, the item is refused.",
            kind: ir::Kind::Note,
        },
    ),
];
