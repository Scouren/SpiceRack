//! Cards: Python analysis arguments → one `Analysis` per card.
//! `CircuitSimulator.x()` runs it, `Testbench.add_x()` queues it; both
//! build it here. Cites the ngspice manual's analysis cards.
//!
//! Contents:
//! - `pub(super) fn dc_sweeps(kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Analysis>`
//! - `pub(super) fn sweep(variation: &str, points: u32, start: f64, stop: f64) -> PyResult<FreqSweep>`
//! - `pub(super) fn ac(variation, points, start, stop) -> PyResult<Analysis>`
//! - `pub(super) fn transient(step, stop, start, max_step, uic) -> Analysis`
//! - `pub(super) fn noise(output, reference, source, variation, points, start, stop, points_per_summary) -> PyResult<Analysis>`
//! - `pub(super) fn tf(output: &str, source: &str) -> PyResult<Analysis>`
//! - `pub(super) fn sensitivity(output: &str, ac: Option<FreqSweep>) -> Analysis`
//! - `pub(super) fn polezero(nodes: [&str; 4], tf_type: &str, pz_type: &str) -> PyResult<Analysis>`
//! - `pub(super) fn distortion(variation, points, start, stop, f2overf1) -> PyResult<Analysis>`
//! - `pub(super) fn spectre_sweep(param, start, stop, step, inner, inner_type) -> PyResult<Analysis>`
//! - `pub(super) fn spectre_monte_carlo(iterations, inner, inner_type, seed) -> PyResult<Analysis>`
//! - `pub(super) fn pss(fundamental, stabilization, observe_node, points_per_period, harmonics, sc_iter, steady_coeff, uic) -> Analysis`
//! - `pub(super) fn s_param(variation, points, start, stop) -> PyResult<Analysis>`
//! - `pub(super) fn harmonic_balance(frequencies: Vec<f64>, harmonics: Option<Vec<u32>>) -> Analysis`
//! - `pub(super) fn stability(probe, variation, points, start, stop) -> PyResult<Analysis>`
//! - `pub(super) fn network_params(output_current, input_source, z_in, z_out) -> String`
//! - `pub(super) fn spectre_statement(kind, name, nodes, params) -> Result<String, String>`

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use super::analysis::{Analysis, DcSweep, FreqSweep};

use crate::syntax::Lang;
use crate::syntax::statement::Statement;

/// Extract DC sweep params: kwargs like Vinput=slice(-2, 5, 0.01)
pub(super) fn dc_sweeps(kwargs: Option<Bound<'_, PyDict>>) -> PyResult<Analysis> {
    let mut sweeps = Vec::new();
    if let Some(dict) = kwargs {
        for (k, v) in dict.iter() {
            sweeps.push(DcSweep {
                source: k.extract::<String>()?,
                start: v.getattr("start")?.extract::<f64>()?,
                stop: v.getattr("stop")?.extract::<f64>()?,
                step: v.getattr("step")?.extract::<f64>()?,
            });
        }
    }
    if sweeps.is_empty() {
        return Err(PyValueError::new_err(
            "dc() requires at least one sweep parameter",
        ));
    }
    Ok(Analysis::Dc { sweeps })
}

/// Where a Python sweep name becomes a [`super::keywords::Sweep`]: anything
/// but dec, oct, lin or log (any case) is a `ValueError` here.
pub(super) fn sweep(variation: &str, points: u32, start: f64, stop: f64) -> PyResult<FreqSweep> {
    Ok(FreqSweep {
        variation: variation.parse().map_err(PyValueError::new_err)?,
        points,
        start,
        stop,
    })
}

/// `.ac`; `variation` per [`sweep`].
pub(super) fn ac(variation: &str, points: u32, start: f64, stop: f64) -> PyResult<Analysis> {
    Ok(Analysis::Ac {
        sweep: sweep(variation, points, start, stop)?,
    })
}

/// `.tran step stop [start [max_step]] [uic]`.
pub(super) fn transient(
    step: f64,
    stop: f64,
    start: Option<f64>,
    max_step: Option<f64>,
    uic: bool,
) -> Analysis {
    Analysis::Transient {
        step,
        stop,
        start,
        max_step,
        uic,
    }
}

