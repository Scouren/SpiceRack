//! Diagnostics as rows: every stage appends `Diag { code, span, arg }` and keeps going; nothing
//! here prints, and nothing returns early on a diagnostic (C14). Rendering happens once, in the
//! root crate, sorted by position, from the assembled catalogue.
//!
//! This crate's own catalogue: `X` (an invariant violation: a bug in a reader, pass or builder,
//! never the user's deck; one per `Violation`), `U` (a feature the target lacks; `arg` is the
//! `Feature::index`), `S` (a statement read but not representable; one per `OpaqueClass`), and
//! `P9001..=P9099` for the IR text reader (`text::parse`). Every
//! other owner keeps its letters and numeric ranges in its own table; a root-crate test checks
//! the assembled catalogue for collisions. Codes are never renumbered or reused (D24 freezes them
//! at 1.0).
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, C14; `REWRITE-PLAN.md` §4.5; `compiler-construction` rule 5.
//!
//! Contents:
//! - `pub struct Code`, `Code::new(u8, u16) -> Code`, `pub enum Kind`, `pub struct Info`,
//!   `pub struct Diag`, `pub struct Diags`
//! - `Diags::new() -> Diags`, `Diags::push(&mut self, Code, Span, u32)`,
//!   `Diags::rows(&self) -> &[Diag]`, `Diags::len(&self) -> usize`,
//!   `Diags::is_empty(&self) -> bool`
//! - `pub const UNSUPPORTED: Code`, `pub const TEXT_SYNTAX: Code`,
//!   `pub const fn violation_code(Violation) -> Code`,
//!   `pub const fn opaque_code(OpaqueClass) -> Code`, `pub fn info(Code) -> Option<Info>`

use std::fmt;

use crate::check::Violation;
use crate::item::OpaqueClass;
use crate::span::Span;

/// A diagnostic code: a letter naming the owner class and a number in that owner's range,
/// displayed `X0001`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code {
    /// ASCII uppercase: `L P R S U W X`.
    pub letter: u8,
    /// 1..=9999.
    pub number: u16,
}

/// What a diagnostic does to the output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Kind {
    /// Informs; the output is still produced.
    Note,
    /// The output is refused.
    Refusal,
}

/// A code's catalogue row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    /// One line, no trailing period.
    pub title: &'static str,
    /// The clause or decision it enforces (`IR-KERNEL.md §9`, a manual section).
    pub cite: &'static str,
    /// The `--explain` text.
    pub explain: &'static str,
    /// Note or refusal.
    pub kind: Kind,
}

/// One diagnostic. `arg`'s meaning is stated per code (an index, a feature, a count); 0 when
/// the code takes none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diag {
    /// What happened.
    pub code: Code,
    /// Where; `Span::NONE` for built input.
    pub span: Span,
    /// The code's argument.
    pub arg: u32,
}

/// Appended diagnostics, in append order (sorting is the renderer's).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diags {
    pub(crate) rows: Vec<Diag>,
}

/// The code of an unsupported feature on the target: `U0001`, `arg` the feature's index.
pub const UNSUPPORTED: Code = Code::new(b'U', 1);

/// A line of IR text that `text::parse` cannot read: `P9001` (the IR text reader's range is
/// `P9001..=P9099`); `arg` 0.
pub const TEXT_SYNTAX: Code = Code::new(b'P', 9001);

const _: () = assert!(size_of::<Code>() == 4);
const _: () = assert!(size_of::<Diag>() == 20);

impl Code {
    /// A code. Asserts `letter` is ASCII uppercase and `number` in `1..=9999`.
    pub const fn new(letter: u8, number: u16) -> Code {
        assert!(
            letter.is_ascii_uppercase(),
            "a code's letter is ASCII uppercase"
        );
        assert!(number >= 1 && number <= 9999, "a code's number is 1..=9999");
        Code { letter, number }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:04}", self.letter as char, self.number)
    }
}

impl Diags {
    /// An empty list.
    pub fn new() -> Diags {
        Diags::default()
    }

    /// Appends one row.
    pub fn push(&mut self, code: Code, span: Span, arg: u32) {
        self.rows.push(Diag { code, span, arg });
    }

    /// Every row, in append order.
    pub fn rows(&self) -> &[Diag] {
        &self.rows
    }

