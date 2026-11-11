//! Spectre decks read and written for another simulator, the constructs
//! the Spectre reader's rounds 2 and 3 added: a Verilog-A module instance
//! (ngspice's OSDI `N` card, compiled by OpenVAF; VACASK's model of the
//! module, compiled by VACASK; each run against the closed form) and a
//! parameter `sweep` over an analysis (Xyce's `.step`,
//! run on real Xyce). Spectre itself is licence-gated: its text is checked
//! by the Reference's clause.
//!
//! Unsupported fails, never skips: without `ngspice`, `openvaf`, `vacask`
//! (with its `openvaf-r`) and `Xyce` on PATH every run fails; run the
//! tests in `nix develop .#testing`.
//!
//! Cites: Spectre Reference 19.1 pp.406-409 (`sweep`), Spectre Reference
//! 5.0 (`ahdl_include`); ngspice manual §13; VACASK `docs/cir-loading.md`;
//! Xyce RG `.STEP`.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

use spicerack::backend::Dialect;
use spicerack::result::RawData;
use spicerack::syntax::diag::{Code, Kind, info};
use spicerack::syntax::render::render_text;

// ---- private ----

/// `deck` (in `d`) written for `target`, or its refusal codes.
fn written(deck: &str, d: Dialect, target: Dialect) -> Result<String, Vec<Code>> {
    render_text(deck, d, "deck", target)
        .map(|w| w.text)
        .map_err(|e| e.diags.iter().map(|d| d.code).collect())
}

/// The refusal codes of `w` (none when it was written).
fn refusals(w: Result<String, Vec<Code>>) -> Vec<Code> {
    let all = w.err().unwrap_or_default();
    all.into_iter()
        .filter(|&c| info(c).kind == Kind::Refusal)
        .collect()
}

/// The first plot `d` writes for `deck`.
fn run(d: Dialect, deck: &str) -> RawData {
    d.run(deck)
        .unwrap_or_else(|e| panic!("{d:?} failed: {e}\n{deck}"))
}

/// Node `name`'s first value in `raw` (ngspice names it `v(name)`).
fn node(raw: &RawData, name: &str) -> f64 {
    let wrapped = format!("v({name})");
    let at = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case(name) || v.name.eq_ignore_ascii_case(&wrapped));
    raw.real_data[at.unwrap_or_else(|| panic!("no node {name} in {:?}", raw.variables))][0]
}

/// Node `node`'s values Xyce prints for SPICE deck `deck` over its `.dc`
/// (a `.print dc v(node)` added; Xyce writes `<deck>.prn`, one row a
/// point, every step's rows in turn).
fn xyce_dc(deck: &str, node: &str) -> Vec<f64> {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let deck = deck.replace("\n.end", &format!("\n.print dc v({node})\n.end"));
    std::fs::write(dir.path().join("deck.cir"), &deck).expect("write the deck");
    let out = std::process::Command::new("Xyce")
        .arg("deck.cir")
        .current_dir(dir.path())
        .output()
        .expect("Xyce on PATH: run in nix develop .#testing");
    let prn = std::fs::read_to_string(dir.path().join("deck.cir.prn"))
        .unwrap_or_else(|_| panic!("{}\n{deck}", String::from_utf8_lossy(&out.stdout)));
    prn.lines()
        .filter_map(|l| l.split_whitespace().last()?.parse().ok())
        .collect()
}

