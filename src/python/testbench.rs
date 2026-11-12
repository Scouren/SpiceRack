//! Testbench: a DUT plus stimulus, PDK libraries and queued analyses →
//! one multi-analysis deck, and its run → one typed result per analysis.
//! `ModelLibrary` is the PDK input it takes.
//!
//! Contents:
//! - `pub(super) struct PyTestbench` (Python `Testbench`, extends `CircuitSimulator`)
//! - `pub(super) struct PyModelLibrary` (Python `ModelLibrary`)

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// `either`, `Num`, `Wave`, `Value`: names `pymethods_with_waves!` resolves here.
use super::analysis::{Analysis, PssBase};
use super::args::{Num, PyValueArg, kw_params, param_text};
use super::card::{self, dc_sweeps};
use super::circuit::{PyCircuit, hoisted};
use super::deck::{Deck, check_backend_name};
use super::model::{Card, Func, ModelLibrary, Value, params};
use super::results::typed_result;
use super::simulator::PySimulator;
use super::source::{Wave, either, source, wave_source};
use super::subcircuit::{PySubcircuit, top_net};

#[pyclass(name = "Testbench", extends = PySimulator)]
pub(super) struct PyTestbench {}

#[pyclass(name = "ModelLibrary")]
pub(super) struct PyModelLibrary {
    inner: ModelLibrary,
}

// ---- private ----

