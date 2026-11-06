//! The built-in languages, each declared with the public definition API
//! (`syntax::def`) in a file of its own, registered in a fixed order on
//! the first read of the registry (`lang`): a definition that extends or
//! names another (ESPice extends ngspice, Spectre's `spice` region is
//! HSPICE's) comes after it.
//!
//! Cites: none (each definition cites its manual).
//!
//! Contents:
//! - `pub(crate) fn register_builtins()`

mod cdl;
mod espice;
mod hspice;
mod ltspice;
mod ngspice;
mod ngspice_given;
mod spectre;
mod vacask;
mod verilog;
mod xyce;

use crate::syntax::def::register;
use crate::syntax::{Dialect, Lang};

/// Registers every built-in, at its constant handle.
///
/// # Panics
/// When a built-in's definition does not register, or lands at another
/// handle than its constant.
pub(crate) fn register_builtins() {
    let order = [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Ltspice,
        Dialect::Spectre,
        Dialect::Vacask,
        Dialect::Cdl,
        Dialect::Verilog,
    ];
    for d in order {
        let got = match d {
            Dialect::Ngspice => register(ngspice::ngspice()).expect("ngspice registers"),
            Dialect::Espice => register(espice::espice()).expect("espice registers"),
            Dialect::Hspice => register(hspice::hspice()).expect("hspice registers"),
            Dialect::Xyce => register(xyce::xyce()).expect("xyce registers"),
            Dialect::Ltspice => register(ltspice::ltspice()).expect("ltspice registers"),
            Dialect::Spectre => register(spectre::spectre()).expect("spectre registers"),
            Dialect::Vacask => register(vacask::vacask()).expect("vacask registers"),
            Dialect::Cdl => register(cdl::cdl()).expect("cdl registers"),
            Dialect::Verilog => register(verilog::verilog()).expect("verilog registers"),
        };
        assert_eq!(got, Lang::from(d), "{d:?} at its handle");
    }
    let given = register(ngspice_given::ngspice_given()).expect("ngspice-given registers");
    assert_eq!(given, Lang::NGSPICE_GIVEN);
}
