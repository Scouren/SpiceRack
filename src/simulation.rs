//! A deck rendered for one simulator → the backend runs the text verbatim
//! → `RawData`, one per plot or, matched by plot name (ngspice and ESPice
//! `Plotname`s, checked on both), one per analysis. The simulator is
//! picked first (`backend::select`), since the deck's text is that
//! simulator's dialect.
//!
//! Cites: none (plot names are ngspice's and ESPice's `Plotname` lines).
//!
//! Contents:
//! - `pub fn run(sim: Sim, deck: &str) -> Result<RawData, BackendError>`
//! - `pub fn run_all(sim: Sim, deck: &str) -> Result<Vec<RawData>, BackendError>`
//! - `pub fn run_each(sim: Sim, deck: &str, analyses: &[Kind]) -> Result<Vec<Vec<RawData>>, BackendError>`

use crate::backend::{BackendError, Dialect, Kind, Plot, Sim};
use crate::result::RawData;
use crate::syntax::render::LTSPICE_EACH;

/// Runs `deck`, written in `sim`'s dialect, on `sim`, tagging the result
/// with the backend and its parsed `.meas` values. The first plot:
/// [`run_all`] returns them all.
pub fn run(sim: Sim, deck: &str) -> Result<RawData, BackendError> {
    let first = run_all(sim, deck)?.into_iter().next();
    Ok(first.expect("a raw file read holds at least one plot"))
}

/// [`run`], returning every plot the run wrote (ngspice: one per analysis,
/// in run order), each tagged with the backend and the run's `.meas` values.
pub fn run_all(sim: Sim, deck: &str) -> Result<Vec<RawData>, BackendError> {
    let mut plots = sim.run_all(deck)?;
    // The built-in dialect the simulator's decks are a kind of: its
    // `.meas` lines and vector names are that family's.
    let family = sim.family();
    // LTspice writes `.meas` results to its log, everyone else to stdout;
    // every plot carries the same run's text.
    let p = &plots[0];
    let text = if family == Dialect::Ltspice && !p.log_content.is_empty() {
        &p.log_content
    } else {
        &p.stdout
    };
    // In the deck's order, a measure with no value a row too; of a stepped
    // run (one plot a step, ZERO D42) each plot carries its step's rows.
    let printed = crate::measure_parse::parse_measures(text, Some(family));
    let measures = crate::measure_parse::in_deck_order(deck, printed, family);
    for (k, p) in plots.iter_mut().enumerate() {
        p.backend_hint = Some(family);
        p.measures = crate::measure_parse::at_step(&measures, k);
    }
    Ok(plots)
}

/// [`run_all`], one entry per analysis of the deck, `analyses` in its
/// order: that analysis's plots, one, or with `.step` one per step in step
/// order (ZERO D42), or none for a `Fourier`, which prints its table and
/// writes none. ngspice runs a deck's analyses in its own order, so a plot
/// is matched by its `Plotname` (ngspice's and ESPice's), not by position;
/// VACASK's are in deck order, one per analysis. A lone analysis takes the
/// first plot, as [`run`], or every step.
///
/// # Errors
/// As [`run_all`], and `SimulationError` when an analysis's plot is not
/// among those the run wrote, or it is an analysis whose plot is not
/// matched here (`pss`, `hb`, `stb`, the Spectre-only
/// ones) run beside others.
pub fn run_each(
    sim: Sim,
    deck: &str,
    analyses: &[Kind],
) -> Result<Vec<Vec<RawData>>, BackendError> {
    if sim.family() == Dialect::Ltspice && analyses.len() > 1 {
        return ltspice_each(sim, deck, analyses);
    }
    let mut plots: Vec<Option<RawData>> = run_all(sim, deck)?.into_iter().map(Some).collect();
    assert!(!plots.is_empty(), "a run returns at least one plot");
    let stepped = plots.iter().flatten().any(|p| !p.params.is_empty());
    if analyses.len() <= 1 {
        if !stepped {
            plots.truncate(1);
        }
        return Ok(vec![plots.into_iter().flatten().collect()]);
    }
    let missing = |what: &str| {
        BackendError::SimulationError(format!("no {what} plot among those the run wrote"))
    };
    if stepped {
        // ponytail: steps come from LTspice alone, one analysis a deck
        // (P1-007); a stepped run of several analyses is matched when a
        // backend writes one.
        return Err(missing("per-analysis stepped"));
    }
    if sim.family() == Dialect::Vacask {
        if plots.len() != analyses.len() {
            return Err(missing(&format!(
                "one-per-analysis ({} plots, {} analyses)",
                plots.len(),
                analyses.len()
            )));
        }
        return Ok(plots.into_iter().flatten().map(|p| vec![p]).collect());
    }
    let mut out = Vec::with_capacity(analyses.len());
    for &a in analyses {
        let name = match a.spec().plot {
            Plot::Table => {
                out.push(Vec::new());
                continue;
            }
            Plot::Named(name) => name,
            Plot::Unmatched(py_name) => {
                return Err(missing(&format!("matchable {py_name}")));
            }
        };
        let i = plots
            .iter()
            .position(|p| p.as_ref().is_some_and(|p| p.plot_name.starts_with(name)))
            .ok_or_else(|| missing(name))?;
        out.push(plots[i].take().into_iter().collect());
    }
    assert_eq!(out.len(), analyses.len());
    Ok(out)
}

// ---- private ----

/// [`run_each`] on LTspice, which runs one analysis a deck: each analysis
/// card after the writer's [`LTSPICE_EACH`] line run alone in the deck, a
/// `.four` with the `.tran` (and no run of its own).
///
/// # Errors
/// As [`run_all`], and `SimulationError` when the deck's analysis cards
/// are not one per analysis.
fn ltspice_each(
    sim: Sim,
    deck: &str,
    analyses: &[Kind],
) -> Result<Vec<Vec<RawData>>, BackendError> {
    let mismatch = || {
        BackendError::SimulationError(format!(
            "LTspice: the deck's analysis cards after `{LTSPICE_EACH}` are not its {} analyses",
            analyses.len()
        ))
    };
    let (head, rest) = deck.split_once(LTSPICE_EACH).ok_or_else(mismatch)?;
    let (cards, tail): (Vec<&str>, Vec<&str>) = rest
        .lines()
        .filter(|l| !l.trim().is_empty())
        .partition(|l| !l.trim().eq_ignore_ascii_case(".end"));
    if cards.len() != analyses.len() {
        return Err(mismatch());
    }
    let four: Vec<&str> = cards
        .iter()
        .copied()
        .filter(|c| is_card(c, ".four"))
        .collect();
    let mut out = Vec::with_capacity(analyses.len());
    for card in &cards {
        if is_card(card, ".four") {
            out.push(Vec::new());
            continue;
        }
        let with_four = if is_card(card, ".tran") {
            four.join("\n")
        } else {
            String::new()
        };
        let one = format!("{head}{card}\n{with_four}\n{}\n", tail.join("\n"));
        let plots = run_all(sim, &one)?;
        let stepped = plots.iter().any(|p| !p.params.is_empty());
        out.push(match stepped {
            true => plots,
            false => plots.into_iter().take(1).collect(),
        });
    }
    assert_eq!(out.len(), analyses.len());
    Ok(out)
}

/// Whether `line` is the `word` card (`.tran 1n 1u`, any case).
fn is_card(line: &str, word: &str) -> bool {
    let first = line.split_whitespace().next().unwrap_or("");
    first.eq_ignore_ascii_case(word)
}
