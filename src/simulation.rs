//! A deck rendered for one simulator → the backend runs the text verbatim
//! → `RawData`, one per plot or, matched by plot name (ngspice and ESPice
//! `Plotname`s, checked on both), one per analysis. The simulator is
//! picked first (`backend::select`), since the deck's text is that
//! simulator's dialect.
//!
//! Cites: ngspice-45 `src/spicelib/analysis/cktnewan.c:34-35` and
//! `cktdojob.c:210-218` (a deck's dot cards run kind by kind, the last
//! card of a kind first); plot names are ngspice's and ESPice's
//! `Plotname` lines.
//!
//! Contents:
//! - `pub fn run(sim: Dialect, deck: &str) -> Result<RawData, BackendError>`
//! - `pub fn run_all(sim: Dialect, deck: &str) -> Result<Vec<RawData>, BackendError>`
//! - `pub fn run_each(sim: Dialect, deck: &str, analyses: &[Kind]) -> Result<Vec<Vec<RawData>>, BackendError>`

use crate::backend::{BackendError, Dialect, Kind, Plot};
use crate::result::RawData;
use crate::syntax::render::LTSPICE_EACH;

/// Runs `deck`, written in `sim`'s dialect, on `sim`, tagging the result
/// with the backend and its parsed `.meas` values. The first plot:
/// [`run_all`] returns them all.
pub fn run(sim: Dialect, deck: &str) -> Result<RawData, BackendError> {
    let first = run_all(sim, deck)?.into_iter().next();
    Ok(first.expect("a raw file read holds at least one plot"))
}

/// [`run`], returning every plot the run wrote (ngspice: one per analysis,
/// in run order), each tagged with the backend and the run's `.meas` values.
pub fn run_all(sim: Dialect, deck: &str) -> Result<Vec<RawData>, BackendError> {
    let mut plots = sim.run_all(deck)?;
    // LTspice writes `.meas` results to its log, everyone else to stdout;
    // every plot carries the same run's text.
    let p = &plots[0];
    let text = if sim == Dialect::Ltspice && !p.log_content.is_empty() {
        &p.log_content
    } else {
        &p.stdout
    };
    let measures = if text.is_empty() {
        vec![]
    } else {
        crate::measure_parse::parse_measures(text, Some(sim))
    };
    for p in &mut plots {
        p.backend_hint = Some(sim);
        p.measures.clone_from(&measures);
    }
    Ok(plots)
}

