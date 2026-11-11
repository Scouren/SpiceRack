//! Cadence Spectre subprocess backend.
//!
//! Spectre is licence-gated and not installed here. Flags come from the
//! *Spectre Circuit Simulator Reference*, Product Version 19.1 (`[REF]`):
//! p.26 `-raw` and `-format`; p.494 a `.scs` file is read in Spectre input
//! mode without a `simulator lang=spectre` line. The runs settle-remote
//! recorded on Spectre 25.1 (2026-11-07, P1-040) fixed the rest: `-raw`
//! takes a file (a directory fails), and the plots are nutmeg.
//!
//! Contents:
//! - `pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError>`

use std::path::Path;
use std::process::Command;

use super::{BackendError, exec, read_results, scratch};
use crate::result::RawData;

pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError> {
    let (dir, scs) = scratch(netlist, "circuit.scs")?;
    let raw = dir.path().join("out.raw");
    let stdout = exec(&mut command(&scs, &raw), "spectre", None)?;
    read_results(&raw, stdout)
}

// ---- private ----

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
