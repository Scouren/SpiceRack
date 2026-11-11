//! Subcircuit: element, instance and directive calls → their ngspice
//! cards in one `Body` (Circuit extends it). Nets are named here: `gnd` is
//! ground `0` only at the top level. Cites ngspice manual ch.3-4 (each
//! element card's field order).
//!
//! Contents:
//! - `pub(super) struct PySubcircuit` (Python `Subcircuit`)
//! - `pub(super) fn top_net(n: &str) -> String`
//! - `pub(super) fn removed(method: &str, instead: &str) -> PyErr`
//! - `pub(super) fn write_file(path: &str, text: &str) -> PyResult<()>`,
//!   `pub(super) fn read_file(path: &str) -> PyResult<String>`

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// `either`, `Num`, `Wave`, `Value`: names `pymethods_with_waves!` resolves here.
use super::args::{Num, PyValueArg, kw_params};
use super::circuit::hoisted;
use super::deck::runtime_err;
use super::model::{Body, Card, Value, dec, need, num, params};
use super::source::{Wave, either, source, wave_source};
use super::verilog::verilog_block;

#[pyclass(name = "Subcircuit", subclass)]
#[derive(Clone)]
pub(super) struct PySubcircuit {
    pub(super) inner: Body,
    /// `gnd` names the global ground `0`: true only for the top-level `Circuit`.
    pub(super) ground_alias: bool,
}

/// A node name at the top level: `gnd` (any case) is the global ground `0`.
pub(super) fn top_net(n: &str) -> String {
    if n.eq_ignore_ascii_case("gnd") {
        "0".into()
    } else {
        n.into()
    }
}

/// The error a removed JSON method raises: `NotImplementedError` naming
/// its replacement.
pub(super) fn removed(method: &str, instead: &str) -> PyErr {
    pyo3::exceptions::PyNotImplementedError::new_err(format!(
        "{method}() was removed with the JSON form; use {instead} (the deck's source text)"
    ))
}

/// `text` written to `path`; `OSError` naming the path.
pub(super) fn write_file(path: &str, text: &str) -> PyResult<()> {
    std::fs::write(path, text)
        .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{path}: {e}")))
}

/// The text at `path`; `OSError` naming the path.
pub(super) fn read_file(path: &str) -> PyResult<String> {
    std::fs::read_to_string(path)
        .map_err(|e| pyo3::exceptions::PyOSError::new_err(format!("{path}: {e}")))
}

// ---- private ----

