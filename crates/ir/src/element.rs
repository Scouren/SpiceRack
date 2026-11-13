//! Circuit elements: the closed list of element kinds and their constant facts (terminal roles
//! and counts, the master a kind names, the detail table it uses), the element header row, and
//! the circuit owner's detail tables. Builder calls in (`build_circuit.rs`), rows out to writers
//! and passes; a kind's typed fields are its detail row, read through the typed accessors in
//! `passive.rs`, `source.rs`, `controlled.rs`, `semi.rs`, `line.rs` and `wave.rs`.
//!
//! Invariants (checked by `RowFacts::check`): the terminal count lies in `ElemKind::terminals`;
//! `detail` is a row of the kind's table exactly when `ElemKind::detail` is not
//! `DetailKind::None`, and [`NO_ROW`] otherwise; a kind whose `master_kind` is `None` names no
//! master (`model.name` `NONE`) unless it is `Unresolved`, which always names one and is never
//! bound. A kind is the meaning, never a letter: `B` on HSPICE, `E … VALUE=` and `bsource` are
//! one `Behav` (INV-constructs B16).
//!
//! Detail rows are reached generically through the kernel's [`DetailRow`]
//! (`design.detail::<Passive>(i)`, `builder.detail(row)`, `editor.set_detail(i, row)`, A6); the
//! impls for every row type of [`CircuitDetails`] are here.
//!
//! Cites: `rewrite/IR-KERNEL.md` §6, C2, C12, Addenda A2 and A4; `rewrite/IR-CIRCUIT.md` §2;
//! `rewrite/INV-constructs.md` D1–D136 (D44 terminal classes, D45 master positions), B8–B10,
//! B16, B17, B24; `rewrite/INV-semantics.md` 6.01, 6.26.
//!
//! Contents:
//! - `pub enum ElemKind` (`ALL`, `COUNT`), `ElemKind::terminals(self) -> (u8, u8)`,
//!   `ElemKind::detail(self) -> DetailKind`, `ElemKind::has_detail(self) -> bool`,
//!   `ElemKind::needs_model(self) -> bool`, `ElemKind::master_kind(self) -> Option<DefKind>`
//! - `pub enum DetailKind` (`ALL`, `COUNT`)
//! - `pub struct Element`, `pub enum MultLaw` (`ALL`, `COUNT`), `pub const NO_ROW`
//! - `pub struct CircuitDetails`; `DetailRow` (kernel, A6) for every row type it holds
//! - `RowFacts` for `Element`; crate: `pub(crate) fn design_features(&Design, &mut FeatureSet)`,
//!   `pub(crate) fn design_check(&Design, &mut Diags)`

use std::collections::HashSet;

use crate::build::Builder;
use crate::check::Violation;
use crate::controlled::{Behav, Controlled, Delay, FreqTable, Laplace, Poly, TableSource};
use crate::design::{Design, DetailRow};
use crate::diag::{Diags, violation_code};
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, ItemRef, NameId, NameList, ParamList, Ref, ScopeId};
use crate::item::{DefKind, ItemKind};
use crate::line::Line;
use crate::passive::{Coupling, Parasitics, Passive};
use crate::semi::{Semi, Switch};
use crate::source::{Distof, PortSpec, Source};
use crate::text::Words;
use crate::wave::{Am, Exp, Pattern, Pulse, Pwl, Sffm, Sin, TrNoise, TrRandom};

