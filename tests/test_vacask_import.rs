//! VACASK decks read and written for another simulator: the oracle is real
//! VACASK on the original deck against real ngspice on the deck written
//! from it, every compared vector equal to the tolerance both meet on the
//! closed form; a construct the target cannot run is refused by its code,
//! which the VACASK docs clause in the test names.
//!
//! Unsupported fails, never skips: without `vacask` and `ngspice` on PATH
//! every run fails; run the tests in `nix develop .#testing`.
//!
//! Cites: VACASK 36be560 `docs/cmd-overview.md` (the control commands),
//! `cmd-errorhandling.md` (`abort`), `cmd-print.md`, `cmd-postprocess.md`,
//! `input-embed.md`, `cmd-elaboration.md`; ngspice manual.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

use spicerack::backend::Dialect;
use spicerack::result::RawData;
use spicerack::syntax::diag::{Code, Kind, info};
use spicerack::syntax::render::{Written, render_text};

/// A divider (10 V over 1k and 9k: v(2) = 9 V) with the control commands
/// and an `embed` that only steer a VACASK run and its scripts.
const SCRIPTED: &str = "Scripted divider

ground 0

load \"resistor.osdi\"

model resistor resistor
model vsource vsource

v1 (1 0) vsource dc=10
r1 (1 2) resistor r=1k
r2 (2 0) resistor r=9k

