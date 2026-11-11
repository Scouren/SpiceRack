//! Results: a `RawData` → the Python result classes (`RawData`,
//! `OperatingPoint`, one class per swept analysis), and an `Analysis` →
//! which of them its run returns.
//!
//! Contents:
//! - `pub(super) struct PyRawData` (Python `RawData`)
//! - `pub(super) struct PyOperatingPoint` (Python `OperatingPoint`)
//! - `PyDcAnalysis`, `PyAcAnalysis`, `PyTransientAnalysis`, `PyNoiseAnalysis`,
//!   `PyTransferFunctionAnalysis`, `PySensitivityAnalysis`, `PyPoleZeroAnalysis`,
//!   `PyDistortionAnalysis`, `PyPssAnalysis`, `PySParamAnalysis`,
//!   `PyHarmonicBalanceAnalysis`, `PyStabilityAnalysis`, `PyTransientNoiseAnalysis`:
//!   `pub(super) struct`s from `analysis_class!`
//! - `pub(super) fn add_classes(m: &Bound<'_, PyModule>) -> PyResult<()>`
//! - `pub(super) fn typed_result(py: Python<'_>, a: &Analysis, plots: Vec<RawData>) -> PyResult<Py<PyAny>>`

use std::collections::HashMap;

use pyo3::exceptions::{PyAttributeError, PyKeyError};
use pyo3::prelude::*;
use pyo3::types::PyList;

use super::analysis::Analysis;
use crate::result::{RawData, Results};

/// Raw simulation data wrapper -- returned by spectre_sweep and spectre_montecarlo.
#[pyclass(name = "RawData")]
pub(super) struct PyRawData {
    pub(super) inner: crate::result::RawData,
}

#[pyclass(name = "OperatingPoint")]
pub(super) struct PyOperatingPoint {
    pub(super) inner: crate::result::Results,
}

/// Every swept analysis is the same `result::Results`; each Python class differs only
/// in its name and the name of its axis getter (`sweep`, `frequency`, `time`).
macro_rules! analysis_class {
    ($ty:ident, $name:literal $(, $axis:ident)?) => {
        #[pyclass(name = $name)]
        pub(super) struct $ty {
            pub(super) inner: crate::result::Results,
        }

        #[pymethods]
        impl $ty {
            fn __getitem__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyList>> {
                let col = self.inner.get(name).ok_or_else(|| PyKeyError::new_err(format!("Node '{}' not found", name)))?;
                float_list(py, &col)
            }

            fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyList>> {
                let col = self.inner.get(name).ok_or_else(|| PyAttributeError::new_err(format!("No node '{}'", name)))?;
                float_list(py, &col)
            }

            $(
                #[getter]
                fn $axis<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> { float_list(py, &self.inner.axis()) }
            )?

            #[getter]
            fn measures(&self) -> HashMap<String, f64> { measures_to_dict(&self.inner.raw.measures) }

            /// The `.step` point this result is: {parameter: value}, empty
            /// when nothing steps.
            #[getter]
            fn params(&self) -> HashMap<String, f64> { self.inner.raw.params.iter().cloned().collect() }

            #[getter]
            fn nodes(&self) -> Vec<String> { self.inner.nodes.keys().cloned().collect() }

            #[getter]
            fn branches(&self) -> Vec<String> { self.inner.branches.keys().cloned().collect() }

            /// |H| per point. `result[node]` returns the real part, not this.
            fn magnitude(&self, name: &str) -> PyResult<Vec<f64>> {
                complex_part(&self.inner, name, |v| v.norm())
            }

            /// 20*log10(|H|) per point.
            fn magnitude_db(&self, name: &str) -> PyResult<Vec<f64>> {
                complex_part(&self.inner, name, |v| 20.0 * v.norm().log10())
            }

            /// Phase in degrees (ngspice's own `vp()` reports radians; this does not).
            fn phase(&self, name: &str) -> PyResult<Vec<f64>> {
                complex_part(&self.inner, name, |v| v.arg().to_degrees())
            }
        }
    };
}

/// The analysis classes and [`add_classes`], which registers them: a new
/// result class is one row here.
macro_rules! analysis_classes {
    ($(($ty:ident, $name:literal $(, $axis:ident)?)),* $(,)?) => {
        $(analysis_class!($ty, $name $(, $axis)?);)*

        /// Adds `OperatingPoint`, every analysis class and `RawData` to
        /// the `_native` module.
        pub(super) fn add_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_class::<PyOperatingPoint>()?;
            $(m.add_class::<$ty>()?;)*
            m.add_class::<PyRawData>()
        }
    };
}

