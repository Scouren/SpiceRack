//! VACASK: run `vacask` on a native deck, read back every per-analysis `.raw`.
//!
//! Cites: VACASK's CLI; `docs/cmd-analysis-mc.md` ("Output": an `mc` loop's
//! raw file names).
//!
//! Contents:
//! - `pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError>`

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{BackendError, exec, read_results, scratch, sorted_files};
use crate::result::RawData;

pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError> {
    // The deck runs in a scratch dir so the per-analysis raw files land
    // somewhere disposable, which moves any relative `load`/`include` path
    // out from under the user's cwd. Resolve those before writing.
    let (dir, sim) = scratch(&absolutize_paths(netlist), "circuit.sim")?;
    // -se/-sp: a deck may carry `embed`/`postprocess` steps, which write
    // files and shell out to Python. Neither produces a raw file.
    let stdout = exec(
        Command::new("vacask")
            .args(["-se", "-sp"])
            .arg(&sim)
            .current_dir(dir.path()),
        "vacask",
        None,
    )?;
    // A failed analysis can exit 0 with nothing written, so each analysis's
    // raw file is the real success test.
    let files = raw_files_for(dir.path(), netlist).map_err(|missing| {
        BackendError::SimulationError(format!(
            "vacask wrote no {missing}\nstdout: {}",
            super::tail(&stdout)
        ))
    })?;
    let outer = outer_sweeps(netlist);
    let mut plots = Vec::with_capacity(files.len());
    for (k, raw) in files.iter().enumerate() {
        let mut read = read_results(raw, stdout.clone())?;
        // A deck that declares no analysis this can see has no sweeps either.
        let n = outer.get(k).copied().unwrap_or(0);
        read.iter_mut().for_each(|p| sweeps_last(p, n));
        plots.extend(read);
    }
    assert!(
        plots.len() >= files.len(),
        "every raw file read holds at least one plot"
    );
    Ok(plots)
}

// ---- private ----

/// The raw file stems a deck's analyses write, in order: `analysis <name>
/// <kind> ...` writes `<name>`; inside `mc <loop> samples=<n>` … `endmc`,
/// `<loop>.<i>.<name>` for each sample `i` from 1 to `n`.
fn analysis_names(netlist: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut mc: Option<(&str, u32)> = None;
    for l in netlist.lines() {
        let mut w = l.split_whitespace();
        match (w.next(), w.next()) {
            (Some("mc"), Some(name)) => {
                let samples = w.find_map(|kv| kv.strip_prefix("samples="));
                mc = Some((name, samples.and_then(|n| n.parse().ok()).unwrap_or(0)));
            }
            (Some("endmc"), _) => mc = None,
            (Some("analysis"), Some(name)) => match mc {
                Some((lp, n)) => out.extend((1..=n).map(|i| format!("{lp}.{i}.{name}"))),
                None => out.push(name.to_string()),
            },
            _ => {}
        }
    }
    out
}

/// For each analysis [`analysis_names`] lists, in the same order, how many
/// of the leading columns of its raw file are sweeps around it that are
/// not its own axis: VACASK writes each enclosing `sweep`'s variable first,
/// outermost first, then the analysis's own (pinned build: `step_r`,
/// `frequency`, … for an `ac` inside `sweep step_r`). An `op`'s innermost
/// sweep is its axis (a `.dc`); every other kind has its own (time,
/// frequency).
fn outer_sweeps(netlist: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let (mut sweeps, mut mc) = (0, 1);
    for l in netlist.lines() {
        let mut w = l.split_whitespace();
        match w.next() {
            Some("sweep") => sweeps += 1,
            Some("mc") => {
                let n = w.find_map(|kv| kv.strip_prefix("samples="));
                mc = n.and_then(|n| n.parse().ok()).unwrap_or(0);
            }
            Some("endmc") => mc = 1,
            Some("analysis") => {
                let kind = w.nth(1);
                let own = usize::from(kind == Some("op") && sweeps > 0);
                out.extend(std::iter::repeat_n(sweeps - own, mc));
                sweeps = 0;
            }
            _ => {}
        }
    }
    out
}

/// Moves `raw`'s first `n` columns after the others, so column 0 is the
/// analysis's own axis and each step's value is a vector of its own.
fn sweeps_last(raw: &mut RawData, n: usize) {
    let cols = raw.variables.len();
    assert!(
        n < cols.max(1),
        "an analysis's raw file holds its own columns"
    );
    assert!(raw.real_data.len() == cols || raw.real_data.is_empty());
    raw.variables.rotate_left(n);
    raw.variables
        .iter_mut()
        .enumerate()
        .for_each(|(i, v)| v.index = i);
    if raw.real_data.len() == cols {
        raw.real_data.rotate_left(n);
    }
    if raw.complex_data.len() == cols {
        raw.complex_data.rotate_left(n);
    }
}

