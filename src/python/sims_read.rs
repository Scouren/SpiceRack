//! The provisional read hook of a Python `Simulator(..., read=fn)`
//! (`spicerack.provisional`: outside the 1.0 freeze): a registered
//! simulator's `Runner::Hook` id → its Python function → what the
//! function returned for a finished run, `(raw, measures)`, as the run's
//! plots. `raw` is a raw file's path, a `RawData` or a sequence of them;
//! `measures` is `None` or a dict in the `spicerack.measures/1` shape
//! (`python/spicerack/export.py` writes one). Every field read from the
//! answer is checked here, once; a function that raises or answers
//! anything else fails the run (`SimulationError`, opening `read hook: `).
//!
//! Cites: none (the dict's fields are `export.py`'s; its schema is
//! `tests/fixtures/schemas/spicerack.measures-1.schema.json`).
//!
//! Contents:
//! - `pub(super) const UNREGISTERED: u32`
//! - `pub(super) fn register(py: Python<'_>, def: &SimDef, f: &Py<PyAny>) -> Result<Sim, SimRegisterError>`
//! - `pub(super) fn read(id: u32, dir: &Path) -> Result<Vec<RawData>, BackendError>`

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};

use super::results::PyRawData;
use crate::backend::{self, BackendError, Runner, Sim, SimDef, SimRegisterError};
use crate::measure_parse::at_step;
use crate::rawfile::parse_raw_all;
use crate::result::{MeasureResult, RawData};

/// The hook id of a `Simulator` not yet registered: no function is held
/// under it.
pub(super) const UNREGISTERED: u32 = u32::MAX;

/// The schema a `measures` dict names.
const SCHEMA: &str = "spicerack.measures/1";

/// The most plots, analyses or measures one answer is read for: an
/// endless iterable is refused.
const ITEMS_MAX: usize = 1 << 16;

/// The Python read functions, by the id their simulator's `Runner::Hook`
/// holds. One is added when a simulator whose function is not yet here is
/// registered, and never freed, as the registry's row is never freed:
/// fewer entries than `backend::SIMS_MAX`.
static HOOKS: Mutex<Vec<Py<PyAny>>> = Mutex::new(Vec::new());

/// Registers `def`, a `Runner::Hook` simulator still [`UNREGISTERED`],
/// with `f` as its reader. The hook id is `f`'s place in the table, and
/// one function is one entry, so an equal `Simulator` registered again is
/// the definition already held. `f` is held from here only when the
/// registry took a definition that names a new entry.
///
/// # Errors
/// The registry's refusal; nothing is held then.
pub(super) fn register(
    py: Python<'_>,
    def: &SimDef,
    f: &Py<PyAny>,
) -> Result<Sim, SimRegisterError> {
    let mut held = HOOKS.lock().unwrap_or_else(|p| p.into_inner());
    let known = held.iter().position(|h| h.as_ptr() == f.as_ptr());
    let at = known.unwrap_or(held.len());
    assert!(at < backend::SIMS_MAX, "a hook per registered simulator");
    let mut def = def.clone();
    match &mut def.runner {
        Runner::Hook { id, .. } => *id = at as u32,
        Runner::BuiltIn(_) | Runner::Command { .. } => {
            unreachable!("a `Simulator` built with `read` is a hook")
        }
    }
    let sim = backend::register_simulator(def)?;
    if at == held.len() {
        held.push(f.clone_ref(py));
    }
    assert!(at < held.len(), "the hook the registered row names is held");
    Ok(sim)
}

/// The plots of the finished run in `dir`, as the Python function held
/// under `id` reads them (`backend::ReadHook`'s form). Each plot carries
/// its measures: plot `k` holds step `k`'s rows and those of no step
/// (`measure_parse::at_step`), as a built-in's run.
///
/// # Errors
/// `SimulationError` opening `read hook: `: the function raised (the
/// exception's type and message follow), or it answered something that is
/// not the `(raw, measures)` the module header describes (the part at
/// fault is named).
pub(super) fn read(id: u32, dir: &Path) -> Result<Vec<RawData>, BackendError> {
    assert!(id != UNREGISTERED, "a registered simulator's hook");
    assert!(dir.is_absolute(), "the run's scratch directory");
    let answered = Python::attach(|py| -> PyResult<Vec<RawData>> {
        // The table's lock is taken inside the interpreter's, as `register` does.
        let f = HOOKS.lock().unwrap_or_else(|p| p.into_inner())[id as usize].clone_ref(py);
        let got = f.call1(py, (dir.to_string_lossy().into_owned(),))?;
        let pair = got.bind(py).cast::<PyTuple>().ok().filter(|t| t.len() == 2);
        let pair = pair.ok_or_else(|| wrong("it returns a (raw, measures) tuple"))?;
        let (raw, measures) = (pair.get_item(0)?, pair.get_item(1)?);
        let mut plots = plots(&raw, dir)?;
        let rows = match measures.is_none() {
            true => Vec::new(),
            false => rows(&measures)?,
        };
        for (k, p) in plots.iter_mut().enumerate() {
            p.measures = at_step(&rows, k);
        }
        Ok(plots)
    });
    answered.map_err(|e| BackendError::SimulationError(format!("read hook: {e}")))
}

// ---- private ----

