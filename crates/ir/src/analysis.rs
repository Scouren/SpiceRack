//! Analyses: the closed list of analysis kinds, the analysis header row, and the run owner's detail tables.
//!
//! Owner: run (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-RUN.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-RUN.md`.
//!
//! Contents: `pub enum AnalysisKind` (`ALL`, `COUNT`), `pub struct Analysis`, `pub struct RunDetails`

/// An analysis kind (`rewrite/IR-RUN.md` §2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AnalysisKind {
    /// run: to define (Op, Dc, Ac, Tran, …).
    Placeholder,
}

/// An analysis header row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Analysis {
    /// run: to define (name, detail).
    pub kind: AnalysisKind,
}

/// The run owner's detail tables (the design's `run` field).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunDetails {} // run: to define (one Vec per detail row type)

const _: () = assert!(size_of::<Analysis>() == 1);

impl AnalysisKind {
    /// Every kind, in declaration order.
    pub const ALL: [AnalysisKind; 1] = [AnalysisKind::Placeholder];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

// run: to define — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Analysis);
