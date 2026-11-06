//! Deck: a Python circuit's blocks → its ngspice source text (the deck's
//! own text: the parser reads it, `syntax::render` writes it for the
//! target) → the netlist one backend runs, or the `RawData` of running it.
//! Verilog blocks are lowered here first, so the text holds what simulates
//! them.
//!
//! Each statement is spelled in a dialect that has it: ngspice's card, or,
//! for what only Spectre says (the analyses with no ngspice card, and
//! `pss` on a Spectre target: `Analysis::in_spectre_region`), its Spectre
//! statement in a `simulator lang=spectre` region of the source
//! (`Lang::NGSPICE_GIVEN`); a Fourier analysis's harmonic count is
//! ngspice's `.options nfreqs=` card, or the analysis in LTspice's or
//! Spectre's words in its region (`Analysis::fourier_count`). A line
//! given as one dialect's text (`extra_line`) sits in that dialect's
//! region. The renderer writes a region's text for its own dialect family
//! and refuses it (`W0001`) for any other; nothing is added to the text it
//! writes. ngspice's `ngbehavior` is an `.options ngbehavior=` card.
//! Beside the text, in `render::Source`: the top's Verilog-A and OSDI
//! loads (ngspice loads them from a `.control` command, which the reader
//! keeps no load of).
//!
//! Cites: ngspice manual ch.2 (deck order: title first, `.end` last).
//!
//! Contents:
//! - `pub(super) struct Deck`
//! - `Deck::netlist(&self, backend: &str) -> PyResult<String>`,
//!   `Deck::run(&self, backend: Option<&str>) -> PyResult<RawData>`,
//!   `Deck::run_each(&self, backend: Option<&str>) -> PyResult<Vec<Option<RawData>>>`,
//!   `Deck::to_source(&self) -> PyResult<String>`, `Deck::issues(&self, backend: &str) -> Vec<String>`,
//!   `Deck::card(body: &Body, name: &str) -> PyResult<String>`
//! - `pub(super) fn check_backend_name(name: &str) -> PyResult<()>`
//! - `pub(super) fn region_dialect(name: &str) -> Result<Dialect, String>`
//! - `pub(super) fn runtime_err(e: impl std::fmt::Display) -> PyErr`

use crate::syntax::Lang;

use std::borrow::Cow;
use std::fmt::Write as _;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

use super::analysis::{Analysis, nfreqs};
use super::model::{Bench, Body, Func, ModelLibrary, dec, params};
use super::verilog::lower_verilog;
use crate::backend::{Dialect, Kind, feature};
use crate::result::RawData;
use crate::syntax::render;

/// A whole deck as Python built it: the top (a `Circuit`, or a
/// `Testbench`'s DUT), its subcircuit definitions, and the testbench.
#[derive(Clone, Debug, Default)]
pub(super) struct Deck {
    pub(super) top: Body,
    pub(super) defs: Vec<Body>,
    pub(super) bench: Option<Bench>,
    pub(super) libraries: Vec<ModelLibrary>,
    pub(super) functions: Vec<Func>,
    /// `.global` nodes.
    pub(super) globals: Vec<String>,
}

impl Deck {
    /// The deck written in `backend`'s dialect: every dialect the renderer
    /// writes (`render::writes`), runnable here or not (HSPICE, Xyce, CDL,
    /// Verilog are written, never run).
    pub(super) fn netlist(&self, backend: &str) -> PyResult<String> {
        let d = Dialect::from_backend_name(&backend.to_ascii_lowercase())
            .filter(|d| render::writes(*d))
            .ok_or_else(|| PyValueError::new_err(format!("unknown backend '{backend}'")))?;
        self.lowered()?.write(d).map_err(PyRuntimeError::new_err)
    }

    /// Runs the deck on `backend` (`None`: the best installed one for its
    /// first analysis and its features); the first plot.
    pub(super) fn run(&self, backend: Option<&str>) -> PyResult<RawData> {
        let (sim, text) = self.for_run(backend)?;
        crate::simulation::run(sim, &text).map_err(runtime_err)
    }

