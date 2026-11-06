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

use spicerack::backend::Dialect;
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::render_text;

/// A `B` source beside the deck's own `E1`, its current measured.
const B_AND_E1: &str = "* t\nV1 in 0 1\nB1 out 0 V=2*v(in)\nR1 out 0 1k\n\
    E1 x 0 in 0 3\nR2 x 0 1k\n.tran 1u 10u\n\
    .meas tran ib find i(B1) at=5u\n.meas tran vx find v(x) at=5u\n.end\n";

// ---- private ----

/// The deck `text` read as `from`, written for `to`; panics on a refusal.
fn write(text: &str, from: Dialect, to: Dialect) -> String {
    render_text(text, from, "t", to)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.messages))
        .text
}

/// The codes the refusal of `text` (read as `from`, written for `to`) has.
fn refusal(text: &str, from: Dialect, to: Dialect) -> Vec<Code> {
    match render_text(text, from, "t", to) {
        Ok(w) => panic!("written, not refused:\n{}", w.text),
        Err(e) => e.diags.iter().map(|d| d.code).collect(),
    }
}

/// `deck` run on real ngspice: the operating point's `v(name)`.
fn ngspice_op(deck: &str, name: &str) -> f64 {
    let raw = Dialect::Ngspice
        .run(deck)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{deck}"));
    let want = format!("v({name})");
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case(&want) || v.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no {want} in {:?}", raw.variables));
    raw.real_data[i][0]
}

/// A file in the temporary directory holding `text`: its path.
fn scratch(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("spice-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
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
    let v = ngspice_op(&ng, "out");
    assert!((v - 0.75).abs() < 1e-9, "v(out) = {v}\n{ng}");
    // HSPICE `.INCLUDE`, Xyce RG `.INCLUDE`: the same card.
    for to in [Dialect::Espice, Dialect::Hspice, Dialect::Xyce] {
        let out = write(&deck, Dialect::Ltspice, to);
        let card = out.lines().find(|l| l.contains("half.lib")).unwrap();
        assert!(card.starts_with(".include "), "{to:?}: {card}");
    }
    // A `.lib file section` stays a section call.
    let sec = deck.replace("half.lib\n", "half.lib tt\n");
    let ng = write(&sec, Dialect::Ltspice, Dialect::Ngspice);
    assert!(ng.contains("half.lib tt\n"), "{ng}");
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

/// Xyce RG: `POLY` is no function of an expression (the Xyce corpus's
/// `MPDE/sandler24.cir`, `BGND 99 0 V={POLY(2) V(3) V(4) 0 0.5 0.5}`): the
/// parser keeps the text (P0002). Its own dialect gets it back as written;
/// every other target refuses it (W0032), where the old writer copied it
/// into an ngspice deck unchecked. The neighbour with a parsed expression is
/// written everywhere.
#[test]
fn text_that_did_not_parse_reaches_its_own_dialect_only() {
    let deck =
        "* t\nV3 3 0 1\nV4 4 0 1\nB1 99 0 V={POLY(2) V(3) V(4) 0 0.5 0.5}\nR1 99 0 1k\n.op\n.end\n";
    let own = write(deck, Dialect::Xyce, Dialect::Xyce);
    assert!(own.contains("POLY(2) V(3) V(4) 0 0.5 0.5"), "{own}");
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
    let good = deck.replace("{POLY(2) V(3) V(4) 0 0.5 0.5}", "{0.5*V(3)+0.5*V(4)}");
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
/// element cards) has no `.backanno` and no ngspice element `Y`; no reader
/// reads CDL, so text the reader kept as the source's is another
/// dialect's there. The old layout copied it inside the `.SUBCKT`; it is
/// left out (W0028) and a kept element is a comment (W0021).
#[test]
fn cdl_leaves_out_text_no_rule_read() {
    let deck = "* t\nR1 a 0 1k\nyfoo a 0 bar\n.backanno\n.end\n";
    let w = render_text(deck, Dialect::Ltspice, "t", Dialect::Cdl).expect("written");
    assert!(
        !w.text.contains(".backanno") && !w.text.contains("yfoo"),
        "{}",
        w.text
    );
    assert!(w.text.contains("* error: W0001 "), "{}", w.text);
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
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
    let v = ngspice_op(&deck, "out");
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
    let w = render_text(deck, Dialect::Ngspice, "t", Dialect::Xyce).expect("written");
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
/// `postprocess` runs a program over the results, `save` selects outputs;
/// none changes a number, so each is left out (W0025, W0023) and the
/// written deck runs on real ngspice to the divider's 0.5 V. `alter`
/// changes a value between analyses: no SPICE card has it, so it is
/// refused (W0001). The old writer copied each as a card (`abort always`
/// an A element to ngspice).
#[test]
fn a_vacask_control_line_is_left_out_or_refused_off_vacask() {
    let deck = "t\nground 0\nload \"spice/resistor.osdi\"\nmodel r resistor\n\
        model v vsource\nv1 (a 0) v dc=1\nr1 (a b) r r=1k\nr2 (b 0) r r=1k\n\
        control\n  abort always\n  save v(b)\n  print stats\n  analysis op1 op\n\
        postprocess(PYTHON, \"runme.py\")\nendc\n";
    let w = render_text(deck, Dialect::Vacask, "t", Dialect::Ngspice).expect("written");
    for word in ["abort", "print", "postprocess", "save v(b)"] {
        assert!(!w.text.contains(word), "{word}\n{}", w.text);
    }
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert!(
        codes.contains(&Code::W0025) && codes.contains(&Code::W0023),
        "{codes:?}"
    );
    let v = ngspice_op(&w.text, "b");
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
    let w = render_text(deck, Dialect::Ngspice, "t", Dialect::Ltspice).expect("written");
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
    let v = ngspice_op(&w, "a");
    assert!((v - 8.0).abs() < 1e-12, "{v}\n{w}");
    let ng = "* t\n.func lim(x,p,n) {x<p?p:x>n?n:x}\nV1 a 0 {lim(7,0,5)}\nR1 a 0 1k\n.op\n.end\n";
    let w = write(ng, Dialect::Ngspice, Dialect::Xyce);
    assert!(w.contains("{x<p?p :x>n?n :x}"), "{w}");
    assert!((ngspice_op(ng, "a") - 5.0).abs() < 1e-12);
}