pymethods_with_waves! {
#[pymethods]
impl PySubcircuit {
    #[new]
    #[pyo3(signature = (name, ports=None, **params))]
    fn new(name: &str, ports: Option<Vec<String>>, params: Option<Bound<'_, PyDict>>) -> PyResult<Self> {
        let inner = Body {
            name: name.to_string(),
            ports: ports.unwrap_or_default(),
            params: kw_params(params)?,
            ..Default::default()
        };
        Ok(Self { inner, ground_alias: false })
    }

    #[getter]
    fn gnd(&self) -> String {
        "0".to_string()
    }

    /// The subcircuit's name, as `X` instances refer to it.
    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    /// Port names in declaration order: the order an instance connects nets in.
    #[getter]
    fn ports(&self) -> Vec<String> {
        self.inner.ports.clone()
    }

    // ── Elements ──

    /// `value` is a number, a `UnitValue` or an expression string; `**kwargs`
    /// are instance parameters (`tc1=1e-3`, `m=2`). `raw_spice` is the text
    /// after the nodes; when it gives no resistance ([`lacks_value`]),
    /// `value` is written before it.
    #[pyo3(signature = (name, positive, negative, value, *, raw_spice=None, **kwargs))]
    fn R(&mut self, name: &str, positive: &str, negative: &str, value: PyValueArg, raw_spice: Option<&str>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let value = match raw_spice {
            Some(text) if lacks_value(text) => {
                Value::Raw(format!("{} {}", value.value().text(), text.trim()).trim_end().to_string())
            }
            Some(text) => Value::Raw(text.to_string()),
            None => value.value(),
        };
        self.two('R', name, positive, negative, &value, kwargs)
    }

    /// `charge` gives the capacitor by its charge, an expression of its
    /// voltage (`charge="1n*v(a,b)"`: ngspice manual §3.3.9, `Q = 'expr'`),
    /// in place of `value`; exactly one of the two.
    #[pyo3(signature = (name, positive, negative, value=None, *, charge=None, **kwargs))]
    fn C(&mut self, name: &str, positive: &str, negative: &str, value: Option<PyValueArg>, charge: Option<&str>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let value = match (value, charge) {
            (Some(v), None) => v.value(),
            (None, Some(q)) => Value::Raw(format!("q={}", PyValueArg::Expr(q.into()).value().text())),
            _ => return Err(PyValueError::new_err("C: give `value` or `charge=`, exactly one")),
        };
        self.two('C', name, positive, negative, &value, kwargs)
    }

    #[pyo3(signature = (name, positive, negative, value, **kwargs))]
    fn L(&mut self, name: &str, positive: &str, negative: &str, value: PyValueArg, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        self.two('L', name, positive, negative, &value.value(), kwargs)
    }

    /// Couples two or more inductors of this body, each by its card name
    /// (`"L1"`, in any case) and added before the call (ngspice manual
    /// §3.3.12: `KXXXXXXX LYYYYYYY LZZZZZZZ value`; ngspice-45 takes further
    /// inductors before the value and couples every pair with it).
    /// `coupling` is a number or an expression (`"{k}"`). `ValueError` for
    /// a name that is no `L` here (ngspice: "coupling to non-existent
    /// inductor"), for one named twice, and for fewer than two.
    #[pyo3(signature = (name, *inductors, coupling))]
    fn K(&mut self, name: &str, inductors: Vec<String>, coupling: PyValueArg) -> PyResult<()> {
        if inductors.len() < 2 {
            return Err(PyValueError::new_err(format!("K{name}: couples two or more inductors, {} given", inductors.len())));
        }
        let ls = inductors.iter().map(|l| self.inductor(name, l)).collect::<PyResult<Vec<_>>>()?;
        let twice = ls.iter().enumerate().find_map(|(i, l)| ls[..i].iter().any(|m| m.eq_ignore_ascii_case(l)).then_some(l));
        if let Some(l) = twice {
            return Err(PyValueError::new_err(format!("K{name}: '{l}' is named twice")));
        }
        let k = match coupling.value() {
            Value::Num(v) => dec(v),
            v => v.text(),
        };
        self.push(name, format!("K{name} {} {k}", ls.join(" ")));
        Ok(())
    }

    /// `portnum` makes the source RF port `portnum` (from 1) for
    /// `s_param`, with `z0` ohms in series in every analysis (ngspice's
    /// `portnum n z0 x`; ngspice and ESPice only).
    #[pyo3(signature = (name, positive, negative, value, *, ac=None, ac_phase=None, portnum=None, z0=50.0))]
    fn V(&mut self, name: &str, positive: &str, negative: &str, value: PyValueArg, ac: Option<f64>, ac_phase: Option<f64>, portnum: Option<u32>, z0: f64) {
        let port = portnum.map(|n| (n, z0));
        let c = source(false, name, self.net(positive), self.net(negative), value.value(), ac, ac_phase, port, None);
        self.inner.elements.push(c);
    }

    #[pyo3(signature = (name, positive, negative, value, *, ac=None, ac_phase=None))]
    fn I(&mut self, name: &str, positive: &str, negative: &str, value: PyValueArg, ac: Option<f64>, ac_phase: Option<f64>) {
        let c = source(true, name, self.net(positive), self.net(negative), value.value(), ac, ac_phase, None, None);
        self.inner.elements.push(c);
    }

    #[pyo3(signature = (name, positive, negative, *, expression))]
    fn BV(&mut self, name: &str, positive: &str, negative: &str, expression: &str) {
        self.behavioral('V', name, positive, negative, expression);
    }

    #[pyo3(signature = (name, positive, negative, *, expression))]
    fn BI(&mut self, name: &str, positive: &str, negative: &str, expression: &str) {
        self.behavioral('I', name, positive, negative, expression);
    }

    /// A voltage source given by an expression of node voltages and branch
    /// currents (PySpice's name): the card `E<name> n+ n- value={expr}`
    /// (ngspice-45 manual §5.2.2), read as the behavioural source `BV` is.
    #[pyo3(signature = (name, positive, negative, *, expression))]
    fn NonLinearVoltageSource(&mut self, name: &str, positive: &str, negative: &str, expression: &str) {
        self.nonlinear('E', name, positive, negative, expression);
    }

    /// As `NonLinearVoltageSource`, a current from `positive` through the
    /// source to `negative`: `G<name> n+ n- value={expr}` (manual §5.3.2).
    #[pyo3(signature = (name, positive, negative, *, expression))]
    fn NonLinearCurrentSource(&mut self, name: &str, positive: &str, negative: &str, expression: &str) {
        self.nonlinear('G', name, positive, negative, expression);
    }

    #[pyo3(signature = (name, positive, negative, *, control_positive, control_negative, voltage_gain))]
    fn E(&mut self, name: &str, positive: &str, negative: &str, control_positive: &str, control_negative: &str, voltage_gain: f64) {
        let nets = self.nets(&[positive, negative, control_positive, control_negative]);
        self.push(name, format!("E{name} {nets} {}", dec(voltage_gain)));
    }

    #[pyo3(signature = (name, positive, negative, *, control_positive, control_negative, transconductance))]
    fn G(&mut self, name: &str, positive: &str, negative: &str, control_positive: &str, control_negative: &str, transconductance: f64) {
        let nets = self.nets(&[positive, negative, control_positive, control_negative]);
        self.push(name, format!("G{name} {nets} {}", dec(transconductance)));
    }

    #[pyo3(signature = (name, positive, negative, *, vsense, current_gain))]
    fn F(&mut self, name: &str, positive: &str, negative: &str, vsense: &str, current_gain: f64) {
        let nets = self.nets(&[positive, negative]);
        self.push(name, format!("F{name} {nets} {vsense} {}", dec(current_gain)));
    }

    #[pyo3(signature = (name, positive, negative, *, vsense, transresistance))]
    fn H(&mut self, name: &str, positive: &str, negative: &str, vsense: &str, transresistance: f64) {
        let nets = self.nets(&[positive, negative]);
        self.push(name, format!("H{name} {nets} {vsense} {}", dec(transresistance)));
    }

    /// `**kwargs` are instance parameters (`area=2`, `m=4`), as on `M`.
    #[pyo3(signature = (name, anode, cathode, *, model, **kwargs))]
    fn D(&mut self, name: &str, anode: &str, cathode: &str, model: &str, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let nets = self.nets(&[anode, cathode]);
        self.push(name, format!("D{name} {nets} {model}{}", params(&kw_params(kwargs)?)));
        Ok(())
    }

    /// `substrate` and `thermal` are the optional 4th and 5th terminals.
    #[pyo3(signature = (name, collector, base, emitter, *, model, substrate=None, thermal=None, **kwargs))]
    fn Q(
        &mut self, name: &str, collector: &str, base: &str, emitter: &str, model: &str,
        substrate: Option<&str>, thermal: Option<&str>, kwargs: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        if thermal.is_some() && substrate.is_none() {
            return Err(pyo3::exceptions::PyValueError::new_err("Q: a thermal node needs a substrate node before it"));
        }
        let mut nets = vec![collector, base, emitter];
        nets.extend(substrate.into_iter().chain(thermal));
        let nets = self.nets(&nets);
        self.push(name, format!("Q{name} {nets} {model}{}", params(&kw_params(kwargs)?)));
        Ok(())
    }

    #[pyo3(signature = (name, collector, base, emitter, *, model, substrate=None, thermal=None, **kwargs))]
    fn BJT(
        &mut self, name: &str, collector: &str, base: &str, emitter: &str, model: &str,
        substrate: Option<&str>, thermal: Option<&str>, kwargs: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        self.Q(name, collector, base, emitter, model, substrate, thermal, kwargs)
    }

    /// `bulk=None` is a 3-terminal device (VDMOS); `extra_nodes` follow the
    /// bulk (VDMOS thermal nodes, SOI body and back gate).
    #[pyo3(signature = (name, drain, gate, source, bulk=None, *, model, extra_nodes=None, **kwargs))]
    fn M(
        &mut self, name: &str, drain: &str, gate: &str, source: &str, bulk: Option<&str>, model: &str,
        extra_nodes: Option<Vec<String>>, kwargs: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let extra = extra_nodes.unwrap_or_default();
        if bulk.is_none() && !extra.is_empty() {
            return Err(PyValueError::new_err("M: extra_nodes follow the bulk node; give bulk too"));
        }
        let mut nets = vec![drain, gate, source];
        nets.extend(bulk);
        nets.extend(extra.iter().map(String::as_str));
        let nets = self.nets(&nets);
        self.push(name, format!("M{name} {nets} {model}{}", params(&kw_params(kwargs)?)));
        Ok(())
    }

    #[pyo3(signature = (name, drain, gate, source, bulk=None, *, model, extra_nodes=None, **kwargs))]
    fn MOSFET(
        &mut self, name: &str, drain: &str, gate: &str, source: &str, bulk: Option<&str>, model: &str,
        extra_nodes: Option<Vec<String>>, kwargs: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        self.M(name, drain, gate, source, bulk, model, extra_nodes, kwargs)
    }

    #[pyo3(signature = (name, drain, gate, source, *, model, **kwargs))]
    fn J(&mut self, name: &str, drain: &str, gate: &str, source: &str, model: &str, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let nets = self.nets(&[drain, gate, source]);
        self.push(name, format!("J{name} {nets} {model}{}", params(&kw_params(kwargs)?)));
        Ok(())
    }

    #[pyo3(signature = (name, drain, gate, source, *, model, **kwargs))]
    fn Z(&mut self, name: &str, drain: &str, gate: &str, source: &str, model: &str, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let nets = self.nets(&[drain, gate, source]);
        self.push(name, format!("Z{name} {nets} {model}{}", params(&kw_params(kwargs)?)));
        Ok(())
    }

    #[pyo3(signature = (name, positive, negative, *, control_positive, control_negative, model))]
    fn S(&mut self, name: &str, positive: &str, negative: &str, control_positive: &str, control_negative: &str, model: &str) {
        let nets = self.nets(&[positive, negative, control_positive, control_negative]);
        self.push(name, format!("S{name} {nets} {model}"));
    }

    #[pyo3(signature = (name, positive, negative, *, vcontrol, model))]
    fn W(&mut self, name: &str, positive: &str, negative: &str, vcontrol: &str, model: &str) {
        let nets = self.nets(&[positive, negative]);
        self.push(name, format!("W{name} {nets} {vcontrol} {model}"));
    }

    #[pyo3(signature = (name, input_positive, input_negative, output_positive, output_negative, *, Z0, TD))]
    fn T(&mut self, name: &str, input_positive: &str, input_negative: &str, output_positive: &str, output_negative: &str, Z0: f64, TD: f64) {
        let nets = self.nets(&[input_positive, input_negative, output_positive, output_negative]);
        self.push(name, format!("T{name} {nets} Z0={} TD={}", dec(Z0), dec(TD)));
    }

    /// A polynomial controlled source, SPICE `POLY(n)`: `kind` is "E", "G",
    /// "F" or "H"; `controls` are node pairs (E/G) or sensing V sources
    /// (F/H); `coefficients` in SPICE's term order.
    #[pyo3(signature = (name, kind, positive, negative, controls, coefficients))]
    fn POLY(&mut self, name: &str, kind: &str, positive: &str, negative: &str, controls: Vec<String>, coefficients: Vec<f64>) -> PyResult<()> {
        let letter = match kind.to_ascii_uppercase().as_str() {
            k @ ("E" | "G" | "F" | "H") => k.chars().next().unwrap(),
            _ => return Err(PyValueError::new_err("POLY kind is E, G, F or H")),
        };
        let nodes = matches!(letter, 'E' | 'G');
        if nodes && !controls.len().is_multiple_of(2) {
            return Err(PyValueError::new_err("POLY E/G controls are node pairs"));
        }
        let (dims, controls) = match nodes {
            true => (controls.len() / 2, controls.iter().map(|c| self.net(c)).collect()),
            false => (controls.len(), controls),
        };
        let coeffs: Vec<String> = coefficients.iter().map(|c| num(*c)).collect();
        let nets = self.nets(&[positive, negative]);
        self.push(name, format!("{letter}{name} {nets} POLY({dims}) {} {}", controls.join(" "), coeffs.join(" ")));
        Ok(())
    }

    /// XSPICE code model instance (A-element).
    ///
    /// ```python
    /// circuit.A(name="adc1", connections=["[vin]", "[dout]"], model="adc_buf")
    /// circuit.A(name="dff1", connections=["[d clk $d_hi $d_hi]", "[q qbar]"], model="dff_model")
    /// ```
    #[pyo3(signature = (*, name, connections, model))]
    fn A(&mut self, name: &str, connections: Vec<String>, model: &str) {
        let mut c = Card::element(name, format!("A{name} {} {model}", connections.join(" ")));
        c.needs = need::XSPICE;
        self.inner.elements.push(c);
    }

    // ── Subcircuit instances ──

    #[pyo3(signature = (subckt, name, *nodes, **params))]
    fn instance(&mut self, subckt: &PySubcircuit, name: &str, nodes: Vec<String>, params: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let subckt_name = subckt.inner.name.clone();
        self.X(name, &subckt_name, nodes, params)
    }

    /// Subcircuit instance by name (e.g. a PDK device). `**params` land on the card as `name=value`.
    #[pyo3(signature = (name, subcircuit_name, *nodes, **params))]
    fn X(&mut self, name: &str, subcircuit_name: &str, nodes: Vec<String>, params: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let mut text = format!("X{name}");
        for n in &nodes {
            text.push(' ');
            text.push_str(&self.net(n));
        }
        let ps = super::model::params(&kw_params(params)?);
        text.push_str(&format!(" {subcircuit_name}{ps}"));
        self.inner.instances.push(Card::element(name, text));
        Ok(())
    }

    /// Defines `subckt` inside this subcircuit: a nested `.subckt`, which
    /// only this subcircuit's instances see (ngspice-45 manual §2.6.1). Its
    /// `include`, `lib` and `osdi` files move up to this body, as
    /// `Circuit.subcircuit` moves them to the top. `ValueError` when
    /// `subckt` holds a `verilog()` block, which has no source form.
    fn subcircuit(&mut self, subckt: &PySubcircuit) -> PyResult<()> {
        // Fallible work before the commit: on an error this body is as it was.
        let mut body = self.inner.clone();
        let def = hoisted(&mut body, &subckt.inner);
        body.raw.push(super::store::definition(&def).map_err(PyValueError::new_err)?);
        self.inner = body;
        Ok(())
    }

    // ── Directives ──

    #[pyo3(signature = (name, kind, **kwargs))]
    fn model(&mut self, name: &str, kind: &str, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        self.inner.models.push(super::model::model_card(name, kind, &kw_params(kwargs)?));
        Ok(())
    }

    fn raw_spice(&mut self, line: &str) {
        self.inner.raw.push(line.to_string());
    }

    fn include(&mut self, path: &str) {
        self.inner.includes.push(path.to_string());
    }

    fn lib(&mut self, path: &str, section: &str) {
        self.inner.libs.push((path.to_string(), section.to_string()));
    }

    /// Load a compiled Verilog-A model (OSDI binary).
    fn osdi(&mut self, path: &str) {
        self.inner.osdi.push(path.to_string());
    }

    fn parameter(&mut self, name: &str, value: &str) {
        self.inner.params.push((name.into(), value.into()));
    }

    /// A `.func` of this subcircuit, as `Testbench.add_function` writes one
    /// at the top: `add_function("scaled", ["x"], "x*k")` (ngspice-45 manual
    /// §2.12). One reading a parameter of the subcircuit stays local to it;
    /// LTspice, Spectre and VACASK define functions at the top only and
    /// refuse it (`W0106`).
    #[pyo3(signature = (name, args, body))]
    fn add_function(&mut self, name: &str, args: Vec<String>, body: &str) {
        self.inner.raw.push(format!(".func {name}({}) {{{body}}}", args.join(", ")));
    }

    /// Load a Verilog-A model (file path or inline source). The `.va` is
    /// recorded, not compiled: the target's writer compiles it with OpenVAF
    /// for ngspice/VACASK, includes it for Spectre, and hands it to ESPice as
    /// `.hdl` (VerA). Returns the resolved `.va` path.
    fn veriloga(&mut self, source_or_path: &str) -> PyResult<String> {
        let va = crate::veriloga::resolve(source_or_path)
            .map_err(runtime_err)?
            .to_string_lossy().to_string();
        if !self.inner.osdi.contains(&va) { self.inner.osdi.push(va.clone()); }
        Ok(va)
    }

    /// Add a Verilog module, lowered when the netlist is emitted.
    ///
    /// mode="simulate": compile with iverilog, use ngspice d_cosim XSPICE model.
    /// mode="synthesize": synthesize with Yosys to gate-level SPICE subcircuit calls.
    ///
    /// ```python
    /// circuit.verilog(
    ///     source="counter.v",
    ///     mode="simulate",
    ///     instance_name="cnt1",
    ///     connections={"clk": "clk_net", "count": ["bit0", "bit1", "bit2"]},
    /// )
    /// ```
    #[pyo3(signature = (*, source, mode="simulate", instance_name, connections, pdk=None, liberty=None, spice_models=None))]
    fn verilog(
        &mut self,
        py: pyo3::Python<'_>,
        source: &str,
        mode: &str,
        instance_name: &str,
        connections: HashMap<String, Py<PyAny>>,
        pdk: Option<&str>,
        liberty: Option<&str>,
        spice_models: Option<&str>,
    ) -> PyResult<()> {
        let vb = verilog_block(py, source, mode, instance_name, &connections, pdk, liberty, spice_models)?;
        self.inner.verilog.push(vb);
        Ok(())
    }

    // ── Store (the deck's source text) ──

    /// The ngspice text of a deck defining this subcircuit, behind a
    /// `* spicerack-source ngspice` line: its loads, then `.subckt` …
    /// `.ends` holding the cards its calls wrote.
    fn to_source(&self) -> PyResult<String> {
        super::store::to_source(&self.inner).map_err(PyValueError::new_err)
    }

    /// The subcircuit `text` defines (its first `.subckt`), read in
    /// `dialect` (`None`: the one its first line names, else ngspice;
    /// another dialect's text is written as ngspice first).
    #[staticmethod]
    #[pyo3(signature = (text, dialect=None))]
    fn from_source(text: &str, dialect: Option<&str>) -> PyResult<Self> {
        let d = dialect.map(super::def_::parse_lang).transpose().map_err(PyValueError::new_err)?;
        let inner = super::store::from_source(text, d).map_err(PyValueError::new_err)?;
        Ok(Self { inner, ground_alias: false })
    }

    /// Writes [`Self::to_source`] to `path`.
    fn save(&self, path: &str) -> PyResult<()> {
        let text = self.to_source()?;
        write_file(path, &text)
    }

    /// The subcircuit the file at `path` defines (as [`Self::from_source`]).
    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        Self::from_source(&read_file(path)?, None)
    }

    // The JSON form went with the IR (D3: the store is source text).
    fn to_json(&self) -> PyResult<String> {
        Err(removed("to_json", "to_source()"))
    }

    #[staticmethod]
    fn from_json(_json: &str) -> PyResult<Self> {
        Err(removed("from_json", "Subcircuit.from_source(text, dialect)"))
    }

    fn save_json(&self, _path: &str) -> PyResult<()> {
        Err(removed("save_json", "save(path)"))
    }

    #[staticmethod]
    fn load_json(_path: &str) -> PyResult<Self> {
        Err(removed("load_json", "Subcircuit.load(path)"))
    }

    fn __str__(&self) -> String {
        format!("Subcircuit('{}', ports={:?})", self.inner.name, self.inner.ports)
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}
}

