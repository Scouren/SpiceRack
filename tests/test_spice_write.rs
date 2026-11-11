//! The SPICE-family writers' conformance gaps (`render/spice/`), each
//! against an oracle the writer does not compute: real ngspice running the
//! written deck to a closed form, or the target manual's clause.
//!
//! Unsupported fails, never skips: no ngspice on PATH fails the run tests
//! (`nix develop .#testing`).
//!
//! Cites: per test.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

#[path = "common/oracle.rs"]
mod oracle;

use oracle::{refusal, write};
use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;

/// A `B` source beside the deck's own `E1`, its current measured.
const B_AND_E1: &str = "* t\nV1 in 0 1\nB1 out 0 V=2*v(in)\nR1 out 0 1k\n\
    E1 x 0 in 0 3\nR2 x 0 1k\n.tran 1u 10u\n\
    .meas tran ib find i(B1) at=5u\n.meas tran vx find v(x) at=5u\n.end\n";

// ---- private ----

/// A file in the temporary directory holding `text`: its path.
fn scratch(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("spice-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    p
}

/// Real ESPice reading `deck` as `tokenizer`: each `.meas` result by name.
fn espice_measures(deck: &str, tokenizer: &str) -> impl Fn(&str) -> f64 {
    let bin = std::env::var("ESPICE").unwrap_or_else(|_| "espice".into());
    let path = scratch("espice.sp", deck);
    let out = std::process::Command::new(&bin)
        .arg(format!("--tokenizer={tokenizer}"))
        .arg(&path)
        .output()
        .unwrap_or_else(|e| panic!("{bin}: {e}: run the tests in `nix develop .#testing`"));
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{stdout}\n{deck}");
    let ms = spicerack::measure_parse::parse_measures(&stdout, Some(Dialect::Espice));
    move |name: &str| {
        let m = ms.iter().find(|m| m.name.eq_ignore_ascii_case(name));
        m.unwrap_or_else(|| panic!("no .meas {name} in\n{stdout}"))
            .value
    }
}

// ── LTspice `.lib file` (no section) ──

/// LTspice help (`.lib`): `.lib file` with no section reads the whole file.
/// ngspice §2.7 takes `.lib file section` only (ngspice-45 stops on the
/// sectionless card: "unimplemented dot command '.lib'"); §2.6 `.include
/// file` reads the whole file. Real ngspice on the written deck: the
/// divider 1 V · 3k/(1k+3k) = 0.75 V, so the file was read.
#[test]
fn an_ltspice_whole_file_lib_is_an_include_on_the_spice_targets() {
    let lib = scratch("half.lib", "R2 out 0 3k\n");
    let deck = format!(
        "* t\nV1 in 0 1\nR1 in out 1k\n.lib {}\n.op\n.end\n",
        lib.display()
    );
    let ng = write(&deck, Dialect::Ltspice, Dialect::Ngspice);
    let v = oracle::op(Dialect::Ngspice, &ng, "out");
    assert!((v - 0.75).abs() < 1e-9, "v(out) = {v}\n{ng}");
    // HSPICE `.INCLUDE`, Xyce RG `.INCLUDE`: the same card.
    for to in [Dialect::Espice, Dialect::Hspice, Dialect::Xyce] {
        let out = write(&deck, Dialect::Ltspice, to);
        let card = out.lines().find(|l| l.contains("half.lib")).unwrap();
        assert!(card.starts_with(".include "), "{to:?}: {card}");
    }
    // LTspice's `.lib` has no section (LTspice help `.LIB <filename>`):
    // the rest of the card is the file, quoted for its blank (§2.6).
    let sec = deck.replace("half.lib\n", "half.lib tt\n");
    let ng = write(&sec, Dialect::Ltspice, Dialect::Ngspice);
    assert!(ng.contains("half.lib tt\"\n"), "{ng}");
}

// ── Verilog-A and OSDI loads on LTspice ──

/// LTspice help: no Verilog-A compiler and no OSDI loader, so a deck that
/// loads one cannot run its device there; the old writer left the load out
/// and wrote the instance anyway. Refused (W0004), each form.
#[test]
fn ltspice_refuses_a_verilog_a_or_osdi_load() {
    let va = "// t\nsimulator lang=spectre\nahdl_include \"m.va\"\nr1 (a 0) resistor r=1k\n";
    assert_eq!(
        refusal(va, Dialect::Spectre, Dialect::Ltspice),
        [Code::W0004]
    );
    let osdi = "t\nload \"m.osdi\"\nr1 (a 0) resistor r=1k\n";
    assert_eq!(
        refusal(osdi, Dialect::Vacask, Dialect::Ltspice),
        [Code::W0004]
    );
    // The legal neighbour: the same deck with no load is written.
    let none = va.replace("ahdl_include \"m.va\"\n", "");
    let _ = write(&none, Dialect::Spectre, Dialect::Ltspice);
}

// ── HSPICE's `E`/`G` for a `B` source, in the name map ──

/// HSPICE has no `B` element: `B1` is `E…  vol=` (HSPICE `E` element,
/// behavioural form). The rename is the map's, so `i(B1)` follows it and
/// the deck's own `E1` keeps its name. Real ESPice (`--tokenizer=hspice`)
/// on the written deck against the closed form: `B1` drives 2 V into 1k,
/// so `i(B1)` = -2 mA (SPICE's sign: into the + node); `E1` gives 3 V.
#[test]
fn hspice_renames_a_b_source_where_its_references_name_it() {
    let hs = write(B_AND_E1, Dialect::Ngspice, Dialect::Hspice);
    assert!(hs.contains("\nE1 x 0 in 0 3\n"), "{hs}");
    let m = espice_measures(&hs, "hspice");
    assert!((m("ib") + 2e-3).abs() < 1e-12, "{hs}");
    assert!((m("vx") - 3.0).abs() < 1e-9, "{hs}");
}

// ── `log` sweeps ──

/// Spectre's `log=N` (N points, logarithmic) has no SPICE spelling: the
/// ngspice manual §11.3 `.ac`/`.noise`/`.sp`/`.sens` take `dec|oct|lin`,
/// HSPICE `.AC` DEC/OCT/LIN/POI, Xyce RG `.AC` DEC/OCT/LIN, LTspice `.ac`
/// oct/dec/lin. Real ngspice-45 runs nothing from `.ac log` (the old writer
/// copied it). Refused W0031 on every SPICE target; the `dec` neighbour is
/// written and runs.
#[test]
fn a_log_sweep_is_refused_on_the_spice_targets() {
    let deck = "* t\nV1 in 0 0 AC 1\nR1 in out 1k\nC1 out 0 1u\n.ac log 31 1 1k\n.end\n";
    let spectre = "// t\nsimulator lang=spectre\nv1 (in 0) vsource dc=0 mag=1\n\
                   r1 (in out) resistor r=1k\nc1 (out 0) capacitor c=1u\n\
                   a1 ac start=1 stop=1k log=31\n";
    let spice = [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Ltspice,
    ];
    for to in spice {
        assert_eq!(refusal(deck, Dialect::Ngspice, to), [Code::W0031], "{to:?}");
        assert_eq!(
            refusal(spectre, Dialect::Spectre, to),
            [Code::W0031],
            "{to:?}"
        );
    }
    assert!(Dialect::Ngspice.run(deck).is_err(), "ngspice ran `.ac log`");
    let dec = write(
        &deck.replace("log 31", "dec 10"),
        Dialect::Ngspice,
        Dialect::Ngspice,
    );
    let raw = Dialect::Ngspice
        .run(&dec)
        .expect("ngspice runs the dec sweep");
    assert_eq!(raw.complex_data[0].len(), 31, "{dec}");
}

// ── text that did not parse ──

/// An expression of two terms with no operator between them (`V={V(3)
/// V(4)}`) is no expression of any dialect: the parser keeps the text
/// (P0002). (Xyce's `V={POLY(2) V(3) V(4) 0 0.5 0.5}`, this test's first
/// case, is read as the E POLY card it spells since stream D's b_poly.) Its own dialect gets it back as written;
/// every other target refuses it (W0032), where the old writer copied it
/// into an ngspice deck unchecked. The neighbour with a parsed expression is
/// written everywhere.
#[test]
fn text_that_did_not_parse_reaches_its_own_dialect_only() {
    let deck = "* t\nV3 3 0 1\nV4 4 0 1\nB1 99 0 V={V(3) V(4)}\nR1 99 0 1k\n.op\n.end\n";
    let own = write(deck, Dialect::Xyce, Dialect::Xyce);
    assert!(own.contains("V(3) V(4)"), "{own}");
    let others = [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Ltspice,
        Dialect::Cdl,
    ];
    for to in others {
        let codes = refusal(deck, Dialect::Xyce, to);
        assert!(codes.contains(&Code::W0032), "{to:?}: {codes:?}");
    }
    let good = deck.replace("{V(3) V(4)}", "{0.5*V(3)+0.5*V(4)}");
    for to in others {
        let _ = write(&good, Dialect::Xyce, to);
    }
    // Unparsed text in a statement the target leaves out (CDL writes no
    // `.meas`, W0028) is not copied, so the deck is written there.
    let meas = "* t\nV1 1 0 1\nR1 1 0 1k\n.tran 1n 10n\n.meas tran x find v(1 at=5n\n.end\n";
    let _ = write(meas, Dialect::Ngspice, Dialect::Cdl);
    let codes = refusal(meas, Dialect::Ngspice, Dialect::Hspice);
    assert!(codes.contains(&Code::W0032), "{codes:?}");
}

// ── CDL ──

/// Cadence CDL (an LVS netlist: `.SUBCKT`, `.ENDS`, `*.PININFO`, `.END`,
/// element cards) has no `.wave` and no ngspice element `Y`; no reader
/// reads CDL, so text the reader kept as the source's is another
/// dialect's there. The old layout copied it inside the `.SUBCKT`; it is
/// left out (W0028) and a kept element is a comment (W0021). (`.backanno`
/// the reader leaves out on every target, W0036.)
#[test]
fn cdl_leaves_out_text_no_rule_read() {
    let deck = "* t\nR1 a 0 1k\nyfoo a 0 bar\n.backanno\n.wave x.wav 16 44100 a\n.end\n";
    let w = oracle::written(deck, Dialect::Ltspice, Dialect::Cdl);
    assert!(
        !w.text.contains(".backanno") && !w.text.contains(".wave") && !w.text.contains("yfoo"),
        "{}",
        w.text
    );
    assert!(w.text.contains("* error: W0001 "), "{}", w.text);
    let codes = oracle::notes(&w);
    assert!(
        codes.contains(&Code::W0028) && codes.contains(&Code::W0021),
        "{codes:?}"
    );
}

/// ngspice §2.4.3: an instance's name is the whole word, its letter
/// included (`XX1` is `XX1`), as the name map declares every card. The old
/// layout dropped a leading `X` of the name after the letter (`XX1` was
/// `X1`, another instance's name here).
#[test]
fn cdl_names_an_instance_as_the_map_declares_it() {
    let deck = "* t\n.subckt sub a\nR1 a 0 1k\n.ends\nXX1 n 0 sub\nX1 m 0 sub\n.end\n";
    let w = write(deck, Dialect::Ngspice, Dialect::Cdl);
    assert!(
        w.contains("\nXX1 n 0 sub\n") && w.contains("\nX1 m 0 sub\n"),
        "{w}"
    );
}

// ── the file role ──

/// A library (no analysis, control block or top-level source) is framed as
/// a deck on the SPICE targets: its title a `*` comment, its `.end` read as
/// nothing where it is included. Real ngspice on a deck including the
/// written library: 1 V across 1k into 3k || 3k is 0.6 V, every card read.
/// HSPICE `.END` ends the run even in an included file, so the HSPICE
/// library has none; a deck that runs ends with one.
#[test]
fn a_library_is_framed_as_a_deck_where_that_reads_as_nothing() {
    let lib = "* lib\nR2 out 0 3k\n";
    let ng = write(lib, Dialect::Ngspice, Dialect::Ngspice);
    assert!(ng.trim_end().ends_with("\n.end"), "{ng}");
    let path = scratch("framed.lib", &ng);
    let deck = format!(
        "* t\nV1 in 0 1\n.include \"{}\"\nR1 in out 1k\nR3 out 0 3k\n.op\n.end\n",
        path.display()
    );
    let v = oracle::op(Dialect::Ngspice, &deck, "out");
    assert!((v - 0.6).abs() < 1e-9, "v(out) = {v}\n{ng}");
    let hs = write(lib, Dialect::Ngspice, Dialect::Hspice);
    assert!(!hs.to_ascii_lowercase().contains(".end"), "{hs}");
    let runs = write(
        "* t\nV1 a 0 1\nR1 a 0 1k\n.op\n.end\n",
        Dialect::Ngspice,
        Dialect::Hspice,
    );
    assert!(runs.trim_end().ends_with("\n.end"), "{runs}");
}

// ── Xyce options ──

/// Xyce RG `.options <pkg>`: SPICE's ABSTOL is NONLIN ABSTOL (the residual
/// tolerance, 1e-12 A by default in both), METHOD (trap, gear), MAXORD and
/// DELMAX are TIMEINT's, SCALE is PARSER's; Xyce 7.10 takes each name
/// without "No options parameter … found" (run by hand). ACCT shapes only
/// the listing (ngspice §11.1.1): left out, W0024. VNTOL, a voltage update
/// tolerance, has no Xyce option (the old writer spelled it NONLIN ABSTOL,
/// a current one): refused, W0017.
#[test]
fn xyce_takes_the_options_its_reference_guide_names() {
    let deck = "* t\nV1 a 0 1\nR1 a 0 1k\n.options acct abstol=1n method=gear maxord=2 delmax=1n scale=1u\n.tran 1n 10n\n.end\n";
    let w = oracle::written(deck, Dialect::Ngspice, Dialect::Xyce);
    for line in [
        ".options nonlin abstol=1n\n",
        ".options timeint method=gear maxord=2 delmax=1n\n",
        ".options parser scale=1u\n",
    ] {
        assert!(w.text.contains(line), "{line}{}", w.text);
    }
    assert!(!w.text.contains("acct"), "{}", w.text);
    assert!(w.notes.diags.iter().any(|d| d.code == Code::W0024));
    let vntol = deck.replace("acct ", "vntol=1u ");
    assert_eq!(
        refusal(&vntol, Dialect::Ngspice, Dialect::Xyce),
        [Code::W0017]
    );
}

// ── VACASK control lines ──

/// VACASK `simulator/cmd.cpp` (the `control` commands): `abort` sets when
/// a failed command stops the script, `print` writes to the console,
/// `postprocess` runs a program over the results; none changes a number,
/// so each is left out (W0120); `save v(b)` is the save list (`.save
/// v(b)`), and the written deck runs on real ngspice to the divider's
/// 0.5 V. `alter`
/// changes a value between analyses: no SPICE card has it, so it is
/// refused (W0001). The old writer copied each as a card (`abort always`
/// an A element to ngspice).
#[test]
fn a_vacask_control_line_is_left_out_or_refused_off_vacask() {
    let deck = "t\nground 0\nload \"spice/resistor.osdi\"\nmodel r resistor\n\
        model v vsource\nv1 (a 0) v dc=1\nr1 (a b) r r=1k\nr2 (b 0) r r=1k\n\
        control\n  abort always\n  save v(b)\n  print stats\n  analysis op1 op\n\
        postprocess(PYTHON, \"runme.py\")\nendc\n";
    let w = oracle::written(deck, Dialect::Vacask, Dialect::Ngspice);
    for word in ["abort", "print", "postprocess"] {
        assert!(!w.text.contains(word), "{word}\n{}", w.text);
    }
    let codes = oracle::notes(&w);
    assert!(codes.contains(&Code::W0120), "{codes:?}");
    assert!(w.text.contains(".save v(b)"), "{}", w.text);
    let v = oracle::op(Dialect::Ngspice, &w.text, "b");
    assert!((v - 0.5).abs() < 1e-9, "{v}\n{}", w.text);
    let alter = deck.replace("print stats", "alter r1 r=2k");
    let codes = refusal(&alter, Dialect::Vacask, Dialect::Ngspice);
    assert!(codes.contains(&Code::W0001), "{codes:?}");
}

// ── noise summary ──

/// ngspice manual §15.3.4: `.noise … pts_per_summary` adds each device's
/// contribution every N points; LTspice's `.NOISE` (help: output, source,
/// sweep, points, start, stop) has no such field, so it is left out with a
/// note (W0033), where the old writer dropped it silently; ngspice keeps it.
#[test]
fn ltspice_notes_the_noise_summary_it_leaves_out() {
    let deck = "* t\nV1 in 0 dc 0 ac 1\nR1 in out 1k\nC1 out 0 1n\n\
        .noise v(out) V1 dec 10 1k 100meg 5\n.end\n";
    let w = oracle::written(deck, Dialect::Ngspice, Dialect::Ltspice);
    assert!(w.text.contains("dec 10 1k 100meg\n"), "{}", w.text);
    assert!(
        w.notes.diags.iter().any(|d| d.code == Code::W0033),
        "{}",
        w.text
    );
    assert!(write(deck, Dialect::Ngspice, Dialect::Ngspice).contains("100meg 5\n"));
}

// ── nested ternaries ──

/// Xyce RG §2.1.18 reads `?:` by C's grammar: `x>0?y>0?4:8:8` at x=1,
/// y=-1 is 8 (Xyce 7.10, run by hand, on `xyce/XDM/HSPICE/OTHER_PARSING/
/// ternary_operator.cir`'s VAL4). ngspice-45 reads the unparenthesized
/// text as 4, so the written deck parenthesizes the inner ternary and real
/// ngspice gives 8. Xyce 7.10 reads `p:n` as one name ("syntax error in
/// expression" on ngspice's `func-1.sp`, run by hand), so its `:` gets a
/// blank before it (`p :n` runs, giving 5 here as ngspice does).
#[test]
fn a_nested_ternary_keeps_its_meaning_across_dialects() {
    let deck =
        "* t\n.func val4(x,y) {x>0?y>0?4 :8 :8}\nV1 a 0 {val4(1,-1)}\nR1 a 0 1k\n.op\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    let v = oracle::op(Dialect::Ngspice, &w, "a");
    assert!((v - 8.0).abs() < 1e-12, "{v}\n{w}");
    let ng = "* t\n.func lim(x,p,n) {x<p?p:x>n?n:x}\nV1 a 0 {lim(7,0,5)}\nR1 a 0 1k\n.op\n.end\n";
    let w = write(ng, Dialect::Ngspice, Dialect::Xyce);
    assert!(w.contains("{x<p?p :x>n?n :x}"), "{w}");
    assert!((oracle::op(Dialect::Ngspice, ng, "a") - 5.0).abs() < 1e-12);
}

// ── transient step ──

/// Xyce and LTspice take `.tran 0 <stop>` (Xyce RG `.TRAN`: a zero
/// initial step is chosen by the simulator); ngspice-45 stops on it
/// ("TSTEP is invalid, must be greater than zero", `xyce/TEMPCAP/
/// tempcap.cir`). The written step is the span over 50, ngspice's own
/// default step cap (manual §11.3.10, TMAX), and real ngspice charges the
/// RC (1 ms) to 1 - e^-5 of 1 V at 5 ms.
#[test]
fn a_zero_transient_step_is_a_positive_one_on_ngspice() {
    let deck = "* t\nV1 in 0 1\nR1 in out 1k\nC1 out 0 1u IC=0\n.tran 0 5m UIC\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(
        w.contains(".tran 1e-4 5m") || w.contains(".tran 100u 5m"),
        "{w}"
    );
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("v(out)"));
    let v = *raw.real_data[i.expect("v(out)")].last().unwrap();
    let want = 1.0 - (-5.0f64).exp();
    assert!((v - want).abs() < 1e-3, "{v} vs {want}\n{w}");
}

