//! The LTspice cell: spice-datasets' LTspice decks that take no model from
//! outside the deck, each on LTspice 26.1.1 against the ngspice deck written
//! from it on ngspice (`main.rs`'s `sweep`, the cell `ltspice-ngspice`), and
//! one inline pair for the merge tier. LTspice runs in `nix develop
//! .#testing-ltspice` only (ZERO D40); CI carries the marker
//! `LTSPICE_CI_PENDING_MIRROR`, under which LTspice must be absent.
//!
//! The comparer's tolerances hold unchanged. LTspice 26.1.1's defaults are
//! RELTOL 1e-3, ABSTOL 1 pA, VNTOL 1 uV and GMIN 1e-12, the comparer's own,
//! and TRTOL 2.0, inside the transient bound taken for ngspice's 7. The
//! harness asks LTspice for uncompressed double-precision vectors
//! (`sim.rs`), so no float32 term is added.
//!
//! Cites: mismatches.md §4.1 (the LTspice cell); settle-lt-hspice,
//! mismatches 151 (LTspice's tolerance defaults; TRTOL 2.0 by run);
//! LTspice 26.1.1 help, `.OPTIONS` (`plotwinsize`, `numdgt`).
//!
//! Contents: none (tests only).

use std::path::Path;

use spicerack::syntax::Dialect;
use spicerack::syntax::render::render_text;

use super::sim::{self, Ran, Sim};
#[cfg(feature = "corpus")]
use super::{Corpus, Target, sweep};
use super::{SECS, cmp};

/// spice-datasets' LTspice decks that name no model from outside the deck
/// ([`self_contained`]): an ADI part's model is encrypted and LTspice's own
/// library is not in the corpus, so LTspice alone runs the others.
#[cfg(feature = "corpus")]
const LTSPICE: Corpus = Corpus {
    name: "ltspice",
    var: "LTSPICE_CORPUS",
    ext: "net",
    keep: self_contained,
    dialect: Dialect::Ltspice,
    sim: Sim::Ltspice,
    targets: &[Target {
        cell: "ltspice-ngspice",
        dialect: Dialect::Ngspice,
        sim: Sim::Ngspice,
        control: None,
    }],
};

// ---- private ----

#[test]
fn p1_019_ltspice_cross_sim_cell() {
    // One pair through the cell's own path: an LC low-pass on LTspice 26.1.1
    // (the oracle: the source's simulator) against the ngspice deck written
    // from it on ngspice, every shared vector within the comparer's ac
    // tolerance, 2 * (RELTOL * swing + floor). LTspice gives an inductor 1
    // mOhm in series by default (LTspice help, `L`), which the written deck
    // carries as a resistor. Run by hand on both (research/impl/evidence/
    // P1-019/cell-pair.txt): 61 points each, V(out) equal to 1e-15.
    if pending_mirror() {
        return;
    }
    assert!(
        Sim::Ltspice.on_path(),
        "ltspice not on PATH: run in `nix develop .#testing-ltspice`"
    );
    let src = "* p1_019 lc low-pass\nV1 in 0 AC 1\nL1 in out 1m\nC1 out 0 1u\nR1 out 0 50\n\
               .ac dec 20 100 100k\n.end\n";
    let deck = Path::new("/nonexistent/p1_019.net");
    let written = render_text(src, Dialect::Ltspice, "p1_019", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("refused: {}", e.messages))
        .text;
    let run = |sim: Sim, text: &str| match sim::run(sim, deck, text, &[], SECS) {
        Ran::Plots(p) => p,
        Ran::Timeout => panic!("{}: past {SECS} s", sim.name()),
        Ran::Failed(why) => panic!("{}: {why}\n{text}", sim.name()),
    };
    let (a, b) = (run(Sim::Ltspice, src), run(Sim::Ngspice, &written));
    assert_eq!(a.len(), 1, "LTspice's raw file holds the ac plot alone");
    assert_eq!(a[0].re(0).len(), 61, "3 decades, 20 points each");
    if let Err(why) = cmp::compare(&a, &b, cmp::Tol::of(&[src, &written])) {
        panic!("{why}\n{written}");
    }
}

#[cfg(feature = "corpus")]
#[test]
fn ltspice_corpus() {
    if pending_mirror() {
        return;
    }
    sweep(&LTSPICE, None);
}

/// Whether CI's marker is set. CI has no LTspice MSI yet (TESTING.md, Known
/// gaps): there LTspice must be absent and its tests run nothing, and they
/// fail once it runs, until the marker goes.
fn pending_mirror() -> bool {
    let marked = std::env::var_os("LTSPICE_CI_PENDING_MIRROR").is_some();
    assert!(
        !marked || !Sim::Ltspice.on_path(),
        "LTspice runs: remove LTSPICE_CI_PENDING_MIRROR"
    );
    marked
}

/// Whether the deck at `path` takes no model from outside it: no `.lib` or
/// `.include`, and each `X` line's subcircuit (its last field before any
/// `name=value`) defined by a `.subckt` in it.
#[cfg(feature = "corpus")]
fn self_contained(path: &str) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let text = spicerack::import::decode(&bytes).to_ascii_lowercase();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let outside = |l: &&str| l.starts_with(".lib") || l.starts_with(".inc");
    if lines.iter().any(outside) {
        return false;
    }
    let defined: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix(".subckt"))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    lines.iter().filter(|l| l.starts_with('x')).all(|l| {
        let fields = l.split_whitespace().skip(1).filter(|f| !f.contains('='));
        fields.last().is_some_and(|m| defined.contains(&m))
    })
}
