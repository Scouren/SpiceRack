//! Spectre deck structure (preamble, includes, subcircuits, models, ic,
//! nodeset, save, measures, options) -> text checked against the manual.
//!
//! Cites: `[REF]`, `[CMP]` and `[KUN]` (see `main.rs`), per test.
//!
//! Contents: none (tests only).

use super::*;

// ---- private ----

/// `[KUN]` B.2: "all Spectre netlists must begin with a `lang=spectre`
/// statement"; `//` starts a comment.
/// `[REF]` p.494 `include "filename"`, p.493 `include "file" section=name`.
/// `[REF]` p.541 / `[CMP]` p.888 `ahdl_include "VerilogAfile.va"`.
/// `[REF]` p.506 `parameters <param=value> [param=value]...`.
#[test]
fn spectre_deck_preamble_matches_documented_statements() {
    let deck =
        "Preamble\n.include /pdk/models.scs\n.lib /pdk/corners.scs tt\n.param vdd=1.8\n.end\n";
    // An ngspice deck has no load card: the load comes beside the text.
    let netlist = spectre_with(deck, &["/m/bsimcmg.va".to_string()]).unwrap();

    assert!(netlist.starts_with("// Preamble\n"), "{netlist}");
    assert!(netlist.contains("\nsimulator lang=spectre\n"), "{netlist}");
    assert!(
        netlist.contains("ahdl_include \"/m/bsimcmg.va\""),
        "{netlist}"
    );
    assert!(netlist.contains("include \"/pdk/models.scs\""), "{netlist}");
    assert!(
        netlist.contains("include \"/pdk/corners.scs\" section=tt"),
        "{netlist}"
    );
    assert!(netlist.contains("parameters vdd=1.8"), "{netlist}");
    // Spectre has no `.end`.
    assert!(!netlist.contains(".end"), "{netlist}");
}

/// A `.lib file` naming no section reads the whole file (LTspice's form):
/// `[CMP]` p.875 `include "filename"`. An empty `section=` is no section
/// name (`[REF]` p.493 `section=name`).
#[test]
fn a_sectionless_library_call_includes_the_whole_file() {
    let deck = "t\n.lib /pdk/models.lib\nR1 a 0 1k\n.op\n.end\n";
    let netlist =
        spicerack::syntax::render::render_text(deck, Dialect::Ltspice, "t", Dialect::Spectre)
            .unwrap()
            .text;
    assert!(
        netlist.contains("\ninclude \"/pdk/models.lib\"\n"),
        "{netlist}"
    );
    assert!(!netlist.contains("section="), "{netlist}");
}

/// `[REF]` p.535 verbatim:
/// ```text
/// subckt coax (i1 o1 i2 o2)
///      parameters zin=50 zout=50 vin=1 vout=1 len=0
/// ends coax
/// ```
/// plus the instance form `Coax1 pin nin out gnd coax zin=75 zout=150 len=35m`.
#[test]
fn spectre_subckt_and_instance_match_documented_forms() {
    let emitted = spectre("defs\n.subckt coax i1 o1 PARAMS: zin=50\n.ends coax\n.end\n").unwrap();
    assert!(
        emitted.contains("\n\nsubckt coax (i1 o1)\nparameters zin=50\nends coax\n"),
        "{emitted}"
    );
    let netlist = spectre("inst\nXCoax1 pin nin coax zin=75\n.end\n").unwrap();
    assert!(
        netlist.contains("xcoax1 (pin nin) coax zin=75"),
        "{netlist}"
    );
}

/// `[CMP]` p.628 `model resmod resistor rsh=150 l=2u w=2u etch=0.05u tc1=0.1`,
/// `[REF]` p.498 `model nch bsim3v3 type=n mobmod=1 capmod=2 version=3.1`.
/// No documented Cadence example parenthesises model parameters.
#[test]
fn spectre_model_statement_has_no_parentheses() {
    let netlist = spectre("models\n.model nch bsim3v3 type=n version=3.1\n.end\n").unwrap();
    assert!(
        netlist.contains("model nch bsim3v3 type=n version=3.1"),
        "{netlist}"
    );
    assert!(
        !netlist.contains("bsim3v3 ("),
        "model params must not be parenthesised: {netlist}"
    );
}

/// `[REF]` p.490 `ic 7=0 out=1 ...`, p.502 `nodeset 7=0 out=1 ...`,
/// p.518 `save 7 out OpAmp1.comp ...` — bare signal names, never `V(x)`.
#[test]
fn spectre_ic_nodeset_save_use_bare_signal_names() {
    let netlist =
        spectre("t\n.ic V(out)=1.2\n.nodeset V(mid)=0.5\n.save V(out)\n.op\n.end\n").unwrap();
    assert!(netlist.contains("\nic out=1.2\n"), "{netlist}");
    assert!(netlist.contains("\nnodeset mid=500m\n"), "{netlist}");
    assert!(netlist.contains("\nsave out\n"), "{netlist}");
}

