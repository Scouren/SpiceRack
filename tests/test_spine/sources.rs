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
