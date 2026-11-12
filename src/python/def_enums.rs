//! The definition API's vocabulary as Python classes, one per Rust enum,
//! named as the enum: each unit variant a class attribute named as the
//! variant (its `Debug` name; a Python keyword gets a trailing `_`:
//! `Join.None_`), each data variant a constructor of the variant's name
//! (`Scale.Exp(6)`, `Alt.Omit(Code.W0030)`). The binding has no vocabulary
//! of its own: a class's attributes are generated from the Rust variants,
//! and an exhaustive `match` fails to compile when a variant is added to
//! the enum and not here (`DIALECT-PLUGIN.md` §4, oracle 9).
//!
//! Cites: none.
//!
//! Contents:
//! - `macro_rules! py_enum`, `macro_rules! py_units`, `macro_rules! py_all` (scoped to `python`
//!   by `#[macro_use]`: `def_tag.rs` declares `Tag`'s class with them)
//! - one `pub(super) struct Py<Enum>(pub(super) <Enum>)` per enum, `From<Py<Enum>> for <Enum>`,
//!   `Py<Enum>::UNITS` (`PyTag`: `def_tag.rs`, re-exported here); `pub(super) struct PyQuoting`,
//!   `pub(super) struct PyEnd`, `From<PyQuoting> for Quoting`
//! - `pub(super) fn add_enums(m: &Bound<'_, PyModule>) -> PyResult<()>`

use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::syntax::Dialect;
use crate::syntax::behaviour::{
    AnalysisForm, CardRules, ControlBlock, Coupling, DotRule, EndRule, ExprEval, IncludeScope,
    LoadForm, ModelSemantics, NamedParams, NamedRules, NetlistRules, NumberReading, OptionsForm,
    ParamForm, Parasitics, SourceRules, ValueQuoting, Writer,
};
use crate::syntax::def::{Devices, Keywords, Operators};
use crate::syntax::diag::{Code, Kind};
use crate::syntax::lang::CONSTRUCTS;
use crate::syntax::lexicon::{
    Arithmetic, Backslash, Caret, Dollar, End, Join, Nesting, Quote, Quoting, Scale, Tail,
};
use crate::syntax::render::capability::{Alt, Construct, Rule};
use crate::syntax::render::spelling::FuncForm;
use crate::syntax::statement::{Analysis, Command, Statement};

pub(super) use super::def_tag::PyTag;

/// `Quoting { escapes, protects, across_breaks }`.
#[pyclass(name = "Quoting", module = "spicerack.def_", eq, frozen)]
#[derive(Clone, Copy, PartialEq)]
pub(super) struct PyQuoting(pub(super) Quoting);

/// `End { col1, then_blank, stops }`.
#[pyclass(name = "End", module = "spicerack.def_", eq, frozen)]
#[derive(Clone, Copy, PartialEq)]
pub(super) struct PyEnd(pub(super) End);

/// Defines `$py`, the Python class of enum `$rust`: its unit variants
/// `$units` (class attributes, added by [`add_enums`]), its data variants'
/// names `$data` (constructors, written in `$extra`), and a compile-time
/// check that `$v` and `$pat` cover every variant.
macro_rules! py_enum {
    ($py:ident, $name:literal, $rust:ident, units = $units:expr,
     covers = [$($v:ident),*] $(| $pat:pat_param)*, data = [$($d:literal),*] { $($extra:tt)* }) => {
        #[pyclass(name = $name, module = "spicerack.def_", eq, frozen)]
        #[derive(Clone, Copy, PartialEq)]
        pub(super) struct $py(pub(super) $rust);

        impl From<$py> for $rust {
            fn from(p: $py) -> $rust {
                p.0
            }
        }

        impl $py {
            /// The unit variants, in declaration order.
            pub(super) const UNITS: &'static [$rust] = $units;
        }

        #[pymethods]
        impl $py {
            /// The Rust variants' names: the unit ones, then the data ones.
            #[staticmethod]
            fn variants() -> Vec<String> {
                let units = Self::UNITS.iter().map(|v| format!("{v:?}"));
                units.chain([$($d.to_string()),*]).collect()
            }

            fn __repr__(&self) -> String {
                format!(concat!($name, ".{:?}"), self.0)
            }

            $($extra)*
        }

        const _: fn($rust) = |v| match v {
            $($rust::$v)|* $(| $pat)* => (),
        };
    };
}

