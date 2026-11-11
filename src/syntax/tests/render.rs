//! The renderer over block nesting, kit answers, numbers and a given
//! deck's `.step`: a deck nested 10⁴ deep in `.subckt`s and `.if`s is
//! written in Debug, what `syntax::resolve` answers for a kit file reaches
//! the deck written from it, a number with each suffix of each reader,
//! inside an expression, reads back as the same value in every target, a
//! `.step log` is Spectre's sweep or refused, and a region's Spectre
//! `montecarlo` is Spectre's and VACASK's `mc` or refused, and a `.model`
//! type's master rows hold one row per type and LEVEL. Each test names
//! its clause, the text derived by hand, and how many checks it makes.
//!
//! Contents: none (tests only).

use crate::syntax::Lang;

use crate::syntax::Dialect;
use crate::syntax::lexicon::Scale;
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
    // Spectre Reference 19.1 p.406-410: `sweep … log=N`, N steps (p.407),
    // so the card's five points are `log=4`; the analyses in its braces.
    // Xyce RG p.155 (`.step` lin, dec, oct, list), LTspice help
    // (`.step` lin, oct, dec, list) and VACASK `docs/cmd-sweep.md` (lin,
    // dec, oct, values) have no total-count log sweep: `W0016`. Checks: 4.
    let text = "t\nR1 a 0 1k\n.param g=1\n.step log g 1 100 5\n.op\n.end\n";
    let head = "sweep1_g sweep param=g start=1 stop=100 log=4 {";
    let w = given(text, Dialect::Spectre).expect("written");
    assert!(w.lines().any(|l| l == head), "{w}");
    for d in [Dialect::Xyce, Dialect::Ltspice, Dialect::Vacask] {
        let refused = given(text, d).expect_err("refused");
        assert!(refused.contains("W0016"), "{d:?}: {refused}");
    }
}

#[test]
fn a_region_montecarlo_is_spectres_and_vacasks_mc_or_refused() {
    // Spectre Reference 19.1 p.174 (`montecarlo numruns= seed= { … }`),
    // p.166 (`variations`, default `process`: the top's draws); VACASK
    // `docs/cmd-analysis-mc.md` at codeberg `d7b4db4` (`mc … endmc` around
    // the analyses, every call site drawn; `lh=0` is plain sampling; the
    // VACASK `flake.nix` pins, `bcd48e2`, has no `mc`, so no run checks
    // it); ngspice has no such statement (`W0015`); a draw inside a
    // subcircuit is `mismatch`, which the default leaves out and VACASK
    // would vary: written at its nominal (ZERO-LEDGER P4-020), and refused
    // (`W0115`) where a second montecarlo takes it. Checks: 9.
    let deck = |sub: &str, vary: &str| {
        format!(
            "t\n.subckt s a\nR1 a 0 {sub}\n.ends\nX1 n s\nR2 n 0 {{agauss(1k, 100, 1)}}\n\
             .tran 1u 1m\nsimulator lang=spectre\n\
             mc1 montecarlo numruns=20 seed=42{vary} {{\n  tran1 tran\n}}\nsimulator lang=spice\n.end\n"
        )
    };
    let top = deck("1k", "");
    let spectre = given(&top, Dialect::Spectre).expect("written");
    let mc: Vec<&str> = spectre
        .lines()
        .skip_while(|l| !l.contains("montecarlo"))
        .take(3)
        .collect();
    assert_eq!(
        mc,
        ["mc1 montecarlo numruns=20 seed=42 {", "  tran1 tran", "}"],
        "{spectre}"
    );
    let vacask = given(&top, Dialect::Vacask).expect("written");
    let mc: Vec<&str> = vacask
        .lines()
        .skip_while(|l| !l.contains("mc mc1"))
        .take(3)
        .collect();
    assert_eq!(mc[0], "  mc mc1 samples=20 seed=42 lh=0", "{vacask}");
    assert!(mc[1].starts_with("    analysis tran2 tran "), "{vacask}");
    assert_eq!(mc[2], "  endmc", "{vacask}");
    assert!(
        given(&top, Dialect::Ngspice)
            .expect_err("refused")
            .contains("W0015")
    );
    let mismatch = deck("{agauss(1k, 100, 1)}", "");
    let nominal = given(&mismatch, Dialect::Vacask).expect("written");
    assert!(nominal.contains("r1 (a 0) resistor r=(1k)"), "{nominal}");
    assert!(
        nominal.contains("r2 (n 0) resistor r=agauss(1k, 100, 1)"),
        "{nominal}"
    );
    let all = deck("{agauss(1k, 100, 1)}", " variations=all");
    let all = given(&all, Dialect::Vacask).expect("written");
    assert_eq!(all.matches("r=agauss(1k, 100, 1)").count(), 2, "{all}");
    let second = "mc2 montecarlo numruns=20 variations=all {\ntran2 tran\n}\nsimulator lang=spice";
    let both = mismatch.replace("simulator lang=spice", second);
    assert!(
        given(&both, Dialect::Vacask)
            .expect_err("refused")
            .contains("W0115")
    );
}

