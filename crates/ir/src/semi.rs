//! Semiconductor and switch instances' typed fields: the few instance fields every
//! semiconductor family shares (area, initial conditions and when they apply, the Newton first
//! guess, instance temperature) and a switch's control and initial state. Everything else on a
//! semiconductor instance is a named parameter of the header (`Element::params`) in its
//! family's own names. Builder pushes in, rows out through `design.semi(&e)`,
//! `design.switch(&e)`.
//!
//! The switching law is the model's family (`ModelFamily::SwitchHyst`, `SwitchLogCubic`,
//! `Relay`, …; INV-semantics 6.19), never a field here.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §2 (semiconductors, switches); `rewrite/INV-constructs.md`
//! D24–D30, D43, D57, D64–D67, D113, D117, D118; `rewrite/INV-semantics.md` 6.19, 6.26–6.28,
//! 6.33.
//!
//! Contents:
//! - `pub enum InitialState` (`ALL`, `COUNT`)
//! - `pub struct Semi`, `pub struct Switch`
//! - `Design::semi(&self, &Element) -> Semi`, `Design::switch(&self, &Element) -> Switch`

use crate::design::Design;
use crate::element::Element;
use crate::id::{ExprId, ExprList, NameId};
use crate::passive::IcMode;

/// A device's first guess for the operating-point Newton iteration: a hint, never a forced
/// state (ng `off`/`on`, sp `region=off`, ng/lt switch `ON`/`OFF`; D43, D57, 6.28). Xyce has
/// no switch state (its reader writes `Unset`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InitialState {
    /// No hint.
    Unset,
    /// Start off.
    Off,
    /// Start on.
    On,
}

/// The detail row of a diode, BJT, MOSFET, JFET, MESFET or IGBT.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Semi {
    /// Area factor (dimensionless), positional on ng D, xy D and J (`D1 a 0 dmod 2`, D64,
    /// D113) or `area=`. `NONE`: 1. A diode's effective area under ng level-3 geometry is the
    /// model family's law over the `w`/`l` parameters, not this field (6.27).
    pub area: ExprId,
    /// Initial conditions in the device's order (diode: vd; BJT: vbe, vce; MOSFET: vds, vgs,
    /// vbs; JFET/MESFET: vds, vgs); `List::EMPTY` for none.
    pub ic: ExprList,
    /// Instance temperature (°C). `NONE`: the circuit temperature. Native: ng xy hs.
    pub temp_abs: ExprId,
    /// Instance temperature offset (K). `NONE`: 0. Native: ng xy hs va.
    pub dtemp: ExprId,
    /// When `ic` applies; `IcMode::None` exactly when `ic` is empty.
    pub ic_mode: IcMode,
    /// The operating point's first guess.
    pub initial: InitialState,
}

/// The detail row of a voltage or current switch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switch {
    /// The sensing element (B17) of an `ISwitch` (W), by name; `NONE` on a `VSwitch`.
    pub sense: NameId,
    /// The control as an expression (xy `CONTROL={expr}`, D118); `NONE` when the control is
    /// the voltage across terminals 3 and 4 (or `sense`'s current). A `VSwitch` with a
    /// control expression has 2 terminals.
    pub control: ExprId,
    /// The operating point's first guess; it seeds the operating point only.
    pub initial: InitialState,
}

const _: () = assert!(size_of::<InitialState>() == 1);
const _: () = assert!(size_of::<Semi>() == 24);
const _: () = assert!(size_of::<Switch>() == 12);

impl InitialState {
    /// Every member, in declaration order.
    pub const ALL: [InitialState; 3] = [InitialState::Unset, InitialState::Off, InitialState::On];
    /// Member count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The row of a diode, BJT, MOSFET, JFET, MESFET or IGBT. Asserts the kind.
    pub fn semi(&self, element: &Element) -> Semi {
        todo!()
    }

    /// The row of a voltage or current switch. Asserts the kind.
    pub fn switch(&self, element: &Element) -> Switch {
        todo!()
    }
}