    /// Runs the deck once; one plot per queued analysis, in queue order.
    pub(super) fn run_each(&self, backend: Option<&str>) -> PyResult<Vec<Option<RawData>>> {
        let (sim, text) = self.for_run(backend)?;
        let kinds: Vec<Kind> = self.analyses().iter().map(Analysis::kind).collect();
        crate::simulation::run_each(sim, &text, &kinds).map_err(runtime_err)
    }

    /// The deck's stored source (`store::with_loads`): its ngspice text,
    /// a model library at its ngspice path behind the store's line of its
    /// every path (`store::library_marker`).
    pub(super) fn to_source(&self) -> PyResult<String> {
        let deck = self.lowered()?;
        let text = deck
            .source(Dialect::Ngspice)
            .map_err(PyRuntimeError::new_err)?;
        let mut out = String::with_capacity(text.len() + 256);
        let mut fresh: Vec<&ModelLibrary> = deck.libraries.iter().filter(|l| !l.placed).collect();
        for l in text.lines() {
            if let Some(i) = fresh.iter().position(|lib| lib.line(&lib.path) == l) {
                out.push_str(&super::store::library_marker(fresh.remove(i)));
            }
            out.push_str(l);
            out.push('\n');
        }
        Ok(super::store::with_loads(&out, &deck.top.osdi))
    }

    /// `backend::check` as `"Severity: message"` lines, then the writer's
    /// refusal of the deck for `backend`, if it refuses it.
    pub(super) fn issues(&self, backend: &str) -> Vec<String> {
        let elements = self.bodies().map(|b| b.elements.len() + b.instances.len());
        let mut out: Vec<String> = crate::backend::check(self.needs(), elements.sum(), backend)
            .into_iter()
            .map(|issue| format!("{:?}: {}", issue.severity, issue.message))
            .collect();
        let refused =
            crate::backend::parse(backend).map(|d| self.lowered().map(|deck| deck.write(d)));
        if let Some(Ok(Err(why))) = refused {
            out.push(format!(
                "Error: Backend '{backend}' does not write this deck: {why}"
            ));
        }
        out
    }

    /// The ngspice card of `body`'s first element or instance called
    /// `name` (without its letter), as a deck holding it alone writes it.
    pub(super) fn card(body: &Body, name: &str) -> PyResult<String> {
        let c = body.elements.iter().chain(&body.instances);
        let Some(c) = c.into_iter().find(|c| c.name == name) else {
            return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                "Element '{name}' not found"
            )));
        };
        let one = Deck {
            top: Body {
                elements: vec![c.clone()],
                ..Default::default()
            },
            ..Default::default()
        };
        let deck = one
            .write(Dialect::Ngspice)
            .map_err(PyRuntimeError::new_err)?;
        // Drop the title line and `.end`.
        let lines: Vec<&str> = deck.lines().collect();
        Ok(lines[1..lines.len() - 1].join("\n"))
    }
}

/// Refuse a backend name at selection time rather than at the first run.
pub(super) fn check_backend_name(name: &str) -> PyResult<()> {
    if crate::backend::parse(name).is_some() {
        return Ok(());
    }
    let names = crate::backend::RUNNABLE
        .map(Dialect::name)
        .into_iter()
        .chain(Dialect::ALIASES.map(|a| a.0));
    Err(PyValueError::new_err(format!(
        "unknown backend {name:?}; expected one of {}",
        names.collect::<Vec<_>>().join(", ")
    )))
}

/// The dialect family a line given as `name`'s text is kept in: one the
/// deck's source has a `simulator lang=` region for.
///
/// # Errors
/// A name no dialect has; a dialect with no region (CDL, Verilog).
pub(super) fn region_dialect(name: &str) -> Result<Dialect, String> {
    let d = name.parse::<Dialect>()?.family();
    let regions = Lang::NGSPICE_GIVEN.lexicon().expect("a reader").regions;
    match regions.iter().any(|(n, _)| *n == d.name()) {
        true => Ok(d),
        false => {
            let names: Vec<&str> = regions.iter().map(|r| r.0).collect();
            Err(format!(
                "no {} text in a deck; one of {}",
                d.name(),
                names.join(", ")
            ))
        }
    }
}

/// Any error as a Python `RuntimeError` carrying its `Display` text.
pub(super) fn runtime_err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

