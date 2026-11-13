//! The decks that run under a compatibility mode (R87): read with their folder's `.spiceinit`
//! (`Sources::load`, R87a), every deck whose plain run differs from its run under the mode holds
//! a statement the reader refuses for the mode or the start-up file (`P1016` ps, `P1017` lt,
//! `P1018` a start-up command), and no deck holds a statement read as invalid.
//!
//! Oracle: ngspice-45 itself, each deck run with and without its `.spiceinit`
//! (`tests/fixtures/dialect/ngspice/mode_decks.txt`, provenance in its header): the deck's mode
//! changes its numbers or not. A missing corpus fails, never skips. Built with `--features
//! corpus`.
//!
//! Contents: tests only.

#![cfg(feature = "corpus")]

#[path = "common/memcap.rs"]
mod memcap;

use std::path::PathBuf;

use spicerack::dialect::{self, Sources};
use spicerack::ir;

/// The codes that name a start-up file's cause.
const MODE_CODES: [&str; 3] = ["P1016", "P1017", "P1018"];

// ---- private ----

#[test]
fn a_deck_its_mode_changes_holds_a_refusal_naming_the_mode() {
    let root = PathBuf::from(
        std::env::var_os("SPICE_BENCH")
            .expect("SPICE_BENCH is set by `nix develop .#testing` (unsupported fails)"),
    );
    let table = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/dialect/ngspice/mode_decks.txt"
    ))
    .expect("the run table");
    let reader = dialect::by_name("ngspice")
        .and_then(|r| r.read)
        .expect("ngspice reads");
    let (mut unflagged, mut invalid, mut no_init) = (Vec::new(), Vec::new(), Vec::new());
    let rows: Vec<(&str, &str)> = table
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once(' '))
        .collect();
    assert_eq!(rows.len(), 41, "the run table names the 41 mode decks");
    for (deck, verdict) in rows {
        let path = root.join(deck).to_string_lossy().into_owned();
        let src = Sources::load(&[&path], ir::Role::Deck, reader, None, &mut |f| {
            std::fs::read(f)
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        });
        if src.environment().is_none() {
            no_init.push(deck);
        }
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        reader.read(&src, &mut design.builder(), &mut diags);
        let codes: Vec<String> = diags.rows().iter().map(|d| d.code.to_string()).collect();
        if verdict == "differs" && !codes.iter().any(|c| MODE_CODES.contains(&c.as_str())) {
            unflagged.push(deck);
        }
        let bad = diags.rows().iter().any(|d| {
            d.item.kind == ir::ItemKind::Opaque
                && d.item.index != u32::MAX
                && design.row::<ir::item::Opaque>(d.item.index).class
                    == ir::OpaqueClass::InvalidInSource
        });
        if bad {
            invalid.push(deck);
        }
    }
    assert!(no_init.is_empty(), "no .spiceinit found: {no_init:?}");
    assert!(
        unflagged.is_empty(),
        "decks their mode changes, read with no mode refusal: {unflagged:?}"
    );
    assert!(
        invalid.is_empty(),
        "statements read as invalid: {invalid:?}"
    );
}
