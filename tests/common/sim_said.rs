//! What a simulator said about its run, read off its output: its own error report
//! (`reported`, which fails a checked run, `sim::run_checked`) and the lines a failed run's
//! reason quotes (`errors`, LTspice's `ltspice_said`). A child of `sim.rs` (`#[path]`), for the
//! 500-line cap.
//!
//! Cites: the simulators' own output, recorded in `.#testing` (ngspice-45 "Error: ..." on R70's
//! F7 deck; VACASK "Variable or constant '$temp' not defined." with exit 0).
//!
//! Contents: `pub(super) fn reported(sim: Sim, log: &str) -> Option<String>`,
//! `pub(super) fn errors(log: &str) -> String`, `pub(super) fn ltspice_said(log: &Path) -> String`

use std::path::Path;

use super::Sim;

/// The error lines `sim` printed in `log`, the first four joined; `None` when it printed none
/// ([`run_checked`]'s forms).
pub(super) fn reported(sim: Sim, log: &str) -> Option<String> {
    let lines: Vec<&str> = match sim {
        Sim::Ngspice | Sim::NgspiceHsa => log
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("Error") || l.contains("Simulation interrupted"))
            .collect(),
        Sim::Vacask => {
            let run = log.split_once("Simulating:").map_or(log, |(_, r)| r);
            let words = ["not defined", "error", "aborted", "failed"];
            run.lines()
                .map(str::trim)
                .filter(|l| {
                    let low = l.to_ascii_lowercase();
                    words.iter().any(|w| low.contains(w))
                })
                .collect()
        }
        _ => Vec::new(),
    };
    (!lines.is_empty()).then(|| lines.into_iter().take(4).collect::<Vec<_>>().join(" | "))
}

/// The first four lines of `log` that name an error, each with the line
/// after it when that one names the cause instead (ngspice's "Error on line
/// N", Xyce's "Netlist error in file F at or near line N"), so a reason
/// reads `Netlist error ... line 8: Model is required for device Q0`.
pub(super) fn errors(log: &str) -> String {
    const CAUSES: [&str; 6] = [
        "error",
        "undefined",
        "level must",
        "not enough",
        "unknown",
        "no such",
    ];
    // VACASK names no error: its messages are the lines after
    // `Simulating:` (its banner, `This is vacask unknown.`, is not one).
    if let Some((_, run)) = log.split_once("Simulating:") {
        let said: Vec<&str> = run
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|l| {
                !l.is_empty()
                    && !l.starts_with("Running analysis")
                    && !l.starts_with("Elapsed time")
            })
            .take(4)
            .collect();
        if !said.is_empty() {
            return said.join(" | ");
        }
    }
    let all: Vec<&str> = log.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in all.iter().enumerate() {
        let low = l.to_lowercase();
        if !CAUSES.iter().any(|c| low.contains(c))
            || low.contains("fatal error in ngspice")
            || low.starts_with("this is vacask")
        {
            continue;
        }
        // Xyce wraps its location ("... at" / "or near line 8", "... at or" /
        // "near line 8"): the cause
        // is the line after the wrap.
        let after = |k: usize| all.get(i + k).copied().unwrap_or("");
        let next = if after(1).starts_with("or near") || after(1).starts_with("near line") {
            after(2)
        } else {
            after(1)
        };
        let located = low.starts_with("netlist error in file") || low.starts_with("error on line");
        out.push(if located {
            format!("{l}: {next}")
        } else {
            l.to_string()
        });
        if out.len() == 4 {
            break;
        }
    }
    out.join(" | ")
}

/// Why an LTspice run failed: its log's lines after `Start Time:`, less
/// the settings it lists (`solver = Normal`, `Maximum thread count: 32`,
/// `Total elapsed time: …`) and each line's Windows path, the first four.
pub(super) fn ltspice_said(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let run = text.split_once("Start Time:").map_or("", |(_, r)| r);
    let setting =
        |l: &str| l.contains(" = ") || l.starts_with("Maximum thread") || l.starts_with("Total ");
    let said: Vec<&str> = run
        .lines()
        .skip(1)
        .map(|l| l.rsplit('\\').next().unwrap_or(l).trim())
        .filter(|l| !l.is_empty() && !setting(l))
        .take(4)
        .collect();
    said.join(" | ")
}