/// The kind of an element: one member per device meaning (INV-constructs §D). Each member's doc
/// states its terminal roles in order, its terminal count (`terminals`), whether it names a
/// master and of which definition kind (`needs_model`, `master_kind`), and its detail row
/// (`detail`). Terminal roles are positional in the role order stated here; a reader binds a
/// dialect's node order to these roles (INV-semantics 6.26).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ElemKind {
    /// Resistor (D1, D2, D52, D81, D104). Terminals (p, n), exactly 2. Optional `Model`
    /// master. Detail `Passive`.
    Resistor,
    /// Capacitor (D4, D6, D7, D86, D87). Terminals (p, n), exactly 2. Optional `Model`
    /// master. Detail `Passive`.
    Capacitor,
    /// Inductor (D9, D85). Terminals (p, n), exactly 2. Optional `Model` master. Detail
    /// `Passive`.
    Inductor,
    /// Charge-defined capacitor: i = dq/dt with q an expression of the circuit (D5: ng/hs/es
    /// `q=`, xy `Q={}`, lt `Q=`, sp `bsource q=`, va `i=ddt(q)`). Terminals (p, n), exactly 2.
    /// No master. Detail `Behav` with `OutKind::Charge`.
    ChargeCapacitor,
    /// Flux-defined inductor: v = dφ/dt with φ an expression of the inductor's own current
    /// (D80: lt `L … Flux=`; INV-semantics 6.16, 6.31). Terminals (p, n), exactly 2. No master.
    /// Detail `Behav` with `OutKind::Flux`.
    FluxInductor,
    /// Mutual coupling of inductors (D10, D55, D56, D114, D115). No terminals. Optional
    /// `Model` master (a nonlinear core, xy `K … model`). Detail `Coupling`.
    Coupling,
    /// Independent voltage source (D12, E1–E38). Terminals (p, n), exactly 2; positive branch
    /// current flows into `p`. Optional `Model` master of its builtin family (A12 C5). Detail `Source`.
    VSource,
    /// Independent current source (D13). Terminals (p, n), exactly 2; positive current flows
    /// from `p` through the source to `n`. Optional `Model` master of its builtin family (A12 C5). Detail `Source`.
    ISource,
    /// RF port: a source behind a reference impedance (D15, D97, D111). Terminals (p, n),
    /// exactly 2. No master. Detail `Source` with a `PortSpec` row.
    Port,
    /// Voltage-controlled voltage source (D16, D60, D78). Terminals (out+, out−, ctl+, ctl−),
    /// exactly 4. Optional `Model` master of its builtin family (A12 C5). Detail `Controlled`
    /// (`sense` `NONE`).
    Vcvs,
    /// Voltage-controlled current source (D17). Terminals (out+, out−, ctl+, ctl−), exactly
    /// 4. Optional `Model` master of its builtin family (A12 C5). Detail `Controlled` (`sense`
    /// `NONE`).
    Vccs,
    /// Current-controlled current source (D18). Terminals (out+, out−), exactly 2. Optional `Model`
    /// master of its builtin family (A12 C5). Detail `Controlled` (`sense` the sensing element).
    Cccs,
    /// Current-controlled voltage source (D18). Terminals (out+, out−), exactly 2. Optional `Model`
    /// master of its builtin family (A12 C5). Detail `Controlled` (`sense` the sensing element).
    Ccvs,
    /// Polynomial controlled source (D19, D106). Terminals (out+, out−), exactly 2; the
    /// controls are in the detail. No master. Detail `Poly`.
    Poly,
    /// Piecewise-linear table source (D21, D76, D92, D102). Terminals (out+, out−), exactly
    /// 2. No master. Detail `TableSource`.
    Table,
    /// Behavioural source: an output relation given by an expression of the circuit (D20,
    /// D22, D63, D83, D116, D124, D130). Terminals (out+, out−), exactly 2. No master. Detail
    /// `Behav`.
    Behav,
    /// Laplace transfer source (D23, D91). Terminals (out+, out−, in+, in−), 4; or (out+,
    /// out−), 2, when the detail's `input` is an expression. No master. Detail `Laplace`.
    Laplace,
    /// Frequency-table controlled source (D61; INV-semantics 5.28). Terminals as `Laplace`:
    /// 4, or 2 with an `input` expression. No master. Detail `FreqTable`.
    FreqTable,
    /// Delayed voltage-controlled voltage source (D90, D122 `YDELAY`, D131 sp `delay`;
    /// INV-semantics 5.30). Terminals (out+, out−, in+, in−), exactly 4. No master. Detail
    /// `Delay`.
    Delay,
    /// Diode (D24, D64–D66, D113). Terminals (anode, cathode[, thermal]), 2 to 3 (ng thermal
    /// node, D65). `Model` master required. Detail `Semi`.
    Diode,
    /// Bipolar transistor (D25, D67). Terminals (c, b, e[, s[, t]]), 3 to 5: substrate, then
    /// the thermal node, as the family defines (HICUM, VBIC 5-terminal). `Model` master
    /// required. Detail `Semi`.
    Bjt,
    /// MOSFET (D26, D68, D126). Terminals (d, g, s[, b[, …]]), 3 to 7: bulk, then the
    /// family's extra nodes in its order (BSIMSOI body and back gate, VDMOS thermal `tj tc`).
    /// `Model` master required. Detail `Semi`.
    Mosfet,
    /// Junction FET (D27). Terminals (d, g, s), exactly 3. `Model` master required. Detail
    /// `Semi`.
    Jfet,
    /// MESFET or HFET (D28). Terminals (d, g, s), exactly 3. `Model` master required. Detail
    /// `Semi`.
    Mesfet,
    /// IGBT (D103: lt `Z` on an `NIGBT`/`PIGBT` model). Terminals (c, g, e), exactly 3.
    /// `Model` master required. Detail `Semi`.
    Igbt,
    /// Voltage-controlled switch (D29, D57, D117, D118). Terminals (p, n, ctl+, ctl−), 4; or
    /// (p, n), 2, when the detail's `control` is an expression (xy `CONTROL=`). `Model` master
    /// required. Detail `Switch`.
    VSwitch,
    /// Current-controlled switch (D30). Terminals (p, n), exactly 2. `Model` master required.
    /// Detail `Switch` (`sense` the sensing element).
    ISwitch,
    /// Lossless transmission line (D31, D84, D73). Terminals (a+, a−, b+, b−), exactly 4.
    /// Optional `Model` master of `ModelFamily::TlineIdeal` (VACASK `tline_ideal`, A20). Detail
    /// `Line`.
    LosslessLine,
    /// Lossy line, LTRA (D32, D72). Terminals (a+, a−, b+, b−), exactly 4. `Model` master
    /// required. Detail `Line`.
    LossyLine,
    /// Uniform distributed RC line, URC (D33, D71). Terminals (a, b, c): the two ends and the
    /// capacitive reference, exactly 3. `Model` master required. Detail `Line`.
    RcLine,
    /// Single lossy line, TXL (D34). Terminals (a+, a−, b+, b−), exactly 4. `Model` master
    /// required. Detail `Line`.
    SingleLossyLine,
    /// Coupled lossy lines, CPL (D35, D72). Terminals (in_1 … in_k, in_ref, out_1 … out_k,
    /// out_ref), 2k + 2 for k conductors, 4 to 18 (CPL holds at most 8, D134). `Model` master
    /// required. Detail `Line`.
    CoupledLines,
    /// XSPICE code model instance (D37, D51, D69). Terminals as the code model defines, 0 to
    /// 255 (node groups flattened in the model's port order). `Model` master required (the
    /// model's family is `CodeModel`). No detail.
    CodeModel,
    /// Instance of a Verilog-A/OSDI module through a model card (D39, D74, B24: ng `N` on
    /// `.model m <module>`, va instance of a model of a loaded module). Terminals in the
    /// module's port order, 0 to 255. `Model` master required. No detail.
    ModuleModel,
    /// Instance of a loaded Verilog-A module named directly (B24: sp by module name, hs `X`
    /// of an `.hdl` module). Terminals in the module's port order, 0 to 255. `Module` master
    /// required. No detail.
    Module,
    /// Subcircuit instance (B8, B9). Terminals in the subcircuit's port order, 0 to 255.
    /// `Subckt` master required. No detail.
    Instance,
    /// Instance of a master the reader could not classify (D133: declared in a file that was
    /// not read). Terminals as written, 0 to 255. Names a master that is never bound
    /// (`model.def` stays `NONE`, the linker skips it). No detail.
    Unresolved,
}

