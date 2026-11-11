//! Simulator backends: the [`RUNNABLE`] dialects, one row each in
//! `BACKENDS`, and the analysis kinds, one row each in `Kind::spec`.
//! Every simulator is a row of the registry (`registry.rs`, a [`Sim`]
//! handle each): `BACKENDS` is the five built-ins it registers first.
//!
//! A backend executes netlist text verbatim, in its own [`Dialect`]. Turning
//! a deck into that text is `syntax::render`'s job. Netlist text in, [`RawData`] plots
//! out, through one subprocess per run.
//!
//! Adding a simulator: its file here (`run`) and one `BACKENDS` row; the
//! kinds it runs that are not universal name it in their `Kind::spec` arm.
//! Adding an analysis kind: one `Kind` variant and its `Kind::spec` arm.
//!
//! Cites: Spectre Reference 19.1 p.17 (`.measure`); each simulator's file
//! cites its own CLI.
//!
//! Contents:
//! - `pub enum BackendError`
//! - `pub const RUNNABLE: [Dialect; BACKENDS.len()]`
//! - `pub use crate::syntax::Dialect`
//! - `pub mod feature`: `XSPICE`, `OSDI`, `MEASURES`, `STEP_PARAMS`, `CONTROL_BLOCKS`,
//!   `LAPLACE`, `VERILOG_COSIM`, `ALL` (`u8` bits)
//! - `pub(crate) fn runs(d: Dialect, needs: u8) -> bool`
//! - `pub fn parse(name: &str) -> Option<Dialect>`
//! - `Dialect::capabilities(self) -> u8`, `Dialect::run(self, netlist: &str) -> Result<RawData, BackendError>`,
//!   `Dialect::run_all(self, netlist: &str) -> Result<Vec<RawData>, BackendError>`
//! - `pub fn available() -> &'static [Dialect]`
//! - `pub use registry::{Installed, Runner, SIMS_MAX, Sim, SimDef, SimRegisterError, by_name,
//!   register_simulator, simulators}` (`registry.rs`: the simulator registry)
//! - `pub use kind::Kind`, `pub(crate) use kind::{Plot, Spec}` (`kind.rs`: the analysis kinds)
//! - `pub fn select(kind: Kind, override_backend: Option<&str>, needs: u8, writes: impl Fn(Dialect) -> bool) -> Result<Dialect, BackendError>`
//! - `pub struct Issue`, `pub enum IssueSeverity`,
//!   `pub fn check(needs: u8, elements: usize, backend: &str) -> Vec<Issue>`
//! - `pub mod espice`, `ltspice`, `ngspice`, `spectre`, `vacask`

mod kind;
mod registry;

pub mod espice;
pub mod ltspice;
pub mod ngspice;
pub mod spectre;
pub mod vacask;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use feature::*;

pub use kind::Kind;
pub(crate) use kind::{Plot, Spec};
pub use registry::{
    Installed, Runner, SIMS_MAX, Sim, SimDef, SimRegisterError, by_name, register_simulator,
    simulators,
};

use crate::rawfile;
use crate::result::RawData;
pub use crate::syntax::Dialect;

/// What a deck needs from its simulator, one bit per feature
/// ([`Dialect::capabilities`] is the same mask: what a simulator does).
pub mod feature {
    pub const XSPICE: u8 = 1 << 0;
    pub const OSDI: u8 = 1 << 1;
    pub const MEASURES: u8 = 1 << 2;
    pub const STEP_PARAMS: u8 = 1 << 3;
    pub const CONTROL_BLOCKS: u8 = 1 << 4;
    pub const LAPLACE: u8 = 1 << 5;
    pub const VERILOG_COSIM: u8 = 1 << 6;
    /// Every bit above.
    pub const ALL: u8 =
        XSPICE | OSDI | MEASURES | STEP_PARAMS | CONTROL_BLOCKS | LAPLACE | VERILOG_COSIM;
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("No simulator backend found. Install ngspice: sudo apt install ngspice")]
    NoBackend,

    #[error(
        "Analysis '{analysis}' not supported by {backend}.\n\n\
        This analysis requires {required}. Either:\n  \
        1. Install {required}\n  \
        2. Alternative: {alternative}"
    )]
    UnsupportedAnalysis {
        analysis: String,
        backend: String,
        required: String,
        alternative: String,
    },

    #[error("Simulation failed: {0}")]
    SimulationError(String),

    #[error("Raw file parse error: {0}")]
    RawParseError(#[from] rawfile::RawFileError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// The dialects a simulator here runs (`BACKENDS`, in its order); a
/// universal analysis runs on each, in this order of preference.
pub const RUNNABLE: [Dialect; BACKENDS.len()] = {
    let mut out = [Dialect::Ngspice; BACKENDS.len()];
    let mut i = 0;
    while i < BACKENDS.len() {
        out[i] = BACKENDS[i].dialect;
        i += 1;
    }
    out
};

/// One problem [`check`] finds with running a deck on a backend.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub severity: IssueSeverity,
    pub message: String,
}

