//! Passive elements' typed fields: the detail row of a resistor, capacitor or inductor (value,
//! initial condition and when it applies, temperature laws, noise, parasitics), the parasitic
//! side row, and the coupling row of mutual inductance. Builder pushes in, rows out through the
//! typed accessors `design.passive(&e)`, `design.coupling(&e)`.
//!
//! Every default on which two simulators differ is a field the reader fills (IR-CIRCUIT §1):
//! a valueless Xyce resistor is `value` 1000 (6.09); LTspice's hidden parasitics are written
//! into `Parasitics` (6.05); an R/C/L whose value reads the circuit keeps that expression with
//! the law stated on `value` (6.14, 6.16), never a rewrite into a B source.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §1, §2 (passives, coupling); `rewrite/INV-constructs.md`
//! D1–D10, D52, D53, D55, D56, D79–D82, D85–D87, D99, D104, D112, D114, D115;
//! `rewrite/INV-semantics.md` 6.02–6.18, 6.31–6.33.
//!
//! Contents:
//! - `pub enum IcMode`, `pub enum TcBase`, `pub enum TcLaw` (each `ALL`, `COUNT`)
//! - `pub struct Passive`, `Passive::PLAIN`, `pub struct Parasitics`, `pub struct Coupling`
//! - `Design::passive(&self, &Element) -> Passive`,
//!   `Design::coupling(&self, &Element) -> Coupling`

use crate::design::Design;
use crate::element::{DetailKind, Element, NO_ROW};
use crate::id::{ExprId, NameList};

/// When a device initial condition (`ic=` on a C, L, diode, BJT, MOSFET) applies. Two
/// simulators give the same `ic=` two meanings, so the row states which (IR-CIRCUIT §1;
/// INV-constructs D6; INV-semantics 6.28; REWRITE-PLAN §3.3 `device_ic_in_op`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IcMode {
    /// No initial condition: the row's `ic` is `NONE`.
    None,
    /// Applied only when the transient skips the operating point (`uic`); the operating point
    /// ignores it. ngspice, ESPice, HSPICE.
    UicOnly,
    /// Held during the operating point that starts the transient. Xyce, LTspice.
    InOp,
}

/// Where a behavioural temperature coefficient's ΔT counts from (AMEND-A12 A15 C2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TcBase {
    /// ΔT = T − tnom: the rule of every simulator for a constant-valued element.
    Tnom,
    /// ΔT = T − 27 °C whatever `tnom` is: ngspice's B source and its circuit-valued R/C/L
    /// (`asrcload.c:41`). Only on a row with a behavioural value and a coefficient.
    Fixed27,
}

/// How a circuit-valued capacitor's temperature factor applies (AMEND-A12 A15 C2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TcLaw {
    /// The capacitance is multiplied by 1 + tc1·ΔT + tc2·ΔT².
    Direct,
    /// ngspice: the B source behind a circuit-valued capacitor divides by that polynomial
    /// (`inpcom.c:6747`). Only on a capacitor whose value reads the circuit, with a coefficient.
    Reciprocal,
}

/// The detail row of a resistor, capacitor or inductor (`ElemKind::Resistor`, `Capacitor`,
/// `Inductor`). Every value field is an expression; `NONE` means what its doc says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Passive {
    /// The resistance (Ω), capacitance (F) or inductance (H). `NONE` only when the model gives
    /// it (ngspice `rsh` × squares, a model `r`/`c`); a reader fills its simulator's default
    /// when both are silent (Xyce valueless R is 1000 Ω). An expression that reads the circuit
    /// states the law of a nonlinear element: for a resistor the instantaneous resistance
    /// R(v, i, t) (ng runs it as a B source, D53; lt `R=`, sp `r=` expression, D3, D81); for a
    /// capacitor the incremental capacitance, i = C(v)·dv/dt (ng, hs VCCAP, sp `c=` with
    /// `ctype=0`; INV-semantics 6.16); for an inductor the incremental inductance,
    /// v = L(i)·di/dt. Native: all. Evidence: D1, D4, D9, D52, D104; 6.09, 6.10, 6.14, 6.16.
    pub value: ExprId,
    /// Initial condition: the capacitor's voltage (V) or the inductor's current (A); `NONE`
    /// for none, and always `NONE` on a resistor. Native: ng hs xy lt sp; va has no instance
    /// `ic` (its reader writes the transient's `ic` list). Evidence: D6.
    pub ic: ExprId,
    /// Temperature coefficient 1 (1/K): value × (1 + tc1·ΔT + tc2·ΔT²), ΔT = T_inst − tnom.
    /// `NONE`: the model's coefficient, or 0 without a model. Native: ng xy lt hs (`TC` is an
    /// alias of TC1 there) sp; PSpice/xy/lt `TC=a,b` is read into tc1, tc2. Evidence: 6.12,
    /// D2, D112.
    pub tc1: ExprId,
    /// Temperature coefficient 2 (1/K²), with `tc1`'s law. `NONE`: the model's, or 0.
    /// Evidence: 6.12.
    pub tc2: ExprId,
    /// Exponential temperature coefficient (%/K): value × 1.01^(tce·ΔT); when given it takes
    /// precedence over `tc1`/`tc2` (ng, xy). `NONE`: none. Native: ng xy va lt; hs and sp have
    /// none. Evidence: 6.13, D112.
    pub tce: ExprId,
    /// The instance's absolute temperature (°C), replacing the circuit temperature for this
    /// element. `NONE`: the circuit temperature. Native: ng xy va (`temp=`). Evidence: D2, D112.
    pub temp_abs: ExprId,
    /// Temperature offset from the circuit temperature (K). `NONE`: 0. Native: ng xy va hs (hs
    /// has only the offset; xy and va have both fields, IR-CIRCUIT §2). Evidence: D2, 6.15.
    pub dtemp: ExprId,
    /// A resistor's resistance in AC analyses (Ω), ng `ac=`. `NONE`: `value`. Always `NONE` on
    /// C and L. Native: ng only. Evidence: D2, 6.17.
    pub ac: ExprId,
    /// The ngspice instance scale (dimensionless): it divides a resistor's conductance and
    /// multiplies a capacitor's capacitance and an inductor's inductance (`indtemp.c:79`). `NONE`: 1. Native: ng (Spectre's geometry scale
    /// is folded into `mult` by its reader). Evidence: D2, 6.07.
    pub scale: ExprId,
    /// Row in the parasitics table, or `NO_ROW` for none. Evidence: D8, 6.04, 6.05.
    pub parasitics: u32,
    /// When `ic` applies; `IcMode::None` exactly when `ic` is `NONE`.
    pub ic_mode: IcMode,
    /// Whether the element adds its thermal noise. `false` for lt `noiseless`, ng `noisy=0`,
    /// sp `isnoisy=no`, hs `NOISE=0`, and for an ngspice behavioural R/C/L without `noisy=1`
    /// (6.14). Evidence: D82, 6.17.
    pub noisy: bool,
    /// Where `tc1`/`tc2`'s ΔT counts from; `Tnom` for every constant-valued element.
    pub tc_base: TcBase,
    /// How the temperature factor of a circuit-valued capacitor applies; `Direct` elsewhere.
    pub tc_law: TcLaw,
}

