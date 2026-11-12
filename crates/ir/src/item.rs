//! What an item can be: the closed list of item kinds (all three owners'), the scope, definition,
//! file-role, opaque-class and load enums, and the kernel's rows (structure and definitions).
//! The circuit and run owners' rows live in their own files (`element.rs`, `model.rs`,
//! `analysis.rs`, `plan.rs`, `option.rs`, `output.rs`, `stats.rs`).
//!
//! Every row is `Copy`, holds handles and scalars only, and has a size assertion (C2). A row that
//! opens a scope names its body [`ScopeId`]; that scope's `owner` is the item (checked).
//!
//! Cites: `rewrite/IR-KERNEL.md` §5, C2, C3, C12; `REWRITE-PLAN.md` §3.3 (structure,
//! definitions), §3.4 (`Opaque` classes); `rewrite/INV-constructs.md` A, B, C.
//!
//! Contents:
//! - `pub enum ItemKind`, `pub enum ScopeKind`, `pub enum DefKind`, `pub enum Role`,
//!   `pub enum OpaqueClass`, `pub enum LoadKind`; on each: `ALL`, `COUNT`
//! - `pub struct Title`, `pub struct Comment`, `pub struct Include`, `pub struct Global`,
//!   `pub struct Load`, `pub struct Opaque`, `pub struct Subckt`, `pub struct Section`,
//!   `pub struct If`, `pub struct Branch`, `pub struct Param`, `pub struct Func`

use crate::id::{BranchList, ExprId, NameId, NameList, ParamList, ScopeId, TextId};

/// The kind of an item: which table holds its row (C3). The complete list of all three owners.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ItemKind {
    // ---- structure (kernel) ----
    /// The deck's title. Row [`Title`]. Only in a `Deck` file's top scope, first if present.
    Title,
    /// A comment kept as a statement. Row [`Comment`].
    Comment,
    /// Read another file, whole or one section of it. Row [`Include`].
    Include,
    /// Declare nets global to the whole design. Row [`Global`].
    Global,
    /// Load a Verilog-A source or a compiled OSDI binary. Row [`Load`].
    Load,
    /// A statement with no node: kept as its dialect's text. Row [`Opaque`].
    Opaque,
    /// A subcircuit definition; opens a [`ScopeKind::Subckt`] scope. Row [`Subckt`].
    Subckt,
    /// A named library section; opens a [`ScopeKind::Section`] scope. Row [`Section`].
    Section,
    /// A conditional netlist: branches, each a condition and a [`ScopeKind::Branch`] scope.
    /// Row [`If`].
    If,
    // ---- definitions (kernel) ----
    /// A parameter definition. Row [`Param`].
    Param,
    /// A user function definition. Row [`Func`].
    Func,
    // ---- definitions (circuit, run) ----
    /// A device model. Row `model::Model` (circuit).
    Model,
    /// A statistics block (process and mismatch variations). Row `stats::Statistics` (run).
    Statistics,
    // ---- circuit ----
    /// A circuit element of any `ElemKind`. Row `element::Element` (circuit).
    Element,
    // ---- run ----
    /// An analysis. Row `analysis::Analysis` (run).
    Analysis,
    /// A sweep over a range, a list or a table of states; opens a [`ScopeKind::Loop`] scope of
    /// run items. Row `plan::Loop` (run).
    Loop,
    /// The circuit state changes from here on. Row `plan::Alter` (run).
    Alter,
    /// A simulator option. Row `option::OptionRow` (run).
    Option,
    /// Temperature and nominal temperature. Row `option::Env` (run).
    Env,
    /// Initial conditions and node sets. Row `option::NodeSet` (run).
    NodeSet,
    /// What to save. Row `output::Save` (run).
    Save,
    /// What to print. Row `output::Print` (run).
    Print,
    /// A measurement. Row `output::Measure` (run).
    Measure,
}

/// What a scope is; it fixes which item kind owns it (checked).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ScopeKind {
    /// A file's top scope; owner `ItemRef::NONE`, parent `NONE`.
    File,
    /// A subcircuit body; owner a `Subckt` item.
    Subckt,
    /// A section body; owner a `Section` item.
    Section,
    /// One branch of a conditional; owner an `If` item.
    Branch,
    /// A loop body; owner a `Loop` item (run).
    Loop,
}