/// A unit-only enum listed in full.
macro_rules! py_units {
    ($py:ident, $name:literal, $rust:ident, [$($v:ident),* $(,)?]) => {
        py_enum!($py, $name, $rust, units = &[$($rust::$v),*], covers = [$($v),*], data = [] {});
    };
}

/// A unit-only enum whose Rust side lists every variant (`ALL`).
macro_rules! py_all {
    ($py:ident, $name:literal, $rust:ident, $all:expr) => {
        py_enum!($py, $name, $rust, units = $all, covers = [] | _, data = [] {});
    };
}

py_units!(PyJoin, "Join", Join, [None, Brackets, Parens]);
py_units!(PyDollar, "Dollar", Dollar, [Text, AfterBlank, Hspice]);
py_units!(
    PyBackslash,
    "Backslash",
    Backslash,
    [None, Glue, Hspice, Join, Tight]
);
py_units!(PyCaret, "Caret", Caret, [Pow, Xor, None, Refused]);
py_units!(PyNesting, "Nesting", Nesting, [Free, Explicit]);
py_units!(PyArithmetic, "Arithmetic", Arithmetic, [Ngspice, Decimal]);
py_units!(PyKeywords, "Keywords", Keywords, [Spice]);
py_units!(PyOperators, "Operators", Operators, [Spice]);
py_units!(
    PyDevices,
    "Devices",
    Devices,
    [Spice, NgspiceOwn, LossyLine, XycePort, Spectre, Vacask]
);
py_units!(
    PyRule,
    "Rule",
    Rule,
    [
        LaplaceXfer,
        LaplaceTable,
        LaplaceAttr,
        StatisticsDraws,
        RackVars,
        VacaskCalls,
        StartupRamp,
    ]
);
py_units!(PyFuncForm, "FuncForm", FuncForm, [Func, Param, Real, None]);
py_units!(PyKind, "Kind", Kind, [Note, Refusal]);
py_units!(
    PyWriter,
    "Writer",
    Writer,
    [Cards, Cdl, Verilog, Spectre, Vacask]
);
py_units!(
    PyNumberReading,
    "NumberReading",
    NumberReading,
    [Ngspice, Decimal]
);
py_units!(
    PyModelSemantics,
    "ModelSemantics",
    ModelSemantics,
    [Ngspice, Other]
);
py_units!(PySourceRules, "SourceRules", SourceRules, [Ngspice, Spice]);
py_units!(
    PyCardRules,
    "CardRules",
    CardRules,
    [Spice, Ngspice, Hspice, Ltspice, Xyce]
);
py_units!(
    PyNamedRules,
    "NamedRules",
    NamedRules,
    [None, Spectre, Vacask]
);
py_units!(
    PyOptionsForm,
    "OptionsForm",
    OptionsForm,
    [Spice, XycePackages, Spectre, Vacask]
);
py_units!(
    PyLoadForm,
    "LoadForm",
    LoadForm,
    [Ngspice, Espice, Hspice, Xyce, Ltspice, None]
);
py_units!(
    PyControlBlock,
    "ControlBlock",
    ControlBlock,
    [Ngspice, None]
);
py_units!(
    PyAnalysisForm,
    "AnalysisForm",
    AnalysisForm,
    [Ngspice, Espice, Hspice, Xyce, Ltspice, Plain,]
);
py_units!(PyEndRule, "EndRule", EndRule, [Always, WhenRuns]);
py_units!(
    PyValueQuoting,
    "ValueQuoting",
    ValueQuoting,
    [AsWritten, Hspice]
);
py_units!(PyCoupling, "Coupling", Coupling, [Pairs, OneCard]);
py_units!(PyParamForm, "ParamForm", ParamForm, [Plain, Xyce]);
py_units!(
    PyIncludeScope,
    "IncludeScope",
    IncludeScope,
    [Local, Global]
);
py_units!(
    PyExprEval,
    "ExprEval",
    ExprEval,
    [C, Ngspice, Xyce, Hspice, Ltspice]
);
py_units!(
    PyNetlistRules,
    "NetlistRules",
    NetlistRules,
    [Spice, Ngspice, Xyce, Hspice, Ltspice]
);
py_units!(
    PyParasitics,
    "Parasitics",
    Parasitics,
    [Elements, Ltspice, Netlist]
);
py_units!(
    PyNamedParams,
    "NamedParams",
    NamedParams,
    [None, Spectre, Vacask]
);
py_all!(PyConstruct, "Construct", Construct, &CONSTRUCTS);
py_all!(PyCode, "Code", Code, Code::ALL);
py_all!(PyAnalysis, "Analysis", Analysis, &Analysis::ALL);
py_all!(PyCommand, "Command", Command, &Command::ALL);
py_all!(PySimulator, "Simulator", Dialect, &Dialect::ALL);

