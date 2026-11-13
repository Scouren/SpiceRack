//! `Sources::load`'s walk and `Reference::resolve`.
//!
//! Oracles: a hand-built include graph with its expected breadth-first order; paths resolved
//! by hand.
//!
//! Contents: tests only.

use std::collections::HashMap;

use spicerack::dialect::{self, Reference, Sources, irtext::IrText};
use spicerack::ir;

// ---- private ----

#[test]
fn sources_load_walks_references_breadth_first_once_each() {
    // deck -> a, b; a -> b, sub/c; sub/c -> ../a (seen), d (missing); b names a Verilog-A file.
    let files: HashMap<&str, &str> = HashMap::from([
        (
            "top/deck.ir",
            "file \"deck\" deck\n  include \"a.ir\"\n  include \"./b.ir\"\n",
        ),
        ("top/a.ir", "  include \"b.ir\"\n  include \"sub/c.ir\"\n"),
        ("top/b.ir", "  load veriloga \"r.va\"\n"),
        (
            "top/sub/c.ir",
            "  include \"../a.ir\"\n  include \"d.ir\"\n",
        ),
    ]);
    let mut asked = Vec::new();
    let mut loader = |p: &str| {
        asked.push(p.to_string());
        files.get(p).map(|t| t.to_string())
    };
    let src = Sources::load(&["top/deck.ir"], ir::Role::Deck, &IrText, &mut loader);
    let paths: Vec<&str> = src.files().iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        paths,
        ["top/deck.ir", "top/a.ir", "top/b.ir", "top/sub/c.ir"]
    );
    assert_eq!(src.entries().len(), 1);
    assert_eq!(src.form(), ir::Role::Deck);
    assert_eq!(asked.last().map(String::as_str), Some("top/sub/d.ir"));
    assert!(
        !asked.iter().any(|p| p.ends_with("r.va")),
        "Verilog-A is not followed"
    );
}

#[test]
fn sources_load_stops_at_the_depth_cap() {
    // f0 -> f1 -> ... : DEPTH_MAX + 1 files are read (depths 0..=DEPTH_MAX).
    let n = usize::from(dialect::DEPTH_MAX) + 20;
    let texts: Vec<String> = (0..n)
        .map(|i| format!("  include \"f{}.ir\"\n", i + 1))
        .collect();
    let mut loader = |p: &str| {
        let i: usize = p.strip_prefix('f')?.strip_suffix(".ir")?.parse().ok()?;
        texts.get(i).cloned()
    };
    let src = Sources::load(&["f0.ir"], ir::Role::Library, &IrText, &mut loader);
    assert_eq!(src.files().len(), usize::from(dialect::DEPTH_MAX) + 1);
    assert_eq!(src.form(), ir::Role::Library);
}

#[test]
fn a_reference_resolves_beside_its_file() {
    let r = Reference::File("../m/x.lib".into());
    assert_eq!(r.resolve("kit/deck/a.sp"), "kit/m/x.lib");
    assert_eq!(
        Reference::File("/abs/y.lib".into()).resolve("kit/a.sp"),
        "/abs/y.lib"
    );
    assert_eq!(Reference::VerilogA("r.va".into()).resolve("a.sp"), "r.va");
}

#[test]
fn a_parent_reference_above_the_root_is_a_missing_file_never_a_sibling() {
    // The pair agent's finding: `..` above the root was dropped, so `../lib.ir` from the root's
    // `deck.ir` became the unrelated `lib.ir` beside it. It is a path no root file answers.
    let files: HashMap<&str, &str> = HashMap::from([
        ("deck.ir", "file \"deck\" deck\n  include \"../lib.ir\"\n"),
        ("lib.ir", "  param wrong = 1\n"),
    ]);
    let mut asked = Vec::new();
    let mut loader = |p: &str| {
        asked.push(p.to_string());
        files.get(p).map(|t| t.to_string())
    };
    let src = Sources::load(&["deck.ir"], ir::Role::Deck, &IrText, &mut loader);
    let paths: Vec<&str> = src.files().iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(paths, ["deck.ir"], "the sibling is not taken");
    assert_eq!(asked, ["deck.ir", "../lib.ir"]);
    assert_eq!(Reference::File("../../x".into()).resolve("a/b.sp"), "../x");
}

#[test]
fn a_written_path_is_spelled_from_the_including_files_directory() {
    // R78: the text a writer spells for an included file, from the including file's key and
    // the included one's; each expected value written by hand.
    let cases: &[(&str, &str, Option<&str>)] = &[
        ("deck.sp", "lib.inc", Some("lib.inc")),
        ("a/deck.sp", "a/lib.inc", Some("lib.inc")),
        ("deck.sp", "models/lib.inc", Some("models/lib.inc")),
        ("a/b/deck.sp", "a/lib.inc", Some("../lib.inc")),
        ("a/b/deck.sp", "c/d/lib.inc", Some("../../c/d/lib.inc")),
        ("a/deck.sp", "a/deck.sp", Some("deck.sp")),
        (
            "demo/spice/inv.sim",
            "demo/spice/bsim3v3.inc",
            Some("bsim3v3.inc"),
        ),
        // a key above the root (the loader keeps its `..`) or an absolute one: not stated
        ("a/deck.sp", "../lib.inc", None),
        ("a/deck.sp", "/abs/lib.inc", None),
        ("/abs/deck.sp", "a/lib.inc", None),
    ];
    for &(from, to, want) in cases {
        assert_eq!(
            dialect::relative_path(from, to).as_deref(),
            want,
            "{from} -> {to}"
        );
    }
}

#[test]
fn each_writer_spells_an_include_from_the_including_files_directory() {
    // R78: `a/deck`'s include of the key `a/lib.inc` is `lib.inc` in every writer, and a key
    // above the root is written as read with the spine's note.
    let text = "file \"a/deck.sp\" deck\n  title \"t\"\n  include \"a/lib.inc\"\n  \
                include \"../up.inc\"\nfile \"a/lib.inc\" library\n  comment \"c\"\n\
                file \"../up.inc\" library\n  comment \"u\"\n";
    for (row, want) in [("ngspice", "\"lib.inc\""), ("vacask", "\"lib.inc\"")] {
        let to = dialect::by_name(row).expect("the row");
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        let deck = design.entry();
        let out = dialect::write(&design, deck, to, &mut diags);
        assert!(out.contains(want), "{row}:\n{out}");
        assert!(!out.contains("a/lib.inc"), "{row}:\n{out}");
        assert!(out.contains("../up.inc"), "{row}:\n{out}");
        let notes: Vec<ir::Code> = diags.rows().iter().map(|d| d.code).collect();
        assert_eq!(notes, [dialect::PATH_AS_READ], "{row}");
    }
}