/// What a definition defines. References name the kind they expect (an instance's master is a
/// `Subckt`, `Model` or `Module`; an expression's parameter is a `Param`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DefKind {
    /// A subcircuit; item a `Subckt`.
    Subckt,
    /// A model (one per bin); item a `Model`.
    Model,
    /// A parameter; item a `Param`, or a `Subckt` for its head defaults.
    Param,
    /// A user function; item a `Func`. Also the def of a formal argument read in its body.
    Func,
    /// A library section; item a `Section`.
    Section,
    /// A Verilog-A module (or an OSDI device); item the `Load` that brings it in.
    Module,
}

/// What a file is to the design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Role {
    /// A deck: line 1 is its title; it may hold analyses and run items.
    Deck,
    /// A library: definitions only, no title.
    Library,
    /// A Verilog-A source: no items; its modules are `Module` definitions of a `Load`.
    VerilogA,
    /// Named by an include or load but not read (not found, refused, or not given).
    Missing,
}

/// Why a statement is `Opaque` (REWRITE-PLAN §3.4). Each class has its `S` code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OpaqueClass {
    /// The reader has no rule for it yet: work to do. A strict read refuses it.
    Unread,
    /// Only its own simulator has it (Xyce `YADC`, HSPICE `.stateye`).
    Private,
    /// Changes no number (listing and display cards).
    Inert,
    /// Its own simulator rejects it.
    InvalidInSource,
    /// Data-dependent control flow (a script).
    Script,
}

/// What a `Load` brings in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LoadKind {
    /// A Verilog-A source (`ahdl_include`, `.hdl`): compiled by the simulator or the backend.
    VerilogA,
    /// A compiled OSDI binary (ngspice `pre_osdi`, VACASK `load`): no source behind it.
    Osdi,
}

/// The deck's title line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Title {
    /// The text as written, without the line break.
    pub text: TextId,
}

/// A comment kept as a statement, in item order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comment {
    /// The comment's text without its comment marker.
    pub text: TextId,
}

/// Read another file in place. The target is the design file whose path is `path`
/// (`Design::find_file`); none means the file was not read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Include {
    /// The target file's path as the design's file table names it (the reader resolves the
    /// spelling relative to the includer); the spelled path when the target is missing.
    pub path: NameId,
    /// The section to read (a library call), or `NONE` for the whole file.
    pub section: NameId,
}

/// Nets global to the whole design.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Global {
    /// The nets, deduplicated, in first-seen order; never ground.
    pub nets: NameList,
}

/// Load a module file. The modules it defines are `DefKind::Module` definitions whose item is
/// this load.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    /// Source or binary.
    pub kind: LoadKind,
    /// The file's path, as for [`Include::path`].
    pub path: NameId,
}

/// A statement with no node, kept as its dialect's text. A writer copies an `Opaque` of its own
/// text family and refuses any other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opaque {
    /// The dialect whose text this is (the registry's name: `ngspice`, `spectre`, …); never
    /// `NONE`.
    pub dialect: NameId,
    /// Why it has no node.
    pub class: OpaqueClass,
    /// The whole statement's text, continuation lines joined as the dialect joins them.
    pub text: TextId,
}

/// A subcircuit definition. Defines `DefKind::Subckt` `name`, and one `DefKind::Param` per head
/// default (item: this subckt) visible in its body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Subckt {
    /// The subcircuit's name.
    pub name: NameId,
    /// Ports in order; may be empty.
    pub ports: NameList,
    /// Head parameter defaults (`PARAMS:`, Spectre `parameters` in the body); value `NONE` for
    /// a parameter declared without a default.
    pub params: ParamList,
    /// Its body, a [`ScopeKind::Subckt`] scope.
    pub body: ScopeId,
}

/// A named library section (`.lib name … .endl`, `section … endsection`). Defines
/// `DefKind::Section` `name`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    /// The section's name.
    pub name: NameId,
    /// The Spectre `library` that wraps it, or `NONE`.
    pub library: NameId,
    /// Its body, a [`ScopeKind::Section`] scope.
    pub body: ScopeId,
}

