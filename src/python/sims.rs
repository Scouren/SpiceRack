//! Python's simulator registry (`spicerack.Simulator`,
//! `register_simulator`, `simulators`): Python arguments (or a
//! `spicerack.simulator/1` file's keys, passed as keywords) → a checked
//! `backend::SimDef` → the registry. Every field is validated here, once,
//! when the `Simulator` is built: a bad value is a `ValueError` opening
//! `R0040 <title>: <field>: `.
//!
//! Cites: none (the fields are `backend::SimDef`'s).
//!
//! Contents:
//! - `pub(super) struct PySim`
//! - `pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()>`

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyString};

use crate::backend::{self, Installed, Kind, Output, Piece, Runner, SimDef, SimRegisterError};
use crate::syntax::diag::{Code, info};

// A simulator's descriptor, checked (`SimDef::check`) when it was built.
#[pyclass(name = "Simulator", module = "spicerack", eq, frozen)]
#[derive(Clone, PartialEq)]
pub(super) struct PySim {
    def: SimDef,
}

/// The keyword fields of `Simulator(name, ...)`, as a
/// `spicerack.simulator/1` file spells them.
const FIELDS: [&str; 7] = [
    "dialect",
    "command",
    "output",
    "analyses",
    "features",
    "verilog",
    "verilog_a",
];

/// The most names one `analyses` or `features` value is read for: more
/// than either vocabulary holds, so an endless iterable is refused.
const NAMES_MAX: usize = 64;

/// Adds `Simulator`, `register_simulator` and `simulators` to `m`.
pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySim>()?;
    m.add_function(wrap_pyfunction!(register_simulator, m)?)?;
    m.add_function(wrap_pyfunction!(simulators, m)?)
}

// ---- private ----

#[pymethods]
impl PySim {
    // Every field arrives untyped and is checked here, so a wrong type and
    // a wrong value are the same `R0040`, with the field named.
    #[new]
    #[pyo3(signature = (name=None, **fields))]
    fn new<'py>(
        name: Option<&Bound<'py, PyAny>>,
        fields: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Self> {
        let field = |key: &str| -> PyResult<Option<Bound<'py, PyAny>>> {
            let v = match fields {
                Some(d) => d.get_item(key)?,
                None => None,
            };
            Ok(v.filter(|v| !v.is_none()))
        };
        let required = |key: &str| field(key)?.ok_or_else(|| bad(key, "required"));
        if let Some(d) = fields {
            // Sorted: the key named is the same on every run.
            let mut keys: Vec<String> = d.keys().iter().map(|k| k.to_string()).collect();
            keys.sort();
            if let Some(k) = keys.iter().find(|k| !FIELDS.contains(&k.as_str())) {
                return Err(bad(k, "not a field of a simulator descriptor"));
            }
        }

        let name = text("name", name.ok_or_else(|| bad("name", "required"))?)?;
        let dialect = text("dialect", &required("dialect")?)?;
        let dialect = super::def_::parse_lang(&dialect).map_err(|e| bad("dialect", &e))?;
        let words: Vec<String> = required("command")?
            .extract()
            .map_err(|_| bad("command", "a list of words, one per argument"))?;
        let argv = words.iter().map(|w| backend::parse_arg(w));
        let argv = argv.collect::<Result<Vec<_>, _>>().map_err(refused)?;
        let program = match argv.first().map(Vec::as_slice) {
            Some([Piece::Lit(program)]) => program.clone(),
            // `check` refuses it below: "it has no program".
            None => String::new(),
            Some(_) => {
                let why = "its first word is the program: it holds no placeholder";
                return Err(bad("command", why));
            }
        };
        let output = text("output", &required("output")?)?;
        let Some(&(_, output)) = Output::NAMES.iter().find(|n| n.0 == output) else {
            let all = Output::NAMES.iter().map(|n| n.0);
            return Err(bad("output", &one_of(&output, all)));
        };
        let mut analyses = 0u32;
        for n in names("analyses", &required("analyses")?)? {
            // One name may be several kinds (`tran`: a Fourier, a Spectre
            // sweep and Monte Carlo wrap a transient).
            let kinds = Kind::ALL.iter().filter(|k| k.name() == n);
            let bits = kinds.fold(0u32, |m, &k| m | 1 << k as u32);
            if bits == 0 {
                let all = Kind::ALL.iter().map(|k| k.name());
                return Err(bad("analyses", &one_of(&n, all)));
            }
            analyses |= bits;
        }
        let mut features = 0u8;
        if let Some(v) = field("features")? {
            for n in names("features", &v)? {
                let bit = backend::features().find(|f| f.0 == n);
                let all = backend::features().map(|f| f.0);
                features |= bit.ok_or_else(|| bad("features", &one_of(&n, all)))?.1;
            }
        }
        let flags = [
            ("verilog", backend::feature::VERILOG_COSIM),
            ("verilog_a", backend::feature::OSDI),
        ];
        for (key, bit) in flags {
            if let Some(v) = field(key)? {
                let on = v.cast::<PyBool>().map_err(|_| bad(key, "true or false"))?;
                if on.is_true() {
                    features |= bit;
                }
            }
        }