/// Whether an [`Issue`] stops the run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IssueSeverity {
    Error,
    Warning,
}

// What each simulator can do, as `feature` bits.
const NGSPICE_CAPS: u8 = XSPICE | OSDI | MEASURES | CONTROL_BLOCKS | LAPLACE | VERILOG_COSIM;
const LTSPICE_CAPS: u8 = MEASURES | STEP_PARAMS | LAPLACE;
// VACASK: `load "x.osdi"`; no measurement statement ("Command not found");
// `var` + `sweep` does parameter sweeps; its `control` is not ngspice's.
const VACASK_CAPS: u8 = OSDI | STEP_PARAMS;
// Spectre: `.measure` per Spectre Reference 19.1 p.17. No Verilog
// co-simulation: a `verilog(mode="simulate")` block is ngspice's XSPICE
// `d_cosim` (`python::verilog`), which Spectre cannot run.
const SPECTRE_CAPS: u8 = OSDI | MEASURES | STEP_PARAMS;
// ESPice: Verilog-A via VerA `.hdl` (counts as osdi); `.step` lands every
// point as a separate plot in one raw file, which parse_raw does not split.
// Laplace: its own `E/G … LAPLACE` card (`render::spice::laplace`).
const ESPICE_CAPS: u8 = OSDI | MEASURES | LAPLACE;

/// One simulator SpiceRack drives: its dialect, what it can do
/// (`feature` bits), whether it is installed, and the run (its file's
/// `run`). A new simulator is a row here; the order is [`RUNNABLE`]'s.
struct Backend {
    dialect: Dialect,
    caps: u8,
    installed: fn() -> bool,
    run: fn(&str) -> Result<Vec<RawData>, BackendError>,
}

const BACKENDS: [Backend; 5] = [
    Backend {
        dialect: Dialect::Ngspice,
        caps: NGSPICE_CAPS,
        installed: || is_on_path("ngspice"),
        run: ngspice::run,
    },
    Backend {
        dialect: Dialect::Ltspice,
        caps: LTSPICE_CAPS,
        installed: || ltspice::find().is_some(),
        run: ltspice::run,
    },
    Backend {
        dialect: Dialect::Vacask,
        caps: VACASK_CAPS,
        installed: || is_on_path("vacask"),
        run: vacask::run,
    },
    Backend {
        dialect: Dialect::Spectre,
        caps: SPECTRE_CAPS,
        installed: || is_on_path("spectre"),
        run: spectre::run,
    },
    Backend {
        dialect: Dialect::Espice,
        caps: ESPICE_CAPS,
        installed: || is_on_path("espice"),
        run: espice::run,
    },
];

// One row per dialect: a second row for one would never be reached.
const _: () = {
    let mut i = 0;
    while i < BACKENDS.len() {
        let mut j = i + 1;
        while j < BACKENDS.len() {
            assert!(BACKENDS[i].dialect as u8 != BACKENDS[j].dialect as u8);
            j += 1;
        }
        i += 1;
    }
};

// Seven features, one distinct bit each, inside the `u8`.
const _: () = assert!(feature::ALL.count_ones() == 7);

/// Each feature a backend may lack, how bad that is, and what to say
/// (`{}` is the backend).
const MISSING: &[(u8, IssueSeverity, &str)] = &[
    (
        XSPICE,
        IssueSeverity::Error,
        "Backend '{}' does not support XSPICE A-elements (only ngspice does)",
    ),
    (
        OSDI,
        IssueSeverity::Error,
        "Backend '{}' does not support OSDI/Verilog-A model loading",
    ),
    (
        MEASURES,
        IssueSeverity::Error,
        "Backend '{}' does not support .meas directives",
    ),
    (
        STEP_PARAMS,
        IssueSeverity::Error,
        "Backend '{}' does not support .step param sweeps",
    ),
    (
        CONTROL_BLOCKS,
        IssueSeverity::Error,
        "Backend '{}' does not support .control blocks (ngspice only)",
    ),
    (
        LAPLACE,
        IssueSeverity::Warning,
        "Backend '{}' does not support Laplace-domain B-sources",
    ),
    (
        VERILOG_COSIM,
        IssueSeverity::Error,
        "Backend '{}' does not support Verilog co-simulation",
    ),
];

