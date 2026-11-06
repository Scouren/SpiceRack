//! Deck structure on VACASK: subcircuits and instances, `.step` parameters,
//! initial conditions, and several analyses in one deck in order.
//!
//! Cites: real VACASK.
//!
//! Contents: none (tests only).

use spicerack::syntax::Lang;

use super::*;

// ---- private ----

#[test]
fn subcircuits_and_their_instances_run() {
    let deck = "with_subckt\n.subckt divider in out PARAMS: rtop=1k rbot=1k\nR1 in out {rtop}\nR2 out 0 {rbot}\n.ends divider\nX1 vdd mid divider rtop=1k rbot=3k\nVdd vdd 0 4\n.op\n.end\n";
    let raw = run(deck);
    // 4 V * 3k/(1k+3k) = 3 V.
    close(col(&raw, "mid")[0], 3.0, 1e-9);
}

#[test]
fn step_params_sweep_a_netlist_variable() {
    // A `var` declared in the control block IS visible to the netlist, so a
    // `.step` of a netlist parameter runs on VACASK.
    let deck = "stepped\n.param rtop=1k\nR1 vdd out {rtop}\nR2 out 0 1k\nVdd vdd 0 2\n.step rtop 1000 3000 1000\n.op\n.end\n";
    let raw = run(deck);
    let out = col(&raw, "out");
    assert_eq!(out.len(), 3, "{:?}", out);
    close(out[0], 2.0 * 1000.0 / 2000.0, 1e-9);
    close(out[1], 2.0 * 1000.0 / 3000.0, 1e-9);
    close(out[2], 2.0 * 1000.0 / 4000.0, 1e-9);
}

// Upstream VACASK (`36be560`) reads a mixed list as `{"out", 0.5}`;
// `["out"; 0.5]`, the fork's form and still its docs/cmd-analysis-tran.md:26,
// is "Parser syntax error, unexpected ;" there.
#[test]
fn initial_conditions_reach_the_analysis() {
    let deck = "ic\nR1 vin out 1k\nC1 out 0 1u\nV1 vin 0 1\n.ic V(out)=0.5\n.tran 25u 1m 0 25u UIC\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(netlist.contains("ic={\"out\", 5e-1}"), "{}", netlist);
    let raw = run(deck);
    close(col(&raw, "out")[0], 0.5, 1e-9);
}

#[test]
fn run_all_returns_every_analysis_in_deck_order() {
    // VACASK writes one raw file per analysis; run_all reads each, in the
    // order the deck declares them. Oracle: the divider's closed forms,
    // out = vdd/2 in every analysis (1.5 V at the 3 V rail).
    let deck = rc(".op\n.dc Vdd 0 2 1\n.tran 100u 1m 0 100u");
    let netlist = emit(&deck).expect("emit vacask deck");
    assert!(
        have_vacask(),
        "vacask not on PATH: run the tests in `nix develop .#testing`"
    );
    let plots = Dialect::Vacask
        .run_all(&netlist)
        .unwrap_or_else(|e| panic!("{e}\n{netlist}"));
    assert_eq!(
        plots.len(),
        3,
        "{:?}",
        plots.iter().map(|p| &p.plot_name).collect::<Vec<_>>()
    );
    close(col(&plots[0], "out")[0], 1.5, 1e-9);
    assert_eq!(col(&plots[1], "vsweep"), vec![0.0, 1.0, 2.0]);
    for (v, want) in col(&plots[1], "out").into_iter().zip([0.0, 0.5, 1.0]) {
        assert!((v - want).abs() <= 1e-9, "dc out {v}, want {want}");
    }
    let time = col(&plots[2], "time");
    close(*time.last().unwrap(), 1e-3, 1e-9);
    assert!(
        col(&plots[2], "out")
            .iter()
            .all(|v| (v - 1.5).abs() <= 1e-9)
    );
    // `run` is the first analysis's plot, as for every backend.
    assert_eq!(run(&deck).plot_name, plots[0].plot_name);
    assert_eq!(col(&run(&deck), "out").len(), 1);
}

#[test]
fn a_sectionless_library_call_includes_the_whole_file() {
    // LTspice's `.lib file` reads the whole file. VACASK's `include "f"`
    // does too; `include "f" section=` with no name is its syntax error
    // ("expected section name": `lib/dfllexer.l`, `LIBSECTION`).
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("half.inc");
    std::fs::write(&lib, "r3 (b 0) resistor r=1e3\n").unwrap();
    let deck = format!(
        "t\nV1 a 0 1\nR1 a b 1k\nR2 b 0 1k\n.lib {}\n.op\n.end\n",
        lib.display()
    );
    let netlist = emit_from(
        &deck,
        Dialect::Ltspice,
        Lang::LTSPICE.lexicon().expect("a reader"),
    )
    .unwrap();
    assert!(!netlist.contains("section="), "{netlist}");
    // 1 V over 1k into 1k || 1k: 1/3 V.
    close(col(&run_text(&netlist), "b")[0], 1.0 / 3.0, 1e-9);
}

