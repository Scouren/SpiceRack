//! One deck on one simulator -> every plot it wrote. Each run gets a fresh
//! directory holding a link to every file beside the deck, so relative
//! includes resolve and the simulator's own output files (Xyce's `.prn`,
//! VACASK's per-analysis `.raw`) land somewhere writable; the corpora sit
//! read-only in the nix store.
//!
//! Cites: the simulators' command lines: ngspice manual §16.4.1 (`-b`, `-r`,
//! `-D ngbehavior=`), Xyce Users' Guide 7.10 §2.1.2 (`-r <file>`), VACASK
//! `vacask -h` (`-se`, `-sp`), ESPice `espice --help` (`--rawfile`,
//! `--format`, `--tokenizer`), LTspice 26.1.1 help: Command Line Switches
//! (`-b`: "will leave the data in file deck.raw") and `.OPTIONS`
//! (`plotwinsize=0`: no waveform compression; `numdgt` above 6: "double
//! precision is used for dependent variable data").
//!
//! Contents:
//! - `pub enum Sim`, `Sim::name(self) -> &'static str`, `Sim::on_path(self) -> bool`
//! - `pub enum Ran`
//! - `pub fn run(sim: Sim, deck: &Path, text: &str, files: &[(String, String)], secs: u64) -> Ran`
//! - `pub fn refined(sim: Sim, text: &str, n: f64) -> Option<String>`

use std::path::Path;
use std::process::Command;

use spicerack::result::{RawData, VarInfo, VarType};

/// A simulator a cell runs, with the reading mode it is started in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sim {
    Ngspice,
    /// ngspice in its HSPICE compatibility mode (`ngbehavior=hsa`).
    NgspiceHsa,
    Xyce,
    Vacask,
    /// ESPice reading ngspice text (`espice --tokenizer=ngspice`), which is
    /// what SpiceRack writes for it: untold, ESPice 0.3.0 reads a deck
    /// holding a card ngspice lacks (`.fft`, `.data`, …) as HSPICE text.
    Espice,
    /// ESPice reading HSPICE text (`espice-hspice --tokenizer=hspice`): the
    /// revision `flake.nix` pins for its HSPICE reader (ZERO-LEDGER P1-063),
    /// whatever revision `espice` is at.
    EspiceHspice,
    /// That pinned ESPice reading ngspice text (`espice-hspice
    /// --tokenizer=ngspice`): the control of the cell [`Sim::EspiceHspice`]
    /// is the target of, the same build.
    EspicePinned,
    /// LTspice 26.1.1 under Wine (`ltspice`, `nix develop .#testing-ltspice`).
    Ltspice,
}

/// What one run gave.
pub enum Ran {
    Plots(Vec<RawData>),
    /// Past the CPU budget, or the output one ([`FSIZE`]): a resource
    /// limit of the harness, not a result.
    Timeout,
    /// No result: the reason, with the simulator's first error lines.
    Failed(String),
}

/// Bytes one simulator may write to any one file: a runaway transient's raw
/// file dies at the cap (SIGXFSZ) instead of filling the disk.
const FSIZE: u64 = 2 << 30;

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
            Sim::EspicePinned => "espice-pinned",
            Sim::Ltspice => "ltspice",
        }
    }

    /// Whether the simulator's program is on `PATH` (looked up, not run:
    /// LTspice started with no deck opens its window and never returns).
    pub fn on_path(self) -> bool {
        let program = match self {
            Sim::Ngspice | Sim::NgspiceHsa => "ngspice",
            Sim::Xyce => "Xyce",
            Sim::Vacask => "vacask",
            Sim::Espice => "espice",
            Sim::EspiceHspice | Sim::EspicePinned => "espice-hspice",
            Sim::Ltspice => "ltspice",
        };
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&path).any(|d| d.join(program).is_file())
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