    /// Row count.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no row was appended.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// `X0001` + the violation's position in `Violation::ALL`; `arg` per variant's doc.
pub const fn violation_code(v: Violation) -> Code {
    Code::new(b'X', v as u16 + 1)
}

/// `S0001` + the class's position in `OpaqueClass::ALL`; `arg` 0. All are notes: a strict read
/// turns `Unread` into its reader's own refusal.
pub const fn opaque_code(class: OpaqueClass) -> Code {
    Code::new(b'S', class as u16 + 1)
}

/// The catalogue row of one of this crate's codes, or `None` for another owner's code.
pub fn info(code: Code) -> Option<Info> {
    let at = (code.number as usize).checked_sub(1)?;
    match code.letter {
        b'X' => Violation::ALL.get(at).map(|&v| {
            let (title, explain) = violation_text(v);
            Info {
                title,
                cite: "rewrite/IR-KERNEL.md §9",
                explain,
                kind: Kind::Refusal,
            }
        }),
        b'S' => OpaqueClass::ALL.get(at).map(|&c| {
            let (title, explain) = opaque_text(c);
            Info {
                title,
                cite: "REWRITE-PLAN.md §3.4",
                explain,
                kind: Kind::Note,
            }
        }),
        b'U' if code == UNSUPPORTED => Some(Info {
            title: "a feature the target cannot spell",
            cite: "REWRITE-PLAN.md §3.5, §4.4",
            explain: "the design uses a feature (arg: its Feature::index) outside the writer's FEATURES and no pass removed it; the writer's citation table says why the target lacks it.",
            kind: Kind::Refusal,
        }),
        b'P' if code == TEXT_SYNTAX => Some(Info {
            title: "a line of IR text that does not parse",
            cite: "crates/ir/src/text.rs (grammar)",
            explain: "the line is skipped and the rest of the text is still read.",
            kind: Kind::Refusal,
        }),
        _ => None,
    }
}

// ---- private ----

/// Title and explanation of each invariant's code.
fn violation_text(v: Violation) -> (&'static str, &'static str) {
    match v {
        Violation::HandleRange => (
            "a handle out of its table's range",
            "a row, list or pool holds a handle that is neither NONE where NONE is allowed nor below its table's length, or a list overruns its pool. The producer of the design wrote a row by hand or kept a stale id.",
        ),
        Violation::ScopeTree => (
            "the scope tree is not a forest of file scopes",
            "a scope reaches no file scope, the parent chain has a cycle, a file scope has a parent or another file, or an item is listed in two scopes.",
        ),
        Violation::ScopeOwner => (
            "a scope's owner does not match it",
            "a non-file scope's owner is of the wrong kind for the scope, does not name the scope as its body, or is not listed in the scope's parent.",
        ),
        Violation::ExprOrder => (
            "an expression child is not below its parent",
            "children are appended before their parent, so a child id at or above its parent's means the expression table was edited in place.",
        ),
        Violation::ExprArity => (
            "an argument count outside its arity",
            "a call or draw has an argument count its function or distribution does not take, or a ternary's then/else pair overruns the expression pool.",
        ),
        Violation::TerminalCount => (
            "an element's terminal count outside its kind's range",
            "the element kind's `terminals()` range does not hold the row's terminal list length.",
        ),
        Violation::DetailRow => (
            "a detail row present when the kind has none, or missing when it has one",
            "an element's `detail` index must name a row of its kind's detail table exactly when the kind has one.",
        ),
        Violation::RefTarget => (
            "a reference bound to the wrong definition",
            "a resolved reference names a definition of a kind its field does not accept or of another name, or a parameter read is bound to a measure outside a measure.",
        ),
        Violation::NonFinite => (
            "a literal whose value is not finite",
            "a decimal literal's value overflows a double; the NaN gate was bypassed.",
        ),
        Violation::EmptyName => (
            "an empty name",
            "a row names something with the empty string; names are non-empty.",
        ),
        Violation::ProbeContext => (
            "a circuit read in a field constant in time",
            "a V, I, Time, Freq, Ddt or Idt appears in an expression field that may not read the circuit (a Handle::Expr: a parameter, a model value, an analysis argument).",
        ),
        Violation::DuplicateDef => (
            "two definitions of one kind and name in one scope",
            "a reader keeps one winner per duplicate by its simulator's rule before it pushes.",
        ),
        Violation::OpaqueEmpty => (
            "an opaque statement without a dialect or text",
            "an Opaque row must name its dialect and keep the statement's text.",
        ),
        Violation::LiteralShape => (
            "a literal that breaks its invariants",
            "a decimal literal's mantissa, fraction digits, exponent or zero form is outside what Literal states.",
        ),
        Violation::FieldDomain => (
            "a row's field outside its domain",
            "a field holds a value its row does not allow, or two fields disagree (a name that must be set, a repeated port, an else branch before the last).",
        ),
        Violation::DuplicateElement => (
            "two elements of one name in one scope",
            "an element's name is its whole name and is unique in its scope across kinds (A13, R23), so a by-name reference names one element.",
        ),
    }
}

/// Title and explanation of each opaque class's code.
fn opaque_text(c: OpaqueClass) -> (&'static str, &'static str) {
    match c {
        OpaqueClass::Unread => (
            "a statement the reader has no rule for",
            "kept as its dialect's text; a strict read refuses it.",
        ),
        OpaqueClass::Private => (
            "a statement only its own simulator has",
            "kept as its dialect's text; only that dialect's writer copies it.",
        ),
        OpaqueClass::Inert => (
            "a statement that changes no number",
            "kept as its dialect's text; a writer of another family leaves it out.",
        ),
        OpaqueClass::InvalidInSource => (
            "a statement its own simulator rejects",
            "kept as its dialect's text with this note.",
        ),
        OpaqueClass::Script => (
            "data-dependent control flow",
            "kept as its dialect's text; a script is not a static run plan.",
        ),
        OpaqueClass::Hint => (
            "a solver hint of its own simulator",
            "kept as its dialect's text; its own family copies it, another family leaves it out: the numbers may move within the solver's tolerance.",
        ),
    }
}