#[test]
fn a_spectre_verilog_a_instance_is_an_ngspice_osdi_instance() {
    // Spectre: `ahdl_include` a module and instantiate it by name, its
    // parameters on the instance (Spectre Reference 5.0, ahdl_include);
    // ngspice: a compiled module's instance is an `N` card of a `.model`
    // of the module (manual §13), so each instance carries its own model
    // with its parameters. A Verilog-A resistor (I = V / r) in a divider:
    // v(2) = 10 * 9k / 10k.
    let dir = tempfile::tempdir().expect("a scratch directory");
    let va = "`include \"disciplines.vams\"\nmodule vres(a, b);\n  inout a, b;\n  electrical a, b;\n\
              parameter real r = 1k;\n  analog I(a, b) <+ V(a, b) / r;\nendmodule\n";
    std::fs::write(dir.path().join("vres.va"), va).expect("write the module");
    // The load by absolute path: ngspice compiles it where it runs.
    let deck = format!(
        "simulator lang=spectre\nahdl_include \"{}\"\nv1 (1 0) vsource dc=10\n\
         r1 (1 2) vres r=1k\nr2 (2 0) vres r=9k\nop1 dc\n",
        dir.path().join("vres.va").display()
    );
    let path = dir.path().join("deck.scs");
    std::fs::write(&path, deck).expect("write the deck");
    let kit = spicerack::import::kit(
        &[path],
        spicerack::syntax::resolve::Form::Deck,
        Dialect::Spectre,
        dir.path(),
    );
    let w = spicerack::syntax::render::render_kit(
        &kit,
        spicerack::syntax::resolve::FileId(0),
        "deck",
        Dialect::Ngspice,
    )
    .unwrap_or_else(|d| panic!("{}", d.messages));
    assert!(
        w.text.contains(".model Nr1__va vres (r=1k)") && w.text.contains("Nr1 1 2 Nr1__va"),
        "{}",
        w.text
    );
    assert!(
        (node(&run(Dialect::Ngspice, &w.text), "2") - 9.0).abs() < 1e-6,
        "{}",
        w.text
    );
    // Spectre writes the instance back as read.
    let fid = spicerack::syntax::resolve::FileId(0);
    let sp = spicerack::syntax::render::render_kit(&kit, fid, "deck", Dialect::Spectre)
        .unwrap_or_else(|d| panic!("{}", d.messages));
    assert!(sp.text.contains("r1 (1 2) vres r=1k"), "{}", sp.text);
}

#[test]
fn a_spectre_parameter_sweep_is_a_step_on_xyce() {
    // Spectre Reference 19.1 pp.406-409: `sweep param=p start= stop= step=
    // { analysis }` repeats the analysis over p; Xyce RG `.STEP p start
    // stop step` (and LTspice `.step param p …`) does; ngspice runs the
    // points as a `foreach` loop of its control script (manual §13.6.4);
    // ESPice has neither (W0016). The divider's v(2) = 10 rl / (1k + rl).
    let deck = "simulator lang=spectre\nparameters rl=1k\nv1 (1 0) vsource dc=10\n\
                r1 (1 2) resistor r=1k\nr2 (2 0) resistor r=rl\n\
                swp sweep param=rl start=1k stop=9k step=8k {\n  \
                dc1 dc dev=v1 param=dc start=10 stop=10 step=1\n}\n";
    let xy = written(deck, Dialect::Spectre, Dialect::Xyce).expect("written");
    assert!(
        xy.to_ascii_lowercase().contains(".step rl 1000 9000 8000"),
        "{xy}"
    );
    let v = xyce_dc(&xy, "2");
    assert_eq!(v.len(), 2, "{xy}");
    for (x, rl) in v.iter().zip([1e3, 9e3]) {
        assert!((x - 10.0 * rl / (1e3 + rl)).abs() < 1e-6, "{v:?}\n{xy}");
    }
    let lt = written(deck, Dialect::Spectre, Dialect::Ltspice).expect("written");
    assert!(
        lt.to_ascii_lowercase()
            .contains(".step param rl 1000 9000 8000"),
        "{lt}"
    );
    let ng = written(deck, Dialect::Spectre, Dialect::Ngspice).expect("written");
    for line in ["foreach s1 1000 9000", "alterparam rl=$s1", "reset"] {
        assert!(ng.lines().any(|l| l == line), "{line}: {ng}");
    }
    let es = written(deck, Dialect::Spectre, Dialect::Espice);
    assert_eq!(refusals(es), vec![Code::W0016]);
}

// ZERO-LEDGER P3-098. Oracle: Spectre Reference 19.1 p.43 (`lin`, `log`:
// the number of steps; `dec`: points per decade): a count that is no whole
// number from 1 is no sweep, so the statement stays Spectre's text. VACASK
// 36be560 on `points=-3`: "Number of intervals … must be nonnegative".
#[test]
fn p3_098_a_sweep_count_out_of_range_is_no_sweep() {
    let head = "simulator lang=spectre\nv1 (in 0) vsource dc=1 mag=1\nr1 (in 0) resistor r=1k\n";
    for count in ["lin=-3", "lin=0", "lin=2.5", "lin=1e10", "dec=-1"] {
        let deck = format!("{head}ac1 ac start=1 stop=2 {count}\n");
        let ng = written(&deck, Dialect::Spectre, Dialect::Ngspice);
        assert_eq!(refusals(ng), vec![Code::W0001], "{count}");
    }
    let head = "t\nground 0\nload \"spice/resistor.osdi\"\nmodel resistor sp_resistor\n\
                model vsource vsource\nv1 (in 0) vsource dc=1 mag=1\nr1 (in 0) resistor r=1k\n";
    for points in ["-3", "1e10"] {
        let deck = format!(
            "{head}control\n  analysis ac1 ac from=1 to=2 mode=\"lin\" points={points}\nendc\n"
        );
        let ng = written(&deck, Dialect::Vacask, Dialect::Ngspice);
        assert!(refusals(ng).contains(&Code::W0001), "points={points}");
    }
}