// Exactly one `MISSING` row per feature bit, so a backend `select` refuses
// for a feature always has the issue `check` names for it.
const _: () = {
    let (mut seen, mut i) = (0u8, 0);
    while i < MISSING.len() {
        assert!(seen & MISSING[i].0 == 0, "a feature has two MISSING rows");
        assert!(
            MISSING[i].0.count_ones() == 1,
            "a MISSING row names one feature"
        );
        seen |= MISSING[i].0;
        i += 1;
    }
    assert!(seen == ALL, "a feature has no MISSING row");
};

/// True when `d`'s simulator has every feature in `needs` (`feature`
/// bits); [`check`] reports each bit this finds missing.
pub(crate) fn runs(d: Dialect, needs: u8) -> bool {
    needs & !d.capabilities() == 0
}

/// A user-facing backend name (a [`RUNNABLE`] dialect's name or one of
/// [`Dialect::ALIASES`]) as the dialect it runs; `None` for anything else.
pub fn parse(name: &str) -> Option<Dialect> {
    Dialect::from_backend_name(name).filter(|d| RUNNABLE.contains(d))
}

impl Dialect {
    /// What this dialect's simulator can do, as `feature` bits; nothing
    /// for a dialect no simulator here runs.
    pub fn capabilities(self) -> u8 {
        Sim::of(self).map_or(0, |s| s.row().features)
    }

    /// Executes `netlist` (already in this dialect) and reads the results.
    ///
    /// # Errors
    /// `SimulationError` for a dialect outside [`RUNNABLE`] (HSPICE, Xyce,
    /// CDL, Verilog), and whatever the simulator run returns.
    pub fn run(self, netlist: &str) -> Result<RawData, BackendError> {
        let first = self.run_all(netlist)?.into_iter().next();
        Ok(first.expect("a raw file read holds at least one plot"))
    }

    /// [`Self::run`], returning every plot of the run in file order, each
    /// with the run's stdout. ngspice writes one per analysis (`noise` and
    /// `disto` two) in its own order (`ac`, `dc`, `op`, `tran`, `tf`,
    /// `noise`, `sens` for a deck without a `.control` block), not the
    /// deck's; VACASK one raw file per analysis, read in deck order.
    ///
    /// # Errors
    /// As [`Self::run`].
    pub fn run_all(self, netlist: &str) -> Result<Vec<RawData>, BackendError> {
        let Some(sim) = Sim::of(self) else {
            return Err(BackendError::SimulationError(format!(
                "no {} simulator is driven by SpiceRack",
                self.name()
            )));
        };
        sim.run_all(netlist)
    }
}

/// Backends installed on this machine, in preference order. Cached.
pub fn available() -> &'static [Dialect] {
    static CACHE: OnceLock<Vec<Dialect>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let found: Vec<Dialect> = RUNNABLE
            .into_iter()
            .filter(|&d| Sim::of(d).is_some_and(Sim::installed))
            .collect();
        assert!(found.len() <= RUNNABLE.len());
        found
    })
}