// ---- private ----

impl Deck {
    /// The simulator a run picks and the deck written for it.
    fn for_run(&self, backend: Option<&str>) -> PyResult<(Dialect, String)> {
        let deck = self.lowered()?;
        let first = deck.analyses().first().map_or(Kind::Op, Analysis::kind);
        let writes = |d| deck.write(d).is_ok();
        let sim =
            crate::backend::select(first, backend, deck.needs(), writes).map_err(runtime_err)?;
        let text = deck.write(sim).map_err(runtime_err)?;
        Ok((sim, text))
    }

    /// A copy with every Verilog block expanded into what simulates it.
    fn lowered(&self) -> PyResult<Deck> {
        let mut deck = self.clone();
        for b in std::iter::once(&mut deck.top).chain(&mut deck.defs) {
            lower_verilog(b).map_err(runtime_err)?;
        }
        Ok(deck)
    }

    fn analyses(&self) -> &[Analysis] {
        self.bench.as_ref().map_or(&[], |b| &b.analyses)
    }

    fn bodies(&self) -> impl Iterator<Item = &Body> {
        std::iter::once(&self.top).chain(&self.defs)
    }

    /// What the deck needs from a simulator (`feature` bits).
    fn needs(&self) -> u8 {
        let mut needs = 0;
        for b in self.bodies() {
            if !b.osdi.is_empty() {
                needs |= feature::OSDI;
            }
            if b.verilog
                .iter()
                .any(|v| matches!(v.mode, crate::veriloga::VerilogMode::Simulate))
            {
                needs |= feature::VERILOG_COSIM;
            }
            let control = |l: &String| {
                let t = l.trim().to_lowercase();
                t == ".control" || t.starts_with(".control ")
            };
            if b.raw.iter().any(control) {
                needs |= feature::CONTROL_BLOCKS;
            }
            needs |= b.elements.iter().fold(0, |n, c| n | c.needs);
        }
        if let Some(tb) = &self.bench {
            if !tb.measures.is_empty() {
                needs |= feature::MEASURES;
            }
            if !tb.steps.is_empty() {
                needs |= feature::STEP_PARAMS;
            }
        }
        needs
    }

    /// The deck written for `d`: its ngspice source rendered there.
    fn write(&self, d: Dialect) -> Result<String, String> {
        let name = self.top.name.as_str();
        let source = self.source(d)?;
        let lex = Lang::NGSPICE_GIVEN.lexicon().expect("a reader");
        let tokens = crate::syntax::tokenize(&source, lex);
        let ast = crate::syntax::parse(&tokens, lex);
        let src = render::Source {
            ast: &ast,
            tokens: &tokens,
            dialect: Dialect::Ngspice,
            name,
            kit: None,
            loads: &self.top.osdi,
            numbers: render::Numbers::Given,
        };
        match render::render(&src, d) {
            Ok(w) => Ok(w.text),
            Err(diags) => Err(diags.messages.trim_end().to_string()),
        }
    }