/// Which detail table an element kind's `detail` indexes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DetailKind {
    /// The kind has no detail row; `detail` is [`NO_ROW`].
    None,
    /// `passive::Passive`.
    Passive,
    /// `passive::Coupling`.
    Coupling,
    /// `source::Source`.
    Source,
    /// `controlled::Controlled`.
    Controlled,
    /// `controlled::Poly`.
    Poly,
    /// `controlled::TableSource`.
    Table,
    /// `controlled::Behav`.
    Behav,
    /// `controlled::Laplace`.
    Laplace,
    /// `controlled::FreqTable`.
    FreqTable,
    /// `controlled::Delay`.
    Delay,
    /// `semi::Semi`.
    Semi,
    /// `semi::Switch`.
    Switch,
    /// `line::Line`.
    Line,
}

/// "No row" in a `u32` row index of the circuit tables (`Element::detail`, `Passive::parasitics`,
/// `Source::port`, …).
pub const NO_ROW: u32 = u32::MAX;

/// One element, every kind (IR-KERNEL §6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element {
    /// What it is.
    pub kind: ElemKind,
    /// Its whole name as its dialect spells it (A13, R23: ngspice `r1`, `vdd`, `x1`, the letter
    /// included, folded; Spectre `in1`), unique in its scope across kinds, so a by-name
    /// reference (`Expr::I`, a coupling's inductors, a sensing element) names one element. A
    /// name that does not begin with `letter::spice_letter` is `NameWithoutLetter`.
    pub name: NameId,
    /// Its terminals, in the kind's role order.
    pub terminals: NameList,
    /// Its model or subcircuit master; `def` `NONE` when the card names none or it is
    /// unresolved. Expected definition kind: `ElemKind::master_kind`.
    pub model: Ref,
    /// Instance parameters with no typed field, by the master's own names, in source order. On a
    /// kind with a detail row, no key names one of the row's own fields (its IR name, `m` or
    /// `mult`, `temp` where the row has `temp_abs`; A19).
    pub params: ParamList,
    /// Multiplicity: the element stands for `mult` copies in parallel (INV-semantics 6.01–6.03,
    /// 5.16; B10, D7, D127). Composes multiplicatively down the hierarchy. A reader lowers
    /// `m=`, VACASK `$mfactor` and LTspice `x<n>` here, never to a parameter named `m`; a
    /// reader whose simulator ignores it on this kind (ESPice on X, C, L; Xyce on V/I) leaves
    /// `NONE`. `NONE` = 1.
    pub mult: ExprId,
    /// Row in the kind's detail table, or [`NO_ROW`] when the kind has none.
    pub detail: u32,
    /// How `mult` multiplies (AMEND-A12 A15 C1, R31); anything but `Parallel` only on an
    /// `Instance` with a `mult`.
    pub mult_law: MultLaw,
}

