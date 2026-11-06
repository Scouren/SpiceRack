//! One deck on one simulator -> every plot it wrote. Each run gets a fresh
//! directory holding a link to every file beside the deck, so relative
//! includes resolve and the simulator's own output files (Xyce's `.prn`,
//! VACASK's per-analysis `.raw`) land somewhere writable; the corpora sit
//! read-only in the nix store.
//!
//! Cites: the simulators' command lines: ngspice manual §16.4.1 (`-b`, `-r`,
//! `-D ngbehavior=`), Xyce Users' Guide 7.10 §2.1.2 (`-r <file>`), VACASK
//! `vacask -h` (`-se`, `-sp`), ESPice `espice --help` (`--rawfile`,
//! `--format`, `--tokenizer`).
//!
//! Contents:
//! - `pub enum Sim`, `Sim::name(self) -> &'static str`, `Sim::on_path(self) -> bool`
//! - `pub enum Ran`
//! - `pub fn run(sim: Sim, deck: &Path, text: &str, files: &[(String, String)], secs: u64) -> Ran`

use std::path::Path;
use std::process::Command;

use spicerack::result::RawData;

/// A simulator a cell runs, with the reading mode it is started in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sim {
    Ngspice,
    /// ngspice in its HSPICE compatibility mode (`ngbehavior=hsa`).
    NgspiceHsa,
    Xyce,
    Vacask,
    Espice,
    /// ESPice reading HSPICE text (`--tokenizer=hspice`).
    EspiceHspice,
}

/// What one run gave.
pub enum Ran {
    Plots(Vec<RawData>),
    /// Past the CPU budget.
    Timeout,
    /// No result: the reason, with the simulator's first error lines.
    Failed(String),
}

/// The deck's own name inside the run directory: hidden, so it never shadows
/// a file of the corpus beside it.
const DECK: &str = ".cross_sim_deck";

impl Sim {
    /// A short name for reports and known-failure lists.
    pub fn name(self) -> &'static str {
        match self {
            Sim::Ngspice => "ngspice",
            Sim::NgspiceHsa => "ngspice-hsa",
            Sim::Xyce => "xyce",
            Sim::Vacask => "vacask",
            Sim::Espice => "espice",
            Sim::EspiceHspice => "espice-hspice",
        }
    }

    /// Whether the simulator's program answers on `PATH`.
    pub fn on_path(self) -> bool {
        let (program, arg) = self.program();
        Command::new(program).arg(arg).output().is_ok()
    }

    fn program(self) -> (&'static str, &'static str) {
        match self {
            Sim::Ngspice | Sim::NgspiceHsa => ("ngspice", "--version"),
            Sim::Xyce => ("Xyce", "-v"),
            Sim::Vacask => ("vacask", "-h"),
            Sim::Espice | Sim::EspiceHspice => ("espice", "--help"),
        }
    }
}

/// Runs `text` on `sim` as if it were the file `deck` (beside `deck`'s
/// files, each of `files` in place of the one of its name), at most `secs`
/// CPU seconds.
pub fn run(sim: Sim, deck: &Path, text: &str, files: &[(String, String)], secs: u64) -> Ran {
    assert!(secs > 0, "a CPU budget");
    match run_in(sim, deck, text, files, secs) {
        Ok(r) => r,
        Err(e) => Ran::Failed(e),
    }
}

// ---- private ----

