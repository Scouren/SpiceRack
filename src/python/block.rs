//! Block: one branch of a conditional, a portless `Subcircuit` taking the
//! same element, instance and model calls → `.if (c)` … `.elseif (c)` …
//! `.else` … `.endif` as one entry of a body's lines given as text
//! (`Subcircuit.conditional`).
//!
//! Cites: ngspice-45 manual §2.13 (.IF condition-controlled netlist: a
//! branch holds device instances and `.model` cards, and blocks nest; no
//! `.SUBCKT`, `.INC`, `.LIB` or `.PARAM` inside one).
//!
//! Contents:
//! - `pub(super) struct PyBlock` (Python `Block`, extends `Subcircuit`)
//! - `pub(super) fn conditional(branches, otherwise) -> PyResult<String>`

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use super::model::Body;
use super::subcircuit::PySubcircuit;

#[pyclass(name = "Block", extends = PySubcircuit)]
pub(super) struct PyBlock;

/// The conditional as text, `.if (c)` through `.endif` (no newline after
/// it): each of `branches` a condition and the block the circuit holds
/// while that condition is the first true one, `otherwise` the block when
/// none is.
///
/// # Errors
/// `ValueError`: no branch; a condition that is empty or longer than one
/// line; a block holding what a branch cannot (`lines`).
pub(super) fn conditional(
    branches: &[(String, PyRef<'_, PyBlock>)],
    otherwise: Option<&PyRef<'_, PyBlock>>,
) -> PyResult<String> {
    let arms: Vec<(&str, &Body)> = branches
        .iter()
        .map(|(c, b)| (c.as_str(), &b.as_super().inner))
        .collect();
    text(&arms, otherwise.map(|b| &b.as_super().inner)).map_err(PyValueError::new_err)
}

// ---- private ----

#[pymethods]
impl PyBlock {
    /// `Block()`: an empty branch.
    #[new]
    fn new() -> (Self, PySubcircuit) {
        let base = PySubcircuit {
            inner: Body::default(),
            ground_alias: false,
        };
        (PyBlock, base)
    }

    fn __str__(&self) -> &'static str {
        "Block()"
    }

    fn __repr__(&self) -> &'static str {
        "Block()"
    }
}

/// `.if (c)` … `.endif` over `arms` and `otherwise`, as `conditional`.
fn text(arms: &[(&str, &Body)], otherwise: Option<&Body>) -> Result<String, String> {
    if arms.is_empty() {
        return Err("conditional: takes one (condition, block) or more".into());
    }
    let mut out = String::new();
    for (i, (cond, b)) in arms.iter().enumerate() {
        let c = cond.trim();
        if c.is_empty() || c.contains('\n') {
            return Err(format!(
                "conditional: {cond:?} is no condition: one line, an expression of parameters"
            ));
        }
        let word = if i == 0 { ".if" } else { ".elseif" };
        out.push_str(&format!("{word} ({c})\n"));
        lines(&mut out, b)?;
    }
    if let Some(b) = otherwise {
        out.push_str(".else\n");
        lines(&mut out, b)?;
    }
    out.push_str(".endif");
    // What `lines` takes as a block's own conditional.
    debug_assert!(out.starts_with(".if ("), "{out}");
    Ok(out)
}

/// A branch's lines appended to `out`: `b`'s models, elements and
/// instances, then its lines given as text (element cards and its own
/// conditionals). `Err` for anything else `b` holds: ngspice-45 does not
/// select it with the condition (a `.param` or a `.subckt` inside a
/// branch holds whatever the condition is), so it goes on the body that
/// holds the conditional.
fn lines(out: &mut String, b: &Body) -> Result<(), String> {
    let dot = |l: &&String| l.trim_start().starts_with('.') && !l.starts_with(".if (");
    let what = match b.raw.iter().find(dot) {
        Some(l) => l.lines().next().unwrap_or_default(),
        None if !b.params.is_empty() => "a parameter",
        None if !(b.includes.is_empty() && b.libs.is_empty()) => "an include or a lib",
        None if !(b.osdi.is_empty() && b.verilog.is_empty()) => "a model load",
        None => "",
    };
    if !what.is_empty() {
        return Err(format!(
            "conditional: a block holds elements, instances, models and conditionals; {what} goes on the body that holds the conditional"
        ));
    }
    let cards = b.elements.iter().chain(&b.instances).map(|c| &c.text);
    for l in b.models.iter().chain(cards).chain(&b.raw) {
        out.push_str(l);
        out.push('\n');
    }
    Ok(())
}
