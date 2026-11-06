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

/// Node `name`'s values in `raw` (VACASK names a node bare, ngspice
/// `v(name)`).
fn column<'r>(raw: &'r RawData, name: &str) -> &'r [f64] {
    let wrapped = format!("v({name})");
    let at = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case(name) || v.name.eq_ignore_ascii_case(&wrapped));
    let at = at.unwrap_or_else(|| panic!("no node {name} in {:?}", raw.variables));
    &raw.real_data[at]
}

/// Node `name`'s first value in `raw` (VACASK names a node bare, ngspice
/// `v(name)`).
fn node(raw: &RawData, name: &str) -> f64 {
    column(raw, name)[0]
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

#[test]
fn a_vacask_option_spice_names_alike_is_the_targets_option() {
    // cmd-options-tol.md: `reltol`, `abstol`, `vntol`, `chgtol`, `gmin`
    // mean SPICE's; cmd-options-tran.md: `tran_method` "trap" (and "am2")
    // is the trapezoidal rule, "gear2"/"bdf2" second-order Gear (ngspice
    // manual §11.1.2: `method=trap|gear`, MAXORD default 2).
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    for (opt, card) in [
        ("reltol=1e-6 gmin=1e-15", ".options reltol=1e-6 gmin=1e-15"),
        ("tran_method=\"gear2\"", ".options method=gear"),
        ("tran_method=\"trap\"", ".options method=trap"),
    ] {
        let deck = bare.replace(
            "  analysis op1 op",
            &format!("  options {opt}\n  analysis op1 op"),
        );
        let w = written(&deck, Dialect::Ngspice).expect("written");
        assert!(w.text.contains(card), "{opt}:\n{}", w.text);
        assert!(!codes(&w).contains(&Code::W0024), "{opt}: {:?}", codes(&w));
    }
}

#[test]
fn a_vacask_waveform_source_is_its_value_at_time_zero_in_dc() {
    // dev-builtin-src.md: `dc` is the value of `type="dc"`; a waveform
    // source's value is the waveform's in every analysis, the operating
    // point its value at t = 0 (VACASK run: `dc=5 type="pulse" val0=1`
    // gives v(1) = 1). ngspice takes a source with no DC value at its
    // transient value at t = 0 (manual §4.1).
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    let deck = bare.replace(
        "v1 (1 0) vsource dc=10",
        "v1 (1 0) vsource dc=5 type=\"pulse\" val0=10 val1=2 delay=1m rise=1u",
    );
    let ng = written(&deck, Dialect::Ngspice).expect("written").text;
    let (v, n) = (run(Dialect::Vacask, &deck), run(Dialect::Ngspice, &ng));
    assert!((node(&v, "2") - 9.0).abs() < 1e-9, "VACASK v(2)");
    assert!((node(&n, "2") - 9.0).abs() < 1e-9, "ngspice v(2)\n{ng}");
}

#[test]
fn a_vacask_save_is_the_targets_save_list() {
    // cmd-analysis-tran.md "Save directives": `v(node)`, `i(instance)`,
    // `default` (every node and branch flow); ngspice `.save` (manual
    // §15.6.1) with `all`. A `p(instance,outvar)` is the module's own
    // output: VACASK's resistor gives one parallel instance's current,
    // ngspice's `@r1[i]` all of them (1.5 mA against 0.5 mA at
    // `$mfactor=3`), so it is left out with a note.
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    let deck = bare.replace(
        "  analysis op1 op",
        "  save default v(2) i(v1) p(r1,i)\n  analysis op1 op",
    );
    let w = written(&deck, Dialect::Ngspice).expect("written");
    assert!(w.text.contains(".save all v(2) i(v1)\n"), "{}", w.text);
    assert!(codes(&w).contains(&Code::W0023), "{:?}", codes(&w));
    let n = run(Dialect::Ngspice, &w.text);
    assert!((node(&n, "2") - 9.0).abs() < 1e-9);
    // Only VACASK reads `p(...)`, `default`: its own writer keeps the line.
    let own = written(&deck, Dialect::Vacask).expect("written").text;
    let squeezed: String = own.split_whitespace().collect();
    assert!(squeezed.contains("savedefaultv(2)i(v1)p(r1,i)"), "{own}");
}

#[test]
fn a_vacask_sweep_over_an_operating_point_is_the_targets_dc_sweep() {
    // cmd-sweep.md: `step=` a uniform step, `mode="lin" points=N` N
    // intervals, `mode="dec" points=N` N per decade; ngspice's `.dc` is
    // linear only (manual §15.3.2), so a decade sweep is refused W0038.
    // The divider's v(2) is 0.9 v1.
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    let sweep = |s: &str| {
        bare.replace(
            "  analysis op1 op",
            &format!("  sweep s1 instance=\"v1\" parameter=\"dc\" {s}\n  analysis op1 op"),
        )
    };
    for range in ["from=0 to=10 step=5", "from=0 to=10 mode=\"lin\" points=2"] {
        let deck = sweep(range);
        let ng = written(&deck, Dialect::Ngspice)
            .unwrap_or_else(|c| panic!("{range}: {c:?}"))
            .text;
        let (v, n) = (run(Dialect::Vacask, &deck), run(Dialect::Ngspice, &ng));
        let (vv, nn) = (column(&v, "2"), column(&n, "2"));
        assert_eq!(vv.len(), 3, "{range}: VACASK points");
        assert_eq!(nn.len(), 3, "{range}: ngspice points\n{ng}");
        for (k, (a, b)) in vv.iter().zip(nn).enumerate() {
            assert!(
                (a - 4.5 * k as f64).abs() < 1e-9 && (b - a).abs() < 1e-9,
                "{range}: {a} {b}"
            );
        }
    }
    let dec = sweep("from=1 to=100 mode=\"dec\" points=2");
    assert_eq!(refusals(written(&dec, Dialect::Ngspice)), vec![Code::W0038]);
}

#[test]
fn a_temperature_sweep_is_the_temp_options_sweep_on_vacask() {
    // cmd-sweep.md: `option="name"` sweeps a simulator option (`temp`,
    // cmd-options-temp.md); `instance="temp"` names no instance, so the
    // run fails. ngspice `.dc temp` (manual §15.3.2) and VACASK's own
    // sweep both reach VACASK so.
    let bare = SCRIPTED.split("embed").next().expect("a deck");
    let own = bare.replace(
        "  analysis op1 op",
        "  sweep t option=\"temp\" from=0 to=100 step=50\n  analysis op1 op",
    );
    let ng = "temp sweep\nV1 1 0 10\nR1 1 2 1k\nR2 2 0 9k\n.dc temp 0 100 50\n.end\n";
    let from_ng = render_text(ng, Dialect::Ngspice, "deck", Dialect::Vacask).expect("written");
    for deck in [
        written(&own, Dialect::Vacask).expect("written").text,
        from_ng.text,
    ] {
        assert!(deck.contains("option=\"temp\""), "{deck}");
        assert_eq!(column(&run(Dialect::Vacask, &deck), "2").len(), 3, "{deck}");
    }
}

/// VACASK's module directory (`<prefix>/lib/vacask/mod`, beside the binary
/// on PATH), where its precompiled OSDI files are.
fn vacask_mod() -> std::path::PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    let bin = std::env::split_paths(&path)
        .map(|d| d.join("vacask"))
        .find(|b| b.is_file())
        .expect("vacask on PATH: run in nix develop .#testing");
    let bin = bin.canonicalize().expect("vacask's path");
    bin.parent()
        .and_then(|d| d.parent())
        .expect("a prefix")
        .join("lib/vacask/mod")
}