        let def = SimDef {
            name,
            dialect,
            runner: Runner::Command { argv, output },
            features,
            analyses,
            installed: Installed::OnPath(program),
        };
        def.check().map_err(refused)?;
        Ok(PySim { def })
    }

    #[getter]
    fn name(&self) -> &str {
        &self.def.name
    }

    #[getter]
    fn dialect(&self) -> &'static str {
        self.def.dialect.name()
    }

    /// The argument list as given: one word per entry, placeholders unfilled.
    #[getter]
    fn command(&self) -> Vec<String> {
        let (argv, _) = self.parts();
        let piece = |p: &Piece| match p {
            Piece::Lit(s) => s.clone(),
            Piece::Deck => "{deck}".to_string(),
            Piece::Raw => "{raw}".to_string(),
            Piece::Dir => "{dir}".to_string(),
        };
        let word = |w: &backend::Arg| -> String { w.iter().map(&piece).collect() };
        let words: Vec<String> = argv.iter().map(word).collect();
        assert_eq!(words.len(), argv.len());
        words
    }

    #[getter]
    fn output(&self) -> &'static str {
        let (_, output) = self.parts();
        let named = Output::NAMES.iter().find(|n| n.1 == output);
        named.expect("every output has its name").0
    }

    /// The declared analyses by name, in SpiceRack's order, each once.
    #[getter]
    fn analyses(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for &k in &Kind::ALL {
            if self.def.analyses & (1 << k as u32) != 0 && !out.contains(&k.name()) {
                out.push(k.name());
            }
        }
        assert!(!out.is_empty(), "`check` refuses an empty `analyses`");
        out
    }

    /// The declared features by name (`verilog`, `verilog_a` among them as
    /// `verilog`, `osdi`), in bit order.
    #[getter]
    fn features(&self) -> Vec<&'static str> {
        let declared = backend::features().filter(|f| self.def.features & f.1 != 0);
        declared.map(|f| f.0).collect()
    }

    #[getter]
    fn verilog(&self) -> bool {
        self.def.features & backend::feature::VERILOG_COSIM != 0
    }

    #[getter]
    fn verilog_a(&self) -> bool {
        self.def.features & backend::feature::OSDI != 0
    }

    fn __repr__(&self) -> String {
        format!(
            "Simulator({:?}, dialect={:?})",
            self.def.name,
            self.def.dialect.name()
        )
    }
}

impl PySim {
    /// Its argv template and result reader: Python builds commands only.
    fn parts(&self) -> (&[backend::Arg], Output) {
        match &self.def.runner {
            Runner::Command { argv, output } => (argv.as_slice(), *output),
            Runner::BuiltIn(_) => unreachable!("`new` builds a command"),
        }
    }
}

/// Registers `sim` and returns the name `with_backend()`, `simulator()`
/// and `SPICERACK_BACKEND` select it by. Registering an equal `Simulator`
/// again registers nothing and returns the same name.
#[pyfunction]
fn register_simulator(sim: PyRef<'_, PySim>) -> PyResult<String> {
    let held = backend::register_simulator(sim.def.clone()).map_err(refused)?;
    assert_eq!(held.name(), sim.def.name, "registered under its own name");
    Ok(held.name().to_string())
}

/// Every simulator's name in route order: the built-ins, then each
/// registered one in registration order.
#[pyfunction]
fn simulators() -> Vec<String> {
    let names: Vec<String> = backend::simulators().map(|(_, r)| r.name.clone()).collect();
    assert!(names.len() <= backend::SIMS_MAX);
    names
}

/// `v` as text, or `field`'s refusal.
fn text(field: &str, v: &Bound<'_, PyAny>) -> PyResult<String> {
    v.extract::<String>().map_err(|_| bad(field, "a string"))
}

/// `v` as a list of names: any iterable of `str` (a list, a set, a
/// tuple) but a `str` itself, whose letters would be read as names.
fn names(field: &str, v: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let wrong = || bad(field, "a list of names");
    if v.is_instance_of::<PyString>() {
        return Err(wrong());
    }
    let mut out = Vec::new();
    for item in v.try_iter().map_err(|_| wrong())? {
        let item = item.map_err(|_| wrong())?;
        out.push(item.extract::<String>().map_err(|_| wrong())?);
        if out.len() > NAMES_MAX {
            return Err(bad(field, "too many names"));
        }
    }
    Ok(out)
}

/// "`got` is not one of ...": `all` listed in order, each once.
fn one_of<'a>(got: &str, all: impl Iterator<Item = &'a str>) -> String {
    let mut listed: Vec<&str> = Vec::new();
    for n in all {
        if !listed.contains(&n) {
            listed.push(n);
        }
    }
    format!("'{got}' is not one of {}", listed.join(", "))
}

/// `R0040` for a field the binding itself refuses.
fn bad(field: &str, detail: &str) -> PyErr {
    let title = info(Code::R0040).title;
    PyValueError::new_err(format!("{:?} {title}: {field}: {detail}", Code::R0040))
}

/// The registry's refusal (`R0040`, or `R0042` for a full registry); its
/// message already opens with the field.
fn refused(e: SimRegisterError) -> PyErr {
    let code = e.code();
    PyValueError::new_err(format!("{code:?} {}: {e}", info(code).title))
}
