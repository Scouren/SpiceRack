//! One item kind's rows: hot `Copy` rows and their cold spans in two parallel vectors of equal
//! length (C4). Rows are appended by the builder and replaced in place by the editor; a row is
//! never removed (a pass drops an item by taking it out of its scope's item list).
//!
//! Cites: `rewrite/IR-KERNEL.md` C2, C4, C10; `data-oriented-design` rule 3 (hot/cold split).
//!
//! Contents: `pub struct Table<R>`, `Table::len(&self) -> usize`, `Table::is_empty(&self) -> bool`,
//! `Table::get(&self, u32) -> R`, `Table::span(&self, u32) -> Span`, `Table::rows(&self) -> &[R]`,
//! `Table::new() -> Table<R>`, `Table::push(&mut self, R, Span) -> u32`,
//! `Table::set(&mut self, u32, R)`

#[cfg(doc)]
use crate::id::ROWS_MAX;
use crate::span::Span;

/// The rows of one kind. Invariant: `rows.len() == spans.len() <= ROWS_MAX`.
#[derive(Clone, Debug, PartialEq)]
pub struct Table<R> {
    pub(crate) rows: Vec<R>,
    pub(crate) spans: Vec<Span>,
}

impl<R: Copy> Table<R> {
    /// Row count.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// Whether the table has no row.
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// The row at `index`. Asserts `index < len()`.
    pub fn get(&self, index: u32) -> R {
        todo!()
    }

    /// The span of the row at `index`. Asserts `index < len()`.
    pub fn span(&self, index: u32) -> Span {
        todo!()
    }

    /// Every row, in append order (which is not item order: item order is the scope's list).
    pub fn rows(&self) -> &[R] {
        todo!()
    }

    /// An empty table.
    pub(crate) fn new() -> Table<R> {
        todo!()
    }

    /// Appends a row and returns its index. Asserts [`ROWS_MAX`]. The caller (the builder) has
    /// already refused non-finite numbers (C9) and checked handles.
    pub(crate) fn push(&mut self, row: R, span: Span) -> u32 {
        todo!()
    }

    /// Replaces the row at `index`, keeping its span. Asserts `index < len()`.
    pub(crate) fn set(&mut self, index: u32, row: R) {
        todo!()
    }
}

impl<R: Copy> Default for Table<R> {
    fn default() -> Self {
        Self::new()
    }
}