/// LTspice part parasitics, attached to a capacitor, an inductor, a voltage source or
/// (`rpar` only, through `Behav`) a B source. A writer whose target lacks them gets the pass
/// `expand_parasitics` (explicit R/L/C on internal nodes, INV-semantics 6.04). LTspice's hidden
/// values are written here by its reader: Cpar = 1e-6·C·m on a capacitor with `Lser > 0` and no
/// `Cpar`, Rser = 1 mΩ on an uncoupled inductor with no `Rser` (6.05). `NONE` in a field: that
/// parasitic is absent. Which fields a kind may hold: capacitor all five; inductor `rser`,
/// `rpar`, `cpar`; voltage source `rser`, `cpar` (D8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parasitics {
    /// Series resistance (Ω).
    pub rser: ExprId,
    /// Series inductance (H); capacitor only.
    pub lser: ExprId,
    /// Parallel resistance across the terminals (Ω).
    pub rpar: ExprId,
    /// Parallel capacitance across the terminals (F).
    pub cpar: ExprId,
    /// Resistance across the series inductance only (Ω); capacitor only (D87).
    pub rlshunt: ExprId,
}

/// The detail row of a coupling: every pair of the listed inductors is coupled with
/// coefficient `k` (ng K couples one pair per card and expands three or more into every pair,
/// D56; xy and lt take N inductors on one card; sp `mutual_inductor` and va `mutual` one pair).
/// One row states all three (INV-semantics 6.18, 6.33).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coupling {
    /// The coupled inductors, by element name (B17), at least two, distinct, in source order.
    pub inductors: NameList,
    /// The coupling coefficient, dimensionless, |k| ≤ 1; may be an expression of parameters
    /// (D55) or of time (xy, D115). Never `NONE`.
    pub k: ExprId,
}

const _: () = assert!(size_of::<IcMode>() == 1);
const _: () = assert!(size_of::<Passive>() == 44);
const _: () = assert!(size_of::<Parasitics>() == 20);
const _: () = assert!(size_of::<Coupling>() == 12);

impl TcBase {
    /// Every member, in declaration order.
    pub const ALL: [TcBase; 2] = [TcBase::Tnom, TcBase::Fixed27];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl TcLaw {
    /// Every member, in declaration order.
    pub const ALL: [TcLaw; 2] = [TcLaw::Direct, TcLaw::Reciprocal];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl IcMode {
    /// Every member, in declaration order.
    pub const ALL: [IcMode; 3] = [IcMode::None, IcMode::UicOnly, IcMode::InOp];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Passive {
    /// Every optional field absent, `noisy`; `value` is `NONE` and must be set unless the model
    /// gives it (`Passive { value, ..Passive::PLAIN }`).
    pub const PLAIN: Passive = Passive {
        value: ExprId::NONE,
        ic: ExprId::NONE,
        tc1: ExprId::NONE,
        tc2: ExprId::NONE,
        tce: ExprId::NONE,
        temp_abs: ExprId::NONE,
        dtemp: ExprId::NONE,
        ac: ExprId::NONE,
        scale: ExprId::NONE,
        parasitics: NO_ROW,
        ic_mode: IcMode::None,
        noisy: true,
        tc_base: TcBase::Tnom,
        tc_law: TcLaw::Direct,
    };
}

impl Design {
    /// The passive row of a resistor, capacitor or inductor. Asserts the kind.
    pub fn passive(&self, element: &Element) -> Passive {
        assert!(
            element.kind.detail() == DetailKind::Passive,
            "{:?} has no Passive row",
            element.kind
        );
        self.detail(element.detail)
    }

    /// The coupling row of a coupling. Asserts the kind.
    pub fn coupling(&self, element: &Element) -> Coupling {
        assert!(
            element.kind.detail() == DetailKind::Coupling,
            "{:?} has no Coupling row",
            element.kind
        );
        self.detail(element.detail)
    }
}
