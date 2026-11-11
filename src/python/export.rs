//! `spicerack.export`'s native half: the Python results of a run → the
//! bytes of one ngspice raw file (`rawfile::write_raw`) and, per analysis,
//! what the `spicerack.measures/1` sidecar says of it.
//! `python/spicerack/export.py` writes the two files.
//!
//! Cites: none (the raw layout is `rawfile`'s, the names `normalize`'s).
//!
//! Contents:
//! - `pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()>`

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};

use super::results::plot_of;
use crate::backend::Kind;
use crate::rawfile::{RawFormat, write_raw};
use crate::result::{MeasureResult, MeasureStatus, RawData, VarInfo};
use crate::syntax::Dialect;

/// The most items `results` is read for: an endless one is refused.
const ITEMS_MAX: usize = 1 << 16;

/// A step's point: each stepped parameter and its value
/// (`RawData::params`).
type Point = Vec<(String, f64)>;

/// One measure: its name, its `.meas` card's analysis word (empty: not
/// known), its value (`None`: it has none).
type Row = (String, String, Option<f64>);

/// One exported analysis, as `export.py` unpacks it: its kind's name
/// (`None`: a plot read from a file), its first plot's name, the simulator
/// family that wrote it (`None`: a plot read from a file), the number of
/// its first plot in the raw file, each step's point (`None`: not
/// stepped), each step's measures (one list when not stepped).
type Entry = (
    Option<&'static str>,
    String,
    Option<&'static str>,
    usize,
    Option<Vec<Point>>,
    Vec<Vec<Row>>,
);

/// Adds `_export` to `m`.
pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(export, m)?)
}

// ---- private ----

/// What `spicerack.export` writes from `results` (a run's: per analysis a
/// result, a list of one result per step, or `None` for one with no plot):
/// the raw file's bytes (`ascii`: its values as text), this crate's
/// version, and one [`Entry`] per analysis that has a plot.
///
/// `TypeError` for an item that is no result; `ValueError` for a result
/// no raw file holds ([`file_plots`]) and past [`ITEMS_MAX`] items.
#[pyfunction]
#[pyo3(name = "_export")]
fn export<'py>(
    py: Python<'py>,
    results: &Bound<'py, PyAny>,
    ascii: bool,
) -> PyResult<(Bound<'py, PyBytes>, &'static str, Vec<Entry>)> {
    if plot_of(results).is_some() {
        let one = "export: `results` is one result; pass a sequence of them ([result])";
        return Err(PyTypeError::new_err(one));
    }
    let mut plots: Vec<RawData> = Vec::new();
    let mut entries: Vec<Entry> = Vec::new();
    for (n, item) in results.try_iter()?.enumerate() {
        if n == ITEMS_MAX {
            let many = format!("export: `results` holds more than {ITEMS_MAX} items");
            return Err(PyValueError::new_err(many));
        }
        let item = item?;
        let stepped = item.is_instance_of::<PyList>() || item.is_instance_of::<PyTuple>();
        let steps: Vec<Bound<'py, PyAny>> = match stepped {
            true => item.try_iter()?.collect::<PyResult<_>>()?,
            // A Fourier's `None`: a table on stdout, no plot.
            false if item.is_none() => Vec::new(),
            false => vec![item],
        };
        let first = plots.len();
        let mut head = None;
        let mut points: Vec<Point> = Vec::with_capacity(steps.len());
        let mut rows: Vec<Vec<Row>> = Vec::with_capacity(steps.len());
        for step in &steps {
            // ponytail: a result's columns are copied once, out of the
            // Python object, and moved from there; borrow them when a
            // result outgrows memory.
            let Some((kind, raw)) = plot_of(step) else {
                let class = step.get_type().name()?;
                let not = format!("export: a {class} is not a result");
                return Err(PyTypeError::new_err(not));
            };
            if head.is_none() {
                let backend = raw.backend_hint.map(Dialect::name);
                head = Some((kind.map(Kind::name), raw.plot_name.clone(), backend));
            }
            points.push(raw.params.clone());
            rows.push(raw.measures.iter().map(row).collect());
            plots.extend(file_plots(raw).map_err(PyValueError::new_err)?);
        }
        // No plot (a `None`, an empty list of steps): no entry.
        let Some((kind, plot, backend)) = head else {
            continue;
        };
        let points = stepped.then_some(points);
        entries.push((kind, plot, backend, first, points, rows));
    }
    assert!(
        entries.len() <= plots.len(),
        "an entry is at least one plot"
    );
    assert!(entries.iter().all(|e| e.3 < plots.len()));
    let format = match ascii {
        true => RawFormat::Ascii,
        false => RawFormat::Binary,
    };
    let bytes = write_raw(&plots, format);
    Ok((PyBytes::new(py, &bytes), env!("CARGO_PKG_VERSION"), entries))
}

/// `m` as the sidecar's row: no value for a measure that failed.
fn row(m: &MeasureResult) -> Row {
    let value = (m.status == MeasureStatus::Ok).then_some(m.value);
    assert!(
        value.is_none_or(|v| !v.is_nan()),
        "an ok measure has a value"
    );
    (m.name.clone(), m.analysis.clone(), value)
}