/// `.noise`; `variation` per [`sweep`]. `output` and `reference` are nodes
/// (`out`, or `v(out)` unwrapped): the card wraps them in `V(…)` itself, so
/// any other form is a `ValueError` here.
pub(super) fn noise(
    output: &str,
    reference: &str,
    source: &str,
    variation: &str,
    points: u32,
    start: f64,
    stop: f64,
    points_per_summary: Option<u32>,
) -> PyResult<Analysis> {
    let sweep = sweep(variation, points, start, stop)?;
    Ok(Analysis::Noise {
        output: node(output)?.into(),
        reference: match reference {
            "" => String::new(),
            r => node(r)?.into(),
        },
        source: source.into(),
        sweep,
        points_per_summary,
    })
}

/// `.tf output source`. `output` is the card's own signal (`v(out)`,
/// `v(a,b)`, `i(vload)`; ngspice manual §11.3.9), kept as given, or a bare
/// node, written `v(node)` (ngspice refuses `.tf out Vin`: "voltage or
/// current expected"). Anything else is a `ValueError`.
pub(super) fn tf(output: &str, source: &str) -> PyResult<Analysis> {
    let t = output.trim();
    let signal = inside(t, &["v(", "V(", "i(", "I("])
        .is_some_and(|names| names.split(',').all(|n| plain(n.trim())));
    let output = if signal {
        t.to_string()
    } else {
        format!("v({})", node(t)?)
    };
    Ok(Analysis::Tf {
        output,
        source: source.into(),
    })
}

/// `.sens`: DC without `ac`, else over that AC sweep.
pub(super) fn sensitivity(output: &str, ac: Option<FreqSweep>) -> Analysis {
    Analysis::Sensitivity {
        output: output.into(),
        ac,
    }
}

/// `tf_type` is vol or cur, `pz_type` pol, zer or pz (any case); else
/// a `ValueError`.
pub(super) fn polezero(nodes: [&str; 4], tf_type: &str, pz_type: &str) -> PyResult<Analysis> {
    let (tf_type, pz_type) = (
        tf_type.parse().map_err(PyValueError::new_err)?,
        pz_type.parse().map_err(PyValueError::new_err)?,
    );
    Ok(Analysis::PoleZero {
        nodes: nodes.map(String::from),
        tf_type,
        pz_type,
    })
}

/// `.disto`; `variation` per [`sweep`].
pub(super) fn distortion(
    variation: &str,
    points: u32,
    start: f64,
    stop: f64,
    f2overf1: Option<f64>,
) -> PyResult<Analysis> {
    Ok(Analysis::Distortion {
        sweep: sweep(variation, points, start, stop)?,
        f2overf1,
    })
}

/// A Spectre `sweep` around `inner`; an `inner_type` that is not a Spectre
/// analysis kind is a `ValueError`.
pub(super) fn spectre_sweep(
    param: &str,
    start: f64,
    stop: f64,
    step: f64,
    inner: &str,
    inner_type: &str,
) -> PyResult<Analysis> {
    let inner_type = inner_type.parse().map_err(PyValueError::new_err)?;
    Ok(Analysis::SpectreSweep {
        param: param.into(),
        start,
        stop,
        step,
        inner: inner.into(),
        inner_type,
    })
}

/// A Spectre `montecarlo` around `inner`; `inner_type` as in
/// [`spectre_sweep`].
pub(super) fn spectre_monte_carlo(
    iterations: u32,
    inner: &str,
    inner_type: &str,
    seed: Option<u64>,
) -> PyResult<Analysis> {
    let inner_type = inner_type.parse().map_err(PyValueError::new_err)?;
    Ok(Analysis::SpectreMonteCarlo {
        iterations,
        inner: inner.into(),
        inner_type,
        seed,
    })
}

/// `.pss`, with ngspice's shooting controls (`sc_iter`, `steady_coeff`,
/// `uic`; manual §11.3.12).
#[allow(clippy::too_many_arguments)]
pub(super) fn pss(
    fundamental: f64,
    stabilization: f64,
    observe_node: &str,
    points_per_period: u32,
    harmonics: u32,
    sc_iter: Option<u32>,
    steady_coeff: Option<f64>,
    uic: bool,
) -> Analysis {
    Analysis::Pss {
        fundamental,
        stabilization,
        observe_node: observe_node.into(),
        points_per_period,
        harmonics,
        sc_iter,
        steady_coeff,
        uic,
    }
}

/// `.sp`; `variation` per [`sweep`].
pub(super) fn s_param(variation: &str, points: u32, start: f64, stop: f64) -> PyResult<Analysis> {
    Ok(Analysis::SPar {
        sweep: sweep(variation, points, start, stop)?,
    })
}

