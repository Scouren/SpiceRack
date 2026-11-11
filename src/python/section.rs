//! Section: a library section, `.lib name` … `.endl name`, built from the
//! lines a section holds → one entry of a `Circuit`'s lines given as text
//! (`Circuit.section`). The circuit is then a library file a deck's
//! `.lib file name` selects a section of.
//!
//! Cites: ngspice-45 manual §2.10 (`.LIB libname <...> .ENDL`), §2.11
//! (`.param`), §2.6 (`.subckt`), §2.8 (`.include`); HSPICE command
//! reference `.ENDL (or) .ENDL TT`.
//!
//! Contents:
//! - `pub(super) struct PySection` (Python `Section`), `PySection::text(&self) -> String`

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::args::{kw_params, param_text};
use super::model::model_card;
use super::subcircuit::PySubcircuit;

#[pyclass(name = "Section")]
pub(super) struct PySection {
    name: String,
    /// The section's lines, in call order.
    lines: Vec<String>,
}

impl PySection {
    /// The section as text, `.lib name` through `.endl name` (no newline
    /// after it): what a `Circuit` keeps among its lines given as text.
    pub(super) fn text(&self) -> String {
        assert!(!self.name.is_empty(), "a section is named (`new`)");
        let mut out = format!(".lib {}\n", self.name);
        for l in &self.lines {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(&format!(".endl {}", self.name));
        out
    }
}

// ---- private ----

#[pymethods]
impl PySection {
    /// `Section("tt")`: an empty section called `tt`.
    #[new]
    fn new(name: &str) -> PyResult<Self> {
        // One word: `.lib a b` is a call of section `b` in file `a`.
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(PyValueError::new_err(format!(
                "'{name}' is no section name: one word"
            )));
        }
        Ok(Self {
            name: name.to_string(),
            lines: vec![],
        })
    }

    #[getter]
    fn name(&self) -> &str {
        &self.name
    }

    /// `.param name=value` (the value as `Subcircuit.parameter` takes it).
    fn parameter(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        param_text(name, value).map(|text| self.lines.push(format!(".param {name}={text}")))
    }

    /// A `.model` card, as `Subcircuit.model`.
    #[pyo3(signature = (name, kind, **kwargs))]
    fn model(&mut self, name: &str, kind: &str, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        self.lines.push(model_card(name, kind, &kw_params(kwargs)?));
        Ok(())
    }

    /// `.include path`.
    fn include(&mut self, path: &str) {
        self.lines.push(format!(".include {path}"));
    }

    /// `.lib path section`: another library's section, called from this one.
    fn lib(&mut self, path: &str, section: &str) {
        self.lines.push(format!(".lib {path} {section}"));
    }

    /// `subckt`'s definition, `.subckt` … `.ends`, inside the section (its
    /// own `include` and `lib` lines stay inside the definition).
    /// `ValueError` for one holding a `verilog()` block or a model load
    /// (`osdi`, `veriloga`): a load is the deck's, never a section's.
    fn subcircuit(&mut self, subckt: &PySubcircuit) -> PyResult<()> {
        if !subckt.inner.osdi.is_empty() {
            return Err(PyValueError::new_err(format!(
                "{}: a section carries no model load: call osdi() or veriloga() on the circuit that uses the section",
                subckt.inner.name
            )));
        }
        let def = super::store::definition(&subckt.inner).map_err(PyValueError::new_err)?;
        self.lines.push(def);
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!("Section('{}')", self.name)
    }
}