/// `raw` as the plots a raw file holds it in, every vector under its
/// canonical name (`normalize::canonical`: `out`, `i(v1)`, the names
/// `Results` reads by, the same whichever simulator wrote the plot). One
/// plot; or, of a result whose columns are not all one length (a noise
/// result's totals, one point beside the spectra's many), one plot per
/// length, in the order each length first appears, all under `raw`'s plot
/// name.
///
/// # Errors
/// A result with no vectors; a vector whose canonical name is not one
/// word (a raw file's variable line holds one).
fn file_plots(mut raw: RawData) -> Result<Vec<RawData>, String> {
    if raw.variables.is_empty() {
        let name = &raw.plot_name;
        return Err(format!("export: the result '{name}' holds no vectors"));
    }
    let total = raw.variables.len();
    // (points, plot): the plot of every column `points` long.
    let mut out: Vec<(usize, RawData)> = Vec::new();
    for v in 0..total {
        let var = &raw.variables[v];
        assert!(var.index == v, "variable `{}` is column {v}", var.name);
        let (name, _) = crate::normalize::canonical(&var.name, raw.backend_hint);
        if name.is_empty() || name.contains(char::is_whitespace) {
            return Err(format!("export: the vector name '{name}' is not one word"));
        }
        let var_type = var.var_type;
        let real = raw.real_data.get_mut(v).map(std::mem::take);
        let real = real.unwrap_or_default();
        let complex = raw.complex_data.get_mut(v).map(std::mem::take);
        let complex = complex.unwrap_or_default();
        let points = match complex.is_empty() {
            true => real.len(),
            false => complex.len(),
        };
        let at = out.iter().position(|(n, _)| *n == points);
        let at = at.unwrap_or(out.len());
        if at == out.len() {
            let plot = RawData {
                title: raw.title.clone(),
                plot_name: raw.plot_name.clone(),
                is_complex: raw.is_complex,
                ..RawData::default()
            };
            out.push((points, plot));
        }
        let plot = &mut out[at].1;
        let index = plot.variables.len();
        plot.variables.push(VarInfo {
            index,
            name,
            var_type,
        });
        plot.real_data.push(real);
        plot.complex_data.push(complex);
    }
    let held: usize = out.iter().map(|(_, p)| p.variables.len()).sum();
    assert!(held == total, "every column is in one plot");
    Ok(out.into_iter().map(|(_, p)| p).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rawfile::parse_raw_all;
    use crate::result::VarType;

    /// A real plot `name` of `columns`: (vector name, type, values).
    fn plot(name: &str, columns: Vec<(&str, VarType, Vec<f64>)>) -> RawData {
        let mut raw = RawData {
            title: "p8-100".into(),
            plot_name: name.into(),
            ..RawData::default()
        };
        for (index, (name, var_type, values)) in columns.into_iter().enumerate() {
            raw.variables.push(VarInfo {
                index,
                name: name.into(),
                var_type,
            });
            raw.real_data.push(values);
        }
        raw
    }

    fn names(p: &RawData) -> Vec<&str> {
        p.variables.iter().map(|v| v.name.as_str()).collect()
    }

    /// Oracle: a spec example (the canonical names are `normalize`'s
    /// documented forms, written here as literals) and a round trip
    /// through the real reader.
    #[test]
    fn p8_100_a_plot_is_written_under_canonical_names() {
        let raw = plot(
            "Transient Analysis",
            vec![
                ("time", VarType::Time, vec![0.0, 1e-3]),
                ("V(in)", VarType::Voltage, vec![1.0, 1.0]),
                ("v(out)", VarType::Voltage, vec![0.0, 0.625]),
                ("v1#branch", VarType::Current, vec![-1e-3, -3.75e-4]),
            ],
        );
        let plots = file_plots(raw).unwrap();
        assert_eq!(plots.len(), 1);
        assert_eq!(names(&plots[0]), ["time", "in", "out", "i(v1)"]);
        let back = parse_raw_all(&write_raw(&plots, RawFormat::Binary)).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(names(&back[0]), ["time", "in", "out", "i(v1)"]);
        assert_eq!(back[0].real_data[2], [0.0, 0.625]);
        assert_eq!(back[0].real_data[3], [-1e-3, -3.75e-4]);
    }

    /// Oracle: a round trip through the real reader. ngspice-45 writes a
    /// noise run as two plots, the spectra and one point of totals; a
    /// result holding both is written as two again.
    #[test]
    fn p8_100_columns_of_two_lengths_are_two_plots() {
        let raw = plot(
            "Noise Spectral Density Curves",
            vec![
                ("frequency", VarType::Frequency, vec![10.0, 100.0, 1000.0]),
                ("onoise_spectrum", VarType::Other, vec![4e-9, 3e-9, 1e-9]),
                ("v(onoise_total)", VarType::Voltage, vec![6.4e-8]),
                ("v(inoise_total)", VarType::Voltage, vec![6.5e-8]),
            ],
        );
        let plots = file_plots(raw).unwrap();
        assert_eq!(plots.len(), 2);
        let back = parse_raw_all(&write_raw(&plots, RawFormat::Ascii)).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(names(&back[0]), ["frequency", "onoise_spectrum"]);
        assert_eq!(names(&back[1]), ["onoise_total", "inoise_total"]);
        assert_eq!(back[0].real_data[1], [4e-9, 3e-9, 1e-9]);
        assert_eq!(back[1].real_data[0], [6.4e-8]);
        assert_eq!(back[1].plot_name, "Noise Spectral Density Curves");
    }

    /// Oracle: a spec refusal (`rawfile::write_raw`'s contract: a variable
    /// line holds one word, a plot at least one column).
    #[test]
    fn p8_100_a_result_no_raw_file_holds_is_refused() {
        assert!(file_plots(RawData::default()).is_err());
        let spaced = plot(
            "Operating Point",
            vec![("v(a b)", VarType::Voltage, vec![1.0])],
        );
        let why = file_plots(spaced).unwrap_err();
        assert!(why.contains("'a b'"), "{why}");
    }
}
