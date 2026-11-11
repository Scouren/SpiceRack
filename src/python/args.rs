//! Arguments: Python values and `**kwargs` → card fields (`Value`, a
//! waveform field `Num`, `(name, text)` parameter pairs). Cites no spec:
//! SPICE's own number syntax is why a `UnitValue` is written in SI form.
//!
//! Contents:
//! - `pub(super) enum PyValueArg`, `PyValueArg::value(self) -> Value`
//! - `pub(super) struct Num`, `impl FromPyObject for Num`, `Num::of(v: f64) -> Num`, `Num::text(&self) -> String`,
//!   `Num::zero(&self) -> bool`, `Num::value(&self) -> Value`
//! - `pub(super) fn kw_params(params: Option<Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>>`
//! - `pub(super) fn param_text(name: &str, value: &Bound<'_, PyAny>) -> PyResult<String>`
//! - `pub(super) fn switch_state(state: Option<&str>) -> Result<String, String>`
//! - `pub(super) fn kw_options(kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>>`
//! - `pub(super) fn kw_volts(kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>>`

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict};

use super::keywords::SwitchState;
use super::model::{Value, dec};
use super::unit::PyUnitValue;
use crate::syntax::render::portable_option;

/// A value argument (`R(..., value)`, `V(..., value)`): tried as a number,
/// then a `UnitValue`, then a string.
#[derive(FromPyObject)]
pub(super) enum PyValueArg {
    Float(f64),
    Unit(PyUnitValue),
    /// An expression over parameters (`"rv*2"`, `"{rv*2}"`), written in each
    /// dialect's own quoting.
    Expr(String),
}

/// One waveform field: a number, a unit value, or an expression string.
#[derive(Clone, Debug)]
pub(super) struct Num(pub(super) Value);

