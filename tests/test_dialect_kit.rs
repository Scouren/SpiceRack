//! A kit read in a registered plugin's language, and a built-in reached
//! through the registry's region fallback: the two paths that once took a
//! `Dialect` or skipped the built-ins (DIALECT-PLUGIN.md, "Done").
//!
//! Cites: DIALECT-PLUGIN.md §6.5 (regions), §7.1 (`Lang`).
//!
//! Contents: none (tests only).

#[path = "common/acme.rs"]
mod acme;

use std::path::{Path, PathBuf};

use acme::acme_as;
use spicerack::syntax::def::register;
use spicerack::syntax::render::render_kit;
use spicerack::syntax::resolve::{FileId, Form, Kit};
use spicerack::syntax::{Lang, tokenize};

// ---- private ----

/// Oracle: acme's own text (`#` comments, `.subcircuit`, `x` mega) read
/// through a kit in acme reads as the same deck written in ngspice: the
/// subcircuit and the 2-mega resistor survive, the comment does not.
#[test]
fn a_kit_reads_a_plugins_text() {
    let acme = register(acme_as("acme-kit")).expect("registers");
    let text = "* t\n# a comment\n.subcircuit s a b\nR1 a b 2x\n.endsub\nX1 n 0 s\n.end\n";
    let mut load = |_: &Path| Ok(text.to_string());
    let kit = Kit::load(&[PathBuf::from("top.acme")], Form::Deck, acme, &mut load);
    assert_eq!(kit.lang(), acme);
    let w = render_kit(&kit, FileId(0), "top", Lang::NGSPICE).expect("written");
    assert!(w.text.contains(".subckt s a b"), "{}", w.text);
    assert!(w.text.contains("R1 a b 2meg"), "{}", w.text);
    assert!(!w.text.contains("a comment"), "{}", w.text);
}

/// Oracle: §6.5: in a given deck, `simulator lang=espice` (a built-in its
/// own region rows do not list) opens a region read by ESPice's lexicon,
/// as a plugin's name does; before, the fallback skipped the built-ins.
#[test]
fn a_built_in_without_a_region_row_is_found_in_the_registry() {
    let given = Lang::NGSPICE_GIVEN.lexicon().expect("a reader");
    let espice = Lang::ESPICE.lexicon().expect("a reader");
    let deck =
        "* t\nV1 in 0 1\nsimulator lang=espice\nR1 in 0 1k\nsimulator lang=spice\n.op\n.end\n";
    let t = tokenize(deck, given);
    assert!(
        (0..t.len() as u32).any(|k| std::ptr::eq(t.lexicon_at(k), espice)),
        "a token read by ESPice's lexicon"
    );
}