/// A conditional netlist: the first branch whose condition is non-zero is the netlist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct If {
    /// At least one branch; only the last may have `cond == NONE` (the `else`).
    pub branches: BranchList,
}

/// One branch of an [`If`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Branch {
    /// The condition (non-zero is true), or `NONE` for the `else` branch.
    pub cond: ExprId,
    /// The branch's items, a [`ScopeKind::Branch`] scope.
    pub body: ScopeId,
}

/// A parameter definition, one per name (a card defining several is several items). Defines
/// `DefKind::Param` `name` in its scope. Duplicates were resolved by the reader's own rule: the
/// IR holds the winner only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Param {
    /// The parameter's name.
    pub name: NameId,
    /// Its value; never `NONE`. A string value is an `Expr::Str`.
    pub value: ExprId,
}

/// A user function definition. Defines `DefKind::Func` `name`. In `body`, a formal argument is a
/// `Param` expression whose `Ref.def` is this function's own definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Func {
    /// The function's name.
    pub name: NameId,
    /// Formal arguments in order; distinct.
    pub args: NameList,
    /// The body expression; never `NONE`.
    pub body: ExprId,
}

const _: () = assert!(size_of::<ItemKind>() == 1);
const _: () = assert!(size_of::<Title>() == 4);
const _: () = assert!(size_of::<Comment>() == 4);
const _: () = assert!(size_of::<Include>() == 8);
const _: () = assert!(size_of::<Global>() == 8);
const _: () = assert!(size_of::<Load>() == 8);
const _: () = assert!(size_of::<Opaque>() == 12);
const _: () = assert!(size_of::<Subckt>() == 24);
const _: () = assert!(size_of::<Section>() == 12);
const _: () = assert!(size_of::<If>() == 8);
const _: () = assert!(size_of::<Branch>() == 8);
const _: () = assert!(size_of::<Param>() == 8);
const _: () = assert!(size_of::<Func>() == 16);

impl ItemKind {
    /// Every kind, in declaration order (the order of `Feature::Item` indices).
    pub const ALL: [ItemKind; 23] = [
        ItemKind::Title,
        ItemKind::Comment,
        ItemKind::Include,
        ItemKind::Global,
        ItemKind::Load,
        ItemKind::Opaque,
        ItemKind::Subckt,
        ItemKind::Section,
        ItemKind::If,
        ItemKind::Param,
        ItemKind::Func,
        ItemKind::Model,
        ItemKind::Statistics,
        ItemKind::Element,
        ItemKind::Analysis,
        ItemKind::Loop,
        ItemKind::Alter,
        ItemKind::Option,
        ItemKind::Env,
        ItemKind::NodeSet,
        ItemKind::Save,
        ItemKind::Print,
        ItemKind::Measure,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl ScopeKind {
    /// Every kind, in declaration order.
    pub const ALL: [ScopeKind; 5] = [
        ScopeKind::File,
        ScopeKind::Subckt,
        ScopeKind::Section,
        ScopeKind::Branch,
        ScopeKind::Loop,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl DefKind {
    /// Every kind, in declaration order.
    pub const ALL: [DefKind; 6] = [
        DefKind::Subckt,
        DefKind::Model,
        DefKind::Param,
        DefKind::Func,
        DefKind::Section,
        DefKind::Module,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Role {
    /// Every role, in declaration order.
    pub const ALL: [Role; 4] = [Role::Deck, Role::Library, Role::VerilogA, Role::Missing];
    /// Role count.
    pub const COUNT: usize = Self::ALL.len();
}

impl OpaqueClass {
    /// Every class, in declaration order (the order of the `S` codes).
    pub const ALL: [OpaqueClass; 5] = [
        OpaqueClass::Unread,
        OpaqueClass::Private,
        OpaqueClass::Inert,
        OpaqueClass::InvalidInSource,
        OpaqueClass::Script,
    ];
    /// Class count.
    pub const COUNT: usize = Self::ALL.len();
}

impl LoadKind {
    /// Every kind, in declaration order.
    pub const ALL: [LoadKind; 2] = [LoadKind::VerilogA, LoadKind::Osdi];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}
