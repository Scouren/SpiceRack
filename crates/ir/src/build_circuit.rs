//! The builder's circuit methods: a reader's or builder's element, waveform and model calls in,
//! header rows, detail rows and model rows out (an `impl Builder` block over the kernel's
//! `Builder::push`). Two tiers: the low tier pushes one detail row (`detail`, `wave`) and one
//! header (`element`); the diagonal tier pushes a family's detail row and its header in one call
//! (`passive`, `source`, …), asserting the kind belongs to the family.
//!
//! What every push asserts (C9, C10, IR-KERNEL §6): handles in range, the terminal count within
//! `ElemKind::terminals`, `detail` present exactly when the kind has one, a master named
//! exactly when `needs_model` (and only when `master_kind` allows one), capacities. A reader
//! turns a card that would fail these into a diagnostic and an `Opaque` before it calls.
//!
//! ```ignore
//! let mut b = design.builder();
//! let (_, top) = b.file("rc.sp", Role::Deck);
//!
//! // `R1 a b 1k` (ngspice reader: the type letter is the kind, the digits as written)
//! let one_k = b.literal(Literal { mantissa: 1, frac: 0, exp10: 3, form: NumForm::Scale });
//! let head = b.head("1", &["a", "b"], None, &[], ExprId::NONE);
//! b.passive(top, ElemKind::Resistor, head, Passive { value: one_k, ..Passive::PLAIN }, span);
//!
//! // `V1 in 0 PULSE(0 1 0 1n 1n 5u 10u)`: no DC given, so ngspice's rule (the t = 0 value)
//! let (zero, one) = (b.num(0.0), b.num(1.0));
//! let (edge, width, period) = (b.num(1e-9), b.num(5e-6), b.num(10e-6));
//! let wave = b.wave(Pulse {
//!     v1: zero, v2: one, delay: zero, rise: edge, fall: edge, width, period,
//!     cycles: ExprId::NONE,
//! });
//! let head = b.head("1", &["in", "0"], None, &[], ExprId::NONE);
//! b.source(top, ElemKind::VSource, head, Source {
//!     dc: zero, ac_mag: ExprId::NONE, ac_phase_deg: ExprId::NONE, tc1: ExprId::NONE,
//!     tc2: ExprId::NONE, pac_mag: ExprId::NONE, pac_phase_deg: ExprId::NONE, wave,
//!     noise_psd: List::EMPTY, parasitics: NO_ROW, port: NO_ROW, distof: NO_ROW,
//! }, span);
//!
//! // `X1 in out div m=2`: the reader binds `div` by its own rule; `m` is the multiplicity
//! let two = b.num(2.0);
//! let div = b.name("div");
//! let master = Ref { name: div, def: div_def };
//! let terminals = b.names(&["in", "out"]);
//! let name = b.name("1");
//! let head = ElemHead { name, terminals, model: master, params: List::EMPTY, mult: two };
//! b.instance(top, ElemKind::Instance, head, span);
//!
//! // `.model dmod d (is=1e-14 n=1.05)`: (type word, level) → family by the reader's table
//! let params = { let is = b.num(1e-14); let n = b.num(1.05); b.params(&[("is", is), ("n", n)]) };
//! let name = b.name("dmod");
//! b.model(top, Model {
//!     name, params, bin_of: NameId::NONE, ako: Ref::unresolved(NameId::NONE),
//!     module: Ref::unresolved(NameId::NONE), version: ExprId::NONE, word: NameId::NONE,
//!     family: ModelFamily::Diode, polarity: Polarity::None,
//! }, span);
//! ```
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §2–§5; `rewrite/IR-KERNEL.md` §6, C9, C10, C13;
//! `REWRITE-PLAN.md` §2.4 (caller code first).
//!
//! Contents:
//! - `pub struct ElemHead`
//! - `Builder::head(&mut self, &str, &[&str], Option<&str>, &[(&str, ExprId)], ExprId) ->
//!   ElemHead`
//! - low tier (with the kernel's `Builder::detail`, A6): `Builder::wave<W: WaveRow>(&mut self, W) -> WaveRef`,
//!   `Builder::element(&mut self, ScopeId, ElemKind, ElemHead, u32, Span) -> ItemRef`
//! - per family (each `-> ItemRef`): `Builder::passive`, `Builder::coupling`,
//!   `Builder::source`, `Builder::controlled`, `Builder::poly`, `Builder::table_source`,
//!   `Builder::behav`, `Builder::laplace`, `Builder::freq_table`, `Builder::delay`,
//!   `Builder::semi`, `Builder::switch`, `Builder::line`, `Builder::instance`
//! - `Builder::model(&mut self, ScopeId, Model, Span) -> ItemRef`

