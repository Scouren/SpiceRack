//! A simulator run as a command: an argv template (one [`Arg`] per word,
//! with the `{deck}`, `{raw}` and `{dir}` placeholders) → the process, run
//! on the deck in a scratch directory → its result files, read as an
//! [`Output`] says or by the caller's [`ReadHook`]. No shell: a word is
//! one argv entry whatever its text holds, and every placeholder becomes
//! an absolute path.
//!
//! Cites: none (the readers are `rawfile`'s; the run mechanics are the
//! built-ins' own: `ngspice.rs`, `vacask.rs`, `ltspice.rs`).
//!
//! Contents:
//! - `pub enum Piece`, `pub type Arg`, `pub enum Output`, `Output::NAMES: [(&str, Output); 5]`,
//!   `pub type ReadHook`
//! - `pub fn parse_arg(template: &str) -> Result<Arg, SimRegisterError>`
//! - `pub(super) fn run(argv: &[Arg], output: Output, family: Dialect, netlist: &str) -> Result<Vec<RawData>, BackendError>`
//! - `pub(super) fn run_hook(argv: &[Arg], read: ReadHook, id: u32, family: Dialect, netlist: &str) -> Result<Vec<RawData>, BackendError>`

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{BackendError, SimRegisterError, exec, ltspice, read_results, scratch, vacask};
use crate::result::RawData;
use crate::syntax::Dialect;

/// One part of an argv word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    /// Text, passed as written.
    Lit(String),
    /// `{deck}`: the deck file's absolute path, inside `{dir}`.
    Deck,
    /// `{raw}`: the absolute path the result file is read from: the deck's
    /// path with the extension `raw`.
    Raw,
    /// `{dir}`: the absolute path of the run's scratch directory, deleted
    /// after the results are read.
    Dir,
}

/// One argv word: its pieces, joined with nothing between them
/// (`{dir}/x.log` is `[Dir, Lit("/x.log")]`).
pub type Arg = Vec<Piece>;

/// How a command's results are read back. `NgspiceRaw`, `NgspiceAscii`
/// and `Nutascii` are read by one reader, which tells binary from ASCII
/// (`rawfile::parse_raw_all`): the name says what the simulator writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// The nutmeg raw file at `{raw}`, binary (ngspice `-r`).
    NgspiceRaw,
    /// The nutmeg raw file at `{raw}`, ASCII.
    NgspiceAscii,
    /// VACASK's result files: the command runs with `{dir}` as its working
    /// directory and leaves one `*.raw` file per analysis there. They are
    /// read as the built-in VACASK reads its own (`vacask::read`). For a
    /// simulator whose language is VACASK's: the files the deck's
    /// `analysis` lines name, in the deck's order, each plot's sweep
    /// columns after its own axis. For another language: every `*.raw`,
    /// in file-name order. Either way a plot that holds `onoise` and
    /// `gain` (VACASK's noise plot) gains ngspice's two noise spectra.
    RawDir,
    /// The raw file at `{raw}` and the `.log` beside it (LTspice writes
    /// both next to the deck): the log's `.meas` and `.step` lines are read.
    LtspiceRaw,
    /// The nutmeg ASCII file at `{raw}` (Spectre `-format nutascii`).
    Nutascii,
}

/// Reads a finished command's results the calling crate's own way
/// (`Runner::Hook`; provisional, outside the 1.0 freeze): the caller's
/// hook `id` and the run's scratch directory (absolute; deleted after the
/// call) in, every plot of the run out, each carrying its measures.
pub type ReadHook = fn(id: u32, dir: &Path) -> Result<Vec<RawData>, BackendError>;

/// A finished command: its scratch directory (deleted on drop), the
/// result path, the captured stdout and the deck's text when its language
/// is VACASK's, empty for another: a VACASK deck names its result files
/// (`Output::RawDir`).
struct Ran {
    dir: tempfile::TempDir,
    raw: PathBuf,
    stdout: String,
    vacask_deck: String,
}

