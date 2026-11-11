//! PyO3 Python bindings — identical API surface to original SpiceRack.
//!
//! ```python
//! from spicerack import Circuit
//! from spicerack.unit import *
//! ```
//!
//! Every class spells its calls as ngspice cards (`model`); a deck is that
//! text, read by the parser and written for a target by `syntax::render`
//! (`deck`).
//! Python classes and method names → the `spicerack._native` module. Each
//! file's `#[pymethods]` sit below its separator: no Rust caller uses
//! them, and Python reaches them through `add_class` here. A `#[pyclass]`
//! struct carries no `///`: pyo3 would publish it as the class `__doc__`.
//!
//! Files, by owner:
//! - `unit.rs`: `Unit`, `UnitValue`, the `spicerack.unit` submodule
//! - `args.rs`: Python arguments → card fields
//! - `model.rs`: the cards and the blocks that hold them
//! - `source.rs`: `V`/`I` cards and the waveform-source method table
//! - `analysis.rs`, `analysis_region.rs`: the analyses, their ngspice cards, and
//!   what a dialect region says (Spectre statements, a Fourier harmonic count)
//! - `card.rs`: analysis arguments → `Analysis`
//! - `deck.rs`: the blocks → ngspice source → a backend's netlist or a run's `RawData`
//! - `verilog.rs`: `verilog(...)` blocks and their lowering
//! - `subcircuit.rs`, `circuit.rs`: `Subcircuit`, `Circuit`
//! - `block.rs`: `Block`, a branch of a conditional (`Subcircuit.conditional`)
//! - `section.rs`: `Section`, a library section of a `Circuit`
//! - `store.rs`: a subcircuit's source text and stored file (`to_source`, `save`)
//! - `simulator.rs`, `spectre.rs`: `CircuitSimulator` and its Spectre-only analyses
//! - `sims.rs`: `Simulator`, `register_simulator`, `simulators` (the simulator registry)
//! - `testbench.rs`: `Testbench`, `ModelLibrary`
//! - `def_.rs`, `def_enums.rs`, `def_tag.rs`: the dialect-definition API (`spicerack.def_`)
//! - `results.rs`: `RawData`, `OperatingPoint` and the analysis result classes
//! - `export.rs`: `_export`, the native half of `spicerack.export` (results → a raw file's bytes and its sidecar's rows)
//!
//! Contents:
//! - `pub fn spicerack(m: &Bound<'_, PyModule>) -> PyResult<()>` (`#[pymodule]` `_native`)

use pyo3::prelude::*;

// Macro modules first: `#[macro_use]` scopes their macros to the modules declared after them.
#[macro_use]
mod source;
#[macro_use]
mod spectre;
#[macro_use]
mod def_enums;

mod analysis;
mod analysis_region;
mod args;
mod block;
mod card;
mod circuit;
mod deck;
mod def_;
mod def_tag;
mod export;
mod keywords;
mod model;
mod results;
mod section;
mod sims;
mod simulator;
mod store;
mod subcircuit;
mod testbench;
mod unit;
mod verilog;

use circuit::PyCircuit;
use results::PyRawData;
use simulator::PySimulator;
use subcircuit::PySubcircuit;
use testbench::{PyModelLibrary, PyTestbench};
use unit::{PyUnit, PyUnitValue, create_unit_module};

#[pymodule(name = "_native")]
pub fn spicerack(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCircuit>()?;
    m.add_class::<PyUnit>()?;
    m.add_class::<PyUnitValue>()?;
    m.add_class::<PySimulator>()?;
    results::add_classes(m)?;

    // IR types
    m.add_class::<PySubcircuit>()?;
    m.add_class::<PyTestbench>()?;
    m.add_class::<PyModelLibrary>()?;
    m.add_class::<block::PyBlock>()?;
    m.add_class::<section::PySection>()?;

    // Module-level functions
    m.add_function(wrap_pyfunction!(lint, m)?)?;
    m.add_function(wrap_pyfunction!(read_raw, m)?)?;
    export::add(m)?;
    m.add_function(wrap_pyfunction!(compile_veriloga, m)?)?;
    sims::add(m)?;

    create_unit_module(m)?;
    def_::create_def_module(m)?;
    Ok(())
}

// ---- private ----

/// Lint a SPICE netlist for common issues and backend-specific warnings.
///
/// Returns a dict with "warnings" and "errors" lists.
#[pyfunction]
#[pyo3(signature = (netlist, backend=None))]
fn lint<'py>(
    py: Python<'py>,
    netlist: &str,
    backend: Option<&str>,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let r = crate::lint::lint_netlist(netlist, backend);
    let rows = |fs: &[crate::lint::Finding],
                warn: bool|
     -> PyResult<Vec<Bound<'py, pyo3::types::PyDict>>> {
        fs.iter()
            .map(|f| {
                let d = pyo3::types::PyDict::new(py);
                d.set_item("line", f.line)?;
                d.set_item("message", &f.message)?;
                if let Some(s) = &f.suggestion {
                    d.set_item("suggestion", s)?;
                }
                if warn {
                    d.set_item("backends_affected", f.backends)?;
                }
                Ok(d)
            })
            .collect()
    };
    let out = pyo3::types::PyDict::new(py);
    out.set_item("warnings", rows(&r.warnings, true)?)?;
    out.set_item("errors", rows(&r.errors, false)?)?;
    Ok(out)
}

/// Compile a Verilog-A file path or inline source to OSDI with OpenVAF.
///
/// Returns the path to the compiled .osdi file.
///
/// ```python
/// from spicerack import compile_veriloga
///
/// osdi_path = compile_veriloga("models/comparator.va")
/// circuit.osdi(osdi_path)
/// ```
#[pyfunction]
fn compile_veriloga(source_or_path: &str) -> PyResult<String> {
    crate::veriloga::resolve(source_or_path)
        .and_then(|p| crate::veriloga::compile_osdi(&p))
        .map_err(pyo3::exceptions::PyRuntimeError::new_err)
}

/// Every plot of a `.raw` file, in file order (ngspice, ESPice, VACASK,
/// LTspice, Spectre nutbin; binary or ASCII).
///
/// ```python
/// from spicerack import read_raw
///
/// plots = read_raw("out.raw")
/// noise = next(p for p in plots if p.plot_name == "Integrated Noise")
/// ```
#[pyfunction]
fn read_raw(path: &str) -> PyResult<Vec<PyRawData>> {
    let bytes = std::fs::read(path)
        .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{path}: {e}")))?;
    let plots = crate::rawfile::parse_raw_all(&bytes)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("{path}: {e}")))?;
    Ok(plots.into_iter().map(|inner| PyRawData { inner }).collect())
}
