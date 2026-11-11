//! Simulator output as tables: one [`RawData`] per plot (column-major, as
//! `rawfile` reads it) and [`Results`], the same plot with its vectors looked
//! up by canonical name (`normalize::canonical`).
//!
//! Cites: none (the raw-file layout is `rawfile`'s).
//!
//! Contents:
//! - `pub struct RawData`, `RawData::complex(&self, i: usize) -> Option<&[Complex64]>`,
//!   `RawData::re(&self, i: usize) -> Cow<'_, [f64]>`
//! - `pub struct VarInfo`
//! - `pub enum VarType`, `VarType::parse(word: &str) -> VarType`
//! - `pub enum MeasureStatus`, `pub struct MeasureResult`,
//!   `MeasureResult::ok(name: &str, value: f64) -> MeasureResult`,
//!   `MeasureResult::failed(name: &str) -> MeasureResult`
//! - `pub struct Results`, `Results::from_raw(raw: RawData) -> Results`,
//!   `Results::column(&self, name: &str) -> Option<usize>`,
//!   `Results::get(&self, name: &str) -> Option<Cow<'_, [f64]>>`,
//!   `Results::complex(&self, name: &str) -> Option<&[Complex64]>`,
//!   `Results::axis(&self) -> Cow<'_, [f64]>`

use num_complex::Complex64;
use std::borrow::Cow;
use std::collections::HashMap;

use crate::syntax::Dialect;

/// One simulator plot, stored by column: `real_data[v]` or `complex_data[v]` holds every
/// point of variable `v`. A real plot fills `real_data`; a complex plot fills
/// `complex_data` (a backend may also fill `real_data` for its real-valued vectors).
#[derive(Debug, Clone, Default)]
pub struct RawData {
    /// The `Title:` line: the deck's title.
    pub title: String,
    /// The `Plotname:` line (`Transient Analysis`, ...), which
    /// `simulation::run_each` matches an analysis by.
    pub plot_name: String,
    /// One row per column, in file order; `variables[i].index == i`.
    pub variables: Vec<VarInfo>,
    /// `real_data[v]`: every point of variable `v`; empty for a complex plot.
    pub real_data: Vec<Vec<f64>>,
    /// `complex_data[v]`: every point of variable `v`; empty for a real plot.
    pub complex_data: Vec<Vec<Complex64>>,
    /// `Flags: complex`.
    pub is_complex: bool,
    /// Captured stdout from simulator process (contains .meas results, etc.)
    pub stdout: String,
    /// Captured log file content (LTspice puts .meas results in .log)
    pub log_content: String,
    /// Parsed .meas results extracted from stdout/log
    pub measures: Vec<MeasureResult>,
    /// The simulator that wrote the plot, whose naming rules [`Results`]
    /// reads its vectors by; `None` (a raw file read on its own) reads them
    /// as ngspice names them.
    pub backend_hint: Option<Dialect>,
    /// The `.step` point this plot is: each stepped parameter and its
    /// value, in the simulator's order; empty when nothing steps (ZERO D42:
    /// a stepped run is one plot per step).
    pub params: Vec<(String, f64)>,
}

/// One raw-file variable (a `Variables:` row).
#[derive(Debug, Clone)]
pub struct VarInfo {
    /// Column in `real_data` / `complex_data`.
    pub index: usize,
    /// As the simulator wrote it (`v(out)`, `i(v1)`, `V1:p`); `Results`
    /// reads it through `normalize::canonical`.
    pub name: String,
    pub var_type: VarType,
}

/// A raw-file variable's type word (the `Variables:` section's third field).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Voltage,
    Current,
    Time,
    Frequency,
    /// `param`: LTspice's stepped parameter, column 0 of a stepped
    /// operating point.
    Param,
    /// Any other word (`notype`, LTspice's `device_current`, ...).
    Other,
}

/// Whether a measure gave a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasureStatus {
    /// The simulator printed its value.
    Ok,
    /// It has none: the simulator said so (ngspice's `failed`, LTspice's
    /// `FAIL'ed`), or printed no value for a measure the deck declares.
    Failed,
}