impl PySubcircuit {
    fn net(&self, n: &str) -> String {
        if self.ground_alias {
            top_net(n)
        } else {
            n.into()
        }
    }

    /// The nets joined by blanks, each as [`Self::net`] names it.
    fn nets(&self, ns: &[&str]) -> String {
        ns.iter().map(|n| self.net(n)).collect::<Vec<_>>().join(" ")
    }

    fn push(&mut self, name: &str, text: String) {
        self.inner.elements.push(Card::element(name, text));
    }

    /// `given` when it is the card name (`L1`) of an inductor of this body
    /// (an `L()` element, or a line given as text), for coupling `k`. SPICE
    /// names are case blind.
    fn inductor(&self, k: &str, given: &str) -> PyResult<String> {
        let has = |card: &str| {
            let cards = self.inner.elements.iter().map(|c| c.text.as_str());
            cards
                .chain(self.inner.raw.iter().map(String::as_str))
                .flat_map(str::lines)
                .filter_map(|l| l.split_whitespace().next())
                .any(|w| w.eq_ignore_ascii_case(card))
        };
        if given.starts_with(['L', 'l']) && has(given) {
            return Ok(given.to_string());
        }
        Err(PyValueError::new_err(format!(
            "K{k}: '{given}' names no inductor of '{}': give its full name (`L1`), added with L() first",
            self.inner.name
        )))
    }

