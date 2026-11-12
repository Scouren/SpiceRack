//! Simulator state as items: options, the environment (temperature, nominal temperature), and
//! initial conditions and node sets. A reader's `.options`/`.temp`/`.ic`/`.nodeset` (and their
//! Spectre, VACASK and Xyce forms) in, rows out.
//!
//! The environment is always explicit: a reader fills its simulator's defaults (HSPICE 25 °C for
//! both, the others 27 °C), so a deck read from HSPICE and written for ngspice keeps 25 °C
//! (`INV-semantics.md` 4.01). Several `.temp` lines: the reader keeps its simulator's winner;
//! a list of temperatures is a `Loop` over `Target::TEMP` (ngspice runs one temperature only:
//! its reader keeps 27 °C and notes the list, 4.02).
//!
//! Where an initial condition applies is a field, because simulators disagree
//! (`INV-semantics.md` 8.21): ngspice forces a `.ic` node in the transient's operating point
//! only, Xyce also in `.op` and every `.dc` point. A `NodeSet` is legal in a subcircuit body
//! (LTspice, Xyce: `INV-constructs.md` D88, I25): it then applies to every instance, and its
//! `node` path is relative to that body. At the top its path may reach into an instance (9.01).
//! A device's own initial state (VACASK has no capacitor `ic=` parameter: the value is an entry
//! of the transient's `ic` list; Xyce `.INITCOND`) is a `NodeSetKind::Device` row.
//!
//! Cites: `rewrite/IR-RUN.md` §3; `rewrite/INV-constructs.md` I1, I7–I11, I15, I20, I25, G63;
//! D6, D88; `rewrite/INV-semantics.md` 4.01–4.03, 8.21, 9.01.
//!
//! Contents: `pub struct OptionRow`, `pub struct Env`, `pub struct NodeSet`,
//! `pub enum NodeSetKind`, `pub enum IcScope`; on each enum: `ALL`, `COUNT`; `RowFacts` for
//! `OptionRow`, `Env` and `NodeSet` (`todo_facts!` placeholders, A2)

use crate::id::{ExprId, NameId, NameList};
use crate::option_key::OptionKey;

/// One simulator option. Applies to every run item after it (an option set between analyses
/// is a state change: VACASK `options` in `control`, ngspice `option` in a script); a reader of
/// a dialect whose options are deck-global puts its options before the first run item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptionRow {
    /// The portable key, or `OptionKey::Private`.
    pub key: OptionKey,
    /// `Private`: the option's name as its dialect spells it (Xyce: `package.key`, e.g.
    /// `timeint.reltol`). `NONE` for every other key.
    pub private: NameId,
    /// `Private`: the dialect whose option it is (`ngspice`, `xyce`, …; ngspice's compatibility
    /// mode is `ngbehavior` of `ngspice`, read by the backend from here). `NONE` otherwise.
    pub dialect: NameId,
    /// The value in the key's unit; `NONE` for a flag. A string value is an `Expr::Str`.
    pub value: ExprId,
}

/// The circuit temperature and the nominal temperature, °C. Both always present (IR-RUN §3).
/// One per deck, before its first run item; a change between runs is an `Alter` of
/// `Target::TEMP`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// The circuit temperature; never `NONE`. May be an expression of parameters (ngspice
    /// `.temp {t}`).
    pub temp: ExprId,
    /// The temperature model parameters were measured at; never `NONE`.
    pub tnom: ExprId,
}

/// An initial condition or a node set on one node, or on the voltage between two.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSet {
    /// A forced initial condition or a starting guess.
    pub kind: NodeSetKind,
    /// `Ic`: which operating points hold it. `Nodeset`: always `EveryOp` (checked; a guess
    /// serves every solve).
    pub applies: IcScope,
    /// Whether it applies to the next analysis only (VACASK's analysis-line `ic=[…]` and
    /// `nodeset=[…]`), instead of to every analysis after it.
    pub next_only: bool,
    /// The node: the instance path then the net, relative to the item's scope (a subcircuit
    /// body: each instance's own nodes). `Device`: the element's path. Empty only for
    /// `Nodeset` with every node (ngspice `.nodeset all=v`).
    pub node: NameList,
    /// The second node of a voltage across two nodes (VACASK `ic=["1";"2";1.0]`), or empty for
    /// a node voltage to ground; always empty for `Device`.
    pub reference: NameList,
    /// The voltage, V (`Device`: the state in its own unit: a capacitor's voltage, an
    /// inductor's current); never `NONE`. May be an expression of parameters (Xyce
    /// `.ic v(1)={V1}`).
    pub value: ExprId,
}

/// What a [`NodeSet`] does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NodeSetKind {
    /// The node is held at the value while the operating point is solved, then released
    /// (`.ic`, Spectre `ic`, VACASK `ic=`); under UIC it is the transient's starting value.
    Ic,
    /// The value is a starting guess for the operating point (`.nodeset`, `nodeset=`).
    Nodeset,
    /// The element's own state starts at the value (VACASK's transient `ic` entry for a
    /// device, Xyce `.INITCOND`); held per `applies` like `Ic`. An instance's `IC=` parameter is
    /// the circuit owner's field; a reader whose simulator states it here uses this.
    Device,
}

/// Which operating points a `.ic` holds its node in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IcScope {
    /// Only the transient's initial operating point (ngspice: not `.op`, `.dc` or `.ac`).
    Transient,
    /// Every operating point: `.op`, each `.dc` point, the transient's (Xyce).
    EveryOp,
}

const _: () = assert!(size_of::<OptionRow>() == 16);
const _: () = assert!(size_of::<Env>() == 8);
const _: () = assert!(size_of::<NodeSet>() == 24);

impl NodeSetKind {
    /// Every kind, in declaration order.
    pub const ALL: [NodeSetKind; 3] = [NodeSetKind::Ic, NodeSetKind::Nodeset, NodeSetKind::Device];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl IcScope {
    /// Every scope, in declaration order.
    pub const ALL: [IcScope; 2] = [IcScope::Transient, IcScope::EveryOp];
    /// Scope count.
    pub const COUNT: usize = Self::ALL.len();
}

// run: implementation phase — replace each line with the row's `impl RowFacts` (IR-KERNEL A2)
todo_facts!(OptionRow);
todo_facts!(Env);
todo_facts!(NodeSet);
