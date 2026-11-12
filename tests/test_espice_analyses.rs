//! End-to-end coverage of the analyses the ESPice backend exposes: the
//! shared suite in `common/suite.rs`, run on `Dialect::Espice`.
//!
//! Unsupported fails, never skips: no espice on PATH fails every test. Not
//! covered: XSPICE and `.control` (not ESPice features; Laplace is in
//! `test_laplace_ngspice.rs`), `.disto`/`.pz`/`.noise` (their result layouts
//! are not mapped by this backend yet), `.four` (ESPice writes the table as a
//! raw plot, not to stdout).
//!
//! Cites: the closed forms in `common/suite.rs`; real ESPice.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[macro_use]
mod common;

use spicerack::backend::Dialect;

// ESPice names the `.tf` impedance columns input_resistance and
// output_resistance where ngspice writes v1#input_impedance and
// output_impedance_at_v(out).
analysis_suite!(Dialect::Espice, tf_columns: ("output_resistance", "input_resistance"));

// ---- private ----

/// A deck written for ESPice is ngspice text whatever cards it holds
/// (S4-rf-espice-tokenizer). ESPice 0.3.0 picks its reader from the deck
/// when it is not told (`docs/using/cli.md`, `--tokenizer`: "HSPICE for a
/// card or keyword ngspice lacks", `.fft` among them), and read as HSPICE
/// text the brace below is no expression ("reading as hspice (--tokenizer
/// to override)", ParseError). Oracle: the divider's closed form, 2 V
/// times 1k over 1k + (1+1)·500, a linear solve (1e-9).
#[test]
fn a_deck_holding_an_fft_card_is_read_as_ngspice_text() {
    let deck = "* t\n.param x={1+1}\nV1 1 0 DC 2\nR1 1 2 {x*500}\nR2 2 0 1k\n.tran 10u 1m\n.fft v(2) np=64\n.end\n";
    let raw = run(deck);
    assert_eq!(raw.plot_name, "Transient Analysis");
    let v = column(&raw, "2");
    assert!(close(v[v.len() - 1], 1.0, 1e-9), "v(2) = {:?}", v.last());
}