/// [`run_all`], one entry per analysis of the deck, `analyses` in its
/// order: that analysis's plots, one, or with `.step` one per step in step
/// order (ZERO D42), or none for a `Fourier`, which prints its table and
/// writes none; a noise run's are its spectra and, behind them, the
/// "Integrated Noise" plot ngspice and ESPice write (its totals, one
/// point). ngspice runs a deck's analyses in its own order, so a plot
/// is matched by its `Plotname` (ngspice's and ESPice's), not by position;
/// two analyses of one kind are told apart by run order: ngspice runs the
/// dot cards of a kind last first, the commands of a `.control` block
/// (and ESPice its dot cards) in deck order. VACASK's are in deck order,
/// one per analysis. A lone analysis takes the first plot, as [`run`], or
/// every step.
///
/// # Errors
/// As [`run_all`], and `SimulationError` when an analysis's plot is not
/// among those the run wrote, or it is an analysis whose plot is not
/// matched here (`pss`, `hb`, `stb`, the Spectre-only
/// ones) run beside others.
pub fn run_each(
    sim: Dialect,
    deck: &str,
    analyses: &[Kind],
) -> Result<Vec<Vec<RawData>>, BackendError> {
    if sim == Dialect::Ltspice && analyses.len() > 1 {
        return ltspice_each(deck, analyses);
    }
    let mut plots: Vec<Option<RawData>> = run_all(sim, deck)?.into_iter().map(Some).collect();
    assert!(!plots.is_empty(), "a run returns at least one plot");
    let stepped = plots.iter().flatten().any(|p| !p.params.is_empty());
    if analyses.len() <= 1 {
        if !stepped {
            let second = analyses.first().is_some_and(|&a| also(a, plots.get(1)));
            plots.truncate(1 + usize::from(second));
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
    if sim == Dialect::Vacask {
        if plots.len() != analyses.len() {
            return Err(missing(&format!(
                "one-per-analysis ({} plots, {} analyses)",
                plots.len(),
                analyses.len()
            )));
        }
        return Ok(plots.into_iter().flatten().map(|p| vec![p]).collect());
    }
    // ngspice pushes each dot card's job at the head of its task's list
    // (`cktnewan.c:34-35`) and walks the list once per kind
    // (`cktdojob.c:210-218`): `.ac lin 3 100 300` then `.ac dec 1 1 10`
    // writes the decade plot first (ngspice-45). Commands of a `.control`
    // block run in the block's order.
    let last_first = sim == Dialect::Ngspice && !commands_run(deck);
    let mut out = Vec::with_capacity(analyses.len());
    for &a in analyses {
        let name = match a.spec().plot {
            Plot::Table => {
                out.push(Vec::new());
                continue;
            }
            Plot::Named(name) | Plot::NamedWith(name, _) => name,
            Plot::Unmatched(py_name) => {
                return Err(missing(&format!("matchable {py_name}")));
            }
        };
        let named = |p: &Option<RawData>| p.as_ref().is_some_and(|p| p.plot_name.starts_with(name));
        // The k-th queued analysis of a kind is the k-th plot of that kind
        // from the end where the last card ran first.
        let i = match last_first {
            true => plots.iter().rposition(named),
            false => plots.iter().position(named),
        }
        .ok_or_else(|| missing(name))?;
        let mut own: Vec<RawData> = plots[i].take().into_iter().collect();
        if also(a, plots.get(i + 1)) {
            own.extend(plots[i + 1].take());
        }
        out.push(own);
    }
    assert_eq!(out.len(), analyses.len());
    Ok(out)
}

// ---- private ----

/// Whether `next`, the plot a run wrote right after analysis `a`'s own, is
/// `a`'s too: the one its `Plot::NamedWith` names second (a noise run's
/// "Integrated Noise"). `None` and a plot already taken are not.
fn also(a: Kind, next: Option<&Option<RawData>>) -> bool {
    let Plot::NamedWith(_, then) = a.spec().plot else {
        return false;
    };
    next.and_then(Option::as_ref)
        .is_some_and(|p| p.plot_name.starts_with(then))
}

/// [`run_each`] on LTspice, which runs one analysis a deck: each analysis
/// card after the writer's [`LTSPICE_EACH`] line run alone in the deck, a
/// `.four` with the `.tran` (and no run of its own).
///
/// # Errors
/// As [`run_all`], and `SimulationError` when the deck's analysis cards
/// are not one per analysis.
fn ltspice_each(deck: &str, analyses: &[Kind]) -> Result<Vec<Vec<RawData>>, BackendError> {
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
        let plots = run_all(Dialect::Ltspice, &one)?;
        let stepped = plots.iter().any(|p| !p.params.is_empty());
        out.push(match stepped {
            true => plots,
            false => plots.into_iter().take(1).collect(),
        });
    }
    assert_eq!(out.len(), analyses.len());
    Ok(out)
}

/// Whether a `.control` block of `deck` issues an analysis as a command
/// (`ac lin 3 100 300`), the ngspice writer's shape for a deck with
/// `.meas` or `.four`: the run's plots are then in the block's order. A
/// `run` is no such command (it runs the dot cards), nor is a block that
/// only loads (`pre_osdi`).
fn commands_run(deck: &str) -> bool {
    // The ngspice writer's analysis cards without their dot
    // (`render/spice/bench.rs::W::run_analyses`).
    const COMMANDS: [&str; 11] = [
        "op", "dc", "ac", "tran", "noise", "tf", "sens", "pz", "disto", "sp", "pss",
    ];
    let mut inside = false;
    for line in deck.lines() {
        if is_card(line, ".control") {
            inside = true;
        } else if is_card(line, ".endc") {
            inside = false;
        } else if inside && COMMANDS.iter().any(|c| is_card(line, c)) {
            return true;
        }
    }
    false
}

/// Whether `line` is the `word` card (`.tran 1n 1u`, any case).
fn is_card(line: &str, word: &str) -> bool {
    let first = line.split_whitespace().next().unwrap_or("");
    first.eq_ignore_ascii_case(word)
}
