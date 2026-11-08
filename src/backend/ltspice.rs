//! LTspice: `LTspice -b deck.cir`, natively or under Wine.
//!
//! UNVERIFIED: no LTspice binary is installed here, so every claim in this
//! file comes from LTspice's documented syntax, not from a run.
//!
//! Cites: none (LTspice's command-line help, unverified).
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

/// The LTspice executable, and whether it needs Wine.
pub fn find() -> Option<(PathBuf, bool)> {
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
    let home = std::env::var("HOME").ok()?;
    let user = std::env::var("USER").unwrap_or_default();
    [
        format!("{home}/.wine/drive_c/users/{user}/AppData/Local/Programs/ADI/LTspice/LTspice.exe"),
        format!("{home}/.wine/drive_c/Program Files/ADI/LTspice/LTspice.exe"),
        format!("{home}/.wine/drive_c/Program Files/LTC/LTspiceXVII/XVIIx64.exe"),
    ]
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
}