analysis_classes! {
    (PyDcAnalysis, "DcAnalysis", sweep),
    (PyAcAnalysis, "AcAnalysis", frequency),
    (PyTransientAnalysis, "TransientAnalysis", time),
    (PyNoiseAnalysis, "NoiseAnalysis", frequency),
    (PyTransferFunctionAnalysis, "TransferFunctionAnalysis"),
    (PySensitivityAnalysis, "SensitivityAnalysis"),
    (PyPoleZeroAnalysis, "PoleZeroAnalysis"),
    (PyDistortionAnalysis, "DistortionAnalysis", frequency),
    (PyPssAnalysis, "PssAnalysis", time),
    (PySParamAnalysis, "SParamAnalysis", frequency),
    (PyHarmonicBalanceAnalysis, "HarmonicBalanceAnalysis", frequency),
    (PyStabilityAnalysis, "StabilityAnalysis", frequency),
    (PyTransientNoiseAnalysis, "TransientNoiseAnalysis", time),
}

/// `plots`, analysis `a`'s run, as its single-analysis method returns it:
/// the first plot as `a`'s result class, with the vectors of a plot behind
/// it (a noise run's "Integrated Noise": `onoise_total`, `inoise_total`,
/// one point each) as further vectors, `None` for none (a Fourier's); for
/// a run read one plot per step (their `params` set: LTspice), a list of
/// one result per step, in step order (ZERO D42).
pub(super) fn typed_result(
    py: Python<'_>,
    a: &Analysis,
    plots: Vec<RawData>,
) -> PyResult<Py<PyAny>> {
    if plots.iter().all(|p| p.params.is_empty()) {
        return one_result(py, a, joined(plots));
    }
    let each: Vec<Py<PyAny>> = plots
        .into_iter()
        .map(|p| one_result(py, a, Some(p)))
        .collect::<PyResult<_>>()?;
    Ok(PyList::new(py, each)?.into_any().unbind())
}

// ---- private ----

/// An unstepped analysis's plots as one: the first, with every vector of
/// the plots behind it as further columns (a noise run's "Integrated
/// Noise": `onoise_total` and `inoise_total`, one point each beside the
/// spectra's many). `None` for no plot.
fn joined(plots: Vec<RawData>) -> Option<RawData> {
    let mut plots = plots.into_iter();
    let mut first = plots.next()?;
    for more in plots {
        // A column's index is its place in `real_data`: real plots both.
        assert_eq!(first.real_data.len(), first.variables.len(), "a real plot");
        assert_eq!(more.real_data.len(), more.variables.len(), "a real plot");
        for (v, column) in more.variables.into_iter().zip(more.real_data) {
            let index = first.variables.len();
            first.variables.push(crate::result::VarInfo { index, ..v });
            first.real_data.push(column);
        }
    }
    Some(first)
}

/// `raw` as `a`'s result class; `None` as Python's `None`.
fn one_result(py: Python<'_>, a: &Analysis, raw: Option<RawData>) -> PyResult<Py<PyAny>> {
    let Some(raw) = raw else { return Ok(py.None()) };
    let r = || Results::from_raw(raw);
    Ok(match a {
        Analysis::Op => Py::new(py, PyOperatingPoint { inner: r() })?.into_any(),
        Analysis::Dc { .. } => Py::new(py, PyDcAnalysis { inner: r() })?.into_any(),
        Analysis::Ac { .. } | Analysis::SpectrePac { .. } | Analysis::SpectrePxf { .. } => {
            Py::new(py, PyAcAnalysis { inner: r() })?.into_any()
        }
        Analysis::Transient { .. } | Analysis::Fourier { .. } => {
            Py::new(py, PyTransientAnalysis { inner: r() })?.into_any()
        }
        Analysis::Noise { .. } | Analysis::SpectrePnoise { .. } => {
            Py::new(py, PyNoiseAnalysis { inner: r() })?.into_any()
        }
        Analysis::Tf { source, .. } => Py::new(
            py,
            PyTransferFunctionAnalysis {
                inner: tf_of(r(), source),
            },
        )?
        .into_any(),
        Analysis::Sensitivity { .. } => {
            Py::new(py, PySensitivityAnalysis { inner: r() })?.into_any()
        }
        Analysis::PoleZero { .. } => Py::new(py, PyPoleZeroAnalysis { inner: r() })?.into_any(),
        Analysis::Distortion { .. } => Py::new(py, PyDistortionAnalysis { inner: r() })?.into_any(),
        Analysis::Pss { .. } => Py::new(py, PyPssAnalysis { inner: r() })?.into_any(),
        Analysis::SPar { .. } => Py::new(py, PySParamAnalysis { inner: r() })?.into_any(),
        Analysis::HarmonicBalance { .. } => {
            Py::new(py, PyHarmonicBalanceAnalysis { inner: r() })?.into_any()
        }
        Analysis::Stability { .. } | Analysis::SpectrePstb { .. } => {
            Py::new(py, PyStabilityAnalysis { inner: r() })?.into_any()
        }
        Analysis::TransientNoise { .. } => {
            Py::new(py, PyTransientNoiseAnalysis { inner: r() })?.into_any()
        }
        Analysis::SpectreSweep { .. } | Analysis::SpectreMonteCarlo { .. } => {
            Py::new(py, PyRawData { inner: r().raw })?.into_any()
        }
    })
}