/// Pick the backend for an analysis of `kind`. An explicit name (argument,
/// else `$SPICERACK_BACKEND`) wins if it can run the analysis; otherwise
/// the first installed backend, in `route` order, that has every `needs`
/// bit (`feature`) and whose writer writes the deck (`writes`: the
/// renderer's verdict, so a run never lands where the deck is refused).
pub fn select(
    kind: Kind,
    override_backend: Option<&str>,
    needs: u8,
    writes: impl Fn(Dialect) -> bool,
) -> Result<Dialect, BackendError> {
    let Spec {
        name: analysis,
        route: dialects,
        alternative,
        ..
    } = kind.spec();
    assert!(!dialects.is_empty(), "every kind runs somewhere");
    let env = std::env::var("SPICERACK_BACKEND")
        .ok()
        .filter(|s| !s.is_empty());
    if let Some(name) = override_backend.or(env.as_deref()) {
        let kind = parse(name)
            .ok_or_else(|| BackendError::SimulationError(format!("Unknown backend: {name}")))?;
        if !dialects.contains(&kind) {
            return Err(unsupported(analysis, name, dialects[0], alternative));
        }
        return Ok(kind);
    }

    let available = available();
    if available.is_empty() {
        return Err(BackendError::NoBackend);
    }
    let installed = |d: &&Dialect| available.contains(d) && writes(**d);
    if let Some(&d) = dialects.iter().filter(installed).find(|d| runs(**d, needs)) {
        return Ok(d);
    }
    // ponytail: universal analysis, no backend matches the features — run on
    // the first one that writes the deck and let the simulator report what
    // it cannot do.
    match dialects.iter().find(installed) {
        Some(&d) if dialects[0] == Dialect::Ngspice => Ok(d),
        _ => {
            let names: Vec<_> = available.iter().map(|k| k.name()).collect();
            Err(unsupported(
                analysis,
                &names.join(", "),
                dialects[0],
                alternative,
            ))
        }
    }
}

/// The issues `backend` raises against a deck that needs the `needs` bits
/// (`feature`) and holds `elements` element and instance cards: each
/// feature it lacks, and a large deck on ngspice. Pure: nothing runs.
pub fn check(needs: u8, elements: usize, backend: &str) -> Vec<Issue> {
    let Some(kind) = parse(backend) else {
        return vec![Issue {
            severity: IssueSeverity::Error,
            message: format!("Unknown backend '{backend}'"),
        }];
    };
    let missing = needs & !kind.capabilities();
    let mut issues: Vec<Issue> = MISSING
        .iter()
        .filter(|(f, _, _)| missing & f != 0)
        .map(|&(_, severity, message)| Issue {
            severity,
            message: message.replace("{}", backend),
        })
        .collect();
    if elements > 10_000 && kind == Dialect::Ngspice {
        issues.push(Issue {
            severity: IssueSeverity::Warning,
            message: format!("Large circuit ({elements} elements) may be slow on '{backend}'"),
        });
    }
    issues
}

// ---- private ----

fn unsupported(
    analysis: &str,
    backend: &str,
    required: Dialect,
    alternative: &str,
) -> BackendError {
    let required = required.name().to_string();
    BackendError::UnsupportedAnalysis {
        analysis: analysis.to_string(),
        backend: backend.to_string(),
        required,
        alternative: alternative.to_string(),
    }
}

// ── Subprocess plumbing shared by every backend ──

fn is_on_path(program: &str) -> bool {
    Command::new("which")
        .arg(program)
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A scratch directory holding `netlist` as `file_name`. The directory (and
/// everything the simulator writes into it) is deleted on drop.
fn scratch(netlist: &str, file_name: &str) -> Result<(tempfile::TempDir, PathBuf), BackendError> {
    let dir = tempfile::TempDir::new()?;
    let deck = dir.path().join(file_name);
    std::fs::write(&deck, netlist)?;
    Ok((dir, deck))
}

/// Run `cmd` and return its stdout. A nonzero exit becomes an error carrying
/// the lines that mention an error (from stdout, stderr and the `log` file
/// the simulator wrote), or the output tails when none do.
fn exec(cmd: &mut Command, program: &str, log: Option<&Path>) -> Result<String, BackendError> {
    let out = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            BackendError::SimulationError(format!("{program} not found on $PATH"))
        }
        _ => BackendError::Io(e),
    })?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    let log = log
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let errors: Vec<&str> = stdout
        .lines()
        .chain(stderr.lines())
        .chain(log.lines())
        .filter(|l| l.contains("rror") || l.contains("failed"))
        .collect();
    let detail = if errors.is_empty() {
        format!(
            "stdout: {}\nstderr: {}\nlog: {}",
            tail(&stdout),
            tail(&stderr),
            tail(&log)
        )
    } else {
        errors.join("\n")
    };
    Err(BackendError::SimulationError(format!(
        "{program} exited with {}\n{detail}",
        out.status
    )))
}

