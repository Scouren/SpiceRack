//! Diagnostics as rows: every stage appends `Diag { code, span, arg }` and keeps going; nothing
//! here prints, and nothing returns early on a diagnostic (C14). Rendering happens once, in the
//! root crate, sorted by position, from the assembled catalogue.
//!
//! This crate's own catalogue: `X` (an invariant violation: a bug in a reader, pass or builder,
//! never the user's deck; one per `Violation`), `U` (a feature the target lacks; `arg` is the
//! `Feature::index`), `S` (a statement read but not representable; one per `OpaqueClass`), and
//! `P9001..=P9099` for the IR text reader (`text::parse`), `R9001..=R9099` for the linker
//! (`link`). Every
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
//! - `pub const UNSUPPORTED: Code`, `pub const LINK_UNBOUND: Code`, `pub const TEXT_SYNTAX: Code`,
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

/// A reference `link` left unresolved: `R9001`, a note (the kit may be absent); `arg` the
/// `NameId`. The kernel linker's range is `R9001..=R9099`.
pub const LINK_UNBOUND: Code = Code::new(b'R', 9001);

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
        todo!()
    }
}

impl Diags {
    /// An empty list.
    pub fn new() -> Diags {
        todo!()
    }

    /// Appends one row.
    pub fn push(&mut self, code: Code, span: Span, arg: u32) {
        todo!()
    }

    /// Every row, in append order.
    pub fn rows(&self) -> &[Diag] {
        todo!()
    }

    /// Row count.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// Whether no row was appended.
    pub fn is_empty(&self) -> bool {
        todo!()
    }
}

/// `X0001` + the violation's position in `Violation::ALL`; `arg` per variant's doc.
pub const fn violation_code(v: Violation) -> Code {
    todo!()
}

/// `S0001` + the class's position in `OpaqueClass::ALL`; `arg` 0. All are notes: a strict read
/// turns `Unread` into its reader's own refusal.
pub const fn opaque_code(class: OpaqueClass) -> Code {
    todo!()
}

/// The catalogue row of one of this crate's codes, or `None` for another owner's code.
pub fn info(code: Code) -> Option<Info> {
    todo!()
}
