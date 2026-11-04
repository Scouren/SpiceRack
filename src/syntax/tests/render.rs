//! The renderer over block nesting, kit answers and numbers: a deck nested
//! 10⁴ deep in `.subckt`s and `.if`s is written in Debug, what
//! `syntax::resolve` answers for a kit file reaches the deck written from
//! it, and a number with each suffix of each reader, inside an expression,
//! reads back as the same value in every target. Each test names its
//! clause, the text derived by hand, and how many checks it makes.
//!
//! Contents: none (tests only).

use crate::syntax::Dialect;
use crate::syntax::lexicon::{self, Scale};
use crate::syntax::render::spelling::spelling;
use crate::syntax::render::{render_kit, render_text};
use crate::syntax::token::Tag as T;
use crate::syntax::tokenize;

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
    // `.subckt`. The reader walks every block on a heap stack (rule 7 of
    // the compiler-construction skill), but the writers recurse over a
    // branch's body (and a plan's `Cond` drops recursively), so these run on
    // a 256 MiB thread, as `golden` does, until the writers walk branches on
    // a heap stack (tests/fixtures/transpiler/PHASE6-STATUS.md, "Left").
    // Checks: 3.
    let big = std::thread::Builder::new().stack_size(256 << 20);
    big.spawn(nested_ifs)
        .expect("spawn")
        .join()
        .expect("written");
}

fn nested_ifs() {
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
fn numbers_in_expressions_read_back_in_every_target() {
    // ngspice manual §2.1.3 (`M` milli, `meg` mega), Spectre UG p.39 (`M`
    // mega, `m` milli), HSPICE (`x` mega), LTspice (`µ` micro), VACASK
    // (`meg` and `M` mega): one reader's suffix is another's other scale,
    // so Spectre `2*1M` written as ngspice `2*1M` is 2 milli. Each source
    // reader's every suffix `s`, in `p<k> = 2*1<s>`, is written for every
    // target, and the target's own reader reads the two numbers after
    // `p<k>=` as 2 and the suffix's factor (its table row, by hand:
    // `10^n`, `mil` 25.4e-6). Structural Verilog writes no parameters.
    // Checks: one per source, suffix and target (75 suffixes, 8 targets:
    // 600).
    let mut checks = 0;
    for d in [
        Dialect::Ngspice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Ltspice,
        Dialect::Spectre,
        Dialect::Vacask,
    ] {
        let scales = lexicon::of(d).expect("a reader").numbers.scales;
        let mut deck = String::from(match d {
            Dialect::Spectre => "// t\nsimulator lang=spectre\n",
            _ => "t\n",
        });
        for (k, (s, _)) in scales.iter().enumerate() {
            let e = format!("2*1{s}");
            deck.push_str(&match d {
                Dialect::Spectre | Dialect::Vacask => format!("parameters p{k}={e}\n"),
                Dialect::Hspice => format!(".param p{k}='{e}'\n"),
                _ => format!(".param p{k}={{{e}}}\n"),
            });
        }
        for target in Dialect::ALL.into_iter().filter(|&t| t != Dialect::Verilog) {
            let w = render_text(&deck, d, "n", target).expect("written");
            let lex = spelling(target).lexicon;
            let t = tokenize(&w.text, lex);
            let tags = t.tags();
            for (k, &(s, scale)) in scales.iter().enumerate() {
                let want = match scale {
                    Scale::Exp(n) => format!("1e{n}").parse::<f64>().expect("a power of ten"),
                    Scale::Mil => 25.4e-6,
                };
                let name = format!("p{k}");
                let at = (0..tags.len() - 1)
                    .find(|&i| {
                        tags[i] == T::Ident
                            && tags[i + 1] == T::Equal
                            && t.text(i as u32).eq_ignore_ascii_case(&name)
                    })
                    .unwrap_or_else(|| panic!("{d:?} -> {target:?}: {name} in\n{}", w.text));
                let got: Vec<f64> = (at + 2..tags.len())
                    .filter(|&i| tags[i] == T::Number)
                    .take(2)
                    .map(|i| t.value(i as u32))
                    .collect();
                let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs();
                assert!(
                    got.len() == 2 && close(got[0], 2.0) && close(got[1], want),
                    "{d:?} `2*1{s}` -> {target:?}: {got:?}, want [2, {want:e}]\n{}",
                    w.text
                );
                checks += 1;
            }
        }
    }
    assert_eq!(checks, 600);
}