/// How an element's multiplicity acts (AMEND-A12 A15 C1, R31).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MultLaw {
    /// `mult` whole copies in parallel; through the hierarchy multiplicities compose by
    /// multiplication (xy, hs, the Python API; INV-semantics 6.01).
    Parallel,
    /// ngspice's `m` on an X line (`inpcom.c:4124-4198`): the instance's `mult` is given to
    /// every element of the subcircuit's body that takes a multiplier, replacing that
    /// element's own, and passed on the same way to nested instances; an element of a kind
    /// ngspice does not multiply is left as it is (IR-CIRCUIT §2 lists the kinds). Only on an
    /// `Instance`.
    NgspiceAppend,
}

/// The circuit owner's detail tables (the design's `circuit` field; no spans: a detail row's
/// span is its element's). One `Vec` per row type, appended by the builder, never shrunk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CircuitDetails {
    pub(crate) passives: Vec<Passive>,
    pub(crate) parasitics: Vec<Parasitics>,
    pub(crate) couplings: Vec<Coupling>,
    pub(crate) sources: Vec<Source>,
    pub(crate) ports: Vec<PortSpec>,
    pub(crate) distofs: Vec<Distof>,
    pub(crate) controlled: Vec<Controlled>,
    pub(crate) polys: Vec<Poly>,
    pub(crate) tables: Vec<TableSource>,
    pub(crate) behavs: Vec<Behav>,
    pub(crate) laplaces: Vec<Laplace>,
    pub(crate) freq_tables: Vec<FreqTable>,
    pub(crate) delays: Vec<Delay>,
    pub(crate) semis: Vec<Semi>,
    pub(crate) switches: Vec<Switch>,
    pub(crate) lines: Vec<Line>,
    pub(crate) pulses: Vec<Pulse>,
    pub(crate) sins: Vec<Sin>,
    pub(crate) exps: Vec<Exp>,
    pub(crate) pwls: Vec<Pwl>,
    pub(crate) sffms: Vec<Sffm>,
    pub(crate) ams: Vec<Am>,
    pub(crate) trnoises: Vec<TrNoise>,
    pub(crate) trrandoms: Vec<TrRandom>,
    pub(crate) patterns: Vec<Pattern>,
    /// The laws the design states (R90, `laws.rs`): one set, not a table.
    pub(crate) laws: crate::laws::Laws,
}