#[test]
fn a_vacask_model_of_a_loaded_module_is_an_ngspice_osdi_instance() {
    // cir-masters.md: a model's master is a builtin or a loaded module;
    // ngspice manual §13: an OSDI module is a `.model` type, its instance
    // an `N` card, the module loaded by `pre_osdi`. Both run the same
    // PSP 103.4 binary: the drain current is the same number.
    let osdi = vacask_mod().join("psp103v4.osdi");
    let deck = format!(
        "PSP nmos\n\nground 0\n\nload \"{}\"\n\nmodel vsource vsource\nmodel nch psp103va type=1\n\n\
         vd (d 0) vsource dc=1\nvg (g 0) vsource dc=1\nm1 (d g 0 0) nch w=1u l=1u $mfactor=2\n\n\
         control\n  analysis op1 op\nendc\n",
        osdi.display()
    );
    let ng = written(&deck, Dialect::Ngspice).expect("written").text;
    assert!(
        ng.contains(".model nch psp103va (type=1)") && ng.contains("Nm1 d g 0 0 nch"),
        "{ng}"
    );
    let current = |raw: &RawData, names: &[&str]| {
        let at = raw
            .variables
            .iter()
            .position(|v| names.iter().any(|n| v.name.eq_ignore_ascii_case(n)));
        raw.real_data[at.unwrap_or_else(|| panic!("{:?}", raw.variables))][0]
    };
    let v = current(&run(Dialect::Vacask, &deck), &["vd:flow(br)"]);
    let n = current(&run(Dialect::Ngspice, &ng), &["i(vd)"]);
    assert!(
        v < -1e-5 && ((n - v) / v).abs() < 1e-6,
        "VACASK {v}, ngspice {n}\n{ng}"
    );
    // VACASK and Spectre keep the instance's name.
    let own = written(&deck, Dialect::Vacask).expect("written").text;
    assert!(own.contains("m1 (d g 0 0) nch"), "{own}");
    // No other SPICE target names a compiled module in a deck line.
    for target in [
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Ltspice,
        Dialect::Espice,
    ] {
        assert!(
            refusals(written(&deck, target)).contains(&Code::W0004),
            "{target:?}"
        );
    }
}