control
  abort always
  print device(\"resistor\")
  analysis op1 op
  postprocess(PYTHON, \"runme.py\")
endc

embed \"runme.py\" <<<FILE
print(\"done\")
>>>FILE
";

// ---- private ----

/// `deck` (VACASK text) written for `target`, or its refusal's codes.
fn written(deck: &str, target: Dialect) -> Result<Written, Vec<Code>> {
    render_text(deck, Dialect::Vacask, "deck", target)
        .map_err(|d| d.diags.iter().map(|d| d.code).collect())
}

/// The refusal codes of `w` (none when it was written).
fn refusals(w: Result<Written, Vec<Code>>) -> Vec<Code> {
    let all = w.err().unwrap_or_default();
    all.into_iter()
        .filter(|&c| info(c).kind == Kind::Refusal)
        .collect()
}

/// The note and refusal codes of `w`.
fn codes(w: &Written) -> Vec<Code> {
    w.notes.diags.iter().map(|d| d.code).collect()
}

/// The first plot `d` writes for `deck`.
fn run(d: Dialect, deck: &str) -> RawData {
    d.run(deck)
        .unwrap_or_else(|e| panic!("{d:?} failed: {e}\n{deck}"))
}

/// Node `name`'s first value in `raw` (VACASK names a node bare, ngspice
/// `v(name)`).
fn node(raw: &RawData, name: &str) -> f64 {
    let wrapped = format!("v({name})");
    let at = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case(name) || v.name.eq_ignore_ascii_case(&wrapped));
    let at = at.unwrap_or_else(|| panic!("no node {name} in {:?}", raw.variables));
    raw.real_data[at][0]
}

#[test]
fn a_vacask_script_line_is_left_out_with_a_note_and_the_numbers_stay() {
    // `abort` sets the error policy (cmd-errorhandling.md), `print`
    // writes to the console (cmd-print.md), `postprocess` runs a program
    // after the run (cmd-postprocess.md), `embed` writes a file nothing
    // in the deck loads (input-embed.md): none changes a number.
    for target in [Dialect::Ngspice, Dialect::Spectre] {
        let w = written(SCRIPTED, target).unwrap_or_else(|c| panic!("{target:?}: {c:?}"));
        assert!(
            codes(&w).contains(&Code::W0120),
            "{target:?}: {:?}",
            codes(&w)
        );
        for word in ["abort", "print", "postprocess", "embed", "done"] {
            assert!(
                !w.text.contains(word),
                "{target:?} wrote {word}:\n{}",
                w.text
            );
        }
    }
    let ng = written(SCRIPTED, Dialect::Ngspice).expect("written").text;
    let (v, n) = (run(Dialect::Vacask, SCRIPTED), run(Dialect::Ngspice, &ng));
    assert!((node(&v, "2") - 9.0).abs() < 1e-9, "VACASK v(2)");
    assert!(
        (node(&n, "2") - node(&v, "2")).abs() < 1e-9,
        "ngspice v(2)\n{ng}"
    );
    // VACASK keeps its own script.
    let own = written(SCRIPTED, Dialect::Vacask).expect("written").text;
    for word in ["abort always", "postprocess", "embed \"runme.py\""] {
        assert!(own.contains(word), "VACASK lost {word}:\n{own}");
    }
}

#[test]
fn a_vacask_control_command_that_changes_the_circuit_is_refused() {
    // `elaborate circuit("sub")` makes `sub` the top (cmd-elaboration.md);
    // `alter` changes a parameter between analyses (cmd-alter.md). No
    // SPICE or Spectre deck line says either: refused, not copied.
    for cmd in ["elaborate circuit(\"sub\")", "alter instance(\"r2\") r=1k"] {
        // The deck without its `embed`, so the command is all it refuses.
        let bare = SCRIPTED.split("embed").next().expect("a deck");
        let deck = bare.replace("  analysis op1 op", &format!("  {cmd}\n  analysis op1 op"));
        for target in [Dialect::Ngspice, Dialect::Spectre] {
            assert_eq!(
                refusals(written(&deck, target)),
                vec![Code::W0001],
                "{cmd} on {target:?}"
            );
        }
    }
}

#[test]
fn an_embedded_file_the_deck_loads_is_kept_and_refused_off_vacask() {
    // input-embed.md: the file is written before the circuit is read, so
    // a `load` or `include` of it is part of the circuit.
    let deck = SCRIPTED
        .replace("load \"resistor.osdi\"", "include \"part.inc\"")
        .replace("embed \"runme.py\"", "embed \"part.inc\"");
    assert_eq!(
        refusals(written(&deck, Dialect::Ngspice)),
        vec![Code::W0001]
    );
}

#[test]
fn a_load_of_vacasks_own_linear_device_is_no_load_off_vacask() {
    // dev-vacask.md: `resistor.osdi`, `capacitor.osdi` and `inductor.osdi`
    // are VACASK's own linear devices, found on its own module path; their
    // instances are written as the target's R, C and L, which need no load
    // (ngspice: "Error opening osdi lib", Spectre: an `ahdl_include` of a
    // binary it cannot read).
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    for target in [Dialect::Ngspice, Dialect::Spectre] {
        let w = written(bare, target).unwrap_or_else(|c| panic!("{target:?}: {c:?}"));
        assert!(!w.text.contains("resistor.osdi"), "{target:?}:\n{}", w.text);
    }
}

#[test]
fn a_vacask_mfactor_is_the_targets_multiplier() {
    // cir-mfactor.md: `$mfactor=N` is N devices in parallel; r1 = 1k/2
    // gives v(2) = 10 * 9k / 9.5k. ngspice's R, D, M take `m=` (manual
    // §3.3.1, §7.1, §11.1).
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    let deck = bare.replace("r=1k", "r=1k $mfactor=2");
    let ng = written(&deck, Dialect::Ngspice).expect("written").text;
    let (v, n) = (run(Dialect::Vacask, &deck), run(Dialect::Ngspice, &ng));
    let exact = 10.0 * 9.0 / 9.5;
    assert!((node(&v, "2") - exact).abs() < 1e-9, "VACASK v(2)");
    assert!((node(&n, "2") - exact).abs() < 1e-9, "ngspice v(2)\n{ng}");
}

#[test]
fn a_controlled_source_parameter_the_reader_has_no_field_for_is_kept() {
    // The builtin `vccs` takes `gain` and `$mfactor` (dev-builtin.md); a
    // card holding the gain alone would drop the multiplier.
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    let deck = bare.replace(
        "model vsource vsource",
        "model vsource vsource\nmodel g vccs\ng1 (2 0 1 0) g gain=1m $mfactor=2",
    );
    assert_eq!(
        refusals(written(&deck, Dialect::Ngspice)),
        vec![Code::W0001]
    );
}