/// `**kwargs` as ordered `(name, value)` pairs, values per [`param_value`].
/// A pair it gives no text for (`False`, `None`) is left out; an empty text
/// (`True`) is a bare flag, as `model::params` and the `.options` card
/// write it.
pub(super) fn kw_params(params: Option<Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>> {
    let mut out = vec![];
    if let Some(dict) = params {
        for (k, v) in dict.iter() {
            if let Some(text) = param_value(&v)? {
                out.push((k.extract::<String>()?, text));
            }
        }
    }
    Ok(out)
}

/// The value of `parameter(name, value)` as its `.param` card writes it,
/// per [`param_value`]: a number as Python prints it, a `UnitValue` as its
/// SI value, a string (an expression) as given. A parameter has a value:
/// `None`, a `bool`, an empty string and any other object are a
/// `ValueError`.
pub(super) fn param_text(name: &str, value: &Bound<'_, PyAny>) -> PyResult<String> {
    let typed = !value.is_instance_of::<PyBool>() && value.extract::<PyValueArg>().is_ok();
    let text = match typed {
        true => param_value(value)?.filter(|text| !text.trim().is_empty()),
        false => None,
    };
    match text {
        Some(text) => Ok(text),
        None => Err(PyValueError::new_err(format!(
            "parameter: `{name}` is given {}; its value is a number, a UnitValue or a string",
            value.repr()?
        ))),
    }
}

/// `options(**kw)` as ordered `.options` pairs (D17). Every key is a
/// portable option (`render::portable_option`), written as given; a value
/// is per [`param_value`] (`True` a bare flag). A key of one simulator's
/// own, and `None` or `False` (which set nothing), are a `ValueError`,
/// raised before any pair is kept, so the caller's bench is unchanged.
pub(super) fn kw_options(kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>> {
    let mut out = vec![];
    if let Some(dict) = kwargs {
        for (k, v) in dict.iter() {
            let k = k.extract::<String>()?;
            if !portable_option(&k) {
                return Err(PyValueError::new_err(format!(
                    "options: `{k}` is not a portable option (the `.options` names of the ngspice manual, §11.1); another simulator's own option is a line of its text: extra_line(line, dialect=...)"
                )));
            }
            let Some(text) = param_value(&v)? else {
                return Err(PyValueError::new_err(format!(
                    "options: `{k}` is given None or False, which sets nothing; a flag is `{k}=True`, a value is a number or a string"
                )));
            };
            out.push((k, text));
        }
        assert_eq!(out.len(), dict.len(), "one pair per keyword");
    }
    Ok(out)
}

/// `**kwargs` of node voltages (`out=1.65`, `out="{vdd/2}"`) as ordered
/// `(node, value)` pairs, the value as a card writes it ([`Num::text`]: a
/// number in plain decimal, an expression braced).
pub(super) fn kw_volts(kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Vec<(String, String)>> {
    let mut out = vec![];
    if let Some(dict) = kwargs {
        for (k, v) in dict.iter() {
            out.push((k.extract::<String>()?, v.extract::<Num>()?.text()));
        }
    }
    Ok(out)
}

/// A switch's `initial_state=` as the word after its model: ` ON` or
/// ` OFF`, or nothing when it is not given.
///
/// # Errors
/// A state that is neither word (the message names the two).
pub(super) fn switch_state(state: Option<&str>) -> Result<String, String> {
    let Some(state) = state else {
        return Ok(String::new());
    };
    let state: SwitchState = state.parse()?;
    Ok(format!(" {}", state.name()))
}

impl PyValueArg {
    /// The card value; an expression loses one pair of enclosing braces.
    pub(super) fn value(self) -> Value {
        match self {
            Self::Float(v) => Value::Num(v),
            Self::Unit(uv) => Value::Num(uv.inner.value),
            Self::Expr(e) => {
                let t = e.trim();
                let inner = t
                    .strip_prefix('{')
                    .and_then(|t| t.strip_suffix('}'))
                    .unwrap_or(t);
                Value::Expr(inner.trim().to_string())
            }
        }
    }
}

impl<'py> FromPyObject<'_, 'py> for Num {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, 'py, PyAny>) -> PyResult<Self> {
        Ok(Num(ob.extract::<PyValueArg>()?.value()))
    }
}

impl Num {
    /// A number field.
    pub(super) fn of(v: f64) -> Num {
        Num(Value::Num(v))
    }

    /// The field in a waveform's parentheses: a number as plain decimal,
    /// an expression braced.
    pub(super) fn text(&self) -> String {
        match &self.0 {
            Value::Num(v) => dec(*v),
            v => v.text(),
        }
    }

    /// Whether the field is the number 0.
    pub(super) fn zero(&self) -> bool {
        matches!(self.0, Value::Num(v) if v == 0.0)
    }

    /// The field as a card value (a source's DC value).
    pub(super) fn value(&self) -> Value {
        self.0.clone()
    }
}

// ---- private ----

/// Text of a `**kwargs` parameter value on an element, model or instance card.
///
/// A `UnitValue` is written as its SI value (`1e-6`), never its display form:
/// `str(1 @ u_MOhm)` is `1MOhm`, which every SPICE reads as 1 milliohm, and
/// `1F` reads as one femtofarad. A `bool` is SPICE's flag: `True` is the
/// empty text (the bare word, `off`, `klu`), `False` is `None`, and so is
/// Python's `None`: `str()` of either would put `True`/`None` on the card,
/// which no SPICE reads. Everything else keeps its Python `str()`, so
/// expressions such as `"{w*2}"` pass through verbatim.
fn param_value(v: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if v.is_none() {
        return Ok(None);
    }
    // `bool` first: a Python `bool` is also an `int`, an `int` is not a `bool`.
    if let Ok(flag) = v.extract::<bool>() {
        return Ok(flag.then(String::new));
    }
    if let Ok(uv) = v.extract::<PyUnitValue>() {
        return Ok(Some(format!("{:e}", uv.inner.value)));
    }
    Ok(Some(v.str()?.to_string()))
}