const _: () = assert!(size_of::<ElemKind>() == 1);
const _: () = assert!(size_of::<DetailKind>() == 1);
const _: () = assert!(size_of::<MultLaw>() == 1);
const _: () = assert!(size_of::<Element>() == 40);

macro_rules! detail_rows {
    ($($row:ty => $field:ident;)*) => {
        $(
            impl DetailRow for $row {
                fn rows(design: &Design) -> &[Self] {
                    &design.circuit.$field
                }
                fn rows_mut(design: &mut Design) -> &mut Vec<Self> {
                    &mut design.circuit.$field
                }
            }
        )*
    };
}

detail_rows! {
    Passive => passives;
    Parasitics => parasitics;
    Coupling => couplings;
    Source => sources;
    PortSpec => ports;
    Distof => distofs;
    Controlled => controlled;
    Poly => polys;
    TableSource => tables;
    Behav => behavs;
    Laplace => laplaces;
    FreqTable => freq_tables;
    Delay => delays;
    Semi => semis;
    Switch => switches;
    Line => lines;
    Pulse => pulses;
    Sin => sins;
    Exp => exps;
    Pwl => pwls;
    Sffm => sffms;
    Am => ams;
    TrNoise => trnoises;
    TrRandom => trrandoms;
    Pattern => patterns;
}