/// `[CMP]` p.886: `save` takes nodes and `inst:terminal` currents, the
/// terminal "by name or index" (`L1:1`, "the current through the first
/// terminal"; a resistor's terminals are `1 2`, p.629, a vsource's `p n`,
/// p.683). SPICE's `i(R1)` flows into its first node, so it is `r1:1`;
/// a `.print`'s voltages are their nodes (`vdb`, a difference: views of
/// the saved vectors), ground left out.
#[test]
fn a_print_and_a_save_are_spectre_saves_of_the_same_signals() {
    let deck = "t\nV1 a 0 1\nR1 a b 1k\nR2 b 0 1k\n.save i(R1)\n\
                .print tran v(b) v(a,b) vdb(a,0) i(V1)\n.tran 1u 1m\n.end\n";
    let netlist = spectre(deck).unwrap();
    assert!(netlist.contains("\nsave r1:1\n"), "{netlist}");
    assert!(netlist.contains("\nsave b a b a v1:1\n"), "{netlist}");
    // Every output had a signal: nothing noted.
    let w = spicerack::syntax::render::render_text(deck, Dialect::Ngspice, "t", Dialect::Spectre)
        .unwrap();
    assert!(w.notes.diags.is_empty(), "{}", w.notes.messages);
}

/// An output with no signal form (Xyce's `format=`, an expression) is
/// noted W0023; the deck's signals are still saved.
#[test]
fn a_print_output_with_no_signal_form_is_noted() {
    let deck = "t\nV1 a 0 1\nR1 a 0 1k\n.print tran format=csv v(a) {v(a)*2}\n.tran 1u 1m\n.end\n";
    let w =
        spicerack::syntax::render::render_text(deck, Dialect::Xyce, "t", Dialect::Spectre).unwrap();
    assert!(w.text.contains("\nsave a\n"), "{}", w.text);
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0023], "{}", w.notes.messages);
}

/// `[REF]` p.17: Spectre supports "standard SPICE measurement functions
/// (.measure)". `.measure` is SPICE syntax, so it must sit inside a
/// `simulator lang=spice` region (`[REF]` p.493, `[KUN]` B.2).
#[test]
fn spectre_measure_is_fenced_by_language_switches() {
    let netlist = spectre("t\n.meas tran vmax MAX V(out)\n.tran 1u 1m\n.end\n").unwrap();
    assert!(
        netlist
            .contains("simulator lang=spice\n.meas tran vmax MAX V(out)\nsimulator lang=spectre"),
        "{netlist}",
    );
}

/// `[REF]` p.236 verbatim `myopt options temp=27`; options parameters
/// `reltol` (#1), `vabstol` (#3), `iabstol` (#4), `gmin` (#128),
/// `dcmaxiters` (#56, SPICE's `itl1`). The p.237-240 parameter index has no
/// plain `maxiters`.
#[test]
fn spectre_options_use_documented_parameter_names() {
    let deck = spectre(
        "t\n.options reltol=1e-4 abstol=1e-13 vntol=1e-7 gmin=1e-13 max_iterations=150\n.op\n.end\n",
    )
    .unwrap();
    let s = deck.lines().find(|l| l.starts_with("myopts")).unwrap();
    assert_eq!(
        s,
        "myopts options reltol=100u iabstol=100f vabstol=100n gmin=100f dcmaxiters=150",
    );
    assert!(
        !s.contains(" maxiters="),
        "maxiters is a dc parameter, not an option: {s}"
    );
}

/// `[CMP]` p.886: a node in a subcircuit is `OpAmp1.comp`, the hierarchy
/// unescaped (an escaped `\.` is one name). SPICE's `x1.n`, Xyce's `X1:n`.
#[test]
fn a_saved_subcircuit_node_keeps_its_hierarchy() {
    let netlist = spectre("t\n.save v(x1.n)\n.ic v(x1.n)=0\n.op\n.end\n").unwrap();
    assert!(netlist.contains("\nsave x1.n\n"), "{netlist}");
    assert!(netlist.contains("\nic x1.n=0\n"), "{netlist}");
    let deck = "t\n.print tran v(X1:n)\n.tran 1u 1m\n.end\n";
    let w =
        spicerack::syntax::render::render_text(deck, Dialect::Xyce, "t", Dialect::Spectre).unwrap();
    assert!(w.text.contains("\nsave x1.n\n"), "{}", w.text);
}

/// ngspice §2.7 scopes a `.lib` read inside a `.subckt` to it
/// (`lib-processing/ex1b.sp`: 1.0 V on ngspice; the file left out, the
/// top's `inner` gives 1.3333 V). `[REF]` pp.533-535 has no clause on an
/// `include` in a `subckt` body: refused (W0128), never dropped.
#[test]
fn a_file_read_inside_a_subcircuit_is_refused() {
    let deck = "t\nX1 9 0 sub1\n.subckt sub1 n1 n2\n.lib inner.lib res\nX1 n1 n2 inner\n.ends\n\
                .subckt inner n1 n2\nR4 n1 n2 4k\n.ends\n.op\n.end\n";
    assert_eq!(spectre(deck).unwrap_err(), [Code::W0128]);
}