/// Xyce's zero TSTEP lets Xyce choose its steps; ngspice caps its step at
/// TSTEP (manual §11.3.10), so the span over 50 stepped a 100 Hz sine over
/// 100 ms at 2 ms, 5 points a period (stream F round 4,
/// `MEASURE/AverageTest.cir`, 77 decks). The step is held to a fiftieth of
/// the fastest source's period: real ngspice's v(1) reaches the sine's
/// peak, 1 V, to 0.5%.
#[test]
fn a_zero_transient_step_follows_the_fastest_source() {
    let deck = "* t\nV1 1 0 sin(0 1 100)\nR1 1 0 1k\n.tran 0 100m\n.end\n";
    let w = write(deck, Dialect::Xyce, Dialect::Ngspice);
    let raw = Dialect::Ngspice
        .run(&w)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{w}"));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("v(1)"));
    let peak = raw.real_data[i.expect("v(1)")]
        .iter()
        .cloned()
        .fold(f64::MIN, f64::max);
    assert!((peak - 1.0).abs() < 5e-3, "{peak}\n{w}");
}

// ── a path with a blank ──

/// ngspice §2.6 and Xyce RG `.INCLUDE` read a quoted path; written bare,
/// `my dir/half.lib` is a path and a section. Real ngspice on the written
/// deck: the divider 1 V · 3k/(1k+3k) = 0.75 V, so the file was read.
#[test]
fn a_path_with_a_blank_is_quoted() {
    let lib = scratch("my dir/half.lib", "R2 out 0 3k\n");
    let deck = format!(
        "* t\nV1 in 0 1\nR1 in out 1k\n.include \"{}\"\n.op\n.end\n",
        lib.display()
    );
    let ng = write(&deck, Dialect::Ltspice, Dialect::Ngspice);
    let v = oracle::op(Dialect::Ngspice, &ng, "out");
    assert!((v - 0.75).abs() < 1e-9, "v(out) = {v}\n{ng}");
    let xy = write(&deck, Dialect::Ltspice, Dialect::Xyce);
    assert!(
        xy.contains(&format!(".include \"{}\"", lib.display())),
        "{xy}"
    );
}

