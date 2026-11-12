//! The SPICE card letter of an element: an element row (and its detail, where the letter depends
//! on it) in, the letter its kind takes on a SPICE card out. A neutral fact shared by the four
//! SPICE-card dialects (ngspice, HSPICE, Xyce, LTspice); since A13 (R23) the IR stores an
//! element's whole name, and a name that does not begin with this letter is the feature
//! `CircuitExtra::NameWithoutLetter`.
//!
//! The letters (upper case): R C L for the passives (a charge-defined capacitor C, a
//! flux-defined inductor L), K a coupling, V I the independent sources, P a port (HSPICE, Xyce;
//! ngspice spells a port as a V source), E G F H the linear controlled sources, and for a
//! polynomial, table, Laplace or frequency-table source the letter of its output and control
//! (V out, V control: E; I out, V control: G; I out, I control: F; V out, I control: H; a table,
//! Laplace or frequency table is voltage-controlled), E a delay, B a behavioural source, D Q M J
//! the diode and transistors, Z a MESFET or IGBT (LTspice), S W the switches, T O U Y P the
//! lines, A a code model, N a module through a model card, X a module named directly and a
//! subcircuit instance; `None` for an unresolved master (its kind is not known).
//!
//! Cites: `rewrite/AMEND-A12.md` A13; `rewrite/RULINGS.md` R23; `rewrite/INV-constructs.md` B16,
//! D1–D47.
//!
//! Contents: `pub fn spice_letter(&Design, &Element) -> Option<u8>`,
//! `pub fn letter_of(ElemKind, Quantity, Quantity) -> Option<u8>`

use crate::controlled::Quantity;
use crate::design::Design;
use crate::element::{ElemKind, Element};

/// The SPICE card letter of `element`, upper case, or `None` for a kind SPICE cards do not
/// have. Reads the detail row of a polynomial, table, Laplace or frequency-table source (its
/// output and control). Total: a detail out of range reads as voltage output and control.
pub fn spice_letter(design: &Design, element: &Element) -> Option<u8> {
    todo!()
}

/// The letter of `kind` with output quantity `out` and control quantity `ctl` (both read only
/// for the kinds whose letter depends on them).
pub fn letter_of(kind: ElemKind, out: Quantity, ctl: Quantity) -> Option<u8> {
    todo!()
}
