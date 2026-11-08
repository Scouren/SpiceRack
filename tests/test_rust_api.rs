//! The Rust public API's documented behaviours, one test per item the
//! ZERO-LEDGER P1-038 audit found untested: `render` on a hooked target,
//! `render::writes`, `Numbers::Given` against `Numbers::Read`,
//! `import::dump`, and `import::kit`'s root confinement.
//!
//! Oracles, named in each test: the item's doc (a spec example); a round
//! trip through the reader (`Numbers::Read`); the file system the test
//! builds (the kit's confinement).
//!
//! Contents: none (tests only).

use std::path::PathBuf;

use spicerack::import;
use spicerack::syntax::def::{DialectDef, register};
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::{Numbers, Source, render, writes};
use spicerack::syntax::resolve::{FileId, Form};
use spicerack::syntax::{Dialect, Lang, parse, tokenize};

// ---- private ----

/// `text` read as ngspice and written as ngspice with `numbers`: R1's
/// value as written.
fn r1_value(text: &str, numbers: Numbers) -> String {
    let lex = Lang::NGSPICE.lexicon().expect("a reader");
    let t = tokenize(text, lex);
    let ast = parse(&t, lex);
    let src = Source {
        ast: &ast,
        tokens: &t,
        lang: Lang::NGSPICE,
        name: "t",
        kit: None,
        loads: &[],
        numbers,
    };
    let w = render(&src, Dialect::Ngspice).expect("written");
    let line = w.text.lines().find(|l| l.starts_with("R1 ")).expect("R1");
    line.rsplit(' ').next().expect("a value").to_string()
}

/// The value ngspice's reader gives the number `v`.
fn number(v: &str) -> f64 {
    let line = format!("t\n{v}\n");
    let t = tokenize(&line, Lang::NGSPICE.lexicon().expect("a reader"));
    let k = (0..t.len() as u32)
        .find(|&k| t.tags()[k as usize] == spicerack::syntax::token::Tag::Number)
        .expect("a number");
    t.value(k)
}

/// A fresh directory under the target dir for one test's files.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("root")).expect("a scratch dir");
    dir
}

#[test]
#[should_panic(expected = "render_pending")]
fn render_on_a_target_with_an_external_hook_is_a_caller_bug() {
    // `render`'s # Panics: a target that declares an external hook is
    // written by `render_pending` + `splice`; `render` asserts. The
    // binding calls `render_pending` (python::def_::render); no other
    // caller hands `render` a plugin target. Checks: 1.
    let hooked = register(
        DialectDef::new("api-hooked")
            .extends(Lang::NGSPICE)
            .external_hook(".tran", 1),
    )
    .expect("registers");
    let text = "t\nV1 a 0 1\nR1 a 0 1k\n.tran 1n 1u\n.end\n";
    let lex = Lang::NGSPICE.lexicon().expect("a reader");
    let t = tokenize(text, lex);
    let ast = parse(&t, lex);
    let src = Source {
        ast: &ast,
        tokens: &t,
        lang: Lang::NGSPICE,
        name: "t",
        kit: None,
        loads: &[],
        numbers: Numbers::Read,
    };
    let _ = render(&src, hooked);
}

#[test]
fn every_built_in_dialect_is_written() {
    // `writes`' doc: true for every built-in Dialect today. Checks: 1 per
    // dialect.
    for d in Dialect::ALL {
        assert!(writes(d), "{d:?}");
    }
}

#[test]
fn given_numbers_are_si_and_read_numbers_read_back_as_read() {
    // `Numbers::Given`: in SI form as given (`1e-4` → `100u`).
    // `Numbers::Read`: spelled so ngspice reads back the double the
    // file's reading gave: the written value, read again, is the same
    // f64. Checks: 1 + 3.
    assert_eq!(r1_value("t\nR1 a 0 1e-4\n.end\n", Numbers::Given), "100u");
    for v in ["1e-4", "0.3", "100u"] {
        let text = format!("t\nR1 a 0 {v}\n.end\n");
        let written = r1_value(&text, Numbers::Read);
        assert_eq!(
            number(v).to_bits(),
            number(&written).to_bits(),
            "{v} → {written}"
        );
    }
}

#[test]
fn dump_lists_each_statement_and_the_parse_notes() {
    // `import::dump`'s doc: one row per statement and item, indented by
    // depth, its tag and anchor text; then each diagnostic at line:col.
    // A `.subckt` left open is P0003 (diag catalogue). Checks: 3 + 1 + 1.
    let out = import::dump("t\nR1 a 0 1k\n.subckt s p\n", Dialect::Ngspice).expect("a reader");
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.contains(&"Instance \"R1\""), "{out}");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("  ") && l.ends_with("\"1k\"")),
        "{out}"
    );
    assert!(lines.contains(&"Subckt \".subckt\""), "{out}");
    assert!(out.lines().any(|l| l.starts_with("P0003 at 3:")), "{out}");
    assert!(import::dump("t\n", Dialect::Cdl).is_err());
}

#[test]
fn a_kit_refuses_a_decks_path_out_of_its_root_and_follows_its_own_symlinks() {
    // `import::kit`'s doc: the confinement is lexical, after `..` is
    // resolved: a deck's `../` path out of the root is refused (R0007);
    // a symlink inside the root is part of the root (its installer put it
    // there) and is followed even when it points out. Checks: 2 + 2.
    let dir = scratch("kit_confinement");
    let root = dir.join("root");
    std::fs::write(dir.join("out.sp"), "R9 x 0 1k\n").expect("write");
    std::fs::write(root.join("escape.sp"), "t\n.include ../out.sp\n.end\n").expect("write");
    let kit = import::kit(
        &[root.join("escape.sp")],
        Form::Deck,
        Dialect::Ngspice,
        &root,
    );
    let codes: Vec<Code> = kit.file(FileId(0)).diags.iter().map(|d| d.code).collect();
    assert!(codes.contains(&Code::R0007), "{codes:?}");
    assert!(!kit.files().iter().any(|f| f.text.contains("R9 x 0 1k")));

    std::os::unix::fs::symlink(dir.join("out.sp"), root.join("link.sp")).expect("symlink");
    std::fs::write(root.join("linked.sp"), "t\n.include link.sp\n.end\n").expect("write");
    let kit = import::kit(
        &[root.join("linked.sp")],
        Form::Deck,
        Dialect::Ngspice,
        &root,
    );
    let codes: Vec<Code> = kit.file(FileId(0)).diags.iter().map(|d| d.code).collect();
    assert!(
        !codes.contains(&Code::R0007) && !codes.contains(&Code::R0006),
        "{codes:?}"
    );
    assert!(kit.files().iter().any(|f| f.text.contains("R9 x 0 1k")));
}
