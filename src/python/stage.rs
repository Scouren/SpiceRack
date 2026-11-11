//! `spicerack.diagnostics`' native half: a testbench → what its
//! `spicerack.diagnostics/1` file says that no public call returns. The
//! deck written for a dialect beside the source its diagnostics are
//! located in (`_written`), and the simulator a run picks with the
//! language of its deck (`_route`). `python/spicerack/diagnostics.py` runs
//! the stages and writes the files.
//!
//! Cites: none (the file's fields are ZERO D14's).
//!
//! Contents:
//! - `pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()>`

use pyo3::prelude::*;

use super::error::PyDiagnostic;
use super::simulator::PySimulator;

/// `_written`'s answer: the deck's source (`None`: it has none), the deck
/// (`None`: not written), its notes, and the `spicerack.Error` of a deck
/// that was not written.
type Written = (
    Option<String>,
    Option<String>,
    Vec<PyDiagnostic>,
    Option<PyErr>,
);

/// Adds `_written`, `_route` and `_version` (this crate's) to `m`.
pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(written, m)?)?;
    m.add_function(wrap_pyfunction!(route, m)?)?;
    m.add("_version", env!("CARGO_PKG_VERSION"))
}

// ---- private ----

/// `sim`'s deck written for `dialect` (`queued`: the deck `run()` runs,
/// else the one `netlist()` writes), with what its file needs beside the
/// text: the source the diagnostics' lines count in, and a refusal or a
/// failure as a value. Raises only `ValueError`: no dialect is named
/// `dialect`.
#[pyfunction]
#[pyo3(name = "_written")]
fn written(sim: PyRef<'_, PySimulator>, dialect: &str, queued: bool) -> PyResult<Written> {
    let deck = match queued {
        true => sim.deck_as_queued(),
        false => sim.deck.clone(),
    };
    let (wrote, source) = deck.written_from(dialect)?;
    Ok(match wrote {
        Ok(w) => (source, Some(w.text), w.notes, None),
        Err(e) => (source, None, Vec::new(), Some(e)),
    })
}

/// The simulator `sim.run()` picks and the language its deck is written
/// in, by name. Raises `spicerack.Error` as `run()` does before it writes
/// the deck: a routing refusal, a Verilog block that does not lower.
#[pyfunction]
#[pyo3(name = "_route")]
fn route(sim: PyRef<'_, PySimulator>) -> PyResult<(&'static str, &'static str)> {
    let (picked, lang) = sim.deck_as_queued().routed(sim.backend.as_deref())?;
    Ok((picked.name(), lang.name()))
}