/// `text` with every transient's TMAX at most (TSTOP - TSTART)/n: the
/// refinement oracle (ZERO-LEDGER P1-012, mismatches.md §4.2): a pair that
/// agrees once both simulators take small steps differed numerically.
/// A VACASK deck's `tran` takes `maxstep=` (`vacask::refined`): `None`
/// when it has no transient with a literal `stop=`.
pub fn refined(sim: Sim, text: &str, n: f64) -> Option<String> {
    if sim == Sim::Vacask {
        return super::vacask::refined(text, n);
    }
    // LTspice's `.tran <Tstop>` in the four-number order first.
    let line = |l: &str| match sim {
        Sim::Ltspice => tmax(&ltspice_tran(l), n),
        _ => tmax(l, n),
    };
    Some(text.lines().map(|l| line(l) + "\n").collect())
}

// ---- private ----

fn run_in(
    sim: Sim,
    deck: &Path,
    text: &str,
    files: &[(String, String)],
    secs: u64,
) -> Result<Ran, String> {
    let mut tmp = Some(tempfile::TempDir::new().unwrap_or_else(|e| harness(&e)));
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
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| harness(&e));
        }
        let _ = std::fs::remove_file(&at);
        std::fs::write(&at, body).unwrap_or_else(|e| harness(&e));
    }
    let ext = deck
        .extension()
        .map_or("sp".into(), |e| e.to_string_lossy());
    let file = dir.join(format!("{DECK}.{ext}"));
    let raw = dir.join(".cross_sim_out.raw");
    let text = match sim {
        Sim::Ngspice | Sim::NgspiceHsa => ngspice_text(text, &raw),
        // No waveform compression and double-precision vectors, as
        // `backend::ltspice` runs a deck; line 1 is the title.
        Sim::Ltspice => {
            let (title, body) = text.split_once('\n').unwrap_or((text, ""));
            format!("{title}\n.options plotwinsize=0\n.options numdgt=15\n{body}")
        }
        _ => text.to_string(),
    };
    if matches!(sim, Sim::Ngspice | Sim::NgspiceHsa) {
        link_osdi(dir, &text);
    }
    std::fs::write(&file, &text).unwrap_or_else(|e| harness(&e));
    let mut c = Command::new("timeout");
    c.arg((10 * secs).to_string())
        .arg("prlimit")
        .arg(format!("--cpu={secs}"))
        .arg(format!("--fsize={FSIZE}"))
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
        Sim::Ltspice => {
            c.args(["ltspice", "-b"]);
        }
        Sim::Espice | Sim::EspiceHspice | Sim::EspicePinned => {
            // `espice` is the current build, `espice-hspice` the pinned one
            // (`Sim::on_path`); each is told what its deck is, and only
            // `EspiceHspice`'s is HSPICE text.
            let (pinned, hspice) = (sim != Sim::Espice, sim == Sim::EspiceHspice);
            let reads = if hspice { "hspice" } else { "ngspice" };
            c.arg(if pinned { "espice-hspice" } else { "espice" })
                .arg("--rawfile")
                .arg(&raw)
                .arg("--format=binary")
                .arg(format!("--tokenizer={reads}"));
        }
    }
    let out = c.arg(&file).output().unwrap_or_else(|e| harness(&e));
    // SIGXFSZ (25, or 128 + 25 from a shell): a file past FSIZE.
    let past_fsize = {
        use std::os::unix::process::ExitStatusExt;
        out.status.signal() == Some(25) || out.status.code() == Some(153)
    };
    if timed_out(&out) || past_fsize {
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
        Sim::Vacask => vacask_raws(dir, &text, files),
        // LTspice writes `<deck>.raw` beside the deck.
        Sim::Ltspice => vec![file.with_extension("raw")],
        _ => vec![raw],
    };
    // LTspice says why a run failed in `<deck>.log`, nothing on stdout.
    let said = || match sim {
        Sim::Ltspice => ltspice_said(&file.with_extension("log")),
        _ => errors(&log),
    };
    // It leaves a raw file behind a failed run too (an over-defined
    // matrix): its exit status says the run failed.
    if sim == Sim::Ltspice && !out.status.success() {
        return Ok(Ran::Failed(said()));
    }
    let mut plots = Vec::new();
    for r in &raws {
        let Ok(bytes) = std::fs::read(r) else {
            return Ok(Ran::Failed(format!(
                "no raw file {}: {}",
                r.file_name().unwrap_or_default().to_string_lossy(),
                said()
            )));
        };
        let all = spicerack::rawfile::parse_raw_all(&bytes).map_err(|e| e.to_string())?;
        // ngspice's `write` after a failed analysis writes its constants plot.
        // VACASK's noise plot in ngspice's names and units; any other plot
        // passes through (`vacask::spectrum`).
        plots.extend(
            all.into_iter()
                .filter(|p| !p.plot_name.eq_ignore_ascii_case("constants"))
                .map(super::vacask::spectrum),
        );
    }
    if matches!(sim, Sim::Ngspice | Sim::NgspiceHsa) && dc_loop(&text) {
        plots = join_dc_passes(plots);
    }
    if sim == Sim::Xyce
        && let Some(n) = xyce_noise(&dir.join(format!("{DECK}.{ext}_noise.dat")))
    {
        plots.retain(|p| !p.plot_name.to_ascii_lowercase().contains("noise"));
        plots.push(n);
    }
    if plots.is_empty() {
        return Ok(Ran::Failed(format!("no plot: {}", said())));
    }
    Ok(Ran::Plots(plots))
}

