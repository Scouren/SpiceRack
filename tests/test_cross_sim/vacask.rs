//! VACASK's side of the cross-simulator harness where its files differ
//! from a SPICE simulator's: the step cap of a transient and the noise
//! plot.
//!
//! In: a VACASK deck's text and a refinement level; a plot VACASK wrote.
//! Out: the deck with each transient's `maxstep=` capped; the noise plot
//! in ngspice's names and units.
//!
//! Cites: VACASK `docs/analysis_types.md` (`tran`: `step`, `stop`,
//! `maxstep`; `noise`: `onoise` in V²/Hz, `gain` the power gain from the
//! input source, `n(dev)`); ngspice-45 and VACASK on one divider
//! (`onoise_spectrum` 2.87889e-9, `inoise_spectrum` 5.75779e-9 V/√Hz where
//! VACASK writes 8.28803e-18 and 0.25; ZERO-LEDGER P1-066).
//!
//! Contents:
//! - `pub fn refined(text: &str, n: f64) -> Option<String>`
//! - `pub fn spectrum(plot: RawData) -> RawData`

use spicerack::result::{RawData, VarInfo, VarType};

/// `text` with the `maxstep=` of every `analysis <name> tran` line at most
/// stop/n (a smaller one of the deck's own is kept): `sim::refined` for a
/// VACASK deck. `None` when no line is a transient with a literal `stop=`.
pub fn refined(text: &str, n: f64) -> Option<String> {
    assert!(n >= 1.0, "a refinement divides the span");
    let mut any = false;
    let out: String = text
        .lines()
        .map(|l| {
            let capped = maxstep(l, n);
            any |= capped.is_some();
            capped.unwrap_or_else(|| l.to_string()) + "\n"
        })
        .collect();
    any.then_some(out)
}

/// VACASK's noise plot (`onoise` in V²/Hz, `gain` the power gain) as
/// ngspice's spectrum plot: `onoise_spectrum` = √onoise and
/// `inoise_spectrum` = √(onoise/gain), in V/√Hz. Any other plot as it is.
pub fn spectrum(plot: RawData) -> RawData {
    let col = |name: &str| {
        plot.variables
            .iter()
            .position(|v| v.name.eq_ignore_ascii_case(name))
    };
    let noise = plot.plot_name.to_ascii_lowercase().contains("noise");
    let (Some(on), Some(gain), true) = (col("onoise"), col("gain"), noise) else {
        return plot;
    };
    let (on, gain) = (plot.re(on), plot.re(gain));
    assert_eq!(on.len(), gain.len(), "a gain per frequency");
    let var = |index: usize, name: &str, var_type: VarType| VarInfo {
        index,
        name: name.to_string(),
        var_type,
    };
    RawData {
        plot_name: "Noise Spectral Density Curves".into(),
        variables: vec![
            var(0, "frequency", VarType::Frequency),
            var(1, "onoise_spectrum", VarType::Voltage),
            var(2, "inoise_spectrum", VarType::Voltage),
        ],
        real_data: vec![
            plot.re(0).into_owned(),
            on.iter().map(|v| v.sqrt()).collect(),
            on.iter()
                .zip(gain.iter())
                .map(|(o, g)| (o / g).sqrt())
                .collect(),
        ],
        ..Default::default()
    }
}

// ---- private ----

/// `line`, an `analysis <name> tran … stop=<S>` line, with `maxstep=` at
/// most S/n; `None` for any other line or a `stop=` that is no number.
fn maxstep(line: &str, n: f64) -> Option<String> {
    let w: Vec<&str> = line.split_whitespace().collect();
    if w.len() < 4 || w[0] != "analysis" || w[2] != "tran" {
        return None;
    }
    let value = |key: &str| {
        let v = w.iter().find_map(|x| x.strip_prefix(key))?;
        spicerack::unit::parse_spice_number(v).map(|(x, _, _)| x)
    };
    let cap = value("stop=")? / n;
    let max = value("maxstep=").map_or(cap, |m| m.min(cap));
    let kept: Vec<&str> = w
        .iter()
        .copied()
        .filter(|x| !x.starts_with("maxstep="))
        .collect();
    let indent = &line[..line.len() - line.trim_start().len()];
    Some(format!("{indent}{} maxstep={max:e}", kept.join(" ")))
}