// ZERO-LEDGER P3-022. Oracle: Spectre Reference 19.1 pp.43-44 (`ac`): the
// sweep's limits are `start`/`stop` or `center`/`span`; with `dev=` and
// `param=` the statement sweeps that parameter at the frequency `freq=`.
#[test]
fn p3_022_spectre_source_ac_param_dev() {
    let head = "simulator lang=spectre\nv1 (in 0) vsource dc=1 mag=1\n\
                r1 (in out) resistor r=1k\nc1 (out 0) capacitor c=1u\n";
    // Centre 150 Hz, span 100 Hz, 10 steps: 11 points from 100 Hz to 200 Hz.
    let deck = format!("{head}ac1 ac center=150 span=100 lin=10\n");
    let ng = written(&deck, Dialect::Spectre, Dialect::Ngspice).expect("written");
    assert!(ng.lines().any(|l| l == ".ac lin 11 100 200"), "{ng}");
    // v1's dc from 0 V to 1 V at 1 kHz is no frequency sweep from 0 Hz to
    // 1 Hz: the statement stays Spectre's text.
    let deck = format!("{head}ac1 ac dev=v1 param=dc start=0 stop=1 lin=4 freq=1k\n");
    let ng = written(&deck, Dialect::Spectre, Dialect::Ngspice);
    assert_eq!(refusals(ng), vec![Code::W0001]);
}

#[test]
fn p4_021_a_spectre_verilog_a_load_is_vacasks_own_load() {
    // ZERO-LEDGER P4-021. VACASK `docs/cir-loading.md` (36be560): `load
    // "x.va"` compiles the source itself, so Spectre's `ahdl_include` is
    // that line and the writer runs no OpenVAF. VACASK instantiates a
    // model, never a module ("Master 'vres' not found."), so each instance
    // takes a model of its own with the module's parameters; `m` stays on
    // the instance (`$mfactor`). Oracle: real VACASK against the divider's
    // closed form with r1 = 1k / 2: v(2) = 10 * 9k / 9.5k.
    let dir = tempfile::tempdir().expect("a scratch directory");
    let va = "`include \"disciplines.vams\"\nmodule vres(a, b);\n  inout a, b;\n  electrical a, b;\n\
              parameter real r = 1k;\n  analog I(a, b) <+ V(a, b) / r;\nendmodule\n";
    let source = dir.path().join("vres.va");
    std::fs::write(&source, va).expect("write the module");
    let deck = format!(
        "simulator lang=spectre\nahdl_include \"{}\"\nv1 (1 0) vsource dc=10\n\
         r1 (1 2) vres r=1k m=2\nr2 (2 0) vres r=9k\nop1 dc\n",
        source.display()
    );
    let path = dir.path().join("deck.scs");
    std::fs::write(&path, deck).expect("write the deck");
    let kit = spicerack::import::kit(
        &[path],
        spicerack::syntax::resolve::Form::Deck,
        Dialect::Spectre,
        dir.path(),
    );
    let fid = spicerack::syntax::resolve::FileId(0);
    let w = spicerack::syntax::render::render_kit(&kit, fid, "deck", Dialect::Vacask)
        .unwrap_or_else(|d| panic!("{}", d.messages));
    let load = format!("\nload \"{}\"\n", source.display());
    assert!(w.text.contains(&load), "{}", w.text);
    let compiled = source.with_extension("osdi");
    assert!(!compiled.exists(), "the writer ran OpenVAF");
    let exact = 10.0 * 9.0 / 9.5;
    assert!(
        (node(&run(Dialect::Vacask, &w.text), "2") - exact).abs() < 1e-6,
        "{}",
        w.text
    );
}