#[test]
fn a_print_of_nodes_and_source_currents_is_in_the_raw_file() {
    // VACASK saves every node and source flow: a `.print` of them loses
    // nothing, so it is not noted; a resistor's current is not saved.
    let deck = "t\nV1 a 0 1\nR1 a b 1k\nR2 b 0 1k\n.print dc v(b) i(V1)\n.op\n.end\n";
    let netlist = emit(deck).unwrap();
    let w = spicerack::syntax::render::render_text(deck, Dialect::Ngspice, "t", Dialect::Vacask)
        .unwrap();
    assert!(w.notes.diags.is_empty(), "{}", w.notes.messages);
    let raw = run_text(&netlist);
    close(col(&raw, "b")[0], 0.5, 1e-9);
    close(col(&raw, "v1:flow(br)")[0], -0.5e-3, 1e-9);
    let resistor = deck.replace("i(V1)", "i(R1)");
    let w =
        spicerack::syntax::render::render_text(&resistor, Dialect::Ngspice, "t", Dialect::Vacask)
            .unwrap();
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0023], "{}", w.notes.messages);
}

#[test]
fn a_library_read_inside_a_subcircuit_stays_in_its_scope() {
    // ngspice §2.7 scopes what a `.lib` inside a `.subckt` defines to it
    // (`lib-processing/ex1b.sp`: ngspice 1.0 V). VACASK's `include` is
    // textual (`lib/dfllexer.l`) and a subckt may hold one, so written
    // inside `sub1` the library's `inner` (2k) shadows the top's (4k) there
    // only: 1 mA into 4k || 2k || 4k = 1.0 V. Dropped, it ran 1.3333 V.
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("inner.lib");
    std::fs::write(
        &lib,
        "section res\nsubckt inner (n1 n2)\n  r3 (n1 n2) resistor r=2e3\nends\nendsection\n",
    )
    .unwrap();
    let deck = format!(
        "t\nI1 9 0 -1mA\nX1 9 0 sub1\nR2 9 0 4k\nX3 9 0 inner\n.subckt sub1 n1 n2\n\
         .lib {} res\nX1 n1 n2 inner\n.ends\n.subckt inner n1 n2\nR4 n1 n2 4k\n.ends\n.op\n.end\n",
        lib.display()
    );
    close(col(&run(&deck), "9")[0], 1.0, 1e-9);
}

#[test]
fn a_device_ic_without_uic_is_left_out() {
    // ngspice applies `IC=` only with `.tran … uic` (manual, C and L), so
    // the op sets the capacitor: b = 1 V from t=0. VACASK's `sp_capacitor`
    // has no `ic` ("Parameter 'ic' not found."; `capacitor.va` notes).
    let deck = "t\nV1 a 0 1\nR1 a b 1k\nC1 b 0 1u IC=0.3\n.tran 10u 1m\n.end\n";
    let b = col(&run(deck), "b");
    close(b[0], 1.0, 1e-9);
    close(*b.last().unwrap(), 1.0, 1e-9);
}

#[test]
fn an_included_library_has_no_title_line() {
    // VACASK reads a file's every line as a statement (its decks' first
    // line only is the title): an included library's `t` title was
    // "unexpected newline, expecting (" (iscas85, adder_mos.sp). Written
    // with no title, a deck including it runs: 1 V over 1k into 1k.
    let dir = tempfile::tempdir().unwrap();
    let lib = "lib\n.subckt half a b\nR1 a b 1k\n.ends\n";
    let written = emit(lib).expect("library written");
    assert!(!written.starts_with("lib\n"), "{written}");
    let path = dir.path().join("half.inc");
    std::fs::write(&path, &written).unwrap();
    let deck = format!(
        "t\nV1 a 0 1\nR1 a b 1k\nX1 b 0 half\n.include {}\n.op\n.end\n",
        path.display()
    );
    close(col(&run(&deck), "b")[0], 0.5, 1e-9);
}

#[test]
fn control_lines_keep_their_place_among_the_analyses() {
    // A VACASK control block runs in order (`docs/cmd-overview.md`):
    // `elaborate circuit("x")` makes x the circuit the next analyses run
    // on. Written back with every control line first, both analyses ran on
    // `two` (demo/spice/mos1amp.sim). op1 must see 1 V, op2 2 V.
    let deck = "t\n\nground 0\nload \"spice/resistor.osdi\"\nmodel resistor sp_resistor\n\
                model vsource vsource\n\nsubckt one ()\n  v1 (a 0) vsource dc=1\n  \
                r1 (a 0) resistor r=1k\nends\n\nsubckt two ()\n  v1 (a 0) vsource dc=2\n  \
                r1 (a 0) resistor r=1k\nends\n\ncontrol\n  elaborate circuit(\"one\")\n  \
                analysis op1 op\n  elaborate circuit(\"two\")\n  analysis op2 op\nendc\n";
    let lex = Lang::VACASK.lexicon().expect("a reader");
    let netlist = emit_from(deck, Dialect::Vacask, lex).expect("written");
    let plots = Dialect::Vacask
        .run_all(&netlist)
        .unwrap_or_else(|e| panic!("{e}\n{netlist}"));
    close(col(&plots[0], "a")[0], 1.0, 1e-9);
    close(col(&plots[1], "a")[0], 2.0, 1e-9);
}