/// A `.control` `dc` at a loop's value (`dc v1 $d1 $d1 1`): the `.dc`
/// axis by decade, octave or list that `render/spice/loops.rs` writes as
/// `foreach` passes, a plot a pass.
fn dc_loop(text: &str) -> bool {
    text.lines().any(|l| {
        let l = l.trim_start().to_ascii_lowercase();
        l.starts_with("dc ") && l.contains('$')
    })
}

/// Consecutive real DC plots over the same vectors as one, their points
/// appended in run order: a `.dc` loop's passes as the source's one `.dc`
/// plot (the first axis innermost, as the loops run).
// ponytail: a second `.dc` card beside the loop, over the same vectors,
// joins it too; split by the loop's pass count when a deck has both.
fn join_dc_passes(plots: Vec<RawData>) -> Vec<RawData> {
    let dc = |p: &RawData| {
        !p.is_complex
            && p.plot_name
                .eq_ignore_ascii_case("dc transfer characteristic")
    };
    let mut out: Vec<RawData> = Vec::with_capacity(plots.len());
    for p in plots {
        match out.last_mut() {
            Some(last) if dc(last) && dc(&p) && same_vectors(last, &p) => {
                for (col, more) in last.real_data.iter_mut().zip(p.real_data) {
                    col.extend(more);
                }
            }
            _ => out.push(p),
        }
    }
    out
}

fn same_vectors(a: &RawData, b: &RawData) -> bool {
    a.variables.len() == b.variables.len()
        && a.variables
            .iter()
            .zip(&b.variables)
            .all(|(x, y)| x.name.eq_ignore_ascii_case(&y.name))
}

/// A run directory or process the harness itself cannot make (a full
/// disk, a missing `timeout`): no verdict on the deck, so the test stops.
fn harness(e: &std::io::Error) -> ! {
    panic!("cross_sim harness: {e}")
}

/// Each bare `pre_osdi <name>.osdi` of `text` that is not beside the deck,
/// linked from VACASK's module directory (`<vacask>/../lib/vacask/mod`):
/// a VACASK deck loads a model from its module path (VACASK
/// docs/startup-paths.md), a user running it on ngspice puts that file
/// beside the deck.
fn link_osdi(dir: &Path, text: &str) {
    let names = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pre_osdi"))
        .map(|n| n.trim().trim_matches(['"', '\'']))
        .filter(|n| n.ends_with(".osdi") && !n.contains('/') && !dir.join(n).exists());
    let mut mods = None;
    for n in names {
        let m = mods.get_or_insert_with(vacask_modules);
        if let Some(m) = m
            && m.join(n).is_file()
        {
            let _ = std::os::unix::fs::symlink(m.join(n), dir.join(n));
        }
    }
}

