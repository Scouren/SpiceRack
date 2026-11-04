//! The renderer over block nesting, kit answers and a given deck's `.step`:
//! a deck nested 10⁴ deep in `.subckt`s and `.if`s is written in Debug,
//! what `syntax::resolve` answers for a kit file reaches the deck written
//! from it, and a `.step log` is Spectre's sweep or refused. Each test
//! names its clause, the text derived by hand, and how many checks it
//! makes.
//!
//! Contents: none (tests only).

use crate::syntax::Dialect;
use crate::syntax::render::{render_kit, render_text};

// ---- private ----

#[test]
fn nested_subckts_ten_thousand_deep_are_written() {
    // ngspice manual §2.5: definitions nest; the reader hoists each as
    // `<parent>__<name>` (as the old lowering did), walking the nesting on a
    // heap stack (rule 7 of the compiler-construction skill), so this runs
    // in Debug on a test thread's 2 MiB stack. Checks: 2.
    let n = 10_000;
    let mut src = String::from("t\n");
    for k in 0..n {
        src.push_str(&format!(".subckt s{k} a\nR{k} a 0 1\n"));
    }
    for _ in 0..n {
        src.push_str(".ends\n");
    }
    let w = render_text(&src, Dialect::Ngspice, "deep", Dialect::Ngspice).expect("written");
    assert_eq!(w.text.matches(".subckt ").count(), n);
    let inner = format!(".subckt s{}__s{} a", n - 2, n - 1);
    assert!(w.text.lines().any(|l| l == inner), "{inner}");
}

#[test]
fn nested_ifs_ten_thousand_deep_are_written() {
    // ngspice manual §2.15: `.if` blocks nest; Spectre Reference 19.1 p.491:
    // `if` blocks nest; a `.subckt` may sit in a branch and a branch in a
    // `.subckt`. The reader walks every block on a heap stack, and the
    // writers and the plan's drop walk the branches on one (`read::Walk`;
    // rule 7 of the compiler-construction skill), so this runs in Debug on
    // a test thread's 2 MiB stack, on ngspice, Spectre and VACASK. Checks: 4.
    let n = 10_000;
    let mut spice = String::from("t\n");
    let mut spectre = String::from("t\n");
    let mut mixed = String::from("t\n");
    for k in 0..n {
        spice.push_str(".if (a == 1)\n");
        spectre.push_str("if (a == 1) {\n");
        mixed.push_str(&format!(".subckt s{k} a\n.if (a == 1)\n"));
    }
    spice.push_str("R1 a 0 1\n");
    spectre.push_str("r1 (a 0) resistor r=1\n");
    for _ in 0..n {
        spice.push_str(".endif\n");
        spectre.push_str("}\n");
        mixed.push_str(".endif\n.ends\n");
    }
    for (src, d, head) in [
        (&spice, Dialect::Ngspice, ".if "),
        (&spectre, Dialect::Spectre, "if "),
    ] {
        let w = render_text(src, d, "deep", d).expect("written");
        let ifs = w.text.lines().filter(|l| l.trim_start().starts_with(head));
        assert_eq!(ifs.count(), n, "{d:?}");
    }
    let w = render_text(&mixed, Dialect::Ngspice, "deep", Dialect::Ngspice).expect("written");
    assert_eq!(w.text.matches(".subckt ").count(), n);
    // P462-P509: VACASK's `@if` … `@end`.
    let w = render_text(&spice, Dialect::Ngspice, "deep", Dialect::Vacask).expect("written");
    assert_eq!(
        w.text
            .lines()
            .filter(|l| l.trim_start().starts_with("@if "))
            .count(),
        n
    );
}