impl ElemKind {
    /// Every kind, in declaration order.
    pub const ALL: [ElemKind; 37] = [
        ElemKind::Resistor,
        ElemKind::Capacitor,
        ElemKind::Inductor,
        ElemKind::ChargeCapacitor,
        ElemKind::FluxInductor,
        ElemKind::Coupling,
        ElemKind::VSource,
        ElemKind::ISource,
        ElemKind::Port,
        ElemKind::Vcvs,
        ElemKind::Vccs,
        ElemKind::Cccs,
        ElemKind::Ccvs,
        ElemKind::Poly,
        ElemKind::Table,
        ElemKind::Behav,
        ElemKind::Laplace,
        ElemKind::FreqTable,
        ElemKind::Delay,
        ElemKind::Diode,
        ElemKind::Bjt,
        ElemKind::Mosfet,
        ElemKind::Jfet,
        ElemKind::Mesfet,
        ElemKind::Igbt,
        ElemKind::VSwitch,
        ElemKind::ISwitch,
        ElemKind::LosslessLine,
        ElemKind::LossyLine,
        ElemKind::RcLine,
        ElemKind::SingleLossyLine,
        ElemKind::CoupledLines,
        ElemKind::CodeModel,
        ElemKind::ModuleModel,
        ElemKind::Module,
        ElemKind::Instance,
        ElemKind::Unresolved,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();

    /// The allowed terminal count, `(min, max)` inclusive. A kind whose roles are fixed by its
    /// master (modules, code models, instances) allows 0 to 255; the master's port count is
    /// the reader's and the linker's check, not this table's.
    pub const fn terminals(self) -> (u8, u8) {
        // A real body in the definition phase: this table is the definition.
        match self {
            ElemKind::Coupling => (0, 0),
            ElemKind::Resistor
            | ElemKind::Capacitor
            | ElemKind::Inductor
            | ElemKind::ChargeCapacitor
            | ElemKind::FluxInductor
            | ElemKind::VSource
            | ElemKind::ISource
            | ElemKind::Port
            | ElemKind::Cccs
            | ElemKind::Ccvs
            | ElemKind::Poly
            | ElemKind::Table
            | ElemKind::Behav
            | ElemKind::ISwitch => (2, 2),
            ElemKind::Diode => (2, 3),
            ElemKind::Laplace | ElemKind::FreqTable | ElemKind::VSwitch => (2, 4),
            ElemKind::Jfet | ElemKind::Mesfet | ElemKind::Igbt | ElemKind::RcLine => (3, 3),
            ElemKind::Bjt => (3, 5),
            ElemKind::Mosfet => (3, 7),
            ElemKind::Vcvs
            | ElemKind::Vccs
            | ElemKind::Delay
            | ElemKind::LosslessLine
            | ElemKind::LossyLine
            | ElemKind::SingleLossyLine => (4, 4),
            ElemKind::CoupledLines => (4, 18),
            ElemKind::CodeModel
            | ElemKind::ModuleModel
            | ElemKind::Module
            | ElemKind::Instance
            | ElemKind::Unresolved => (0, 255),
        }
    }

    /// The detail table this kind's `detail` indexes.
    pub const fn detail(self) -> DetailKind {
        match self {
            ElemKind::Resistor | ElemKind::Capacitor | ElemKind::Inductor => DetailKind::Passive,
            ElemKind::ChargeCapacitor | ElemKind::FluxInductor | ElemKind::Behav => {
                DetailKind::Behav
            }
            ElemKind::Coupling => DetailKind::Coupling,
            ElemKind::VSource | ElemKind::ISource | ElemKind::Port => DetailKind::Source,
            ElemKind::Vcvs | ElemKind::Vccs | ElemKind::Cccs | ElemKind::Ccvs => {
                DetailKind::Controlled
            }
            ElemKind::Poly => DetailKind::Poly,
            ElemKind::Table => DetailKind::Table,
            ElemKind::Laplace => DetailKind::Laplace,
            ElemKind::FreqTable => DetailKind::FreqTable,
            ElemKind::Delay => DetailKind::Delay,
            ElemKind::Diode
            | ElemKind::Bjt
            | ElemKind::Mosfet
            | ElemKind::Jfet
            | ElemKind::Mesfet
            | ElemKind::Igbt => DetailKind::Semi,
            ElemKind::VSwitch | ElemKind::ISwitch => DetailKind::Switch,
            ElemKind::LosslessLine
            | ElemKind::LossyLine
            | ElemKind::RcLine
            | ElemKind::SingleLossyLine
            | ElemKind::CoupledLines => DetailKind::Line,
            ElemKind::CodeModel
            | ElemKind::ModuleModel
            | ElemKind::Module
            | ElemKind::Instance
            | ElemKind::Unresolved => DetailKind::None,
        }
    }

    /// Whether the kind has a detail row.
    pub const fn has_detail(self) -> bool {
        !matches!(self.detail(), DetailKind::None)
    }

    /// Whether the element must name a master (`model.name` not `NONE`). For `Unresolved` the
    /// name is required and is never bound.
    pub const fn needs_model(self) -> bool {
        match self {
            ElemKind::Diode
            | ElemKind::Bjt
            | ElemKind::Mosfet
            | ElemKind::Jfet
            | ElemKind::Mesfet
            | ElemKind::Igbt
            | ElemKind::VSwitch
            | ElemKind::ISwitch
            | ElemKind::LossyLine
            | ElemKind::RcLine
            | ElemKind::SingleLossyLine
            | ElemKind::CoupledLines
            | ElemKind::CodeModel
            | ElemKind::ModuleModel
            | ElemKind::Module
            | ElemKind::Instance
            | ElemKind::Unresolved => true,
            ElemKind::Resistor
            | ElemKind::Capacitor
            | ElemKind::Inductor
            | ElemKind::ChargeCapacitor
            | ElemKind::FluxInductor
            | ElemKind::Coupling
            | ElemKind::VSource
            | ElemKind::ISource
            | ElemKind::Port
            | ElemKind::Vcvs
            | ElemKind::Vccs
            | ElemKind::Cccs
            | ElemKind::Ccvs
            | ElemKind::Poly
            | ElemKind::Table
            | ElemKind::Behav
            | ElemKind::Laplace
            | ElemKind::FreqTable
            | ElemKind::Delay
            | ElemKind::LosslessLine => false,
        }
    }

    /// The definition kind the header's `model` must bind to, or `None` when the kind names no
    /// master (then `model.name` is `NONE`) or is `Unresolved` (named, never linked).
    /// `RowFacts::map_refs` passes `model` with this kind, so the linker binds by it.
    pub const fn master_kind(self) -> Option<DefKind> {
        match self {
            ElemKind::Instance => Some(DefKind::Subckt),
            ElemKind::Module => Some(DefKind::Module),
            ElemKind::Resistor
            | ElemKind::Capacitor
            | ElemKind::Inductor
            | ElemKind::Coupling
            | ElemKind::Diode
            | ElemKind::Bjt
            | ElemKind::Mosfet
            | ElemKind::Jfet
            | ElemKind::Mesfet
            | ElemKind::Igbt
            | ElemKind::VSwitch
            | ElemKind::ISwitch
            | ElemKind::LossyLine
            | ElemKind::RcLine
            | ElemKind::SingleLossyLine
            | ElemKind::CoupledLines
            | ElemKind::CodeModel
            | ElemKind::ModuleModel
            | ElemKind::VSource
            | ElemKind::ISource
            | ElemKind::Vcvs
            | ElemKind::Vccs
            | ElemKind::Cccs
            | ElemKind::Ccvs
            | ElemKind::LosslessLine => Some(DefKind::Model),
            ElemKind::ChargeCapacitor
            | ElemKind::FluxInductor
            | ElemKind::Port
            | ElemKind::Poly
            | ElemKind::Table
            | ElemKind::Behav
            | ElemKind::Laplace
            | ElemKind::FreqTable
            | ElemKind::Delay
            | ElemKind::Unresolved => None,
        }
    }
}

impl MultLaw {
    /// Every member, in declaration order.
    pub const ALL: [MultLaw; 2] = [MultLaw::Parallel, MultLaw::NgspiceAppend];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl DetailKind {
    /// Every member, in declaration order.
    pub const ALL: [DetailKind; 14] = [
        DetailKind::None,
        DetailKind::Passive,
        DetailKind::Coupling,
        DetailKind::Source,
        DetailKind::Controlled,
        DetailKind::Poly,
        DetailKind::Table,
        DetailKind::Behav,
        DetailKind::Laplace,
        DetailKind::FreqTable,
        DetailKind::Delay,
        DetailKind::Semi,
        DetailKind::Switch,
        DetailKind::Line,
    ];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl RowFacts for Element {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        crate::circuit_text::element_handles(self, design, out)
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // A real body: A4's contract for this row. A `NONE` name is no reference; the
        // unresolved kind has no `master_kind`, so the linker never sees it.
        let mut row: Element = design.row(at.index);
        if let Some(kind) = row.kind.master_kind()
            && row.model.name != NameId::NONE
        {
            f(&mut row.model, kind);
        }
        design.editor().set(at.index, row);
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        crate::circuit_fields::element_map_exprs(design, at.index, f)
    }

    fn map_element_names(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut [NameId])) {
        crate::circuit_fields::element_map_names(design, at.index, f)
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        crate::circuit_features::element_features(self, design, set)
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        crate::circuit_check::check_element(self, design, at, out)
    }

