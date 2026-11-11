//! The built-in languages, each declared with the public definition API
//! (`syntax::def`) in a file of its own, registered in a fixed order on
//! the first read of the registry (`lang`): a definition that extends or
//! names another (ESPice extends ngspice, Spectre's `spice` region is
//! HSPICE's) comes after it.
//!
//! Every per-dialect difference is a declared row here, never a `match`
//! on the dialect in a stage. The capability column is the one most rows
//! change: one call per `Construct` per file
//! (`.refuse(Construct::X, Code::W…)`, `.native`, `.spell(…, Alt::…)`,
//! `.lift(…, Rule::…)`, `.omit`, `.copy`), found by grepping
//! `Construct::X` in the target's file.
//!
//! - Changing a cell edits that one line, and the same call in the
//!   Python mirror `tests/builtin_dialects.py` (the function named for the
//!   dialect, same order): `tests/test_dialect_def.py` compares the two
//!   rows' dumps, so a missed mirror fails pytest.
//! - `espice.rs` and `ngspice_given.rs` extend ngspice (and `spicerack.rs`
//!   ngspice-given): a construct they do not answer takes ngspice's cell.
//!   A cell changed in `ngspice.rs` is ESPice's too unless `espice.rs`
//!   answers it.
//! - A new construct (a `Construct` variant, `render::capability`) needs a
//!   cell in each file that does not extend another (cdl, hspice, ltspice,
//!   ngspice, spectre, vacask, verilog, xyce) and in each mirror; a
//!   missing one fails `register` with `RegisterError::Unanswered`, naming
//!   the language and the construct, at the first read of any language.
//!   Append it at the end of the column under the stream's `// S1`/`// S2`
//!   /`// S3` marker.
//! - A new built-in: a file here, a `Lang` constant at the next handle
//!   (`lang.rs`), one line in `register_builtins` after what it extends.
//!
//! These files are compiled again by `tests/dialect_defs.rs` in another
//! crate, so they name only `pub` items.
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
mod spicerack;
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
    let neutral = register(spicerack::spicerack()).expect("spicerack registers");
    assert_eq!(neutral, Lang::SPICERACK);
}