#[test]
fn a_module_named_as_an_ngspice_builtin_device_is_refused() {
    // W0121's run: VACASK's `diode.osdi` (module `diode`) as `.model d
    // diode` is ngspice's built-in Diode, and its `N` instance fails.
    let osdi = vacask_mod().join("diode.osdi");
    let deck = format!(
        "D\nground 0\nload \"{}\"\nmodel vsource vsource\nmodel d diode is=1e-12\n\
         v1 (1 0) vsource dc=1\nd1 (1 0) d\ncontrol\n  analysis op1 op\nendc\n",
        osdi.display()
    );
    assert_eq!(
        refusals(written(&deck, Dialect::Ngspice)),
        vec![Code::W0121]
    );
}

#[test]
fn a_spice_module_with_no_spice_kind_keeps_its_load() {
    // A `spice/<m>.osdi` whose module SpiceRack maps to a SPICE card is
    // VACASK's build of ngspice's device (written as that card); any other
    // (`sp_jfet2`, ngspice's JFET level 2 has no mapped kind here) is a
    // loaded module's model, so its `N` instance needs the load
    // (cir-loading.md).
    let deck = "J\nground 0\nload \"spice/jfet2.osdi\"\nmodel vsource vsource\n\
                model nf sp_jfet2 type=1\nv1 (d 0) vsource dc=1\nj1 (d 0 0) nf\n\
                control\n  analysis op1 op\nendc\n";
    let ng = written(deck, Dialect::Ngspice).expect("written").text;
    assert!(
        ng.contains("pre_osdi spice/jfet2.osdi") && ng.contains("Nj1 d 0 0 nf"),
        "{ng}"
    );
}

#[test]
fn a_vacask_model_number_in_parentheses_or_quotes_is_a_bare_spice_number() {
    // VACASK reads `(1)` as an expression and `"4.8"` as a string the
    // module takes (input-numbers.md, input-strings.md); ngspice's
    // `.model` reads neither ("Syntax error: letter [)]" on
    // demo/spice/bsim4v82.inc), and BSIM4's `version` is the number.
    // VACASK's build of a SPICE device names a clashing instance parameter
    // `instance_<p>` (devices/spice/bsim4v8.va header): SPICE's `<p>`.
    let deck = "M\nground 0\nload \"spice/bsim4v8.osdi\"\nmodel vsource vsource\n\
                model n1 sp_bsim4v8 type=1 version=\"4.8\" binunit=( 1) k2=( -0.01)\n\
                v1 (d 0) vsource dc=1\nm1 (d d 0 0) n1 w=1u l=1u instance_trnqsmod=0\n\
                control\n  analysis op1 op\nendc\n";
    let ng = written(deck, Dialect::Ngspice).expect("written").text;
    for word in ["version=4.8 ", "binunit=1 ", "k2=-10m", " trnqsmod=0"] {
        assert!(ng.contains(word), "{word}:\n{ng}");
    }
}