fn run_in(
    sim: Sim,
    deck: &Path,
    text: &str,
    files: &[(String, String)],
    secs: u64,
) -> Result<Ran, String> {
    let mut tmp = Some(tempfile::TempDir::new().map_err(|e| e.to_string())?);
    let kept = tmp
        .as_ref()
        .map(|t| t.path().to_path_buf())
        .unwrap_or_default();
    if std::env::var_os("CROSS_SIM_KEEP").is_some() {
        let _ = tmp.take().map(tempfile::TempDir::keep);
        eprintln!(
            "kept {} on {}: {}",
            deck.display(),
            sim.name(),
            kept.display()
        );
    }
    let dir = kept.as_path();
    if let Some(parent) = deck.parent()
        && let Ok(rd) = std::fs::read_dir(parent)
    {
        for e in rd.flatten() {
            let _ = std::os::unix::fs::symlink(e.path(), dir.join(e.file_name()));
        }
    }
    for (name, body) in files {
        let at = dir.join(name);
        let _ = std::fs::remove_file(&at);
        std::fs::write(&at, body).map_err(|e| e.to_string())?;
    }
    let ext = deck
        .extension()
        .map_or("sp".into(), |e| e.to_string_lossy());
    let file = dir.join(format!("{DECK}.{ext}"));
    let raw = dir.join(".cross_sim_out.raw");
    let text = match sim {
        Sim::Ngspice | Sim::NgspiceHsa => ngspice_text(text, &raw),
        _ => text.to_string(),
    };
    std::fs::write(&file, &text).map_err(|e| e.to_string())?;
    let mut c = Command::new("timeout");
    c.arg((10 * secs).to_string())
        .arg("prlimit")
        .arg(format!("--cpu={secs}"))
        .env("OMP_NUM_THREADS", "1")
        .current_dir(dir);
    match sim {
        Sim::Ngspice | Sim::NgspiceHsa => {
            c.args(["ngspice", "-b"]);
            if sim == Sim::NgspiceHsa {
                c.args(["-D", "ngbehavior=hsa"]);
            }
            if !has_control(&text) {
                c.arg("-r").arg(&raw);
            }
        }
        Sim::Xyce => {
            c.arg("Xyce").arg("-r").arg(&raw);
        }
        Sim::Vacask => {
            c.args(["vacask", "-se", "-sp"]);
        }
        Sim::Espice | Sim::EspiceHspice => {
            c.arg("espice")
                .arg("--rawfile")
                .arg(&raw)
                .arg("--format=binary");
            if sim == Sim::EspiceHspice {
                c.arg("--tokenizer=hspice");
            }
        }
    }
    let out = c.arg(&file).output().map_err(|e| e.to_string())?;
    if timed_out(&out) {
        return Ok(Ran::Timeout);
    }
    let log = (String::from_utf8_lossy(&out.stdout) + String::from_utf8_lossy(&out.stderr))
        .replace(&format!("{}/", dir.display()), "");
    // ngspice runs a MOS model of a level it lacks (PSP's 102, ...) as level
    // 1, with a warning only: a result, of another circuit.
    if log.contains("Setting Level to 1") {
        return Ok(Ran::Failed(format!(
            "ngspice swapped the model: {}",
            errors(&log)
        )));
    }
    let raws = match sim {
        Sim::Vacask => vacask_raws(dir, &text),
        _ => vec![raw],
    };
    let mut plots = Vec::new();
    for r in &raws {
        let Ok(bytes) = std::fs::read(r) else {
            return Ok(Ran::Failed(format!(
                "no raw file {}: {}",
                r.file_name().unwrap_or_default().to_string_lossy(),
                errors(&log)
            )));
        };
        let all = spicerack::rawfile::parse_raw_all(&bytes).map_err(|e| e.to_string())?;
        // ngspice's `write` after a failed analysis writes its constants plot.
        plots.extend(
            all.into_iter()
                .filter(|p| !p.plot_name.eq_ignore_ascii_case("constants")),
        );
    }
    if plots.is_empty() {
        return Ok(Ran::Failed(format!("no plot: {}", errors(&log))));
    }
    Ok(Ran::Plots(plots))
}

/// `timeout` exits 124 itself; a CPU-limit kill (SIGXCPU, SIGKILL at the
/// hard limit) is re-raised on `timeout`, so there is no exit code.
fn timed_out(o: &std::process::Output) -> bool {
    use std::os::unix::process::ExitStatusExt;
    matches!(o.status.code(), Some(124 | 137 | 152)) || matches!(o.status.signal(), Some(9 | 24))
}

/// The first four lines of `log` that name an error, each with the line
/// after it when that one names the cause instead (ngspice's "Error on line
/// N", Xyce's "Netlist error in file F at or near line N"), so a reason
/// reads `Netlist error ... line 8: Model is required for device Q0`.
fn errors(log: &str) -> String {
    const CAUSES: [&str; 6] = [
        "error",
        "undefined",
        "level must",
        "not enough",
        "unknown",
        "no such",
    ];
    let all: Vec<&str> = log.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (i, l) in all.iter().enumerate() {
        let low = l.to_lowercase();
        if !CAUSES.iter().any(|c| low.contains(c)) || low.contains("fatal error in ngspice") {
            continue;
        }
        // Xyce wraps its location ("... at" / "or near line 8"): the cause
        // is the line after the wrap.
        let after = |k: usize| all.get(i + k).copied().unwrap_or("");
        let next = if after(1).starts_with("or near") {
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

/// Whether a `.control` block in `text` issues a run command; ngspice then
/// ignores `-r`, so the block itself writes the raw file.
fn has_control(text: &str) -> bool {
    let mut inside = false;
    for l in text.lines() {
        let w = l
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match w.as_str() {
            ".control" => inside = true,
            ".endc" => inside = false,
            "run" | "op" | "ac" | "tran" | "dc" | "noise" | "tf" if inside => return true,
            _ => {}
        }
    }
    false
}

/// `text` with a pinned seed (gf180's `agauss` draws a fresh value each run
/// otherwise) and, when a `.control` block runs the analyses, a `write` of
/// the current plot at the end of the last block.
fn ngspice_text(text: &str, raw: &Path) -> String {
    let (title, body) = text.split_once('\n').unwrap_or((text, ""));
    let mut out = format!("{title}\n.option seed=1\n{body}");
    if has_control(text)
        && let Some(at) = out.to_ascii_lowercase().rfind("\n.endc")
    {
        out.insert_str(at, &format!("\nwrite {}", raw.display()));
    }
    out
}

/// The raw files a VACASK deck writes, one per `analysis <name>` in deck
/// order (`<name>.raw`).
fn vacask_raws(dir: &Path, text: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for l in text.lines() {
        let mut w = l.split_whitespace();
        if let (Some("analysis"), Some(name)) = (w.next(), w.next()) {
            out.push(dir.join(format!("{name}.raw")));
        }
    }
    out
}