#[test]
fn a_region_spectre_analysis_is_written_by_spectre_and_refused_by_cause() {
    // The Python API's Spectre-only analyses (`src/python/analysis.rs`)
    // in a given deck's spectre region: Spectre writes each as read (a
    // `pss` before its `pac`, Spectre Reference 5.0 p.771, p.748); a target
    // with no such analysis refuses it `W0015` (ngspice ch.15 has no
    // stability, harmonic balance or periodic small-signal analysis;
    // ngspice-45 runs no `.pss`), one with it untranslated `W0001` (Xyce
    // RG `.HB`; VACASK `acstb`, 36be560 `docs/cmd-analysis-acstb.md`,
    // ZERO-LEDGER P4-106). A region `sp` over every port is ngspice's
    // `.sp` ([CMP] p.826).
    let deck = |a: &str| {
        format!(
            "t\nV1 a 0 dc 0 ac 1\nR1 a 0 1k\nsimulator lang=spectre\n{a}\nsimulator lang=spice\n.end\n"
        )
    };
    let stb = deck("stb1 stb start=1 stop=1M dec=10 probe=v1");
    let spectre = given(&stb, Dialect::Spectre).expect("written");
    assert!(
        spectre
            .lines()
            .any(|l| l == "stb1 stb start=1 stop=1M dec=10 probe=v1"),
        "{spectre}"
    );
    for t in [Dialect::Ngspice, Dialect::Ltspice] {
        assert!(
            given(&stb, t).expect_err("refused").contains("W0015"),
            "{t:?}"
        );
    }
    assert!(
        given(&stb, Dialect::Vacask)
            .expect_err("refused")
            .contains("W0001")
    );
    let pac = deck(
        "pss1 pss fund=1M tstab=0 harms=5\npac1 pac start=1 stop=1M dec=10 sweeptype=relative",
    );
    let spectre = given(&pac, Dialect::Spectre).expect("written");
    let at = |w: &str| spectre.find(w).unwrap_or_else(|| panic!("{w}\n{spectre}"));
    assert!(at("pss1 pss fund=1M") < at("pac1 pac start=1"), "{spectre}");
    let hb = deck("hb1 hb fundfreqs=[1M] maxharms=[5]");
    assert!(
        given(&hb, Dialect::Ngspice)
            .expect_err("refused")
            .contains("W0015")
    );
    assert!(
        given(&hb, Dialect::Xyce)
            .expect_err("refused")
            .contains("W0001")
    );
    let sp = deck("sp1 sp start=1k stop=10k dec=2");
    let ng = given(&sp, Dialect::Ngspice).expect("written");
    assert!(ng.lines().any(|l| l == ".sp dec 2 1000 10000"), "{ng}");
}

#[test]
fn numbers_in_expressions_read_back_in_every_target() {
    // ngspice manual §2.1.3 (`M` milli, `meg` mega), Spectre (`M` mega,
    // `m` milli: `lexicon::spectre`'s §2.4), HSPICE (`x` mega), LTspice
    // (`µ` micro), VACASK (`meg` and `M` mega; each lexicon's table cites
    // its manual): one reader's suffix is another's other scale,
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
        let scales = Lang::from(d).lexicon().expect("a reader").numbers.scales;
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
            let lex = Lang::from(target).spelling().lexicon;
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

/// `text` read as the given deck (`Lang::NGSPICE_GIVEN`, the Python
/// API's: ngspice has no `.step`, and its regions hold other dialects)
/// and written for `target`: the text, or the refusal's messages.
fn given(text: &str, target: Dialect) -> Result<String, String> {
    use crate::syntax::render::{Numbers, Source, render};
    use crate::syntax::{parse, tokenize};
    let tokens = tokenize(text, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let ast = parse(&tokens, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let src = Source {
        ast: &ast,
        tokens: &tokens,
        lang: Lang::NGSPICE,
        name: "t",
        kit: None,
        loads: &[],
        numbers: Numbers::Given,
    };
    render(&src, target).map(|w| w.text).map_err(|d| d.messages)
}

#[test]
fn a_model_type_and_level_has_one_master_row_of_a_spice_type() {
    // The master rows' own contract (`DialectDef::model_master`), in every
    // built-in: no (type, LEVEL) in two rows, each row's type one the
    // target reads as SPICE's. Then the three answers against the masters
    // the old matches gave (`[REF]` 19.1 p.498; VACASK 36be560
    // `devices/spice/*.va`): BSIM4 at LEVEL 54, a level-6 MOSFET only on
    // VACASK, a line type passed through on VACASK only. Checks: 8 plus
    // two per row.
    use crate::syntax::behaviour::MasterAnswer as A;
    for d in Dialect::ALL {
        let b = Lang::from(d).behaviours();
        for (i, m) in b.model_masters.iter().enumerate() {
            let twice = b.model_masters[i + 1..]
                .iter()
                .any(|n| n.kind == m.kind && n.levels.iter().any(|l| m.levels.contains(l)));
            assert!(!twice, "{d:?}: {} at a LEVEL twice", m.kind);
            assert!(b.spice_model_types.contains(&m.kind), "{d:?}: {}", m.kind);
        }
    }
    let (sp, va) = (Lang::SPECTRE.behaviours(), Lang::VACASK.behaviours());
    let master = |a: A| match a {
        A::Master(m) => Some((m.master, m.type_)),
        _ => None,
    };
    assert_eq!(
        master(sp.model_master("nmos", 54)),
        Some(("bsim4", Some("n")))
    );
    assert_eq!(
        master(va.model_master("pmos", 54)),
        Some(("bsim4v8", Some("-1")))
    );
    assert_eq!(sp.model_master("nmos", 6), A::Refused);
    assert_eq!(
        master(va.model_master("nmos", 6)),
        Some(("mos6", Some("1")))
    );
    assert_eq!(sp.model_master("ltra", 1), A::Refused);
    assert_eq!(va.model_master("ltra", 1), A::AsWritten);
    assert_eq!(sp.model_master("bsim4", 1), A::AsWritten);
    assert_eq!(va.model_master("res", 1), A::Refused);
}
