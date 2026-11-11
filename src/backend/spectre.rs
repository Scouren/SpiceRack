//! Cadence Spectre subprocess backend.
//!
//! Spectre is licence-gated and not installed here. Flags come from the
//! *Spectre Circuit Simulator Reference*, Product Version 19.1 (`[REF]`):
//! p.26 `-raw` and `-format`; p.494 a `.scs` file is read in Spectre input
//! mode without a `simulator lang=spectre` line. The runs settle-remote
//! recorded on Spectre 25.1 (2026-11-07, P1-040) fixed the rest: `-raw`
//! takes a file (a directory fails), and the plots are nutmeg. `hb` has no
//! nutmeg output (Spectre stops on FATAL SPECTRE-18) and no PSF file of an
//! hb run was kept to read against (ZERO-LEDGER P1-049, D43), so a deck
//! with an `hb` analysis is refused before Spectre starts.
//!
//! Contents:
//! - `pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError>`

use std::path::Path;
use std::process::Command;

use super::{BackendError, exec, read_results, scratch};
use crate::result::RawData;

/// Runs `netlist` on Spectre and reads every plot it wrote.
///
/// # Errors
/// `SimulationError` for a deck with an `hb` analysis, before Spectre
/// starts: Spectre writes no nutmeg output for it (FATAL SPECTRE-18 under
/// `-format nutascii`) and its PSF output is not read. Otherwise whatever
/// the run and the raw-file read return.
pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError> {
    if let Some(name) = hb(netlist) {
        return Err(BackendError::SimulationError(format!(
            "Spectre writes no nutmeg output for the hb analysis `{name}` (FATAL SPECTRE-18 \
             under -format nutascii) and its PSF output is not read"
        )));
    }
    let (dir, scs) = scratch(netlist, "circuit.scs")?;
    let raw = dir.path().join("out.raw");
    let stdout = exec(&mut command(&scs, &raw), "spectre", None)?;
    read_results(&raw, stdout)
}

// ---- private ----

/// The name of `netlist`'s first `hb` analysis (`<name> hb …`), if it holds
/// one.
// ponytail: a line scan of the form the writer lays an analysis out in (one
// line, the name first). A continued statement or a `lang=spice` `.hb`
// reaches Spectre, whose own FATAL then comes back through `exec`; the
// parser, when a deck written elsewhere needs the same answer.
fn hb(netlist: &str) -> Option<&str> {
    netlist.lines().find_map(|l| {
        let mut words = l.split_whitespace();
        let name = words.next()?;
        let comment = name.starts_with("//") || name.starts_with('*');
        (!comment && words.next() == Some("hb")).then_some(name)
    })
}

/// `spectre -format nutascii -raw <raw> <scs>`: every plot into the one
/// file `raw`. A directory fails ("CMI-2011: Unable to open the raw file",
/// settle-remote P1-040, Spectre 25.1; `[REF]` p.26, `-raw` names the
/// results); nutascii is the format `rawfile` reads without a byte order.
fn command(scs: &Path, raw: &Path) -> Command {
    let mut c = Command::new("spectre");
    c.args(["-format", "nutascii", "-raw"]).arg(raw).arg(scs);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p1_048_the_raw_argument_is_a_file() {
        let c = command(Path::new("/d/circuit.scs"), Path::new("/d/out.raw"));
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-format",
                "nutascii",
                "-raw",
                "/d/out.raw",
                "/d/circuit.scs"
            ]
        );
    }
}