/// ngspice-45's `{}` evaluator (numparam, `xpressn.c`) reads
/// `a?b?c:d:e` as `a?c:e`: real ngspice gives 4 for `1?0?4:8:16`, where
/// C's grouping gives 8. That is the source's meaning, so another family
/// gets `1?4:16` with a note (W0034). A function body's is refused
/// (W0035): ngspice reads it by C's grammar in a B source and its own way
/// in a `{}` value; so is a deeper nest, which ngspice reads by neither.
#[test]
fn an_ngspice_nested_ternary_is_written_as_ngspice_reads_it() {
    let deck = "* t\nV1 a 0 {1?0?4:8:16}\nR1 a 0 1k\n.op\n.end\n";
    assert!((oracle::op(Dialect::Ngspice, deck, "a") - 4.0).abs() < 1e-12);
    let w = oracle::written(deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(w.text.contains("{1?4 :16}"), "{}", w.text);
    assert!(w.notes.diags.iter().any(|d| d.code == Code::W0034));
    let hs = write(deck, Dialect::Ngspice, Dialect::Hspice);
    assert!(hs.contains("'1?4:16'"), "{hs}");
    let func = "* t\n.func f(x) {x>0?x>1?4:8:16}\nV1 a 0 {f(1)}\nR1 a 0 1k\n.op\n.end\n";
    assert!(refusal(func, Dialect::Ngspice, Dialect::Xyce).contains(&Code::W0035));
    let deep = deck.replace("1?0?4:8:16", "0?0?4:1?8:16:32");
    assert!(refusal(&deep, Dialect::Ngspice, Dialect::Xyce).contains(&Code::W0035));
}

// ── Xyce prints on ngspice ──

/// ngspice-45's `.print` (manual §15.4.1) takes a kind it runs and bare
/// vectors or expressions: `.print homotopy` and `.print dc` with no `.dc`
/// are "no … analysis found", `{v(1)*2}` is "no such function as {v",
/// and Xyce's `p(R1)` (RG `.PRINT`, device power) leaves the transient
/// "not run" (each run by hand). So a print of a kind the deck does not
/// run and a power accessor are left out (W0023), a braced expression is
/// written bare, and real ngspice runs the deck: v(2) = 0.5 V.
#[test]
fn xyce_prints_are_spelled_or_left_out_on_ngspice() {
    let deck = "* t\nV1 1 0 1\nR1 1 2 1k\nR2 2 0 1k\n.tran 1u 10u\n\
        .print tran v(2) {v(1) * 2} p(R1)\n.print homotopy v(2)\n.print dc v(2)\n.end\n";
    let w = oracle::written(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(w.text.contains(".print tran v(2) v(1)*2\n"), "{}", w.text);
    assert!(
        !w.text.contains("homotopy") && !w.text.contains(".print dc"),
        "{}",
        w.text
    );
    assert!(w.notes.diags.iter().any(|d| d.code == Code::W0023));
    let raw = Dialect::Ngspice
        .run(&w.text)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{}", w.text));
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case("v(2)"));
    let v = *raw.real_data[i.expect("v(2)")].last().unwrap();
    assert!((v - 0.5).abs() < 1e-9, "{v}\n{}", w.text);
}

/// Xyce RG §2.3 (`sdt(x)`, the time integral of x); ngspice's B-source
/// functions (manual §5.1) have `ddt` but no integral, and ngspice-45
/// stops on `sdt` ("no such function 'sdt'", `xyce/Certification_Tests/
/// BUG_963_SON/nonzeroInitialValue.cir`, run by hand): refused (W0045).
#[test]
fn ngspice_refuses_xyces_time_integral() {
    let deck = "* t\nV2 2 0 1\nBsrc 1 0 V=sdt(v(2))\nR1 1 0 1k\n.tran 1u 10u\n.end\n";
    let codes = refusal(deck, Dialect::Xyce, Dialect::Ngspice);
    assert!(codes.contains(&Code::W0045), "{codes:?}");
}

/// ngspice §2.7 scopes what a `.include` or `.lib` inside a `.subckt`
/// defines to that subcircuit (ngspice's `lib-processing/ex1b.sp`: 1.0 V);
/// Xyce 7.10 makes it global, so the library's `inner` replaces the
/// deck's own and the same deck gives 0.8 V (`.include`; a `.lib` there is
/// "Invalid device type"; each run by hand). The writer cannot rename
/// inside a file it does not write: refused on Xyce (W0046).
#[test]
fn xyce_refuses_a_file_read_inside_a_subcircuit() {
    let lib = scratch("inner.lib", ".subckt inner n1 n2\nR3 n1 n2 2k\n.ends\n");
    let deck = format!(
        "* t\nI1 9 0 -1mA\nX1 9 0 sub1\nR2 9 0 4k\nX3 9 0 inner\n.subckt sub1 n1 n2\n\
         .include {}\nX1 n1 n2 inner\n.ends\n.subckt inner n1 n2\nR4 n1 n2 4k\n.ends\n.op\n.end\n",
        lib.display()
    );
    assert!((oracle::op(Dialect::Ngspice, &deck, "9") - 1.0).abs() < 1e-9);
    let codes = refusal(&deck, Dialect::Ngspice, Dialect::Xyce);
    assert!(codes.contains(&Code::W0046), "{codes:?}");
}

// ── a hoisted subcircuit's models ──

/// ngspice manual §2.4.3: a `.model` inside a `.subckt` is seen by the
/// subcircuits nested in it. Written hoisted (`one__inner`), the nested
/// one holds a copy (Xyce_Regression `SUBCKT/subckt_j1.cir`: ngspice-45
/// "Unable to find definition of model sw" before). Real ngspice on the
/// written deck: the switch closes, v(out) = 1 V · 1k/(1k+1) .
#[test]
fn a_hoisted_subcircuit_takes_the_models_around_it() {
    let deck = "* t\nV1 in 0 1\nX1 in out one\nR1 out 0 1k\n.subckt one a b\n\
        .model sw sw(ron=1 roff=1meg vt=0.5)\nX2 a b inner\n.subckt inner p q\nS1 p q p 0 sw\n\
        .ends\n.ends\n.op\n.end\n";
    let w = write(deck, Dialect::Ngspice, Dialect::Ngspice);
    let v = oracle::op(Dialect::Ngspice, &w, "out");
    assert!((v - 1000.0 / 1001.0).abs() < 1e-6, "{v}\n{w}");
}

// ── LTspice's start-up ramp ──

/// Ledger P5-005. LTspice help, `.TRAN`: `startup` solves the operating
/// point with the sources off and ramps them on in the first 20 µs.
/// LTspice 26.1.1 ran this deck (`research/impl/evidence/P5-005`): each
/// top-level V and I whose value is DC only (`V2`'s `AC` beside it) is
/// X·t/20 µs, then X; the SINE and the subcircuit's source are not
/// ramped. The closed forms are that ramp's, checked on LTspice to 1e-7,
/// so each simulator runs the rewrite against a number the writer does
/// not compute. HSPICE and Spectre run nowhere here: their card is the
/// manual's form of the one the four simulators ran.
#[test]
fn p5_005_an_ltspice_startup_ramps_the_top_dc_sources_on_every_target() {
    let deck = "t\nV1 a 0 5\nR1 a b 1k\nC1 b 0 1n\nI1 0 i 1m\nR2 i 0 1k\n\
        V2 q 0 2 AC 1\nR3 q 0 1k\nV3 s 0 SINE(1 1 10k)\nR4 s 0 1k\nV4 z 0 0\nR5 z 0 1k\n\
        X1 c sub\n.subckt sub o\nV1 o 0 3\nR1 o 0 1k\n.ends\n\
        .tran 0 40u 0 0.1u startup\n.end\n";
    // A source's own value is exact at a time point and linear between
    // two: the simulators' roundoff.
    let exact = oracle::Tol {
        rel: 1e-6,
        floor: 1e-6,
    };
    // A node the integrator or the time grid shapes: RELTOL (1e-3).
    let stepped = oracle::Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    let at = oracle::At::Time;
    let cases = [
        // V1 = 5 V: off at t = 0, half way at 10 µs, full after 20 µs.
        ("a", oracle::At::First, 0.0, exact),
        ("a", at(10e-6), 2.5, exact),
        ("a", at(30e-6), 5.0, exact),
        // I1 = 1 mA into 1 kΩ: 1 V · 10/20.
        ("i", at(10e-6), 0.5, exact),
        // V2 = DC 2 AC 1: the AC beside it does not stop the ramp, 2 V · 5/20.
        ("q", at(5e-6), 0.5, exact),
        // The subcircuit's 3 V source is not ramped.
        ("c", oracle::At::First, 3.0, exact),
        ("c", at(10e-6), 3.0, exact),
        // The SINE is not ramped: 1 + sin(2π · 10 kHz · 10 µs) = 1.5877853.
        ("s", at(10e-6), 1.587_785_3, stepped),
        // The RC (τ = 1 kΩ · 1 nF = 1 µs) on the ramp k·t, k = 5 V / 20 µs:
        // k (t − τ (1 − e^(−t/τ))) = 2.5e5 · 9.0000454e-6 = 2.2500114.
        ("b", at(10e-6), 2.250_011_4, stepped),
    ];
    for d in [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Xyce,
        Dialect::Vacask,
    ] {
        let text = write(deck, Dialect::Ltspice, d);
        let plots = oracle::run(d, &text);
        for (node, at, want, tol) in cases {
            let got = oracle::value(&plots, node, at);
            assert!(
                tol.within(got, want),
                "{d:?}: v({node}) = {got} at {at:?}, want {want}\n{text}"
            );
        }
    }
    // HSPICE `Vxxx n+ n- [[DC=] dcval] [tranfun]` (the tranfun's value at
    // time zero overrides the DC value in a transient); Spectre Reference,
    // `vsource`: `dc` for the DC analyses, `type=pwl` + `wave` in time.
    let hs = write(deck, Dialect::Ltspice, Dialect::Hspice);
    assert!(
        hs.lines().any(|l| l == "V1 a 0 5 PWL(0 0 0.00002 5)"),
        "{hs}"
    );
    assert!(hs.lines().any(|l| l == "V4 z 0 0"), "{hs}");
    let sp = write(deck, Dialect::Ltspice, Dialect::Spectre);
    assert!(
        sp.lines()
            .any(|l| l == "v1 (a 0) vsource dc=5 type=pwl wave=[0 0 20u 5]"),
        "{sp}"
    );
}

// ── a Xyce model level on ngspice, ESPice and HSPICE (ledger P3-006) ──

/// ngspice-45 `inpdomod.c` (L73-75, L383-385: the levels in its binary)
/// and `inpfindl.c:51-57` (a level above 99 is computed as level 1): Xyce's
/// PSP 103, power MOSFET 18, VBIC 1.3 (11) and MEXTRAM 504 have no device
/// there, nor in HSPICE (MOSFET Models D-2010.12 Table 1): refused. Xyce
/// 7.10 `level=103` at 1 V, 1 V: 1.111 mA; ngspice-45 on the copied card
/// 0.100 mA (XYCE_CORPUS `PSP103/`, `VBIC13/`, `MEXTRAM/`, `VDMOS/`).
/// HICUM/L2 is Xyce's 234 and ngspice's 8 (both v2.4.0): the same
/// collector node on both simulators (Xyce 0.540334, ngspice 0.540309;
/// their constants differ by 2.8e-5 in kT/q).
#[test]
fn p3_006_xyce_model_levels_copied_ngspice() {
    let cite = "ngspice-45 inpdomod.c L73-75, L383-385; HSPICE MOSFET Models Table 1";
    for (kind, level) in [("nmos", 103), ("nmos", 18), ("npn", 11), ("npn", 504)] {
        let card = match kind {
            "nmos" => "m1 d g 0 0 nm l=1u w=10u",
            _ => "q1 d g 0 nm",
        };
        let deck =
            format!("* t\nvd d 0 1\nvg g 0 1\n{card}\n.model nm {kind} level={level}\n.op\n.end\n");
        for to in [Dialect::Ngspice, Dialect::Espice, Dialect::Hspice] {
            oracle::spec_refusal(cite, &deck, Dialect::Xyce, to, &[Code::W0104]);
        }
    }
    let hicum = "* t\nvc cc 0 1.5\nrc cc c 100\nvb b 0 0.85\nq1 c b 0 0 t qh\nrt t 0 1\n\
        .model qh npn level=234 c10=9.074e-30 qp0=1.008e-13 ibeis=1.328e-19 mbei=1.027 \
        re=1.511 rcx=2.483 rbx=2.568\n.op\n.end\n";
    let tol = oracle::Tol {
        rel: 1e-3,
        floor: 0.0,
    };
    oracle::sim_pair(
        Dialect::Xyce,
        hicum,
        Dialect::Ngspice,
        "c",
        oracle::At::First,
        tol,
    );
    assert!(write(hicum, Dialect::Xyce, Dialect::Ngspice).contains("level=8"));
}

/// Xyce RG 7.10 `M` level table: 10 is BSIM-SOI 3.2. ngspice-45
/// `inpdomod.c:347-348`: 10 and 58 are B4SOI (4.4), 57 is B3SOIPD 2.2.3;
/// HSPICE's 10 is AMD's model (MOSFET Models Table 1). No target device
/// has Xyce's equations (this card: v(d) 0.611222 on Xyce 7.10, 0.563875
/// on ngspice-45 given the copy; XYCE_CORPUS `BSIMSOI3/`): refused.
#[test]
fn p3_007_xyce_nmos_level_written_level() {
    let deck = "* t\nvd dd 0 1.5\nrd dd d 1k\nvg g 0 1.5\nm1 d g 0 0 n1 w=4u l=0.15u\n\
        .model n1 nmos level=10 tox=5n tsi=90n tbox=500n vth0=0.5 u0=0.05 nch=8e17 k1=0.1 \
        eta0=0.5 dsub=0.35 nfactor=0.4 pclm=1 rdsw=100 vsat=80000 shmod=1 rth0=1\n.op\n.end\n";
    let cite = "ngspice-45 inpdomod.c:347-348; Xyce RG 7.10 M level table";
    for to in [Dialect::Ngspice, Dialect::Espice, Dialect::Hspice] {
        oracle::spec_refusal(cite, deck, Dialect::Xyce, to, &[Code::W0104]);
    }
}