/// A `.tf` result that answers the bare names with `source`'s own vectors:
/// a plot holding one `transfer_function(<source>)` and
/// `input_impedance(<source>)` per source of the deck (VACASK's `dcxf`)
/// reads `transfer_function` and `input_impedance` as the analysis's
/// source's. A plot with the bare names already (ngspice, ESPice) is `r`.
fn tf_of(mut r: Results, source: &str) -> Results {
    let source = source.to_lowercase();
    for name in ["transfer_function", "input_impedance"] {
        if let Some(&i) = r.nodes.get(&format!("{name}({source})")) {
            r.nodes.insert(name.to_string(), i);
        }
    }
    assert!(r.nodes.len() + r.branches.len() <= r.raw.variables.len() + 2);
    r
}

#[pymethods]
impl PyRawData {
    #[getter]
    fn title(&self) -> String {
        self.inner.title.clone()
    }
    #[getter]
    fn plot_name(&self) -> String {
        self.inner.plot_name.clone()
    }
    #[getter]
    fn is_complex(&self) -> bool {
        self.inner.is_complex
    }
    #[getter]
    fn variable_names(&self) -> Vec<String> {
        self.inner
            .variables
            .iter()
            .map(|v| v.name.clone())
            .collect()
    }
    fn __getitem__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyList>> {
        float_list(py, &self.inner.re(self.column(name)?))
    }
    fn __getattr__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyList>> {
        self.__getitem__(py, name)
    }
    /// Variable `name` as complex numbers; a real column comes back with zero imaginary parts.
    fn complex<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyList>> {
        let i = self.column(name)?;
        match self.inner.complex(i) {
            Some(c) => PyList::new(py, c),
            None => PyList::new(
                py,
                self.inner
                    .re(i)
                    .iter()
                    .map(|&x| num_complex::Complex64::new(x, 0.0)),
            ),
        }
    }
    #[getter]
    fn measures(&self) -> HashMap<String, f64> {
        measures_to_dict(&self.inner.measures)
    }
    /// The `.step` point: {parameter: value}, empty when nothing steps.
    #[getter]
    fn params(&self) -> HashMap<String, f64> {
        self.inner.params.iter().cloned().collect()
    }
}

impl PyRawData {
    fn column(&self, name: &str) -> PyResult<usize> {
        self.inner
            .variables
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case(name))
            .map(|v| v.index)
            .ok_or_else(|| PyKeyError::new_err(format!("Variable '{}' not found", name)))
    }
}

#[pymethods]
impl PyOperatingPoint {
    fn __getitem__(&self, name: &str) -> PyResult<f64> {
        self.value(name, PyKeyError::new_err)
    }

    fn __getattr__(&self, name: &str) -> PyResult<f64> {
        self.value(name, PyAttributeError::new_err)
    }

    /// Parsed .meas results as {name: value} dict
    #[getter]
    fn measures(&self) -> HashMap<String, f64> {
        measures_to_dict(&self.inner.raw.measures)
    }

    /// The `.step` point: {parameter: value}, empty when nothing steps.
    #[getter]
    fn params(&self) -> HashMap<String, f64> {
        self.inner.raw.params.iter().cloned().collect()
    }
}

impl PyOperatingPoint {
    /// The one point's value; `missing` for no such node, a `ValueError`
    /// for a run that wrote several points (a `.step` around the operating
    /// point), whose values one float cannot hold.
    fn value(&self, name: &str, missing: fn(String) -> PyErr) -> PyResult<f64> {
        let col = self
            .inner
            .get(name)
            .ok_or_else(|| missing(format!("No node '{name}'")))?;
        match *col {
            [v] => Ok(v),
            ref all => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "'{name}' holds {} points, one per .step point: an OperatingPoint holds one",
                all.len()
            ))),
        }
    }
}

/// Convert MeasureResult vec to a Python-friendly dict
fn measures_to_dict(measures: &[crate::result::MeasureResult]) -> HashMap<String, f64> {
    measures.iter().map(|m| (m.name.clone(), m.value)).collect()
}

/// A column as a Python list of floats, built from the stored slice with no
/// intermediate `Vec` (pyo3 has no NumPy here, so a list is what callers
/// get, as before).
fn float_list<'py>(py: Python<'py>, xs: &[f64]) -> PyResult<Bound<'py, PyList>> {
    PyList::new(py, xs)
}

/// Per-point transform of a complex waveform, for `magnitude`/`magnitude_db`/`phase`.
fn complex_part(
    a: &crate::result::Results,
    name: &str,
    f: fn(num_complex::Complex64) -> f64,
) -> PyResult<Vec<f64>> {
    let i = a
        .column(name)
        .ok_or_else(|| PyKeyError::new_err(format!("Node '{}' not found", name)))?;
    let c = a.raw.complex(i).ok_or_else(|| {
        pyo3::exceptions::PyValueError::new_err(format!(
            "Node '{}' has no complex data; the backend returned a real-valued plot",
            name
        ))
    })?;
    Ok(c.iter().map(|&v| f(v)).collect())
}
