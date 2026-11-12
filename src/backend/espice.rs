//! ESPice: `espice deck.sp --rawfile out.raw --format=binary
//! --tokenizer=ngspice`. A deck written for ESPice is ngspice text, and
//! ESPice 0.3.0 picks its reader from the deck when it is not told: a card
//! ngspice lacks (`.fft`, `.data`, `.alter`, `.lin`, …) makes it read the
//! whole deck as HSPICE text, where a brace is no expression and the
//! temperature defaults to 25 °C, not 27.
//!
//! ESPice compiles Verilog-A through VerA at run time (`.hdl`), so a deck with
//! `veriloga()` models needs no OpenVAF. Its first load of a model builds a
//! shared library, cached under `$ESPICE_CACHE` (else `~/.cache/espice`).
//!
//! Cites: ESPice `docs/using/cli.md`, `--tokenizer=FMT` ("default: detected
//! from the deck"; "TNOM 27 instead of 25"); ESPice 0.3.0 run: "reading as
//! hspice (--tokenizer to override)".
//!
//! Contents:
//! - `pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError>`

use std::process::Command;

use super::{BackendError, exec, read_results, scratch};
use crate::result::RawData;

pub(super) fn run(netlist: &str) -> Result<Vec<RawData>, BackendError> {
    let (dir, deck) = scratch(netlist, "deck.sp")?;
    let raw = dir.path().join("out.raw");
    let stdout = exec(
        Command::new("espice")
            .arg(&deck)
            .arg("--rawfile")
            .arg(&raw)
            .arg("--format=binary")
            .arg("--tokenizer=ngspice"),
        "espice",
        None,
    )?;
    read_results(&raw, stdout)
}
