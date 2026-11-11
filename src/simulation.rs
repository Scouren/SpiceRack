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
//! - `pub fn run(sim: Sim, deck: &str) -> Result<RawData, BackendError>`
//! - `pub fn run_all(sim: Sim, deck: &str) -> Result<Vec<RawData>, BackendError>`
//! - `pub fn run_each(sim: Sim, deck: &str, analyses: &[Kind]) -> Result<Vec<Vec<RawData>>, BackendError>`

use crate::backend::{BackendError, Dialect, Kind, Plot, Sim};
use crate::result::{RawData, VarInfo, VarType};
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
/// A two-source `.dc` on ngspice or ESPice, whose plot holds the inner
/// sweep alone, gets its outer sweep as the column `sweep1`, VACASK's
/// name for it (`outer_sweeps`).
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
    // ngspice and ESPice write a nested `.dc`'s inner sweep alone.
    let inner_only = matches!(sim.family(), Dialect::Ngspice | Dialect::Espice);
    if analyses.len() <= 1 {
        if !stepped {
            let second = analyses.first().is_some_and(|&a| also(a, plots.get(1)));
            plots.truncate(1 + usize::from(second));
        }
        let mut each = vec![plots.into_iter().flatten().collect()];
        if inner_only {
            outer_sweeps(deck, analyses, &mut each);
        }
        return Ok(each);
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
    // ngspice pushes each dot card's job at the head of its task's list
    // (`cktnewan.c:34-35`) and walks the list once per kind
    // (`cktdojob.c:210-218`): `.ac lin 3 100 300` then `.ac dec 1 1 10`
    // writes the decade plot first (ngspice-45). Commands of a `.control`
    // block run in the block's order.
    let last_first = sim.family() == Dialect::Ngspice && !commands_run(deck);
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
    if inner_only {
        outer_sweeps(deck, analyses, &mut out);
    }
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

/// Gives each two-source `.dc`'s plot its outer sweep as a last column,
/// `sweep1` (the name the VACASK writer gives its outer `sweep`): ngspice
/// and ESPice write the inner sweep alone (`v(v-sweep)`, once per outer
/// value). `each` is one entry per analysis of `analyses`; the k-th dc
/// analysis is the deck's k-th `.dc` card (a `dc` command inside a
/// `.control` block). The values are the card's, `start2 + k * step2`,
/// added only to a plot that agrees with its card: its points are the
/// outer count times the inner sweep, which is back at its first value at
/// each multiple. A card whose outer fields are not plain numbers adds
/// none.
fn outer_sweeps(deck: &str, analyses: &[Kind], each: &mut [Vec<RawData>]) {
    assert!(each.len() >= analyses.len(), "one entry per analysis");
    let mut inside = false;
    let cards = deck.lines().filter(|line| {
        inside = (inside || is_card(line, ".control")) && !is_card(line, ".endc");
        is_card(line, ".dc") || (inside && is_card(line, "dc"))
    });
    let dcs = analyses.iter().zip(each).filter(|(a, _)| **a == Kind::Dc);
    for ((_, plots), card) in dcs.zip(cards) {
        // `.dc src1 start1 stop1 step1 src2 start2 stop2 step2`
        let outer: Vec<f64> = card.split_whitespace().skip(6).map_while(number).collect();
        let &[start, stop, step] = outer.as_slice() else {
            continue;
        };
        let count = (((stop - start) / step + 1e-9).floor() + 1.0) as usize;
        let real = |p: &&mut RawData| p.real_data.len() == p.variables.len();
        for p in plots.iter_mut().filter(real) {
            let points = p.real_data.first().map_or(0, Vec::len);
            if count == 0 || points == 0 || !points.is_multiple_of(count) {
                continue;
            }
            let (inner, axis) = (points / count, &p.real_data[0]);
            if !(0..points).step_by(inner).all(|i| axis[i] == axis[0]) {
                continue;
            }
            let column = (0..points)
                .map(|i| start + (i / inner) as f64 * step)
                .collect();
            p.variables.push(VarInfo {
                index: p.variables.len(),
                name: "sweep1".to_string(),
                var_type: VarType::Other,
            });
            p.real_data.push(column);
            assert_eq!(p.real_data.len(), p.variables.len());
        }
    }
}

/// A `.dc` card's field as a number (`-0.5`, `1k`, `2V`); `None` for one
/// that is none (a parameter, an expression).
fn number(field: &str) -> Option<f64> {
    let (sign, digits) = match field.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, field.strip_prefix('+').unwrap_or(field)),
    };
    let (value, len, scale) = crate::unit::parse_spice_number(digits)?;
    let unit = &digits[len + scale.len()..];
    unit.chars()
        .all(|c| c.is_ascii_alphabetic())
        .then_some(sign * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The outer column `outer_sweeps` gives a 2-column dc plot (`axis`,
    /// then V2's node `b`) under the deck lines `cards`, if any.
    fn outer(cards: &str, axis: &[f64], b: &[f64]) -> Option<Vec<f64>> {
        let var = |index, name: &str| VarInfo {
            index,
            name: name.to_string(),
            var_type: VarType::Voltage,
        };
        let plot = RawData {
            plot_name: "DC transfer characteristic".to_string(),
            variables: vec![var(0, "v(v-sweep)"), var(1, "v(b)")],
            real_data: vec![axis.to_vec(), b.to_vec()],
            ..RawData::default()
        };
        let mut each = vec![vec![plot]];
        outer_sweeps(&format!("* t\n{cards}\n.end\n"), &[Kind::Dc], &mut each);
        each[0][0].real_data.get(2).cloned()
    }

    #[test]
    fn p3_094_the_outer_sweep_is_the_cards_where_the_plot_agrees() {
        // Oracle: ngspice-45's own `v(b)` (V2's node voltage) and
        // `v(v-sweep)` for each card (research/impl/evidence/P3-094).
        let a9 = [0.0, 0.5, 1.0, 0.0, 0.5, 1.0, 0.0, 0.5, 1.0];
        let b9 = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0];
        assert_eq!(
            outer(".dc V1 0 1 0.5 V2 0 2 1", &a9, &b9),
            Some(b9.to_vec())
        );
        // The ngspice writer's `.control` shape, a scaled number.
        let command = ".control\ndc V1 0 1 500m V2 0 2000m 1\n.endc";
        assert_eq!(outer(command, &a9, &b9), Some(b9.to_vec()));
        // A one-point inner sweep, the outer one descending.
        let down = [2.0, 1.0, 0.0];
        assert_eq!(
            outer(".dc V1 1 1 1 V2 2 0 -1", &[1.0; 3], &down),
            Some(down.to_vec())
        );
        // One source: no outer sweep. A card that is not this plot's (four
        // outer values, nine points) or not plain numbers: none made up.
        assert_eq!(outer(".dc V1 0 1 0.5", &a9, &b9), None);
        assert_eq!(outer(".dc V1 0 1 0.5 V2 0 3 1", &a9, &b9), None);
        assert_eq!(outer(".dc V1 0 1 0.5 V2 0 {top} 1", &a9, &b9), None);
        assert_eq!(outer("dc V1 0 1 0.5 V2 0 2 1", &a9, &b9), None);
    }
}
