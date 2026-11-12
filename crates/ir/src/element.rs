//! Circuit elements: one header row per element (kind, name, terminals, master, parameters,
//! multiplicity) and the detail tables of the kinds that have typed fields. Builder calls in
//! (`build_circuit.rs`), rows out to writers and passes.
//!
//! Owner: circuit (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-CIRCUIT.md` §2). Definition phase
//! stub written by the kernel owner: `Element` is final (IR-KERNEL §6); `ElemKind` and
//! `CircuitDetails` are placeholders the circuit owner replaces.
//!
//! Cites: `rewrite/IR-KERNEL.md` §6; `rewrite/IR-CIRCUIT.md` §2; `rewrite/INV-constructs.md` D.
//!
//! Contents: `pub enum ElemKind` (`ALL`, `COUNT`), `pub struct Element`,
//! `pub struct CircuitDetails`

use crate::controlled::Controlled;
use crate::id::{ExprId, NameId, NameList, ParamList, Ref};
use crate::line::Line;
use crate::passive::Passive;
use crate::source::Source;
use crate::wave::Wave;

/// The kind of an element. Each member's doc states its terminal roles in order, its terminal
/// count range, whether `model` is required, and its detail row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ElemKind {
    /// circuit: to define (the list of IR-KERNEL §6).
    Placeholder,
}

/// One element, every kind (IR-KERNEL §6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element {
    /// What it is.
    pub kind: ElemKind,
    /// Its name, without any type letter.
    pub name: NameId,
    /// Its terminals, in the kind's role order.
    pub terminals: NameList,
    /// Its model or subcircuit master; `def` `NONE` when the card names none or it is
    /// unresolved.
    pub model: Ref,
    /// Instance parameters with no typed field.
    pub params: ParamList,
    /// Multiplicity; `NONE` = 1.
    pub mult: ExprId,
    /// Row in the kind's detail table, or `u32::MAX` when the kind has none.
    pub detail: u32,
}

/// The circuit owner's detail tables (the design's `circuit` field; no spans: a detail row's
/// span is its element's).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CircuitDetails {
    // circuit: to define (one Vec per detail row type)
    pub(crate) passives: Vec<Passive>,
    pub(crate) sources: Vec<Source>,
    pub(crate) waves: Vec<Wave>,
    pub(crate) controlled: Vec<Controlled>,
    pub(crate) lines: Vec<Line>,
}

const _: () = assert!(size_of::<ElemKind>() == 1);
const _: () = assert!(size_of::<Element>() == 40);

impl ElemKind {
    /// Every kind, in declaration order.
    pub const ALL: [ElemKind; 1] = [ElemKind::Placeholder];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

// circuit: to define — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(Element);