    /// The deck as ngspice text, for target `d`: a model library's path
    /// is the one for `d`'s family, a `pss` is Spectre's statement on
    /// Spectre (`Analysis::in_spectre_region`), and a Fourier analysis's
    /// harmonic count is in `d`'s words (`Analysis::fourier_count`), the
    /// only parts that depend on `d`. `Err` names an analysis no dialect
    /// here has a statement for.
    fn source(&self, d: Dialect) -> Result<String, String> {
        let mut out = String::with_capacity(4096);
        let line = |out: &mut String, l: &str| {
            out.push_str(l);
            out.push('\n');
        };
        line(&mut out, &self.top.name);
        if !self.globals.is_empty() {
            line(&mut out, &format!(".global {}", self.globals.join(" ")));
        }
        for f in &self.functions {
            let args = f.args.join(", ");
            line(
                &mut out,
                &format!(".func {}({args}) {{{}}}", f.name, f.body),
            );
        }
        file_cards(&mut out, &self.top);
        for lib in self.libraries.iter().filter(|l| !l.placed) {
            for s in &lib.setup_includes {
                line(&mut out, &format!(".include {s}"));
            }
            line(&mut out, &lib.line(lib.path_for(d)));
        }
        for (k, v) in &self.top.params {
            line(&mut out, &format!(".param {k}={v}"));
        }
        for m in &self.top.models {
            line(&mut out, m);
        }
        for sc in &self.defs {
            out.push('\n');
            let ps = match sc.params.is_empty() {
                true => String::new(),
                false => format!(" PARAMS:{}", params(&sc.params)),
            };
            line(
                &mut out,
                &format!(".subckt {} {}{ps}", sc.name, sc.ports.join(" ")),
            );
            file_cards(&mut out, sc);
            for m in &sc.models {
                line(&mut out, m);
            }
            body(&mut out, sc);
            line(&mut out, &format!(".ends {}", sc.name));
        }
        let mut top = Cow::Borrowed(&self.top);
        for lib in self.libraries.iter().filter(|l| l.placed) {
            let (given, wanted) = (lib.line(&lib.path), lib.line(lib.path_for(d)));
            for l in top
                .to_mut()
                .raw
                .iter_mut()
                .filter(|l| l.lines().any(|x| x == given))
            {
                *l = l
                    .lines()
                    .map(|x| if x == given { wanted.as_str() } else { x })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
        }
        body(&mut out, &top);
        if let Some(tb) = &self.bench {
            bench(&mut out, tb, d)?;
        }
        line(&mut out, ".end");
        Ok(out)
    }
}

/// A block's `.include` and `.lib` cards.
fn file_cards(out: &mut String, b: &Body) {
    for inc in &b.includes {
        let _ = writeln!(out, ".include {inc}");
    }
    for (p, section) in &b.libs {
        let _ = writeln!(out, ".lib {p} {section}");
    }
}

/// A block's elements, then its instances, then its raw lines.
fn body(out: &mut String, b: &Body) {
    for c in b.elements.iter().chain(&b.instances) {
        let _ = writeln!(out, "{}", c.text);
    }
    for l in &b.raw {
        let _ = writeln!(out, "{l}");
    }
}

/// The testbench's cards: stimulus, options, initial conditions, outputs,
/// measures, steps, the analyses, and the regions of other dialects' text.
fn bench(out: &mut String, tb: &Bench, d: Dialect) -> Result<(), String> {
    for c in &tb.stimulus {
        let _ = writeln!(out, "{}", c.text);
    }
    if !tb.options.is_empty() {
        let opts: Vec<String> = tb
            .options
            .iter()
            .map(|(k, v)| match v.is_empty() {
                true => k.clone(),
                false => format!("{k}={v}"),
            })
            .collect();
        let _ = writeln!(out, ".options {}", opts.join(" "));
    }
    if let Some(mode) = &tb.ngbehavior {
        let _ = writeln!(out, ".options ngbehavior={mode}");
    }
    if let Some(t) = tb.temperature {
        let _ = writeln!(out, ".temp {}", dec(t));
    }
    if let Some(t) = tb.nominal_temperature {
        let _ = writeln!(out, ".options tnom={}", dec(t));
    }
    for (n, v) in &tb.ics {
        let _ = writeln!(out, ".ic V({n})={}", dec(*v));
    }
    for (n, v) in &tb.nodesets {
        let _ = writeln!(out, ".nodeset V({n})={}", dec(*v));
    }
    if !tb.saves.is_empty() {
        let _ = writeln!(out, ".save {}", tb.saves.join(" "));
    }
    for (kind, outs) in &tb.prints {
        let sep = if kind.is_empty() { "" } else { " " };
        let _ = writeln!(out, ".print{sep}{kind} {}", outs.join(" "));
    }
    for m in &tb.measures {
        let card = m.get(..5).is_some_and(|p| p.eq_ignore_ascii_case(".meas"));
        let _ = writeln!(out, "{}{m}", if card { "" } else { ".meas " });
    }
    for s in &tb.steps {
        let sweep = s
            .sweep
            .map(|w| format!("{} ", w.name()))
            .unwrap_or_default();
        let (a, b, c) = (dec(s.start), dec(s.stop), dec(s.step));
        let _ = writeln!(out, ".step {sweep}{} {a} {b} {c}", s.param);
    }
    let mut spectre = Vec::new();
    let mut ltspice = Vec::new();
    if d.family() == Dialect::Ngspice
        && let Some(card) = nfreqs(&tb.analyses).map_err(|why| unsupported(&why))?
    {
        let _ = writeln!(out, "{card}");
    }
    for a in &tb.analyses {
        // CDL and Verilog write no analysis.
        let analyses = !matches!(d, Dialect::Spectre | Dialect::Cdl | Dialect::Verilog);
        if a.log_sweep() && analyses {
            let what = format!("frequency sweep 'log' (N points in total) on {}", d.name());
            return Err(unsupported(&what));
        }
        if let Analysis::Fourier { .. } = a
            && let Some(lines) = a.fourier_count(d).map_err(|why| unsupported(&why))?
        {
            match d {
                Dialect::Spectre => spectre.extend(lines),
                Dialect::Ltspice => ltspice.extend(lines),
                other => unreachable!("{other:?} holds no counted Fourier line"),
            }
            continue;
        }
        match a.card().filter(|_| !a.in_spectre_region(d)) {
            Some(card) => {
                let _ = writeln!(out, "{card}");
            }
            None => spectre.push(a.spectre().map_err(|why| unsupported(&why))?),
        }
    }
    region(out, "spectre", &spectre);
    region(out, "ltspice", &ltspice);
    for (l, d) in &tb.extra_lines {
        region(out, d.name(), std::slice::from_ref(l));
    }
    Ok(())
}

/// `lines` inside a `simulator lang=<lang>` … `simulator lang=spice`
/// region; nothing for none.
fn region(out: &mut String, lang: &str, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let _ = writeln!(out, "simulator lang={lang}");
    for l in lines {
        let _ = writeln!(out, "{l}");
    }
    let _ = writeln!(out, "simulator lang=spice");
}

/// The refusal (W0040) of an analysis argument the target has no word for.
fn unsupported(what: &str) -> String {
    let code = crate::syntax::diag::Code::W0040;
    format!("{code:?} {}: {what}", crate::syntax::diag::info(code).title)
}

#[cfg(test)]
mod tests {
    // No test here reaches a `PyErr`: the extension module links no
    // libpython, so a test binary that does fails to link.
    use super::*;
    use crate::python::model::Card;