    /// An R, C or L card: `R<name> n1 n2 value params`.
    fn two(
        &mut self,
        letter: char,
        name: &str,
        p: &str,
        n: &str,
        value: &Value,
        kwargs: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let nets = self.nets(&[p, n]);
        let ps = params(&kw_params(kwargs)?);
        self.push(name, format!("{letter}{name} {nets} {}{ps}", value.text()));
        Ok(())
    }

    /// A B source: `B<name> np nm V=expr` (`I=` with `of` `I`); a Laplace
    /// expression needs a simulator that has one.
    fn behavioral(&mut self, of: char, name: &str, p: &str, n: &str, expression: &str) {
        let nets = self.nets(&[p, n]);
        let mut c = Card::element(name, format!("B{name} {nets} {of}={expression}"));
        if is_laplace(expression) {
            c.needs = need::LAPLACE;
        }
        self.inner.elements.push(c);
    }

    /// An E (`letter` `E`) or G source given by an expression:
    /// `E<name> np nm value={expr}`, one pair of braces whether the caller
    /// wrote them or not; a Laplace expression needs a simulator that has
    /// one, as in [`Self::behavioral`].
    fn nonlinear(&mut self, letter: char, name: &str, p: &str, n: &str, expression: &str) {
        let nets = self.nets(&[p, n]);
        let value = PyValueArg::Expr(expression.into()).value().text();
        let mut c = Card::element(name, format!("{letter}{name} {nets} value={value}"));
        if is_laplace(expression) {
            c.needs = need::LAPLACE;
        }
        self.inner.elements.push(c);
    }