/// Harmonic balance over `frequencies`, seven harmonics each unless given.
pub(super) fn harmonic_balance(frequencies: Vec<f64>, harmonics: Option<Vec<u32>>) -> Analysis {
    let harmonics = harmonics.unwrap_or_else(|| vec![7; frequencies.len()]);
    Analysis::HarmonicBalance {
        frequencies,
        harmonics,
    }
}

/// Spectre `stb` around `probe`; `variation` per [`sweep`].
pub(super) fn stability(
    probe: &str,
    variation: &str,
    points: u32,
    start: f64,
    stop: f64,
) -> PyResult<Analysis> {
    Ok(Analysis::Stability {
        probe: probe.into(),
        sweep: sweep(variation, points, start, stop)?,
    })
}

/// A Spectre analysis statement, `name [(nodes)] kind key=value …`, the
/// text a `simulator lang=spectre` region holds (Spectre Reference 19.1
/// ch.3: `Name ([node1] [node2]) acmatch parameter=value ...` p.49,
/// `dcmm1 dcmatch mth=1e-3 oprobe=vd porti=1` p.73). `name` defaults to
/// `<kind>1`; a value is written as given.
///
/// # Errors
/// A `kind` the Spectre dialect's statement table has no analysis for; a
/// name or key that is no identifier; a node that is empty or holds a blank
/// or a parenthesis; a value that is empty (a `True` flag: Spectre's
/// booleans are `yes` and `no`) or holds a line break.
pub(super) fn spectre_statement(
    kind: &str,
    name: Option<&str>,
    nodes: &[String],
    params: &[(String, String)],
) -> Result<String, String> {
    let spectre = Lang::SPECTRE.lexicon().expect("Spectre has a reader");
    if !matches!(spectre.master(kind), Some(Statement::Analysis(_))) {
        return Err(format!("'{kind}' is no Spectre analysis"));
    }
    let word = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    let mut out = name.map_or_else(|| format!("{kind}1"), str::to_string);
    if !word(&out) {
        return Err(format!("{kind}: '{out}' is no statement name"));
    }
    let blank = |c: char| c.is_whitespace() || c == '(' || c == ')';
    if let Some(n) = nodes.iter().find(|n| n.is_empty() || n.contains(blank)) {
        return Err(format!("{kind}: '{n}' is no node name"));
    }
    if !nodes.is_empty() {
        out.push_str(&format!(" ({})", nodes.join(" ")));
    }
    out.push_str(&format!(" {kind}"));
    for (k, v) in params {
        if !word(k) {
            return Err(format!("{kind}: '{k}' is no parameter name"));
        }
        if v.is_empty() || v.contains(['\n', '\r']) {
            return Err(format!(
                "{kind}: {k}= takes a value on one line (Spectre's booleans are yes and no)"
            ));
        }
        out.push_str(&format!(" {k}={v}"));
    }
    Ok(out)
}

/// LTspice's `.net` card (no other dialect has one), beside an `.ac`.
pub(super) fn network_params(
    output_current: &str,
    input_source: &str,
    z_in: f64,
    z_out: f64,
) -> String {
    format!(".net {output_current} {input_source} Rout={z_out} Rin={z_in}")
}

// ---- private ----

/// What `t` wraps when it is `<prefix>…)` for one of `prefixes`.
fn inside<'a>(t: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes
        .iter()
        .find_map(|p| t.strip_prefix(*p))
        .and_then(|r| r.strip_suffix(')'))
}

/// One node or element name as a card takes it: not empty, with no space,
/// parenthesis, comma or `=`.
fn plain(s: &str) -> bool {
    !s.is_empty() && !s.contains(|c: char| c.is_whitespace() || "(),=".contains(c))
}

/// The node `output` names: `out`, or `v(out)` / `V(out)` unwrapped. A
/// `ValueError` for anything else: wrapped again by the card it would be
/// `V(v(a,b))`, which ngspice-45 reads as a node no element touches and
/// reports as 0.
fn node(output: &str) -> PyResult<&str> {
    let t = output.trim();
    let n = inside(t, &["v(", "V("]).map_or(t, str::trim);
    if plain(n) {
        Ok(n)
    } else {
        Err(PyValueError::new_err(format!(
            "'{output}' is not a node: pass its name (out) or v(out)"
        )))
    }
}