    fn dump(&self, design: &Design, ctx: &crate::text_expr::Ctx, out: &mut String) {
        crate::circuit_text::dump_element(self, design, ctx, out)
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        crate::circuit_parse::parse_element(words, scope, b, diags)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        crate::circuit_gen::generate_element(g, scope, b, features)
    }
}

/// The circuit owner's features of the whole design, which no single row carries (A9). Called
/// once by `features_used` and by `check_legal`. Empty until the circuit owner fills it.
pub(crate) fn design_features(design: &Design, set: &mut FeatureSet) {
    // one design-level circuit feature: a model with no library of its family (A12 C5)
    crate::circuit_features::library_features(design, set);
}

/// The circuit owner's design-level invariants, which no single row can see (A13): appends one
/// `Violation::DuplicateElement` per element whose name an earlier element of the same scope
/// already has (`arg`: the later element's row). Called once by `check`. One pass per scope;
/// the map is lookup only, so the rows come in item order.
pub(crate) fn design_check(design: &Design, out: &mut Diags) {
    let mut seen: HashSet<NameId> = HashSet::new();
    // bounded: every scope once, every item of its list once
    for scope in 0..design.scopes.len() {
        seen.clear();
        for &item in &design.scope_items[scope] {
            if item.kind != ItemKind::Element {
                continue;
            }
            let element: Element = design.row(item.index);
            if !seen.insert(element.name) {
                out.push_at(
                    violation_code(Violation::DuplicateElement),
                    design,
                    item,
                    item.index,
                );
            }
        }
    }
}