impl Output {
    /// Every output by the name a simulator descriptor gives it.
    pub const NAMES: [(&'static str, Output); 5] = [
        ("ngspice-raw", Output::NgspiceRaw),
        ("ngspice-ascii", Output::NgspiceAscii),
        ("raw-dir", Output::RawDir),
        ("ltspice-raw", Output::LtspiceRaw),
        ("nutascii", Output::Nutascii),
    ];
}

/// One argv word's template as its pieces: `{deck}`, `{raw}` and `{dir}`
/// are the placeholders, everything else is literal text.
///
/// # Errors
/// `SimRegisterError::Command`: a `{` that opens none of the three
/// placeholders (there is no escape for a literal brace).
pub fn parse_arg(template: &str) -> Result<Arg, SimRegisterError> {
    let bad = || {
        SimRegisterError::Command(format!(
            "'{template}' holds a placeholder that is not {{deck}}, {{raw}} or {{dir}}"
        ))
    };
    let mut out = Vec::new();
    let mut rest = template;
    // Each pass consumes at least `{x}` of `rest`: at most len/2 passes.
    while let Some(open) = rest.find('{') {
        let close = open + rest[open..].find('}').ok_or_else(bad)?;
        let piece = match &rest[open + 1..close] {
            "deck" => Piece::Deck,
            "raw" => Piece::Raw,
            "dir" => Piece::Dir,
            _ => return Err(bad()),
        };
        if open > 0 {
            out.push(Piece::Lit(rest[..open].to_string()));
        }
        out.push(piece);
        rest = &rest[close + 1..];
    }
    if !rest.is_empty() || out.is_empty() {
        out.push(Piece::Lit(rest.to_string()));
    }
    assert!(!out.is_empty(), "a word has a piece");
    Ok(out)
}

/// Runs `argv` on `netlist` (text in the simulator's dialect, of built-in
/// family `family`) and reads the results `output` names.
///
/// # Errors
/// `SimulationError`: the program is not found, exits nonzero, or wrote no
/// result file; `RawParseError`: a result file does not read.
///
/// # Panics
/// When `argv` is empty (registration refuses it).
pub(super) fn run(
    argv: &[Arg],
    output: Output,
    family: Dialect,
    netlist: &str,
) -> Result<Vec<RawData>, BackendError> {
    let ran = start(argv, output == Output::RawDir, family, netlist)?;
    let plots = read(&ran, output)?;
    assert!(!plots.is_empty(), "a raw file read holds at least one plot");
    Ok(plots)
}

/// [`run`] for a simulator whose results the caller reads
/// (`Runner::Hook`): the command runs the same way, in the caller's
/// working directory; then `read(id, dir)` answers the plots from the
/// scratch directory `dir`, which is deleted when this returns. Each plot
/// then carries the command's stdout.
///
/// # Errors
/// As [`run`] for the command; whatever `read` returns; `SimulationError`
/// when it answers no plot.
///
/// # Panics
/// When `argv` is empty (registration refuses it).
pub(super) fn run_hook(
    argv: &[Arg],
    read: ReadHook,
    id: u32,
    family: Dialect,
    netlist: &str,
) -> Result<Vec<RawData>, BackendError> {
    let ran = start(argv, false, family, netlist)?;
    let dir = std::path::absolute(ran.dir.path())?;
    assert!(ran.raw.starts_with(&dir), "the deck is in `dir`");
    let mut plots = read(id, &dir)?;
    if plots.is_empty() {
        let none = "read hook: it returned no plot";
        return Err(BackendError::SimulationError(none.into()));
    }
    for p in &mut plots {
        p.stdout.clone_from(&ran.stdout);
    }
    Ok(plots)
}

// ---- private ----

/// Writes the deck into a fresh scratch directory and runs the command
/// to completion, with that directory as its working directory when
/// `in_dir`.
fn start(argv: &[Arg], in_dir: bool, family: Dialect, netlist: &str) -> Result<Ran, BackendError> {
    assert!(!argv.is_empty(), "a registered command has a program");
    // Spectre reads a file as its own language by the `.scs` extension.
    let file = match family {
        Dialect::Spectre => "deck.scs",
        _ => "deck.cir",
    };
    let (dir, _) = scratch(netlist, file)?;
    let here = std::path::absolute(dir.path())?;
    let deck = here.join(file);
    let raw = deck.with_extension("raw");
    let words = words(argv, &here, &deck, &raw);
    let mut cmd = Command::new(&words[0]);
    cmd.args(&words[1..]);
    if in_dir {
        cmd.current_dir(&here);
    }
    let program = words[0].to_string_lossy();
    let stdout = exec(&mut cmd, &program, Some(&deck.with_extension("log")))?;
    // Another language's line may open with the word `analysis` (a title),
    // and is not one: only a VACASK deck is read for its result files.
    let vacask_deck = match family {
        Dialect::Vacask => netlist.to_string(),
        _ => String::new(),
    };
    Ok(Ran {
        dir,
        raw,
        stdout,
        vacask_deck,
    })
}

/// The argv of a run: each word's pieces joined, the placeholders as the
/// three absolute paths.
fn words(argv: &[Arg], dir: &Path, deck: &Path, raw: &Path) -> Vec<OsString> {
    assert!(dir.is_absolute() && deck.starts_with(dir) && raw.starts_with(dir));
    let word = |arg: &Arg| {
        let mut w = OsString::new();
        for piece in arg {
            match piece {
                Piece::Lit(text) => w.push(text),
                Piece::Deck => w.push(deck),
                Piece::Raw => w.push(raw),
                Piece::Dir => w.push(dir),
            }
        }
        w
    };
    let out: Vec<OsString> = argv.iter().map(word).collect();
    assert!(out.len() == argv.len(), "one argv entry per word");
    out
}

/// The plots of a finished command, as `output` says to read them.
fn read(ran: &Ran, output: Output) -> Result<Vec<RawData>, BackendError> {
    match output {
        Output::NgspiceRaw | Output::NgspiceAscii | Output::Nutascii => {
            read_results(&ran.raw, ran.stdout.clone())
        }
        Output::LtspiceRaw => ltspice::read(&ran.raw, ran.stdout.clone()),
        Output::RawDir => vacask::read(ran.dir.path(), &ran.vacask_deck, &ran.stdout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: the pieces written out by hand for each template.
    #[test]
    fn p8_203_a_word_template_is_its_pieces() {
        let lit = |s: &str| Piece::Lit(s.to_string());
        assert_eq!(parse_arg("-b").unwrap(), [lit("-b")]);
        assert_eq!(parse_arg("").unwrap(), [lit("")], "an empty word stays one");
        assert_eq!(parse_arg("{deck}").unwrap(), [Piece::Deck]);
        assert_eq!(
            parse_arg("{dir}/x.log").unwrap(),
            [Piece::Dir, lit("/x.log")]
        );
        assert_eq!(
            parse_arg("a{raw}b{deck}}").unwrap(),
            [lit("a"), Piece::Raw, lit("b"), Piece::Deck, lit("}")]
        );
        for bad in ["{nope}", "{deck", "{}", "{ deck }", "x{DIR}"] {
            let e = parse_arg(bad).expect_err(bad).to_string();
            assert!(e.starts_with("command: "), "{e}");
            assert!(e.contains(bad), "{e}");
        }
    }

    /// Oracle: the argv written out by hand: absolute paths, a directory
    /// with a space kept in one entry (argv, no shell), `{dir}` and `{raw}`
    /// inside a longer word.
    #[test]
    fn p8_203_placeholders_become_absolute_paths_in_one_argv_entry_each() {
        let argv: Vec<Arg> = ["sim", "-r", "{raw}", "{deck}", "{dir}/x.log", "--o={raw}"]
            .map(|w| parse_arg(w).expect("a template"))
            .into();
        let dir = Path::new("/t/a b");
        let got = words(&argv, dir, &dir.join("deck.cir"), &dir.join("deck.raw"));
        let want = [
            "sim",
            "-r",
            "/t/a b/deck.raw",
            "/t/a b/deck.cir",
            "/t/a b/x.log",
            "--o=/t/a b/deck.raw",
        ];
        assert_eq!(got, want.map(OsString::from));
    }
}
