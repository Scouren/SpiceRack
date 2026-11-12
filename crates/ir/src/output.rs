//! Outputs as items: what is saved, what is printed, and what is measured. A reader's `.save`,
//! `.probe`, `save`, `.print`/`.plot` and `.meas` in, rows out; the probes they name are rows of
//! `probe.rs`, a measure's typed body a row of `measure.rs`.
//!
//! The save set is always explicit (`INV-semantics.md` 9.03–9.05): a reader expands its
//! simulator's default (ngspice: every node when there is no `.save`; HSPICE: nothing without
//! `.option post`; LTspice: top-level nodes and device currents) and its `all` into `Save` rows
//! of categories, and the union of the `Save` items in force is the set; no later card erases a
//! category. A state checkpoint (HSPICE `.save type=nodeset`, Xyce `.SAVE`, `.DCOP output=`) is
//! not a save: it stays `Opaque` of class `Private`.
//!
//! Cites: `rewrite/IR-RUN.md` §4; `rewrite/INV-constructs.md` H1–H5, H10, H22, H28;
//! `rewrite/INV-semantics.md` 9.03, 9.04, 9.07, 9.08.
//!
//! Contents: `pub struct Save`, `pub struct Print`, `pub enum PrintView`, `pub struct Measure`;
//! `PrintView::ALL`, `PrintView::COUNT`; `RowFacts` for `Save`, `Print` and `Measure`
//! (`todo_facts!` placeholders, A2)

use crate::analysis::AnalysisKind;
use crate::id::NameId;
use crate::measure::MeasureKind;
use crate::probe::ProbeList;

/// Quantities to keep in the results: a member of the deck's save set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Save {
    /// The analysis kind whose results keep them (HSPICE `.probe tran …`); read only when
    /// `every_analysis` is false.
    pub analysis: AnalysisKind,
    /// Whether they are kept for every analysis (`.save`, Spectre and VACASK `save`).
    pub every_analysis: bool,
    /// The quantities, at least one.
    pub probes: ProbeList,
}

/// Quantities to print as a table for one analysis kind (`.print tran v(1)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Print {
    /// The analysis kind the table belongs to.
    pub analysis: AnalysisKind,
    /// Which view of that analysis.
    pub view: PrintView,
    /// The columns, in order, each a separate probe (ngspice reads `print a -b` as `a − b`;
    /// that reader builds one `Expr` probe, `INV-semantics.md` 9.07).
    pub probes: ProbeList,
}

/// Which view of an analysis a print shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrintView {
    /// The analysis's own result (for HB: the spectrum, Xyce `HB_FD`, HSPICE `HB`).
    Main,
    /// HB's time-domain waveform (Xyce `HB_TD`, HSPICE `HBTRAN`).
    HbTime,
}

/// A named measurement over one analysis kind's results. Its body is `detail` in `kind`'s table
/// (`Design::detail`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// The result's name, read by later measures.
    pub name: NameId,
    /// The analysis kind whose results it reads (`.meas tran …`).
    pub analysis: AnalysisKind,
    /// What it computes; names the detail table.
    pub kind: MeasureKind,
    /// Row in the kind's detail table; always present.
    pub detail: u32,
}

const _: () = assert!(size_of::<Save>() == 12);
const _: () = assert!(size_of::<Print>() == 12);
const _: () = assert!(size_of::<Measure>() == 12);

impl PrintView {
    /// Every view, in declaration order.
    pub const ALL: [PrintView; 2] = [PrintView::Main, PrintView::HbTime];
    /// View count.
    pub const COUNT: usize = Self::ALL.len();
}

// run: implementation phase — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Save);
todo_facts!(Print);
todo_facts!(Measure);