    /// A one-resistor deck with `analyses` queued.
    fn deck(analyses: Vec<Analysis>) -> Deck {
        Deck {
            top: Body {
                name: "t".into(),
                elements: vec![Card::element("1", "R1 out 0 1k".into())],
                ..Body::default()
            },
            bench: Some(Bench {
                analyses,
                ..Bench::default()
            }),
            ..Deck::default()
        }
    }

    #[test]
    fn a_pss_is_spectres_statement_on_spectre_and_the_card_elsewhere() {
        // Spectre Reference 19.1 p.272-274 (`pss fund= tstab= harms=`, the
        // observed node as its terminals); ngspice manual §11.3.12 (`.pss
        // gfreq tstab oname psspoints harms`). Checks: 3.
        let pss = Analysis::Pss {
            fundamental: 1e6,
            stabilization: 1e-5,
            observe_node: "out".into(),
            points_per_period: 128,
            harmonics: 10,
            sc_iter: None,
            steady_coeff: None,
            uic: false,
        };
        let d = deck(vec![pss]);
        let spectre = d.write(Dialect::Spectre).expect("written");
        assert!(
            spectre
                .lines()
                .any(|l| l == "pss1 (out 0) pss fund=1M tstab=10u harms=10"),
            "{spectre}"
        );
        let ngspice = d.write(Dialect::Ngspice).expect("written");
        assert!(
            ngspice.contains(".pss 1000000 0.00001 out 128 10"),
            "{ngspice}"
        );
        // The store keeps ngspice's (`to_source`).
        assert!(
            d.source(Dialect::Ngspice)
                .expect("source")
                .contains(".pss ")
        );
    }