/// VACASK writes one raw file per analysis run, named after it: each
/// declared analysis's ([`analysis_names`]), in deck order; for a deck that
/// declares none this can see, every `.raw` in name order. `Err` names the first declared analysis's
/// file that is missing, or `.raw` when nothing was written.
fn raw_files_for(dir: &Path, netlist: &str) -> Result<Vec<PathBuf>, String> {
    let declared: Vec<PathBuf> = analysis_names(netlist)
        .iter()
        .map(|n| dir.join(format!("{n}.raw")))
        .collect();
    if let Some(missing) = declared.iter().find(|p| !p.is_file()) {
        return Err(missing
            .file_name()
            .map_or_else(String::new, |f| f.to_string_lossy().into_owned()));
    }
    if !declared.is_empty() {
        return Ok(declared);
    }
    let all: Vec<PathBuf> = sorted_files(dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "raw"))
        .collect();
    if all.is_empty() {
        Err(".raw".into())
    } else {
        Ok(all)
    }
}

/// Rewrite relative `load "x"` / `include "x"` paths that exist under the
/// caller's cwd into absolute ones. A bare module name (`spice/diode.osdi`)
/// resolves through VACASK's own module path and is left alone.
fn absolutize_paths(netlist: &str) -> String {
    let Ok(cwd) = std::env::current_dir() else {
        return netlist.to_string();
    };
    let mut out = String::with_capacity(netlist.len());
    for line in netlist.lines() {
        let t = line.trim_start();
        let quoted = (t.starts_with("load \"") || t.starts_with("include \""))
            .then(|| line.split('"').nth(1))
            .flatten()
            .filter(|p| !Path::new(p).is_absolute() && cwd.join(p).is_file());
        match quoted {
            Some(p) => out.push_str(&line.replacen(p, &cwd.join(p).display().to_string(), 1)),
            None => out.push_str(line),
        }
        // The trailing newline is load-bearing: VACASK's `endc` wants one.
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_names_are_read_off_the_deck_in_order() {
        let deck = "x\ncontrol\n  analysis op1 op\n  sweep s instance=\"v1\" parameter=\"dc\"\n    analysis dc1 op\nendc\n";
        assert_eq!(analysis_names(deck), ["op1", "dc1"]);
        // `docs/cmd-analysis-mc.md` "Output": `mc1.<i>.op1.raw` per sample.
        let mc = "control\n  mc mc1 samples=2 seed=1 lh=0\n    analysis op2 op\n  endmc\nendc\n";
        assert_eq!(analysis_names(mc), ["mc1.1.op2", "mc1.2.op2"]);
    }

    #[test]
    fn a_step_sweeps_column_moves_after_the_analysis_axis() {
        // What the pinned VACASK wrote for `.step r` around an `ac`, an
        // `op` and a `.dc` (the `vsweep` the op's own axis):
        // `step_r frequency …`, `step_r …`, `step_r vsweep …`.
        let deck = "control\n  sweep step_r variable=\"r\"\n  analysis ac1 ac\n  sweep step_r variable=\"r\"\n  analysis op2 op\n  sweep step_r variable=\"r\"\n  sweep vsweep instance=\"vin\"\n    analysis dc3 op\n  analysis tr4 tran\nendc\n";
        assert_eq!(outer_sweeps(deck), [1, 0, 1, 0]);
        let mc = "  mc mc1 samples=2\n    analysis op2 op\n  endmc\n";
        assert_eq!(outer_sweeps(mc), [0, 0]);
        let var = |n: &str, i| crate::result::VarInfo {
            index: i,
            name: n.into(),
            var_type: crate::result::VarType::Other,
        };
        let mut raw = RawData {
            variables: vec![var("step_r", 0), var("frequency", 1), var("out", 2)],
            real_data: vec![vec![1.0], vec![2.0], vec![3.0]],
            ..RawData::default()
        };
        sweeps_last(&mut raw, 1);
        let names: Vec<_> = raw
            .variables
            .iter()
            .map(|v| (v.name.as_str(), v.index))
            .collect();
        assert_eq!(names, [("frequency", 0), ("out", 1), ("step_r", 2)]);
        assert_eq!(raw.real_data, [[2.0], [3.0], [1.0]]);
    }

    #[test]
    fn raw_files_are_every_declared_analysis_in_deck_order() {
        let dir = tempfile::TempDir::new().unwrap();
        let names = |r: Vec<PathBuf>| {
            r.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        for f in ["tr1.raw", "op1.raw", "zz.raw"] {
            std::fs::write(dir.path().join(f), "").unwrap();
        }
        assert_eq!(
            names(raw_files_for(dir.path(), "analysis tr1 tran\nanalysis op1 op\n").unwrap()),
            ["tr1.raw", "op1.raw"]
        );
        assert_eq!(
            names(raw_files_for(dir.path(), "").unwrap()),
            ["op1.raw", "tr1.raw", "zz.raw"]
        );
        assert_eq!(
            raw_files_for(dir.path(), "analysis op1 op\nanalysis ac1 ac\n"),
            Err("ac1.raw".into())
        );
        let empty = tempfile::TempDir::new().unwrap();
        assert_eq!(raw_files_for(empty.path(), ""), Err(".raw".into()));
    }
}