    /// Adds one waveform source; the generated `*Source` methods call it.
    fn place_wave(
        mut slf: PyRefMut<'_, Self>,
        current: bool,
        name: &str,
        p: &str,
        n: &str,
        wf: Wave,
        dc: Option<f64>,
        ac: Option<f64>,
        ac_phase: Option<f64>,
    ) {
        let c = wave_source(current, name, slf.net(p), slf.net(n), wf, dc, ac, ac_phase);
        slf.inner.elements.push(c);
    }
}

/// Whether `expr` calls `Laplace(...)` (any case): a transfer function only
/// some simulators have a source for.
fn is_laplace(expr: &str) -> bool {
    let lower = expr.to_ascii_lowercase();
    lower
        .find("laplace")
        .is_some_and(|i| lower[i + 7..].trim_start().starts_with('('))
}

/// Whether `raw`, the text after an R card's nodes, gives no resistance: it
/// is empty or opens with a parameter (`tc=1e-3`) other than `r=` (ngspice
/// manual §3.3.4 writes a behavioural resistor `R1 a b r='expr'`).
fn lacks_value(raw: &str) -> bool {
    raw.split_whitespace().next().is_none_or(|w| {
        w.split_once('=')
            .is_some_and(|(k, _)| !k.eq_ignore_ascii_case("r"))
    })
}
