//! Validity, checked rather than assumed: a design in, one `X` diagnostic per broken invariant
//! out (`check`), and one `U` diagnostic per feature the design uses that a target lacks
//! (`check_legal`). `check` runs after every read and every pass; `check_legal` before every
//! write. A violation is a bug in whoever produced the design, never in the user's deck.
//!
//! Both are total over any table contents (a wrong handle is a diagnostic, not a panic), walk
//! with explicit stacks (C11), and report in table order, so the same design gives the same rows.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, C1–C12; `REWRITE-PLAN.md` §2.2 rule 4.
//!
//! Contents: `pub enum Violation`, `Violation::ALL`, `Violation::COUNT`,
//! `pub fn check(&Design, &mut Diags)`, `pub fn check_legal(&Design, &FeatureSet, &mut Diags)`

use crate::design::Design;
use crate::diag::Diags;
use crate::feature::FeatureSet;

/// One invariant of a well-formed design. Each has its `X` code (`diag::violation_code`); the
/// span is the offending item's (or `Span::NONE` for a non-item row) and `arg` is stated per
/// variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Violation {
    /// A handle in some row, list or pool is neither `NONE` (where `NONE` is allowed) nor in its
    /// table's range, or a list's range overruns its pool. `arg`: the handle's value.
    HandleRange,
    /// The scope tree is not a forest of file scopes: a cycle, a scope that reaches no `File`
    /// scope, a file scope with a parent, or an item in two lists. `arg`: the scope.
    ScopeTree,
    /// A scope's kind does not match its owner's item kind, or the owner's row does not name it
    /// as its body. `arg`: the scope.
    ScopeOwner,
    /// An expression's child id is not lower than its own. `arg`: the parent expression.
    ExprOrder,
    /// A call, draw or operator has an argument count outside its arity, or a `Ternary`'s pool
    /// pair overruns. `arg`: the expression.
    ExprArity,
    /// An element's terminal count is outside its kind's range. `arg`: the element row.
    TerminalCount,
    /// A detail row is missing for a kind that has one, or present for one that has none.
    /// `arg`: the item row.
    DetailRow,
    /// A resolved `Ref` names a definition of a kind the referring field does not accept, or
    /// whose name differs. `arg`: the definition.
    RefTarget,
    /// A `Num` row is NaN or infinite (the NaN gate was bypassed). `arg`: the expression.
    NonFinite,
    /// A name used by a row is empty. `arg`: the name.
    EmptyName,
    /// A `V`, `I`, `Time`, `Freq`, `Ddt` or `Idt` appears in an expression whose field may not
    /// read the circuit (a parameter, a model value, an analysis argument). `arg`: the
    /// expression.
    ProbeContext,
    /// Two definitions of one kind and name in one scope. `arg`: the second definition.
    DuplicateDef,
    /// An `Opaque` has no dialect or empty text. `arg`: the opaque row.
    OpaqueEmpty,
}

impl Violation {
    /// Every invariant, in declaration order (the order of the `X` codes).
    pub const ALL: [Violation; 13] = [
        Violation::HandleRange,
        Violation::ScopeTree,
        Violation::ScopeOwner,
        Violation::ExprOrder,
        Violation::ExprArity,
        Violation::TerminalCount,
        Violation::DetailRow,
        Violation::RefTarget,
        Violation::NonFinite,
        Violation::EmptyName,
        Violation::ProbeContext,
        Violation::DuplicateDef,
        Violation::OpaqueEmpty,
    ];
    /// Invariant count.
    pub const COUNT: usize = Self::ALL.len();
}

/// Appends one `X` row per violation found. Appends nothing for a valid design. O(rows).
pub fn check(design: &Design, diags: &mut Diags) {
    todo!()
}

/// Appends one `UNSUPPORTED` row per item that uses a feature outside `legal` (span: the item;
/// `arg`: `Feature::index`). Assumes `check` passed. O(rows).
pub fn check_legal(design: &Design, legal: &FeatureSet, diags: &mut Diags) {
    todo!()
}
