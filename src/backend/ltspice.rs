//! LTspice: `LTspice -b deck.cir`, natively or under Wine.
//!
//! Run here: LTspice 26.1.1 under Wine (`nix develop .#testing-ltspice`,
//! `tests/test_backends.rs`).
//!
//! Cites: LTspice's command-line help (`-b`); the runs above.
//!
//! Contents:
//! - `pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError>`
//! - `pub fn find() -> Option<(PathBuf, bool)>`

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{BackendError, exec, is_on_path, read_results, scratch};
use crate::result::RawData;

pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError> {
    let (exe, wine) = find()
        .ok_or_else(|| BackendError::SimulationError("LTspice not found on this system".into()))?;
    // `plotwinsize=0` disables waveform compression (lossy decimation) and
    // `numdgt=15` switches the raw file to double precision. Raw-file format
    // is run mechanics, so it is set here rather than in the dialect. Both go
    // after the title: line 1 of a SPICE deck is a comment.
    let (title, body) = netlist.split_once('\n').unwrap_or((netlist, ""));
    let title = if title.is_empty() {
        "* spicerack"
    } else {
        title
    };
    let deck = format!("{title}\n.options plotwinsize=0\n.options numdgt=15\n{body}");
    let (_dir, cir) = scratch(&deck, "deck.cir")?;

    let mut cmd = command(&exe, wine, &cir);
    // LTspice reports simulation errors (and `.meas` results) in the log.
    let stdout = exec(&mut cmd, "LTspice", Some(&cir.with_extension("log")))?;
    let log = std::fs::read_to_string(cir.with_extension("log")).unwrap_or_default();
    let mut plots = read_results(&cir.with_extension("raw"), stdout)?;
    plots
        .iter_mut()
        .for_each(|p| p.log_content.clone_from(&log));
    Ok(plots)
}

/// The LTspice executable, and whether it needs Wine: `SPICERACK_LTSPICE`
/// if set, else `ltspice` on PATH, the macOS app, the Windows install, then
/// a Wine install under `WINEPREFIX` (default `$HOME/.wine`).
pub fn find() -> Option<(PathBuf, bool)> {
    if let Some(exe) = std::env::var_os("SPICERACK_LTSPICE") {
        return Some(knob(exe.into()));
    }
    if is_on_path("ltspice") {
        return Some((PathBuf::from("ltspice"), false));
    }
    let mac = PathBuf::from("/Applications/LTspice.app/Contents/MacOS/LTspice");
    if mac.exists() {
        return Some((mac, false));
    }
    #[cfg(target_os = "windows")]
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(format!("{local}/Programs/ADI/LTspice/LTspice.exe"));
        if p.exists() {
            return Some((p, false));
        }
    }
    let prefix = match std::env::var("WINEPREFIX") {
        Ok(p) => p,
        Err(_) => format!("{}/.wine", std::env::var("HOME").ok()?),
    };
    let user = std::env::var("USER").unwrap_or_default();
    wine_installs(&prefix, &user)
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .filter(|_| is_on_path("wine"))
        .map(|p| (p, true))
}

// ---- private ----

/// `exe -b cir`, under `wine` when `wine`. `-b` is the headless switch for
/// a netlist; it writes deck.raw and deck.log beside the input.
fn command(exe: &Path, wine: bool, cir: &Path) -> Command {
    if wine {
        let mut c = Command::new("wine");
        // `-wine`: the LTspice XVII switch for the Windows build under Wine
        // (26.1.1 accepts it).
        // Wine's LTspice reads a Windows path: `/` is its `Z:` drive.
        c.arg(exe)
            .arg("-wine")
            .arg("-b")
            .arg(format!("Z:{}", cir.display()));
        c
    } else {
        let mut c = Command::new(exe);
        c.arg("-b").arg(cir);
        c
    }
}

/// `SPICERACK_LTSPICE`'s executable: a `.exe` off Windows runs under Wine.
fn knob(exe: PathBuf) -> (PathBuf, bool) {
    let wine = !cfg!(windows)
        && exe
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    (exe, wine)
}

/// Where LTspice's installers put it in the Wine prefix `prefix`.
fn wine_installs(prefix: &str, user: &str) -> [String; 3] {
    [
        format!("{prefix}/drive_c/users/{user}/AppData/Local/Programs/ADI/LTspice/LTspice.exe"),
        format!("{prefix}/drive_c/Program Files/ADI/LTspice/LTspice.exe"),
        format!("{prefix}/drive_c/Program Files/LTC/LTspiceXVII/XVIIx64.exe"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p1_005_a_wine_run_names_the_deck_by_its_z_drive_path() {
        // LTspice 26.1.1 under Wine given `-b /tmp/x/deck.cir` finds no deck
        // ("No schematic nor netlist specified", a dialog: headless, the run
        // never returns); given `Z:/tmp/x/deck.cir` it runs it.
        let cir = Path::new("/tmp/x/deck.cir");
        let args = |c: Command| -> Vec<String> {
            c.get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            args(command(Path::new("LTspice.exe"), true, cir)),
            ["LTspice.exe", "-wine", "-b", "Z:/tmp/x/deck.cir"]
        );
        assert_eq!(
            args(command(Path::new("ltspice"), false, cir)),
            ["-b", "/tmp/x/deck.cir"]
        );
    }

    #[test]
    fn p1_009_ltspice_find_honours_the_knob_and_wineprefix() {
        // `SPICERACK_LTSPICE` names the executable, a `.exe` run under Wine
        // off Windows; `WINEPREFIX` replaces `$HOME/.wine` (Wine's own
        // variable for the prefix).
        assert_eq!(
            knob(PathBuf::from("/p/LTspice.exe")),
            (PathBuf::from("/p/LTspice.exe"), !cfg!(windows))
        );
        assert_eq!(
            knob(PathBuf::from("/usr/bin/ltspice")),
            (PathBuf::from("/usr/bin/ltspice"), false)
        );
        assert_eq!(
            wine_installs("/w", "u")[1],
            "/w/drive_c/Program Files/ADI/LTspice/LTspice.exe"
        );
    }
}