fn tail(s: &str) -> &str {
    let mut start = s.len().saturating_sub(1000);
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

/// Read a result file, nutmeg raw or PSF (sniffed from its content).
fn read_results(path: &Path, stdout: String) -> Result<Vec<RawData>, BackendError> {
    let bytes = std::fs::read(path).map_err(|e| {
        BackendError::SimulationError(format!(
            "no result file '{}': {e}\nstdout: {}",
            path.display(),
            tail(&stdout)
        ))
    })?;
    // Spectre's PSF keeps its magic in the trailer. It is not read: the
    // Spectre backend asks for `-format nutascii`.
    if bytes.len() >= 12 && &bytes[bytes.len() - 12..bytes.len() - 4] == b"Clarissa" {
        return Err(BackendError::SimulationError(
            "PSF output is not read; run Spectre with -format nutascii".into(),
        ));
    }
    let mut plots = rawfile::parse_raw_all(&bytes)?;
    plots.iter_mut().for_each(|p| p.stdout.clone_from(&stdout));
    Ok(plots)
}

/// Regular files directly inside `dir`, sorted (read_dir order is not stable).
fn sorted_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_table_filters_by_feature() {
        assert!(runs(Dialect::Ngspice, XSPICE));
        assert!(!runs(Dialect::Vacask, XSPICE));
        assert!(!runs(Dialect::Spectre, XSPICE));
        assert!(!runs(Dialect::Ltspice, OSDI));
        assert!(runs(Dialect::Espice, OSDI));
        assert!(!runs(Dialect::Ngspice, STEP_PARAMS | MEASURES));
        assert!(!runs(Dialect::Vacask, STEP_PARAMS | MEASURES));
        assert!(runs(Dialect::Spectre, STEP_PARAMS | MEASURES));
        assert!(runs(Dialect::Ltspice, STEP_PARAMS | MEASURES));
        assert!(runs(Dialect::Ngspice, 0));
    }

    #[test]
    fn routes_match_the_analysis() {
        use Dialect::*;
        assert_eq!(Kind::spec(Kind::Pz).route, &[Ngspice]);
        assert_eq!(Kind::spec(Kind::Hb).route, &[Vacask, Spectre]);
        assert_eq!(Kind::spec(Kind::TranNoise).route, &[Ngspice]);
        assert_eq!(Kind::spec(Kind::Sens).route, &[Ngspice, Spectre, Espice]);
        assert_eq!(Kind::spec(Kind::SensAc).route, &[Ngspice, Spectre]);
        assert_eq!(Kind::spec(Kind::Tran).route, RUNNABLE);
        assert_eq!(
            Kind::spec(Kind::Tran).route.last(),
            Some(&Espice),
            "espice is never preferred first"
        );
    }

    #[test]
    fn explicit_backend_must_run_the_analysis() {
        assert_eq!(
            select(Kind::Pz, Some("ngspice"), 0, |_| true).unwrap(),
            Dialect::Ngspice
        );
        assert_eq!(
            select(Kind::Dc, Some("vacask-shared"), 0, |_| true).unwrap(),
            Dialect::Vacask
        );
        assert!(
            matches!(select(Kind::Pz, Some("ltspice"), 0, |_| true), Err(BackendError::UnsupportedAnalysis { analysis, .. }) if analysis == "pz")
        );
        assert!(matches!(
            select(Kind::Hb, Some("ngspice"), 0, |_| true),
            Err(BackendError::UnsupportedAnalysis { .. })
        ));
        assert!(select(Kind::Op, Some("xyce"), 0, |_| true).is_err());
    }

    #[test]
    fn check_names_each_missing_feature() {
        let issues = check(XSPICE | LAPLACE, 1, "vacask");
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].severity, IssueSeverity::Error);
        assert!(issues[0].message.contains("XSPICE"), "{issues:?}");
        assert_eq!(issues[1].severity, IssueSeverity::Warning);
        assert!(check(XSPICE, 1, "ngspice").is_empty());
        assert!(
            check(0, 20_000, "ngspice")[0]
                .message
                .contains("Large circuit")
        );
        assert_eq!(check(0, 1, "nope")[0].message, "Unknown backend 'nope'");
    }

    #[test]
    fn names_round_trip() {
        for d in RUNNABLE {
            assert_eq!(parse(d.name()), Some(d));
        }
        for (alias, d) in Dialect::ALIASES {
            assert_eq!(parse(alias), Some(d));
        }
        assert_eq!(parse("hspice"), None, "emitted, never run");
        assert_eq!(parse("Ngspice"), None, "backend names are exact");
        assert!(Dialect::Xyce.run("").is_err());
        assert_eq!(Dialect::Cdl.capabilities(), 0);
    }
}