pymethods_with_waves! {
#[pymethods]
impl PyTestbench {
    /// `dut`: a `Subcircuit` (the deck's top), or a `Circuit`, a whole
    /// deck: the definitions its `subcircuit()` registered, its
    /// `options()` and its `temp()` come with its top.
    #[new]
    fn new(dut: &Bound<'_, PySubcircuit>) -> PyResult<(Self, PySimulator)> {
        let deck = match dut.as_any().cast::<PyCircuit>() {
            Ok(circuit) => PyCircuit::deck(&circuit.try_borrow()?),
            Err(_) => Deck { top: dut.try_borrow()?.inner.clone(), bench: Some(Default::default()), ..Default::default() },
        };
        Ok((Self {}, PySimulator { deck, backend: None, save_currents: false }))
    }

    /// Register a child definition (its include/lib/osdi are hoisted to the top level).
    fn add_subcircuit(mut slf: PyRefMut<'_, Self>, subckt: &PySubcircuit) {
        let deck = &mut slf.as_super().deck;
        let def = hoisted(&mut deck.top, &subckt.inner);
        deck.defs.push(def);
    }

    /// Attach a PDK model library. Codegen resolves the per-backend path and corner.
    fn use_pdk(mut slf: PyRefMut<'_, Self>, pdk: &PyModelLibrary) {
        slf.as_super().deck.libraries.push(pdk.inner.clone());
    }

    /// A `.param name=value` at the top of the deck, the value as
    /// `Subcircuit.parameter` takes it. A parameter the deck has by that
    /// name (the DUT's own, or an earlier call's) takes the new value: a
    /// deck holds one card per name, in the place of the first.
    fn parameter(mut slf: PyRefMut<'_, Self>, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let text = param_text(name, value)?;
        let params = &mut slf.as_super().deck.top.params;
        match params.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            Some(slot) => slot.1 = text,
            None => params.push((name.into(), text)),
        }
        Ok(())
    }

    // ── Stimulus ──

    /// `distof1=(mag, phase)`, `distof2=`: the source's `distortion` input
    /// at F1 and F2, as `Subcircuit.V`'s.
    #[pyo3(signature = (name, positive, negative, value, *, ac=None, ac_phase=None, portnum=None, z0=50.0, distof1=None, distof2=None))]
    fn V(mut slf: PyRefMut<'_, Self>, name: &str, positive: &str, negative: &str, value: PyValueArg, ac: Option<Num>, ac_phase: Option<Num>, portnum: Option<u32>, z0: f64, distof1: Option<(f64, f64)>, distof2: Option<(f64, f64)>) {
        let port = portnum.map(|n| (n, z0));
        let v = source(false, name, top_net(positive), top_net(negative), value.value(), ac, ac_phase, [distof1, distof2], port, None);
        slf.as_super().stim(v);
    }

    /// `distof1`, `distof2`: as `V`'s.
    #[pyo3(signature = (name, positive, negative, value, *, ac=None, ac_phase=None, distof1=None, distof2=None, **kwargs))]
    fn I(mut slf: PyRefMut<'_, Self>, name: &str, positive: &str, negative: &str, value: PyValueArg, ac: Option<Num>, ac_phase: Option<Num>, distof1: Option<(f64, f64)>, distof2: Option<(f64, f64)>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let ps = params(&kw_params(kwargs)?);
        let c = source(true, name, top_net(positive), top_net(negative), value.value(), ac, ac_phase, [distof1, distof2], None, None);
        slf.as_super().stim(c.with_params(&ps));
        Ok(())
    }

    #[pyo3(signature = (name, positive, negative, value))]
    fn R(mut slf: PyRefMut<'_, Self>, name: &str, positive: &str, negative: &str, value: PyValueArg) {
        let text = format!("R{name} {} {} {}", top_net(positive), top_net(negative), value.value().text());
        slf.as_super().stim(Card::element(name, text));
    }

    #[pyo3(signature = (name, positive, negative, value))]
    fn C(mut slf: PyRefMut<'_, Self>, name: &str, positive: &str, negative: &str, value: PyValueArg) {
        let text = format!("C{name} {} {} {}", top_net(positive), top_net(negative), value.value().text());
        slf.as_super().stim(Card::element(name, text));
    }

    // ── Config ──

    fn with_backend(mut slf: PyRefMut<'_, Self>, name: &str) -> PyResult<()> {
        check_backend_name(name)?;
        slf.as_super().backend = Some(name.to_string());
        Ok(())
    }

    /// A line given as `dialect`'s text, written byte for byte on a target
    /// of that dialect's family and refused (`W0001`) on any other.
    #[pyo3(signature = (line, dialect="ngspice"))]
    fn extra_line(mut slf: PyRefMut<'_, Self>, line: &str, dialect: &str) -> PyResult<()> {
        let d = super::deck::region_dialect(dialect).map_err(PyValueError::new_err)?;
        slf.as_super().tb().extra_lines.push((line.to_string(), d));
        Ok(())
    }

    /// A `.print` card: `add_print("tran", "v(out)", "i(vdd)")`; `analysis`
    /// may be `""`. Simulators that print nothing in a rawfile run omit it.
    #[pyo3(signature = (analysis, *outputs))]
    fn add_print(mut slf: PyRefMut<'_, Self>, analysis: &str, outputs: Vec<String>) {
        slf.as_super().tb().prints.push((analysis.to_ascii_lowercase(), outputs));
    }

    /// A `.func`: `add_function("f", ["x"], "x*2")`.
    #[pyo3(signature = (name, args, body))]
    fn add_function(mut slf: PyRefMut<'_, Self>, name: &str, args: Vec<String>, body: &str) {
        slf.as_super().deck.functions.push(Func { name: name.into(), args, body: body.into() });
    }

    /// `.global` nodes: one net under these names in every subcircuit.
    #[pyo3(signature = (*nodes))]
    fn global_nodes(mut slf: PyRefMut<'_, Self>, nodes: Vec<String>) {
        let g = &mut slf.as_super().deck.globals;
        g.extend(nodes.into_iter().filter(|n| !g.contains(n)).collect::<Vec<_>>());
    }

    // ── Analysis builders for multi-analysis decks ──

    fn add_operating_point(mut slf: PyRefMut<'_, Self>) {
        slf.as_super().tb().analyses.push(Analysis::Op);
    }

    #[pyo3(signature = (**kwargs))]
    fn add_dc(mut slf: PyRefMut<'_, Self>, kwargs: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let a = dc_sweeps(kwargs)?;
        slf.as_super().tb().analyses.push(a);
        Ok(())
    }

    #[pyo3(signature = (variation="dec", number_of_points=10, start_frequency=1.0, stop_frequency=1e9))]
    fn add_ac(mut slf: PyRefMut<'_, Self>, variation: &str, number_of_points: u32, start_frequency: f64, stop_frequency: f64) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::ac(variation, number_of_points, start_frequency, stop_frequency)?);
        Ok(())
    }

    #[pyo3(signature = (step_time, end_time, start_time=None, max_time=None, use_initial_condition=false))]
    fn add_transient(mut slf: PyRefMut<'_, Self>, step_time: f64, end_time: f64, start_time: Option<f64>, max_time: Option<f64>, use_initial_condition: bool) {
        slf.as_super().tb().analyses.push(card::transient(step_time, end_time, start_time, max_time, use_initial_condition));
    }

    #[pyo3(signature = (outvar, insrc))]
    fn add_transfer_function(mut slf: PyRefMut<'_, Self>, outvar: &str, insrc: &str) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::tf(outvar, insrc)?);
        Ok(())
    }

    #[pyo3(signature = (outvar, insrc))]
    fn add_tf(slf: PyRefMut<'_, Self>, outvar: &str, insrc: &str) -> PyResult<()> {
        Self::add_transfer_function(slf, outvar, insrc)
    }

    #[pyo3(signature = (output_variable))]
    fn add_dc_sensitivity(mut slf: PyRefMut<'_, Self>, output_variable: &str) {
        slf.as_super().tb().analyses.push(card::sensitivity(output_variable, None));
    }

    #[pyo3(signature = (output_variable, variation="dec", number_of_points=10, start_frequency=100.0, stop_frequency=1e5))]
    fn add_ac_sensitivity(mut slf: PyRefMut<'_, Self>, output_variable: &str, variation: &str, number_of_points: u32, start_frequency: f64, stop_frequency: f64) -> PyResult<()> {
        let a = card::sensitivity(output_variable, Some(card::sweep(variation, number_of_points, start_frequency, stop_frequency)?));
        slf.as_super().tb().analyses.push(a);
        Ok(())
    }

    #[pyo3(signature = (node1, node2, node3, node4, tf_type, pz_type))]
    fn add_polezero(mut slf: PyRefMut<'_, Self>, node1: &str, node2: &str, node3: &str, node4: &str, tf_type: &str, pz_type: &str) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::polezero([node1, node2, node3, node4], tf_type, pz_type)?);
        Ok(())
    }

    #[pyo3(signature = (variation="dec", points=10, start_frequency=100.0, stop_frequency=1e8, f2overf1=None))]
    fn add_distortion(mut slf: PyRefMut<'_, Self>, variation: &str, points: u32, start_frequency: f64, stop_frequency: f64, f2overf1: Option<f64>) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::distortion(variation, points, start_frequency, stop_frequency, f2overf1)?);
        Ok(())
    }

    #[pyo3(signature = (output_node, ref_node, src, variation="dec", points=10, start_frequency=1e3, stop_frequency=1e8, points_per_summary=None))]
    fn add_noise(
        mut slf: PyRefMut<'_, Self>, output_node: &str, ref_node: &str, src: &str, variation: &str, points: u32,
        start_frequency: f64, stop_frequency: f64, points_per_summary: Option<u32>,
    ) -> PyResult<()> {
        let a = card::noise(output_node, ref_node, src, variation, points, start_frequency, stop_frequency, points_per_summary)?;
        slf.as_super().tb().analyses.push(a);
        Ok(())
    }

    #[pyo3(signature = (fundamental_frequency, outputs, num_harmonics=None))]
    fn add_fourier(mut slf: PyRefMut<'_, Self>, fundamental_frequency: f64, outputs: Vec<String>, num_harmonics: Option<u32>) {
        let a = Analysis::Fourier { fundamental: fundamental_frequency, outputs, harmonics: num_harmonics };
        slf.as_super().tb().analyses.push(a);
    }

    #[pyo3(signature = (param, start, stop, step, inner_analysis, inner_type))]
    fn add_spectre_sweep(mut slf: PyRefMut<'_, Self>, param: &str, start: f64, stop: f64, step: f64, inner_analysis: &str, inner_type: &str) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::spectre_sweep(param, start, stop, step, inner_analysis, inner_type)?);
        Ok(())
    }

    #[pyo3(signature = (num_iterations, inner_analysis, inner_type, seed=None))]
    fn add_spectre_monte_carlo(mut slf: PyRefMut<'_, Self>, num_iterations: u32, inner_analysis: &str, inner_type: &str, seed: Option<u64>) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::spectre_monte_carlo(num_iterations, inner_analysis, inner_type, seed)?);
        Ok(())
    }

    #[pyo3(signature = (fundamental_frequency, stabilization_time, observe_node, points_per_period=128, harmonics=10, sc_iter=None, steady_coeff=None, uic=false))]
    fn add_pss(mut slf: PyRefMut<'_, Self>, fundamental_frequency: f64, stabilization_time: f64, observe_node: &str, points_per_period: u32, harmonics: u32, sc_iter: Option<u32>, steady_coeff: Option<f64>, uic: bool) {
        let a = card::pss(fundamental_frequency, stabilization_time, observe_node, points_per_period, harmonics, sc_iter, steady_coeff, uic);
        slf.as_super().tb().analyses.push(a);
    }

    #[pyo3(signature = (variation="dec", number_of_points=10, start_frequency=1e6, stop_frequency=1e10))]
    fn add_s_param(mut slf: PyRefMut<'_, Self>, variation: &str, number_of_points: u32, start_frequency: f64, stop_frequency: f64) -> PyResult<()> {
        let a = card::s_param(variation, number_of_points, start_frequency, stop_frequency)?;
        let a = slf.as_super().with_port(a)?;
        slf.as_super().tb().analyses.push(a);
        Ok(())
    }

    /// LTspice `.net` over an `.ac`: the `.ac` queued, `.net` an LTspice line.
    #[pyo3(signature = (output_current, input_source, z_in=50.0, z_out=50.0, variation="dec", points=100, start_freq=1e3, stop_freq=1e9))]
    fn add_network_params(mut slf: PyRefMut<'_, Self>, output_current: &str, input_source: &str, z_in: f64, z_out: f64, variation: &str, points: u32, start_freq: f64, stop_freq: f64) -> PyResult<()> {
        let ac = card::ac(variation, points, start_freq, stop_freq)?;
        let tb = slf.as_super().tb();
        tb.analyses.push(ac);
        tb.extra_lines.push((card::network_params(output_current, input_source, z_in, z_out), crate::syntax::Dialect::Ltspice.into()));
        Ok(())
    }

    #[pyo3(signature = (fundamental_frequencies, num_harmonics=None))]
    fn add_harmonic_balance(mut slf: PyRefMut<'_, Self>, fundamental_frequencies: Vec<f64>, num_harmonics: Option<card::Harmonics>) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::harmonic_balance(fundamental_frequencies, num_harmonics)?);
        Ok(())
    }

    #[pyo3(signature = (probe, variation="dec", number_of_points=10, start_frequency=1.0, stop_frequency=1e10))]
    fn add_stability(mut slf: PyRefMut<'_, Self>, probe: &str, variation: &str, number_of_points: u32, start_frequency: f64, stop_frequency: f64) -> PyResult<()> {
        slf.as_super().tb().analyses.push(card::stability(probe, variation, number_of_points, start_frequency, stop_frequency)?);
        Ok(())
    }

    #[pyo3(signature = (step_time, end_time))]
    fn add_transient_noise(mut slf: PyRefMut<'_, Self>, step_time: f64, end_time: f64) {
        slf.as_super().tb().analyses.push(Analysis::TransientNoise { step: step_time, stop: end_time });
    }

    #[pyo3(signature = (pss_fundamental, pss_stabilization, pss_harmonics=10, variation="dec", points=100, start_freq=1.0, stop_freq=1e9, sweep_type="relative"))]
    fn add_spectre_pac(mut slf: PyRefMut<'_, Self>, pss_fundamental: f64, pss_stabilization: f64, pss_harmonics: u32, variation: &str, points: u32, start_freq: f64, stop_freq: f64, sweep_type: &str) -> PyResult<()> {
        let pss = PssBase { fundamental: pss_fundamental, stabilization: pss_stabilization, harmonics: pss_harmonics };
        let sweep = card::sweep(variation, points, start_freq, stop_freq)?;
        slf.as_super().tb().analyses.push(Analysis::SpectrePac { pss, sweep, sweep_type: sweep_type.into() });
        Ok(())
    }

    #[pyo3(signature = (pss_fundamental, pss_stabilization, output_node, ref_node, pss_harmonics=10, variation="dec", points=100, start_freq=1.0, stop_freq=1e9))]
    fn add_spectre_pnoise(mut slf: PyRefMut<'_, Self>, pss_fundamental: f64, pss_stabilization: f64, output_node: &str, ref_node: &str, pss_harmonics: u32, variation: &str, points: u32, start_freq: f64, stop_freq: f64) -> PyResult<()> {
        let pss = PssBase { fundamental: pss_fundamental, stabilization: pss_stabilization, harmonics: pss_harmonics };
        let sweep = card::sweep(variation, points, start_freq, stop_freq)?;
        slf.as_super().tb().analyses.push(Analysis::SpectrePnoise { pss, output: output_node.into(), reference: ref_node.into(), sweep });
        Ok(())
    }

    /// `source` is accepted as on `spectre_pxf` and not written: `pxf`
    /// names no input source (`[REF]` p.303-304).
    #[pyo3(signature = (pss_fundamental, pss_stabilization, output_node, source, pss_harmonics=10, variation="dec", points=100, start_freq=1.0, stop_freq=1e9))]
    fn add_spectre_pxf(mut slf: PyRefMut<'_, Self>, pss_fundamental: f64, pss_stabilization: f64, output_node: &str, source: &str, pss_harmonics: u32, variation: &str, points: u32, start_freq: f64, stop_freq: f64) -> PyResult<()> {
        let _ = source;
        let pss = PssBase { fundamental: pss_fundamental, stabilization: pss_stabilization, harmonics: pss_harmonics };
        let sweep = card::sweep(variation, points, start_freq, stop_freq)?;
        slf.as_super().tb().analyses.push(Analysis::SpectrePxf { pss, output: output_node.into(), sweep });
        Ok(())
    }

    #[pyo3(signature = (pss_fundamental, pss_stabilization, probe, pss_harmonics=10, variation="dec", points=100, start_freq=1.0, stop_freq=1e9))]
    fn add_spectre_pstb(mut slf: PyRefMut<'_, Self>, pss_fundamental: f64, pss_stabilization: f64, probe: &str, pss_harmonics: u32, variation: &str, points: u32, start_freq: f64, stop_freq: f64) -> PyResult<()> {
        let pss = PssBase { fundamental: pss_fundamental, stabilization: pss_stabilization, harmonics: pss_harmonics };
        let sweep = card::sweep(variation, points, start_freq, stop_freq)?;
        slf.as_super().tb().analyses.push(Analysis::SpectrePstb { pss, probe: probe.into(), sweep });
        Ok(())
    }

    /// Any other analysis of Spectre's own, as its statement `name
    /// [(nodes)] kind key=value …` in the deck's Spectre region:
    /// `add_spectre_analysis("dcmatch", "dcmm3", ("out", "0"), mth=1e-3)`
    /// (Spectre Reference 19.1 ch.3). Spectre writes it, a target with no
    /// such analysis refuses it (`W0015`). `name` defaults to `<kind>1`; a
    /// value is written as given (`stats="yes"`). `ValueError` for a kind
    /// Spectre does not name, and per [`card::spectre_statement`]. It is
    /// the deck's text: `run()` returns no result for it.
    #[pyo3(signature = (kind, name=None, nodes=None, **params))]
    fn add_spectre_analysis(mut slf: PyRefMut<'_, Self>, kind: &str, name: Option<&str>, nodes: Option<Vec<String>>, params: Option<Bound<'_, PyDict>>) -> PyResult<()> {
        let params = super::args::kw_params(params)?;
        let line = card::spectre_statement(kind, name, &nodes.unwrap_or_default(), &params).map_err(PyValueError::new_err)?;
        slf.as_super().tb().extra_lines.push((line, crate::syntax::Lang::SPECTRE));
        Ok(())
    }

    /// Runs every queued analysis (`add_*`) as one deck and returns one
    /// result per analysis, in queue order, each the type its single-analysis
    /// twin returns (a list of one per step for an LTspice `.step` run) (`op, tran = tb.run()`); `None` for `add_fourier`, whose
    /// tables are the same run's transient result's `fourier`. `ValueError` with nothing queued.
    fn run(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let sim = slf.as_super();
        let deck = sim.deck_as_queued();
        let analyses = deck.bench.as_ref().map(|tb| tb.analyses.clone()).unwrap_or_default();
        if analyses.is_empty() {
            return Err(PyValueError::new_err("no analyses queued: add one with add_operating_point, add_transient, ..."));
        }
        let plots = deck.run_each(sim.backend.as_deref())?;
        assert_eq!(plots.len(), analyses.len(), "run_each returns one entry per analysis");
        analyses.iter().zip(plots).map(|(a, raw)| typed_result(py, a, raw)).collect()
    }

    /// The deck's own ngspice source text, the one every backend's netlist
    /// is written from (its regions hold other dialects' text), behind a
    /// `* spicerack-source ngspice` line; its loads a `pre_osdi` block.
    fn to_source(slf: PyRef<'_, Self>) -> PyResult<String> {
        slf.as_super().deck.to_source()
    }

    /// The testbench deck `text` holds (`dialect` as for
    /// `Subcircuit.from_source`): its statements kept as written, so its
    /// netlists are the ones the testbench that wrote it gave; calls on it
    /// add after them. Its analyses are text: `run()` runs the ones queued
    /// on it.
    #[staticmethod]
    #[pyo3(signature = (text, dialect=None))]
    fn from_source(py: Python<'_>, text: &str, dialect: Option<&str>) -> PyResult<Py<Self>> {
        let d = dialect.map(super::def_::parse_lang).transpose().map_err(PyValueError::new_err)?;
        let top = super::store::read_deck(text, d).map_err(PyValueError::new_err)?;
        let libraries = super::store::libraries(text);
        let deck = Deck { top, libraries, bench: Some(Default::default()), ..Default::default() };
        let sim = PySimulator { deck, backend: None, save_currents: false };
        Py::new(py, PyClassInitializer::from(sim).add_subclass(Self {}))
    }

    /// Writes [`Self::to_source`] to `path`. Not `save(path)`: `save(*outputs)`
    /// is the `.save` card every simulator call takes.
    fn save_source(slf: PyRef<'_, Self>, path: &str) -> PyResult<()> {
        let text = slf.as_super().deck.to_source()?;
        super::subcircuit::write_file(path, &text)
    }

    /// The testbench the file at `path` holds (as [`Self::from_source`]).
    #[staticmethod]
    fn load(py: Python<'_>, path: &str) -> PyResult<Py<Self>> {
        Self::from_source(py, &super::subcircuit::read_file(path)?, None)
    }

    // The JSON form went with the IR (D3: the store is source text).
    fn to_json(_slf: PyRef<'_, Self>) -> PyResult<String> {
        Err(super::subcircuit::removed("to_json", "to_source()"))
    }

    #[staticmethod]
    fn from_json(_json: &str) -> PyResult<Py<Self>> {
        Err(super::subcircuit::removed("from_json", "Testbench.from_source(text, dialect)"))
    }

    fn save_json(_slf: PyRef<'_, Self>, _path: &str) -> PyResult<()> {
        Err(super::subcircuit::removed("save_json", "save_source(path)"))
    }

    #[staticmethod]
    fn load_json(_path: &str) -> PyResult<Py<Self>> {
        Err(super::subcircuit::removed("load_json", "Testbench.load(path)"))
    }

    #[pyo3(signature = (backend="ngspice"))]
    fn netlist(slf: PyRef<'_, Self>, backend: &str) -> PyResult<String> {
        slf.as_super().deck.netlist(backend)
    }

    /// `netlist(dialect)`'s deck and its notes, as `Diagnostic`s: what the
    /// target does not get as written. `netlist()` raises each as a
    /// `UserWarning`; this returns them and warns of none.
    fn netlist_with_notes(
        slf: PyRef<'_, Self>,
        dialect: &str,
    ) -> PyResult<(String, Vec<super::error::PyDiagnostic>)> {
        let written = slf.as_super().deck.written(dialect)?;
        Ok((written.text, written.notes))
    }

    fn __str__(slf: PyRef<'_, Self>) -> String {
        let deck = &slf.as_super().deck;
        let n = deck.bench.as_ref().map_or(0, |tb| tb.analyses.len());
        format!("Testbench(dut='{}', analyses={})", deck.top.name, n)
    }

    fn __repr__(slf: PyRef<'_, Self>) -> String {
        Self::__str__(slf)
    }
}
}