    #[test]
    fn a_fourier_harmonic_count_is_written_in_each_targets_words() {
        // ngspice-45 `src/frontend/fourier.c`: `nfreqs` frequencies, DC the
        // first, so five harmonics are `nfreqs=6`; LTspice help `.FOUR`:
        // `.four <freq> <Nharmonics> <traces>`; Spectre Components p.323-325:
        // `fourier` with `harms`. Xyce's and HSPICE's `.four` have no count.
        // Checks: 6.
        let four = |harmonics| Analysis::Fourier {
            fundamental: 1e3,
            outputs: vec!["V(out)".into()],
            harmonics,
        };
        let d = deck(vec![four(Some(5))]);
        let ngspice = d.write(Dialect::Ngspice).expect("written");
        assert!(ngspice.contains("nfreqs=6"), "{ngspice}");
        let ltspice = d.write(Dialect::Ltspice).expect("written");
        assert!(
            ltspice.lines().any(|l| l == ".four 1k 5 V(out)"),
            "{ltspice}"
        );
        let spectre = d.write(Dialect::Spectre).expect("written");
        let want = "fourh1 (out 0) fourier fund=1k harms=5";
        assert!(spectre.lines().any(|l| l == want), "{spectre}");
        for refused in [Dialect::Xyce, Dialect::Hspice] {
            assert!(d.write(refused).is_err(), "{refused:?}");
        }
        // ngspice's count is the deck's: a second analysis counting nine
        // (none given) cannot share it.
        let two = deck(vec![four(Some(5)), four(None)]);
        assert!(two.write(Dialect::Ngspice).is_err());
    }