/// The plots `raw` names: every plot of a raw file (a path, taken from
/// `dir` when relative; any file `rawfile::parse_raw_all` reads), one
/// `RawData`, or each `RawData` of a sequence, in its order.
fn plots(raw: &Bound<'_, PyAny>, dir: &Path) -> PyResult<Vec<RawData>> {
    if let Ok(one) = raw.cast::<PyRawData>() {
        return Ok(vec![one.borrow().inner.clone()]);
    }
    if let Ok(path) = raw.extract::<PathBuf>() {
        let path = dir.join(path);
        let unread = |e: &dyn std::fmt::Display| wrong(&format!("raw: {}: {e}", path.display()));
        let bytes = std::fs::read(&path).map_err(|e| unread(&e))?;
        return parse_raw_all(&bytes).map_err(|e| unread(&e));
    }
    let takes = "raw: a path, a RawData or a sequence of RawData";
    let mut out = Vec::new();
    for item in items(raw, takes)? {
        let one = item.cast::<PyRawData>().map_err(|_| wrong(takes))?;
        out.push(one.borrow().inner.clone());
    }
    Ok(out)
}

/// The rows of `doc`, a dict in the `spicerack.measures/1` shape, in its
/// order. Read and checked: `schema`; each of `analyses`' `kind` (a
/// `str`, or `None`: not known); each of `measures`' `name`, `analysis`
/// (a place in `analyses`), `step`, `value` and `status`. The other keys
/// (`spicerack`, `backend`, `raw`, `index`, `plot`, `steps`, `reason`)
/// are not read.
fn rows(doc: &Bound<'_, PyAny>) -> PyResult<Vec<MeasureResult>> {
    let doc = doc.cast::<PyDict>();
    let doc = doc.map_err(|_| wrong("measures: a dict or None"))?;
    let schema = key(doc, "schema", "measures: schema")?.extract::<String>();
    if schema.ok().as_deref() != Some(SCHEMA) {
        return Err(wrong(&format!("measures: schema: not {SCHEMA:?}")));
    }
    let mut kinds: Vec<String> = Vec::new();
    let listed = "measures: analyses: a list of dicts";
    for a in items(&key(doc, "analyses", "measures: analyses")?, listed)? {
        let a = a.cast::<PyDict>().map_err(|_| wrong(listed))?;
        let kind = key(a, "kind", "measures: analyses: kind")?;
        let word = match kind.is_none() {
            true => Ok(String::new()),
            false => kind.extract::<String>(),
        };
        kinds.push(word.map_err(|_| wrong("measures: analyses: kind: a str or None"))?);
    }
    let mut out = Vec::new();
    let listed = "measures: measures: a list of dicts";
    for (n, m) in items(&key(doc, "measures", "measures: measures")?, listed)?
        .iter()
        .enumerate()
    {
        let m = m.cast::<PyDict>().map_err(|_| wrong(listed))?;
        let at = |field: &str| format!("measures: measures[{n}].{field}");
        let bad = |field: &str, takes: &str| wrong(&format!("{}: {takes}", at(field)));
        let name = key(m, "name", &at("name"))?.extract::<String>();
        let name = name.map_err(|_| bad("name", "a str"))?;
        let place = key(m, "analysis", &at("analysis"))?.extract::<usize>();
        let analysis = place.ok().and_then(|i| kinds.get(i));
        let analysis = analysis.ok_or_else(|| bad("analysis", "a place in analyses"))?;
        let step = key(m, "step", &at("step"))?;
        let step = match step.is_none() {
            true => Ok(None),
            false => step.extract::<u32>().map(Some),
        };
        let step = step.map_err(|_| bad("step", "a step's number from 0, or None"))?;
        let value = key(m, "value", &at("value"))?;
        let value = match value.is_none() {
            true => Ok(None),
            false => value.extract::<f64>().map(Some),
        };
        let unvalued = "a finite number when status is \"ok\", None when \"failed\"";
        let value = value.map_err(|_| bad("value", unvalued))?;
        let status = key(m, "status", &at("status"))?.extract::<String>();
        let ok = match status.ok().as_deref() {
            Some("ok") => true,
            Some("failed") => false,
            _ => return Err(bad("status", "\"ok\" or \"failed\"")),
        };
        let mut row = match (ok, value) {
            (true, Some(v)) if v.is_finite() => MeasureResult::ok(&name, v),
            (false, None) => MeasureResult::failed(&name),
            _ => return Err(bad("value", unvalued)),
        };
        row.step = step;
        row.analysis.clone_from(analysis);
        out.push(row);
    }
    assert!(out.len() <= ITEMS_MAX, "`items` bounds the rows");
    Ok(out)
}

/// `d[name]`, or `at`'s refusal when the key is missing.
fn key<'py>(d: &Bound<'py, PyDict>, name: &str, at: &str) -> PyResult<Bound<'py, PyAny>> {
    let v = d.get_item(name)?;
    v.ok_or_else(|| wrong(&format!("{at}: missing")))
}

/// The items of the iterable `v`, at most [`ITEMS_MAX`]; `takes` is the
/// refusal of something that is not one.
fn items<'py>(v: &Bound<'py, PyAny>, takes: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut out = Vec::new();
    for item in v.try_iter().map_err(|_| wrong(takes))? {
        if out.len() == ITEMS_MAX {
            return Err(wrong(&format!("{takes}: more than {ITEMS_MAX} items")));
        }
        out.push(item?);
    }
    Ok(out)
}

/// What the function answered is not what a read hook answers.
fn wrong(what: &str) -> PyErr {
    PyValueError::new_err(what.to_string())
}