/// One `.meas` result (`measure_parse`): the value the simulator printed,
/// or that it has none.
#[derive(Debug, Clone)]
pub struct MeasureResult {
    /// Its name as the simulator printed it (ngspice: in lowercase).
    pub name: String,
    /// NaN exactly when `status` is [`MeasureStatus::Failed`].
    pub value: f64,
    pub status: MeasureStatus,
    /// The `.step` point it was measured at, counted from 0 in step order
    /// (ZERO D42: the run's plot of that step); `None` when the run does
    /// not step, and for a row that holds at every step.
    pub step: Option<u32>,
    /// The analysis it is measured on: the word of its `.meas` card,
    /// lowercase (`tran`, `ac`, `dc`, ...); empty when what was read does
    /// not say (a printout read with no deck).
    pub analysis: String,
}

/// Any analysis result: the plot plus normalized-name -> column lookup.
/// Every analysis kind is this one type; the sweep axis (time, frequency, DC sweep)
/// is column 0.
#[derive(Debug, Clone)]
pub struct Results {
    pub raw: RawData,
    /// Canonical node name → column.
    pub nodes: HashMap<String, usize>,
    /// Canonical branch current (`i(v1)`) → column.
    pub branches: HashMap<String, usize>,
}

impl VarType {
    /// The type a raw file's type word names, spelled exactly as ngspice
    /// writes it.
    pub fn parse(word: &str) -> VarType {
        match word {
            "voltage" => VarType::Voltage,
            "current" => VarType::Current,
            "time" => VarType::Time,
            "frequency" => VarType::Frequency,
            "param" => VarType::Param,
            _ => VarType::Other,
        }
    }
}

impl MeasureResult {
    /// The measure `name` with the value the simulator printed, of no step
    /// and no analysis. A NaN is no value: the row is [`Self::failed`].
    pub fn ok(name: &str, value: f64) -> MeasureResult {
        let status = match value.is_nan() {
            true => MeasureStatus::Failed,
            false => MeasureStatus::Ok,
        };
        MeasureResult {
            name: name.to_string(),
            value,
            status,
            step: None,
            analysis: String::new(),
        }
    }

    /// The measure `name` with no value.
    pub fn failed(name: &str) -> MeasureResult {
        MeasureResult::ok(name, f64::NAN)
    }
}

impl RawData {
    /// Complex column `i`, or `None` when that variable is real-valued.
    pub fn complex(&self, i: usize) -> Option<&[Complex64]> {
        self.complex_data
            .get(i)
            .filter(|c| !c.is_empty())
            .map(Vec::as_slice)
    }

    /// Column `i` as reals: a real column borrowed, the real part of a
    /// complex one collected into a new `Vec`; empty if absent.
    pub fn re(&self, i: usize) -> Cow<'_, [f64]> {
        match self.complex(i) {
            Some(c) => Cow::Owned(c.iter().map(|c| c.re).collect()),
            None => Cow::Borrowed(self.real_data.get(i).map_or(&[][..], Vec::as_slice)),
        }
    }
}

impl Results {
    /// `raw` with every column named canonically, as a node or a branch
    /// current; of two columns with one canonical name, the later wins.
    pub fn from_raw(raw: RawData) -> Self {
        let mut nodes = HashMap::new();
        let mut branches = HashMap::new();
        for v in &raw.variables {
            let (name, current) = crate::normalize::canonical(&v.name, raw.backend_hint);
            let map = if v.var_type == VarType::Current || current {
                &mut branches
            } else {
                &mut nodes
            };
            map.insert(name, v.index);
        }
        assert!(nodes.len() + branches.len() <= raw.variables.len());
        Self {
            raw,
            nodes,
            branches,
        }
    }

    /// Column of `name` (canonical form, any case), nodes before branches.
    /// Allocates only when `name` is not already lowercase ASCII.
    pub fn column(&self, name: &str) -> Option<usize> {
        let lower: Cow<str> = if name
            .bytes()
            .all(|b| b.is_ascii() && !b.is_ascii_uppercase())
        {
            Cow::Borrowed(name)
        } else {
            Cow::Owned(name.to_lowercase())
        };
        self.nodes
            .get(lower.as_ref())
            .or_else(|| self.branches.get(lower.as_ref()))
            .copied()
    }

    /// Waveform of `name`; the real part for a complex plot (computed, the
    /// one case that allocates).
    pub fn get(&self, name: &str) -> Option<Cow<'_, [f64]>> {
        self.column(name).map(|i| self.raw.re(i))
    }

    /// Complex waveform of `name`; `None` for no such column or a real one.
    pub fn complex(&self, name: &str) -> Option<&[Complex64]> {
        self.raw.complex(self.column(name)?)
    }

    /// The sweep axis (column 0).
    pub fn axis(&self) -> Cow<'_, [f64]> {
        self.raw.re(0)
    }
}