use crate::build::Builder;
use crate::controlled::{Behav, Controlled, Delay, FreqTable, Laplace, Poly, TableSource};
use crate::element::ElemKind;
use crate::id::{ExprId, ItemRef, NameId, NameList, ParamList, Ref, ScopeId};
use crate::line::Line;
use crate::model::Model;
use crate::passive::{Coupling, Passive};
use crate::semi::{Semi, Switch};
use crate::source::Source;
use crate::span::Span;
use crate::wave::{WaveRef, WaveRow};

/// An element's header fields, as a push takes them (the `Element` row without `kind` and
/// `detail`, which the push sets). Built with [`Builder::head`] or field by field by a reader
/// that binds the master itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElemHead {
    /// The element's name, without a type letter.
    pub name: NameId,
    /// Terminals in the kind's role order.
    pub terminals: NameList,
    /// The master; `name` `NONE` when the card names none.
    pub model: Ref,
    /// Instance parameters with no typed field.
    pub params: ParamList,
    /// Multiplicity; `NONE` = 1.
    pub mult: ExprId,
}

const _: () = assert!(size_of::<ElemHead>() == 32);

impl Builder<'_> {
    /// Interns a header's name, terminals and parameters; `master` becomes an unresolved
    /// reference (`None`: no master), bound later by `link` (A4). For builders without their
    /// own binding rule (Python, passes); a dialect reader binds and builds `ElemHead` itself.
    pub fn head(
        &mut self,
        name: &str,
        terminals: &[&str],
        master: Option<&str>,
        params: &[(&str, ExprId)],
        mult: ExprId,
    ) -> ElemHead {
        todo!()
    }

    // ---- low tier ----

    /// Appends a waveform row and returns its reference. Asserts as `detail`.
    pub fn wave<W: WaveRow>(&mut self, row: W) -> WaveRef {
        todo!()
    }

    /// Appends an element header with an already pushed `detail` row (`NO_ROW` for a kind
    /// without one). Asserts the header invariants of the module header, including that
    /// `detail` is in range of `kind`'s table. The diagonal pushes below call this.
    pub fn element(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        detail: u32,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    // ---- one push per family ----

    /// A resistor, capacitor or inductor. Asserts `kind` is one of them.
    pub fn passive(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Passive,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A coupling (no terminals).
    pub fn coupling(
        &mut self,
        scope: ScopeId,
        head: ElemHead,
        row: Coupling,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A voltage source, current source or port. Asserts `kind` is one of them, and that
    /// `row.port` is set exactly on a `Port`.
    pub fn source(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Source,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A `Vcvs`, `Vccs`, `Cccs` or `Ccvs`. Asserts the kind, and that `sense` is set exactly on
    /// the current-controlled two.
    pub fn controlled(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Controlled,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A polynomial source.
    pub fn poly(&mut self, scope: ScopeId, head: ElemHead, row: Poly, span: Span) -> ItemRef {
        todo!()
    }

    /// A table source.
    pub fn table_source(
        &mut self,
        scope: ScopeId,
        head: ElemHead,
        row: TableSource,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A `Behav`, `ChargeCapacitor` or `FluxInductor`. Asserts the kind and that
    /// `ChargeCapacitor` holds `OutKind::Charge`, `FluxInductor` `OutKind::Flux`, and `Behav`
    /// neither.
    pub fn behav(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Behav,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A Laplace source.
    pub fn laplace(&mut self, scope: ScopeId, head: ElemHead, row: Laplace, span: Span) -> ItemRef {
        todo!()
    }

    /// A frequency-table source.
    pub fn freq_table(
        &mut self,
        scope: ScopeId,
        head: ElemHead,
        row: FreqTable,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A delay element.
    pub fn delay(&mut self, scope: ScopeId, head: ElemHead, row: Delay, span: Span) -> ItemRef {
        todo!()
    }

    /// A diode, BJT, MOSFET, JFET, MESFET or IGBT. Asserts the kind.
    pub fn semi(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Semi,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A voltage or current switch. Asserts the kind, and that `sense` is set exactly on an
    /// `ISwitch`.
    pub fn switch(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Switch,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A line element. Asserts the kind.
    pub fn line(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        row: Line,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A kind with no detail row: a code model, a module instance, a subcircuit instance or an
    /// unresolved master. Asserts the kind.
    pub fn instance(
        &mut self,
        scope: ScopeId,
        kind: ElemKind,
        head: ElemHead,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A model; defines `DefKind::Model` `row.name` in `scope`. Asserts `word` is set exactly
    /// for `CodeModel` and `Other`, `module` named exactly for `Module`.
    pub fn model(&mut self, scope: ScopeId, row: Model, span: Span) -> ItemRef {
        todo!()
    }
}