impl PyTestbench {
    /// Adds one waveform source to the stimulus; the generated `*Source`
    /// methods call it.
    fn place_wave(
        mut slf: PyRefMut<'_, Self>,
        current: bool,
        name: &str,
        p: &str,
        n: &str,
        wf: Wave,
        dc: Option<Num>,
        ac: Option<Num>,
        ac_phase: Option<Num>,
    ) {
        slf.as_super().stim(wave_source(
            current,
            name,
            top_net(p),
            top_net(n),
            wf,
            dc,
            ac,
            ac_phase,
        ));
    }
}

#[pymethods]
impl PyModelLibrary {
    #[new]
    #[pyo3(signature = (path, corner=None, setup_includes=None, **backend_paths))]
    fn new(
        path: &str,
        corner: Option<&str>,
        setup_includes: Option<Vec<String>>,
        backend_paths: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut bp = std::collections::BTreeMap::new();
        if let Some(dict) = backend_paths {
            for (k, v) in dict.iter() {
                bp.insert(k.extract::<String>()?, v.extract::<String>()?);
            }
        }
        Ok(Self {
            inner: ModelLibrary {
                name: std::path::Path::new(path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("library")
                    .to_string(),
                path: path.to_string(),
                corner: corner.map(str::to_string),
                backend_paths: bp,
                setup_includes: setup_includes.unwrap_or_default(),
                placed: false,
            },
        })
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn path(&self) -> String {
        self.inner.path.clone()
    }

    #[getter]
    fn corner(&self) -> Option<String> {
        self.inner.corner.clone()
    }

    fn __str__(&self) -> String {
        format!(
            "ModelLibrary('{}', corner={:?})",
            self.inner.path, self.inner.corner
        )
    }

    fn __repr__(&self) -> String {
        self.__str__()
    }
}
