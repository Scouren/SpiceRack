//! Probes: one typed output quantity (a node voltage, a branch or terminal current, a device
//! quantity, a power, an expression, or a category of them), read by `Save`, `Print`, `Measure`
//! and the analyses that name an output. A reader's output spelling in (`v(x1.n)`, `V(X1:n)`,
//! `i(v1)`, `v1#branch`, `IS(M1)`, `I3(M1)`, `M1:s`, `@m1[gm]`, `vdb(out)`), one row out.
//!
//! Paths are name lists, the instance names then the net or element: no separator character and
//! no case rule lives here (`INV-semantics.md` 9.01). A current is positive INTO the device at
//! the terminal it names: `I` is the current entering the element's first terminal (and leaving
//! its second), `ITerminal` the current entering that terminal from the net. A writer whose
//! simulator reports the other sign negates or inserts a 0 V probe (9.02).
//!
//! There is no `All` kind: each simulator's "all" means another set (ngspice: node voltages and
//! voltage-source currents; LTspice: also device currents, but no subcircuit-internal node), so
//! each reader expands its own `all` and its own default save set into the categories it means
//! (9.03–9.05).
//!
//! Cites: `rewrite/IR-RUN.md` §4; `rewrite/INV-constructs.md` H1, H3, H12, H13, H23, H25–H27;
//! `rewrite/INV-semantics.md` 9.01–9.06; `DESIGN-probe-terminals.md` §2–§3.
//!
//! Contents: `pub struct Probe`, `pub enum ProbeKind`, `pub enum AcPart`, `pub type ProbeList`;
//! on each enum: `ALL`, `COUNT`; `Probe::category(ProbeKind) -> Probe`,
//! `Probe::with_part(self, AcPart) -> Probe`, `Design::probes(&self, ProbeList) -> &[Probe]`

use crate::design::Design;
use crate::id::{ExprId, List, NameList};

/// One output quantity. `a`, `b`, `terminal` and `expr` are read per [`ProbeKind`]; every field
/// a kind does not read is empty, `u8::MAX` or `NONE` (checked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// What is probed.
    pub kind: ProbeKind,
    /// Which part of a complex (small-signal) value; `None` for the value itself.
    pub part: AcPart,
    /// `ITerminal`: the terminal's index in the element kind's terminal order (circuit owner's
    /// `ElemKind` doc; HSPICE `I3(M1)` is index 2, ngspice `i(m1:s)` the index of `s`).
    /// `u8::MAX` otherwise.
    pub terminal: u8,
    /// `V`, `Vdiff`: the node path. `I`, `ITerminal`, `Param`, `Power`: the element path.
    /// Empty for the other kinds.
    pub a: NameList,
    /// `Vdiff`: the second node path (the voltage is `a − b`). `Param`: one name, the device
    /// quantity as its source spelled it (`gm`, `id`). Empty otherwise.
    pub b: NameList,
    /// `Expr`: the expression (it may read `V`, `I`, `Time`, `Freq` and measures). `NONE`
    /// otherwise.
    pub expr: ExprId,
}

/// The kind of a [`Probe`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProbeKind {
    /// A node's voltage to ground.
    V,
    /// The voltage between two nodes, `a − b` (Spectre has none: its writer saves both).
    Vdiff,
    /// The element's branch current, positive into its first terminal.
    I,
    /// The current into one terminal of the element (`.probe i(m1:s)`, Xyce `IS(M1)`, Spectre
    /// `M1:s`).
    ITerminal,
    /// A device's operating-point quantity (ngspice `@m1[gm]`, Xyce `N(m1:x)`, VACASK
    /// `p(m1,i_d)`, Spectre `m1:gm`), named as the source spelled it.
    Param,
    /// The power the element absorbs, W (`p(dev)`, Spectre `dev:pwr`).
    Power,
    /// The value of an expression (ngspice `par('…')`, Xyce `{…}`).
    Expr,
    /// Every node voltage, at every depth of the hierarchy.
    AllV,
    /// Every node voltage of the top level only (LTspice's default save set).
    AllVTop,
    /// Every voltage source's branch current.
    AllSourceI,
    /// Every device terminal current (ngspice `.probe alli`, `savecurrents`; Xyce `I(*)`;
    /// Spectre `:currents`).
    AllI,
    /// Every subcircuit pin current (LTspice `Ix(*)`).
    AllPinI,
    /// The output noise of the noise analysis (`onoise`), V/√Hz.
    OutputNoise,
    /// The input-referred noise of the noise analysis (`inoise`).
    InputNoise,
}

/// Which part of a complex value a probe reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AcPart {
    /// The value itself (complex in a small-signal analysis).
    None,
    /// The real part (`vr`).
    Re,
    /// The imaginary part (`vi`).
    Im,
    /// The magnitude (`vm`).
    Mag,
    /// The phase in degrees (HSPICE, Xyce `vp`).
    PhaseDeg,
    /// The phase in radians (ngspice `vp`).
    PhaseRad,
    /// The magnitude in decibels, `20·log10(|x|)` (`vdb`).
    Db,
}

/// An ordered list of probes, a range into the run owner's probe pool.
pub type ProbeList = List<Probe>;

const _: () = assert!(size_of::<Probe>() == 24);

impl Probe {
    /// A category probe (`AllV`, `AllVTop`, `AllSourceI`, `AllI`, `AllPinI`, `OutputNoise`,
    /// `InputNoise`): no path, no expression. Asserts `kind` is one of those.
    pub const fn category(kind: ProbeKind) -> Probe {
        todo!()
    }

    /// The same probe reading another part.
    pub const fn with_part(self, part: AcPart) -> Probe {
        todo!()
    }
}

impl ProbeKind {
    /// Every kind, in declaration order.
    pub const ALL: [ProbeKind; 14] = [
        ProbeKind::V,
        ProbeKind::Vdiff,
        ProbeKind::I,
        ProbeKind::ITerminal,
        ProbeKind::Param,
        ProbeKind::Power,
        ProbeKind::Expr,
        ProbeKind::AllV,
        ProbeKind::AllVTop,
        ProbeKind::AllSourceI,
        ProbeKind::AllI,
        ProbeKind::AllPinI,
        ProbeKind::OutputNoise,
        ProbeKind::InputNoise,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl AcPart {
    /// Every part, in declaration order.
    pub const ALL: [AcPart; 7] = [
        AcPart::None,
        AcPart::Re,
        AcPart::Im,
        AcPart::Mag,
        AcPart::PhaseDeg,
        AcPart::PhaseRad,
        AcPart::Db,
    ];
    /// Part count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The probes of a list. Asserts the range is inside the probe pool.
    pub fn probes(&self, list: ProbeList) -> &[Probe] {
        todo!()
    }
}