    /// One of each analysis the Python API queues, and the variants whose
    /// arguments some target has no word for.
    fn every_analysis() -> Vec<(&'static str, Analysis)> {
        use crate::python::analysis::{DcSweep, FreqSweep, PssBase};
        use crate::python::keywords::{PzKind, PzTransfer, SpectreAnalysis, Sweep};
        let fs = || FreqSweep {
            variation: Sweep::Dec,
            points: 10,
            start: 1.0,
            stop: 1e6,
        };
        let pb = || PssBase {
            fundamental: 1e6,
            stabilization: 1e-6,
            harmonics: 5,
        };
        vec![
            ("op", Analysis::Op),
            (
                "dc",
                Analysis::Dc {
                    sweeps: vec![DcSweep {
                        source: "Vin".into(),
                        start: 0.0,
                        stop: 1.0,
                        step: 0.1,
                    }],
                },
            ),
            (
                "dc2",
                Analysis::Dc {
                    sweeps: vec![
                        DcSweep {
                            source: "Vin".into(),
                            start: 0.0,
                            stop: 1.0,
                            step: 0.1,
                        },
                        DcSweep {
                            source: "Vin2".into(),
                            start: 0.0,
                            stop: 1.0,
                            step: 0.5,
                        },
                    ],
                },
            ),
            ("ac", Analysis::Ac { sweep: fs() }),
            (
                "ac_log",
                Analysis::Ac {
                    sweep: FreqSweep {
                        variation: Sweep::Log,
                        ..fs()
                    },
                },
            ),
            (
                "ac_oct",
                Analysis::Ac {
                    sweep: FreqSweep {
                        variation: Sweep::Oct,
                        ..fs()
                    },
                },
            ),
            (
                "tran",
                Analysis::Transient {
                    step: 1e-6,
                    stop: 1e-3,
                    start: None,
                    max_step: None,
                    uic: false,
                },
            ),
            (
                "tran_full",
                Analysis::Transient {
                    step: 1e-6,
                    stop: 1e-3,
                    start: Some(1e-4),
                    max_step: Some(1e-6),
                    uic: true,
                },
            ),
            (
                "noise",
                Analysis::Noise {
                    output: "out".into(),
                    reference: "0".into(),
                    source: "Vin".into(),
                    sweep: fs(),
                    points_per_summary: None,
                },
            ),
            (
                "noise_pps",
                Analysis::Noise {
                    output: "out".into(),
                    reference: "0".into(),
                    source: "Vin".into(),
                    sweep: fs(),
                    points_per_summary: Some(5),
                },
            ),
            (
                "tf",
                Analysis::Tf {
                    output: "V(out)".into(),
                    source: "Vin".into(),
                },
            ),
            (
                "sens",
                Analysis::Sensitivity {
                    output: "V(out)".into(),
                    ac: None,
                },
            ),
            (
                "sens_ac",
                Analysis::Sensitivity {
                    output: "V(out)".into(),
                    ac: Some(fs()),
                },
            ),
            (
                "pz",
                Analysis::PoleZero {
                    nodes: ["in".into(), "0".into(), "out".into(), "0".into()],
                    tf_type: PzTransfer::Vol,
                    pz_type: PzKind::Pz,
                },
            ),
            (
                "disto",
                Analysis::Distortion {
                    sweep: fs(),
                    f2overf1: None,
                },
            ),
            (
                "disto_f2",
                Analysis::Distortion {
                    sweep: fs(),
                    f2overf1: Some(0.9),
                },
            ),
            (
                "pss",
                Analysis::Pss {
                    fundamental: 1e6,
                    stabilization: 1e-5,
                    observe_node: "out".into(),
                    points_per_period: 128,
                    harmonics: 10,
                    sc_iter: None,
                    steady_coeff: None,
                    uic: false,
                },
            ),
            (
                "pss_ctl",
                Analysis::Pss {
                    fundamental: 1e6,
                    stabilization: 1e-5,
                    observe_node: "out".into(),
                    points_per_period: 128,
                    harmonics: 10,
                    sc_iter: Some(20),
                    steady_coeff: None,
                    uic: false,
                },
            ),
            (
                "hb",
                Analysis::HarmonicBalance {
                    frequencies: vec![1e6],
                    harmonics: vec![7],
                },
            ),
            ("sp", Analysis::SPar { sweep: fs() }),
            (
                "stb",
                Analysis::Stability {
                    probe: "Iprb".into(),
                    sweep: fs(),
                },
            ),
            (
                "trannoise",
                Analysis::TransientNoise {
                    step: 1e-9,
                    stop: 1e-6,
                },
            ),
            (
                "four",
                Analysis::Fourier {
                    fundamental: 1e3,
                    outputs: vec!["V(out)".into()],
                    harmonics: None,
                },
            ),
            (
                "four_n",
                Analysis::Fourier {
                    fundamental: 1e3,
                    outputs: vec!["V(out)".into()],
                    harmonics: Some(5),
                },
            ),
            (
                "sweep",
                Analysis::SpectreSweep {
                    param: "r".into(),
                    start: 1.0,
                    stop: 2.0,
                    step: 0.5,
                    inner: "ac1".into(),
                    inner_type: SpectreAnalysis::Ac,
                },
            ),
            (
                "mc",
                Analysis::SpectreMonteCarlo {
                    iterations: 5,
                    inner: "ac1".into(),
                    inner_type: SpectreAnalysis::Ac,
                    seed: Some(1),
                },
            ),
            (
                "pac",
                Analysis::SpectrePac {
                    pss: pb(),
                    sweep: fs(),
                    sweep_type: "relative".into(),
                },
            ),
            (
                "pnoise",
                Analysis::SpectrePnoise {
                    pss: pb(),
                    output: "out".into(),
                    reference: "".into(),
                    sweep: fs(),
                },
            ),
            (
                "pxf",
                Analysis::SpectrePxf {
                    pss: pb(),
                    output: "out".into(),
                    sweep: fs(),
                },
            ),
            (
                "pstb",
                Analysis::SpectrePstb {
                    pss: pb(),
                    probe: "Iprb".into(),
                    sweep: fs(),
                },
            ),
        ]
    }

    #[test]
    fn a_run_lands_only_where_the_writer_writes_the_deck() {
        // Oracle: the renderer's verdict. Whatever `select` picks among the
        // installed backends, the deck written for it has no refusal; a
        // route that names a backend whose writer refuses the analysis
        // (`sp` and `stb` on VACASK once did) is skipped, not run.
        for (name, a) in every_analysis() {
            let mut d = deck(vec![a]);
            d.top
                .elements
                .push(Card::element("in", "Vin in 0 1 AC 1".into()));
            let kind = d.analyses()[0].kind();
            let writes = |t| d.write(t).is_ok();
            if let Ok(sim) = crate::backend::select(kind, None, d.needs(), writes) {
                assert!(
                    d.write(sim).is_ok(),
                    "{name} routed to {sim:?}, which refuses it"
                );
            }
        }
    }
}