py_enum!(PyScale, "Scale", Scale, units = &[Scale::Mil], covers = [Mil] | Scale::Exp(_),
    data = ["Exp"] {
    #[staticmethod]
    #[pyo3(name = "Exp")]
    fn exp(e: i32) -> Self {
        Self(Scale::Exp(e))
    }
});

py_enum!(PyQuote, "Quote", Quote, units = &[Quote::Plain, Quote::Name], covers = [Plain, Name]
    | Quote::Expr(_), data = ["Expr"] {
    #[staticmethod]
    #[pyo3(name = "Expr")]
    fn expr(q: PyQuoting) -> Self {
        Self(Quote::Expr(q.0))
    }
});

py_enum!(PyTail, "Tail", Tail, units = &[Tail::Ltspice, Tail::None], covers = [Ltspice, None]
    | Tail::Spice { .. } | Tail::Letters { .. }, data = ["Spice", "Letters"] {
    #[staticmethod]
    #[pyo3(name = "Spice")]
    fn spice(dot: bool) -> Self {
        Self(Tail::Spice { dot })
    }

    #[staticmethod]
    #[pyo3(name = "Letters")]
    fn letters(after_exponent: bool, hex: bool) -> Self {
        Self(Tail::Letters { after_exponent, hex })
    }
});

py_enum!(PyAlt, "Alt", Alt, units = &[
    Alt::ChargeQuoted, Alt::ChargeBraced, Alt::ChargeBare, Alt::VolCur, Alt::BracedBehaviour,
    Alt::FuncParam, Alt::Hdl, Alt::PreOsdi, Alt::Probe, Alt::DcType, Alt::Include,
    Alt::BsourceCharge, Alt::Bsource, Alt::PwlPeriod, Alt::NgBehaviorLine, Alt::SpiceRegion,
    Alt::PwlValue, Alt::XycePort, Alt::XyceLin, Alt::HspiceAm, Alt::AmNoOffset, Alt::HspicePz, Alt::TempList,
    Alt::TernaryValue,
    Alt::ControlScript, Alt::StatesData, Alt::StatesTable, Alt::AlterBlocks,
], covers = [
    ChargeQuoted, ChargeBraced, ChargeBare, VolCur, BracedBehaviour, FuncParam, Hdl, PreOsdi,
    Probe, DcType, Include, BsourceCharge, Bsource, PwlPeriod, NgBehaviorLine, SpiceRegion,
    TernaryValue,
    PwlValue, XycePort, XyceLin, HspiceAm, AmNoOffset, HspicePz, TempList,
    ControlScript, StatesData, StatesTable, AlterBlocks
] | Alt::Omit(_) | Alt::Copy(_), data = ["Omit", "Copy"] {
    #[staticmethod]
    #[pyo3(name = "Omit")]
    fn omit(code: PyCode) -> Self {
        Self(Alt::Omit(code.0))
    }

    #[staticmethod]
    #[pyo3(name = "Copy")]
    fn copy(code: PyCode) -> Self {
        Self(Alt::Copy(code.0))
    }
});

py_enum!(PyDotRule, "DotRule", DotRule,
    units = &[DotRule::SParams, DotRule::Preprocess, DotRule::Hb],
    covers = [SParams, Preprocess, Hb] | DotRule::Note(_), data = ["Note"] {
    #[staticmethod]
    #[pyo3(name = "Note")]
    fn note(code: PyCode) -> Self {
        Self(DotRule::Note(code.0))
    }
});

py_enum!(PyStatement, "Statement", Statement, units = &[Statement::Options], covers = [Options]
    | Statement::Analysis(_) | Statement::Command(_), data = ["Analysis", "Command"] {
    #[staticmethod]
    #[pyo3(name = "Analysis")]
    fn analysis(a: PyAnalysis) -> Self {
        Self(Statement::Analysis(a.0))
    }

    #[staticmethod]
    #[pyo3(name = "Command")]
    fn command(c: PyCommand) -> Self {
        Self(Statement::Command(c.0))
    }
});

