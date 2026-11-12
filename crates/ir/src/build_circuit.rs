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
//! ```
//! use spicerack_ir::passive::Passive;
//! use spicerack_ir::source::{DcRule, Source};
//! use spicerack_ir::wave::{GivenZero, Pulse, PulseLaw};
//! use spicerack_ir::{
//!     check, Design, Diags, ElemHead, ElemKind, ExprId, List, Literal, Model, ModelFamily, MultLaw, NameId,
//!     NumForm, Polarity, Ref, Role, Span, NO_ROW,
//! };
//!
//! let mut design = Design::new();
//! let mut b = design.builder();
//! let (_, top) = b.file("rc.sp", Role::Deck);
//! let span = Span::NONE;
//!
//! // `R1 a b 1k` (an ngspice reader: the kind from the letter, the whole name folded, A13)
//! let one_k = b.literal(Literal { mantissa: 1, frac: 0, exp10: 3, form: NumForm::Scale });
//! let head = b.head("r1", &["a", "b"], None, &[], ExprId::NONE);
//! b.passive(top, ElemKind::Resistor, head, Passive { value: one_k, ..Passive::PLAIN }, span);
//!
//! // `V1 in 0 PULSE(0 1 0 1n 1n 5u 10u)`: no DC given, so ngspice's rule (the t = 0 value)
//! let (zero, one) = (b.num(0.0), b.num(1.0));
//! let (edge, width, period) = (b.num(1e-9), b.num(5e-6), b.num(10e-6));
//! let wave = b.wave(Pulse {
//!     v1: zero, v2: one, delay: zero, rise: edge, fall: edge, width, period,
//!     cycles: ExprId::NONE, law: PulseLaw::Spice, given_zero: GivenZero::AsGiven,
//! });
//! let head = b.head("v1", &["in", "0"], None, &[], ExprId::NONE);
//! let source = Source {
//!     dc: zero, ac_mag: ExprId::NONE, ac_phase_deg: ExprId::NONE, tc1: ExprId::NONE,
//!     tc2: ExprId::NONE, pac_mag: ExprId::NONE, pac_phase_deg: ExprId::NONE, wave,
//!     noise_psd: List::EMPTY, parasitics: NO_ROW, port: NO_ROW, distof: NO_ROW,
//!     dc_rule: DcRule::Given,
//! };
//! b.source(top, ElemKind::VSource, head, source, span);
//!
//! // `X1 in out div m=2`: the reader binds `div` by its own rule; `m` is the multiplicity
//! b.subckt(top, "div", &["p", "n"], &[], span);
//! let div_def = b.design().definitions()[0];
//! let two = b.num(2.0);
//! let terminals = b.names(&["in", "out"]);
//! let name = b.name("x1");
//! let master = Ref { name: div_def.name, def: spicerack_ir::DefId(0) };
//! let head = ElemHead {
//!     name, terminals, model: master, params: List::EMPTY, mult: two, mult_law: MultLaw::Parallel,
//! };
//! b.instance(top, ElemKind::Instance, head, span);
//!
//! // `.model dmod d (is=1e-14 n=1.05)`: (type word, level) → family by the reader's table
//! let params = { let is = b.num(1e-14); let n = b.num(1.05); b.params(&[("is", is), ("n", n)]) };
//! let name = b.name("dmod");
//! let none = Ref::unresolved(NameId::NONE);
//! b.model(top, Model {
//!     name, params, bin_of: NameId::NONE, ako: none, module: none, version: ExprId::NONE,
//!     word: NameId::NONE, family: ModelFamily::Diode, polarity: Polarity::None,
//! }, span);
//!
//! let mut diags = Diags::new();
//! check(&design, &mut diags);
//! assert!(diags.is_empty());
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
//! - `Builder::model(&mut self, ScopeId, Model, Span) -> ItemRef`; crate:
//!   `Builder::define_model`