#[test]
fn kit_answers_reach_the_written_deck() {
    // Phase 3 (TESTING.md, "Golden gate"): the reader reads the kit where
    // the file alone leaves a master open. top.scs's `x1`, `x2` name `leaf`
    // and `m1` names `nch_mod` (Spectre UG p.66), both in lib.scs, so on
    // ngspice they are an `X` of `leaf` and an `M` of `nch_mod` (each name
    // already starts with its letter, which SPICE reads in any case); ngspice
    // ch.3 `R1 a b 1k rmod`, ch.8 `Q1 c b e mod 1.5` (models in another
    // file) keep their model and area. Checks: 5.
    use crate::syntax::resolve::{FileId, Form, Kit, LoadError};
    use std::path::{Path, PathBuf};
    let written = |files: &[(&str, &str)], d: Dialect| {
        let mut load = |p: &Path| {
            let f = files.iter().find(|(q, _)| Path::new(q) == p);
            f.map(|(_, s)| s.to_string()).ok_or(LoadError::NotFound)
        };
        let k = Kit::load(&[PathBuf::from(files[0].0)], Form::Deck, d, &mut load);
        let w = render_kit(&k, FileId(0), "top", Dialect::Ngspice).expect("written");
        w.text
    };
    let top = "// top\ninclude \"lib.scs\"\nsubckt top (a b)\nx1 (a b) leaf w=1u\nx2 a b leaf w=2u\nm1 (a b 0 0) nch_mod w=1u\nends top\n";
    let lib = "// lib\nsubckt leaf (p n)\nends leaf\nmodel nch_mod bsim4 type=n\n";
    let text = written(&[("/top.scs", top), ("/lib.scs", lib)], Dialect::Spectre);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.contains(&"X1 a b leaf w=1u"), "{text}");
    assert!(lines.contains(&"X2 a b leaf w=2u"), "{text}");
    assert!(lines.contains(&"M1 a b 0 0 nch_mod w=1u"), "{text}");
    let deck = "t\n.include m.sp\nR1 a b 1k rmod\nQ1 c b e mod 1.5\n";
    let models = "* m\n.model rmod r rsh=1\n.model mod npn\n";
    let text = written(&[("/d.sp", deck), ("/m.sp", models)], Dialect::Ngspice);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.contains(&"R1 a b 1k rmod"), "{text}");
    assert!(lines.contains(&"Q1 c b e mod 1.5"), "{text}");
}

#[test]
fn a_log_step_is_spectres_sweep_and_refused_where_no_step_has_one() {
    // Spectre Reference 19.1 p.406-410: `sweep … log=N`, the analyses in
    // its braces; Xyce RG p.155 (`.step` lin, dec, oct, list), LTspice help
    // (`.step` lin, oct, dec, list) and VACASK `docs/cmd-sweep.md` (lin,
    // dec, oct, values) have no total-count log sweep: `W0016`. Checks: 4.
    use crate::syntax::lexicon::NGSPICE_GIVEN;
    use crate::syntax::render::{Numbers, Source, render};
    use crate::syntax::{parse, tokenize};
    // The given deck's lexicon (the Python API's): ngspice has no `.step`.
    let text = "t\nR1 a 0 1k\n.param g=1\n.step log g 1 100 5\n.op\n.end\n";
    let tokens = tokenize(text, &NGSPICE_GIVEN);
    let ast = parse(&tokens, &NGSPICE_GIVEN);
    let src = Source {
        ast: &ast,
        tokens: &tokens,
        dialect: Dialect::Ngspice,
        name: "t",
        kit: None,
        loads: &[],
        numbers: Numbers::Given,
    };
    let w = render(&src, Dialect::Spectre).expect("written");
    let head = "sweep1_g sweep param=g start=1 stop=100 log=5 {";
    assert!(w.text.lines().any(|l| l == head), "{}", w.text);
    for d in [Dialect::Xyce, Dialect::Ltspice, Dialect::Vacask] {
        let refused = render(&src, d).expect_err("refused");
        assert!(
            refused.messages.contains("W0016"),
            "{d:?}: {}",
            refused.messages
        );
    }
}