/// Adds every class above to `m`, each unit variant a class attribute.
pub(super) fn add_enums(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyQuoting>()?;
    m.add_class::<PyEnd>()?;
    add(m, PyTag::UNITS, PyTag)?;
    add(m, PyJoin::UNITS, PyJoin)?;
    add(m, PyDollar::UNITS, PyDollar)?;
    add(m, PyBackslash::UNITS, PyBackslash)?;
    add(m, PyCaret::UNITS, PyCaret)?;
    add(m, PyNesting::UNITS, PyNesting)?;
    add(m, PyArithmetic::UNITS, PyArithmetic)?;
    add(m, PyKeywords::UNITS, PyKeywords)?;
    add(m, PyOperators::UNITS, PyOperators)?;
    add(m, PyDevices::UNITS, PyDevices)?;
    add(m, PyRule::UNITS, PyRule)?;
    add(m, PyFuncForm::UNITS, PyFuncForm)?;
    add(m, PyKind::UNITS, PyKind)?;
    add(m, PyWriter::UNITS, PyWriter)?;
    add(m, PyNumberReading::UNITS, PyNumberReading)?;
    add(m, PyModelSemantics::UNITS, PyModelSemantics)?;
    add(m, PySourceRules::UNITS, PySourceRules)?;
    add(m, PyCardRules::UNITS, PyCardRules)?;
    add(m, PyNamedRules::UNITS, PyNamedRules)?;
    add(m, PyOptionsForm::UNITS, PyOptionsForm)?;
    add(m, PyLoadForm::UNITS, PyLoadForm)?;
    add(m, PyControlBlock::UNITS, PyControlBlock)?;
    add(m, PyAnalysisForm::UNITS, PyAnalysisForm)?;
    add(m, PyEndRule::UNITS, PyEndRule)?;
    add(m, PyValueQuoting::UNITS, PyValueQuoting)?;
    add(m, PyCoupling::UNITS, PyCoupling)?;
    add(m, PyParamForm::UNITS, PyParamForm)?;
    add(m, PyIncludeScope::UNITS, PyIncludeScope)?;
    add(m, PyNamedParams::UNITS, PyNamedParams)?;
    add(m, PyExprEval::UNITS, PyExprEval)?;
    add(m, PyParasitics::UNITS, PyParasitics)?;
    add(m, PyNetlistRules::UNITS, PyNetlistRules)?;
    add(m, PyConstruct::UNITS, PyConstruct)?;
    add(m, PyCode::UNITS, PyCode)?;
    add(m, PyAnalysis::UNITS, PyAnalysis)?;
    add(m, PyCommand::UNITS, PyCommand)?;
    add(m, PySimulator::UNITS, PySimulator)?;
    add(m, PyScale::UNITS, PyScale)?;
    add(m, PyQuote::UNITS, PyQuote)?;
    add(m, PyTail::UNITS, PyTail)?;
    add(m, PyAlt::UNITS, PyAlt)?;
    add(m, PyDotRule::UNITS, PyDotRule)?;
    add(m, PyStatement::UNITS, PyStatement)
}

impl From<PyQuoting> for Quoting {
    fn from(p: PyQuoting) -> Quoting {
        p.0
    }
}

// ---- private ----

#[pymethods]
impl PyQuoting {
    #[new]
    fn new(escapes: bool, protects: bool, across_breaks: bool) -> Self {
        Self(Quoting {
            escapes,
            protects,
            across_breaks,
        })
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.0)
    }
}

#[pymethods]
impl PyEnd {
    #[new]
    fn new(col1: bool, then_blank: bool, stops: bool) -> Self {
        Self(End {
            col1,
            then_blank,
            stops,
        })
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.0)
    }
}

/// Adds class `T` to `m`, each of `units` (wrapped by `wrap`) a class
/// attribute named as its Rust variant (`None` → `None_`).
fn add<T, R>(m: &Bound<'_, PyModule>, units: &[R], wrap: fn(R) -> T) -> PyResult<()>
where
    T: pyo3::PyClass + Into<pyo3::PyClassInitializer<T>>,
    R: Copy + std::fmt::Debug,
{
    let py = m.py();
    let ty: Bound<'_, PyType> = py.get_type::<T>();
    for &v in units {
        let name = format!("{v:?}");
        let name = if name == "None" {
            "None_".to_string()
        } else {
            name
        };
        ty.setattr(name.as_str(), Py::new(py, wrap(v))?)?;
    }
    m.add_class::<T>()
}