/// VACASK's module directory, beside its binary on `PATH`.
fn vacask_modules() -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    let bin = std::env::split_paths(&path).find(|d| d.join("vacask").is_file())?;
    let real = std::fs::canonicalize(bin.join("vacask")).ok()?;
    Some(real.parent()?.parent()?.join("lib/vacask/mod"))
}

/// Xyce's noise spectra, which its raw file leaves out (it holds the ac
/// node voltages): `<deck>_noise.dat`, a Tecplot table of frequency,
/// Re/Im of the output and each `*noise*` density in V^2/Hz (for a 50k
/// divider `onoise_spectrum` is 8.3e-16 = 4kTR). As one noise plot in
/// ngspice's units, V/sqrt(Hz) (the square root of each density), the
/// Re/Im columns dropped.
fn xyce_noise(path: &Path) -> Option<RawData> {
    let text = std::fs::read_to_string(path).ok()?;
    let (head, body) = text.split_once("ZONE")?;
    let names: Vec<String> = head
        .split('"')
        .skip(3)
        .step_by(2)
        .map(|n| n.trim().to_string())
        .collect();
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); names.len()];
    for l in body.lines().skip(1) {
        let row: Vec<f64> = l
            .split_whitespace()
            .filter_map(|w| w.parse().ok())
            .collect();
        if row.len() == names.len() {
            row.iter().zip(&mut cols).for_each(|(&v, c)| c.push(v));
        }
    }
    let keep: Vec<usize> = (0..names.len())
        .filter(|&i| i == 0 || names[i].to_ascii_lowercase().contains("noise"))
        .collect();
    let variables = keep
        .iter()
        .enumerate()
        .map(|(k, &i)| VarInfo {
            index: k,
            name: names[i].clone(),
            var_type: if i == 0 {
                VarType::Frequency
            } else {
                VarType::Voltage
            },
        })
        .collect();
    let real_data = keep
        .iter()
        .map(|&i| {
            if i == 0 {
                cols[0].clone()
            } else {
                cols[i].iter().map(|v| v.sqrt()).collect()
            }
        })
        .collect();
    Some(RawData {
        plot_name: "Noise Spectral Density Curves".into(),
        variables,
        real_data,
        ..Default::default()
    })
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
fn ltspice_said(log: &Path) -> String {
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

/// An LTspice `.tran <Tstop> [modifiers]` line as `.tran 0 <Tstop>
/// [modifiers]`, the order `tmax` reads (LTspice 26.1.1 help, `.TRAN`:
/// one number is the stop time; the other form is `.TRAN <Tstep> <Tstop>
/// [Tstart [dTmax]] [modifiers]`); any other line as it is.
fn ltspice_tran(line: &str) -> String {
    let w: Vec<&str> = line.split_whitespace().collect();
    let tran = w.first().is_some_and(|c| c.eq_ignore_ascii_case(".tran"));
    let numbers = w.iter().skip(1).map_while(|x| number(x)).count();
    if !tran || numbers != 1 {
        return line.to_string();
    }
    format!("{} 0 {}", w[0], w[1..].join(" "))
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
/// every plot but `const` at the end of the last block, appended to the
/// one raw file in run order (`$plots` lists the plots oldest first: run
/// on ngspice-45). A script that runs an analysis once a pass of a loop,
/// or once a circuit state, gives a plot each, as a `.step` does on Xyce;
/// a bare `write` kept the last one (ngspice manual §13.5.107 `write`,
/// `appendwrite`; §13.5.92 `strcmp`).
fn ngspice_text(text: &str, raw: &Path) -> String {
    let (title, body) = text.split_once('\n').unwrap_or((text, ""));
    let body = match std::env::var("CROSS_SIM_TMAX")
        .ok()
        .and_then(|n| n.parse::<f64>().ok())
    {
        Some(n) => body.lines().map(|l| tmax(l, n) + "\n").collect(),
        None => body.to_string(),
    };
    let mut out = format!("{title}\n.option seed=1\n{body}");
    if has_control(text)
        && let Some(at) = out.to_ascii_lowercase().rfind("\n.endc")
    {
        let write = format!(
            "\nset appendwrite\nforeach p $plots\nstrcmp _sr_const $p const\n\
             if $_sr_const ne 0\nsetplot $p\nwrite {}\nend\nend",
            raw.display()
        );
        out.insert_str(at, &write);
    }
    out
}

/// A `.tran TSTEP TSTOP [TSTART [TMAX]]` line (or a `.control` `tran`)
/// with TMAX at most (TSTOP - TSTART)/n and TSTEP, the words after the numbers
/// (`uic`) kept; any other line as it is. ngspice, Xyce and ESPice share
/// the order (ngspice manual `.tran`, Xyce RG `.TRAN`).
fn tmax(line: &str, n: f64) -> String {
    let w: Vec<&str> = line.split_whitespace().collect();
    let tran = w
        .first()
        .is_some_and(|c| c.eq_ignore_ascii_case(".tran") || c.eq_ignore_ascii_case("tran"));
    let nums: Vec<f64> = w.iter().skip(1).map_while(|x| number(x)).take(4).collect();
    if !tran || nums.len() < 2 {
        return line.to_string();
    }
    let start = nums.get(2).copied().unwrap_or(0.0);
    // Never coarser than the run was: a TMAX given replaces ngspice's
    // default min(TSTEP, span/50), so a TSTEP finer than span/n stays.
    let cap = match nums[0] {
        step if step > 0.0 => ((nums[1] - start) / n).min(step),
        _ => (nums[1] - start) / n,
    };
    let max = nums.get(3).map_or(cap, |&m| m.min(cap));
    let rest = w[1 + nums.len()..].join(" ");
    format!("{} {} {} {start:e} {max:e} {rest}", w[0], w[1], w[2])
        .trim_end()
        .to_string()
}

/// A SPICE number with its scale suffix (`2m`, `100n`, `1meg`, `5ms`): the
/// longest prefix that reads as a float, then the suffix.
fn number(word: &str) -> Option<f64> {
    let low = word.to_ascii_lowercase();
    let (v, tail) = (1..=low.len())
        .rev()
        .filter(|&i| low.is_char_boundary(i))
        .find_map(|i| low[..i].parse::<f64>().ok().map(|v| (v, &low[i..])))?;
    let scale = [
        ("meg", 1e6),
        ("t", 1e12),
        ("g", 1e9),
        ("k", 1e3),
        ("m", 1e-3),
        ("u", 1e-6),
        ("µ", 1e-6),
        ("n", 1e-9),
        ("p", 1e-12),
        ("f", 1e-15),
    ]
    .iter()
    .find(|(s, _)| tail.starts_with(s))
    .map_or(1.0, |&(_, k)| k);
    Some(v * scale)
}

/// The raw files a VACASK deck writes, one per `analysis <name>` in deck
/// order (`<name>.raw`), then those of the files written beside it (an
/// included file may hold the analysis: `adder_mos.sp`).
fn vacask_raws(dir: &Path, text: &str, files: &[(String, String)]) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let texts = std::iter::once(text).chain(files.iter().map(|f| f.1.as_str()));
    for l in texts.flat_map(str::lines) {
        let mut w = l.split_whitespace();
        if let (Some("analysis"), Some(name)) = (w.next(), w.next()) {
            out.push(dir.join(format!("{name}.raw")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p1_066_an_included_files_analysis_names_a_raw_file() {
        // VACASK names a raw file after each analysis, wherever its line
        // is read from. Oracle: VACASK on a deck including the file that
        // holds `analysis tran1 tran` wrote `tran1.raw`
        // (research/impl/evidence/P1-066-vacask-include-title/vacask-runs.txt),
        // and the names below written out by hand. Checks: 2.
        let d = Path::new("/d");
        let inc = "control\n  analysis tran1 tran stop=1\nendc\n".to_string();
        let files = [("circ.inc".to_string(), inc)];
        let raws = vacask_raws(d, "ground 0\ninclude \"circ.inc\"\n", &files);
        assert_eq!(raws, [d.join("tran1.raw")]);
        let own = vacask_raws(d, "t\ncontrol\n  analysis op1 op\nendc\n", &[]);
        assert_eq!(own, [d.join("op1.raw")]);
    }
}