use crate::build::Builder;
use crate::circuit_check::{Classes, model_ok, verdict};
use crate::controlled::{Behav, Controlled, Delay, FreqTable, Laplace, OutKind, Poly, TableSource};
use crate::element::{DetailKind, ElemKind, Element, MultLaw, NO_ROW};
use crate::id::{ExprId, ItemRef, List, NameId, NameList, ParamList, Ref, ScopeId};
use crate::item::DefKind;
use crate::line::Line;
use crate::model::{Model, ModelFamily};
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
    /// The element's whole name (A13), unique in its scope.
    pub name: NameId,
    /// Terminals in the kind's role order.
    pub terminals: NameList,
    /// The master; `name` `NONE` when the card names none.
    pub model: Ref,
    /// Instance parameters with no typed field.
    pub params: ParamList,
    /// Multiplicity; `NONE` = 1.
    pub mult: ExprId,
    /// How `mult` multiplies (A15 C1): `Parallel` unless an `Instance` states ngspice's law.
    pub mult_law: MultLaw,
}

const _: () = assert!(size_of::<ElemHead>() == 36);

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
        let name = self.name(name);
        let terminals = if terminals.is_empty() {
            List::EMPTY
        } else {
            self.names(terminals)
        };
        let model = Ref::unresolved(master.map_or(NameId::NONE, |m| self.name(m)));
        let params = if params.is_empty() {
            List::EMPTY
        } else {
            self.params(params)
        };
        ElemHead {
            name,
            terminals,
            model,
            params,
            mult,
            mult_law: MultLaw::Parallel,
        }
    }

    // ---- low tier ----

    /// Appends a waveform row and returns its reference. Asserts as `detail`.
    pub fn wave<W: WaveRow>(&mut self, row: W) -> WaveRef {
        let index = self.detail(row);
        WaveRef {
            kind: W::KIND,
            index,
        }
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
        let (lo, hi) = kind.terminals();
        let count = head.terminals.len();
        assert!(
            count >= lo as usize && count <= hi as usize,
            "{kind:?}: terminal count {count} outside {lo}..={hi}"
        );
        let named = head.model.name != NameId::NONE;
        assert!(!kind.needs_model() || named, "{kind:?} needs a master");
        assert!(
            kind.master_kind().is_some() || kind == ElemKind::Unresolved || !named,
            "{kind:?} names no master"
        );
        let row = Element {
            kind,
            name: head.name,
            terminals: head.terminals,
            model: head.model,
            params: head.params,
            mult: head.mult,
            detail,
            mult_law: head.mult_law,
        };
        let v = verdict(self.design(), &row, Classes::Assume);
        assert!(
            v.rows,
            "{kind:?}: detail row {detail} missing, misplaced or out of range"
        );
        assert!(v.domain, "{kind:?}: a field is outside its domain");
        self.push(scope, row, span)
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
        assert!(
            kind.detail() == DetailKind::Passive,
            "{kind:?} is not a passive kind"
        );
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
    }

    /// A coupling (no terminals).
    pub fn coupling(
        &mut self,
        scope: ScopeId,
        head: ElemHead,
        row: Coupling,
        span: Span,
    ) -> ItemRef {
        let detail = self.detail(row);
        self.element(scope, ElemKind::Coupling, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::Source,
            "{kind:?} is not a source kind"
        );
        assert!(
            (row.port != NO_ROW) == (kind == ElemKind::Port),
            "a port row belongs exactly to a Port"
        );
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::Controlled,
            "{kind:?} is not a linear controlled kind"
        );
        let current = matches!(kind, ElemKind::Cccs | ElemKind::Ccvs);
        assert!(
            (row.sense != NameId::NONE) == current,
            "{kind:?}: sense set exactly on Cccs/Ccvs"
        );
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
    }

    /// A polynomial source.
    pub fn poly(&mut self, scope: ScopeId, head: ElemHead, row: Poly, span: Span) -> ItemRef {
        let detail = self.detail(row);
        self.element(scope, ElemKind::Poly, head, detail, span)
    }

    /// A table source.
    pub fn table_source(
        &mut self,
        scope: ScopeId,
        head: ElemHead,
        row: TableSource,
        span: Span,
    ) -> ItemRef {
        let detail = self.detail(row);
        self.element(scope, ElemKind::Table, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::Behav,
            "{kind:?} is not a behavioural kind"
        );
        let fits = match kind {
            ElemKind::ChargeCapacitor => row.output == OutKind::Charge,
            ElemKind::FluxInductor => row.output == OutKind::Flux,
            _ => !matches!(row.output, OutKind::Charge | OutKind::Flux),
        };
        assert!(fits, "output {:?} does not fit {kind:?}", row.output);
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
    }

    /// A Laplace source.
    pub fn laplace(&mut self, scope: ScopeId, head: ElemHead, row: Laplace, span: Span) -> ItemRef {
        let detail = self.detail(row);
        self.element(scope, ElemKind::Laplace, head, detail, span)
    }

    /// A frequency-table source.
    pub fn freq_table(
        &mut self,
        scope: ScopeId,
        head: ElemHead,
        row: FreqTable,
        span: Span,
    ) -> ItemRef {
        let detail = self.detail(row);
        self.element(scope, ElemKind::FreqTable, head, detail, span)
    }

    /// A delay element.
    pub fn delay(&mut self, scope: ScopeId, head: ElemHead, row: Delay, span: Span) -> ItemRef {
        let detail = self.detail(row);
        self.element(scope, ElemKind::Delay, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::Semi,
            "{kind:?} is not a semiconductor kind"
        );
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::Switch,
            "{kind:?} is not a switch kind"
        );
        assert!(
            (row.sense != NameId::NONE) == (kind == ElemKind::ISwitch),
            "{kind:?}: sense set exactly on an ISwitch"
        );
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::Line,
            "{kind:?} is not a line kind"
        );
        let detail = self.detail(row);
        self.element(scope, kind, head, detail, span)
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
        assert!(
            kind.detail() == DetailKind::None,
            "{kind:?} has a detail row"
        );
        self.element(scope, kind, head, NO_ROW, span)
    }

    /// A model; defines `DefKind::Model` `row.name` in `scope`, and for the first bin of a group
    /// (`bin_of` not yet defined there) the group's name too, bound to this bin (A15 C4). Asserts `word` is set exactly
    /// for `CodeModel` and `Other`, `module` named exactly for `Module`.
    pub fn model(&mut self, scope: ScopeId, row: Model, span: Span) -> ItemRef {
        let code = matches!(row.family, ModelFamily::CodeModel | ModelFamily::Other);
        assert!(
            (row.word != NameId::NONE) == code,
            "{:?}: word set exactly for CodeModel and Other",
            row.family
        );
        assert!(
            (row.module.name != NameId::NONE) == (row.family == ModelFamily::Module),
            "{:?}: module named exactly for Module",
            row.family
        );
        assert!(
            model_ok(self.design(), &row),
            "{:?}: polarity outside its domain",
            row.family
        );
        let item = self.push(scope, row, span);
        self.define_model(scope, item, row.name, row.bin_of);
        item
    }

    /// Defines a pushed model's name and, for the first bin of a group in `scope`, the group's
    /// name bound to that bin (A15 C4, R33). Shared by `model` and the IR-text reader.
    pub(crate) fn define_model(
        &mut self,
        scope: ScopeId,
        item: ItemRef,
        name: NameId,
        bin_of: NameId,
    ) {
        self.define(DefKind::Model, name, scope, item);
        if bin_of != NameId::NONE
            && !self
                .design
                .def_index
                .contains_key(&(scope, DefKind::Model, bin_of))
        {
            self.define(DefKind::Model, bin_of, scope, item);
        }
    }
}
