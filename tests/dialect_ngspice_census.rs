//! The census law (R87): a deck ngspice-45 runs clean has no statement the ngspice reader calls
//! invalid in its source (an `InvalidInSource` opaque). The decks are spice-bench's (`SPICE_BENCH`,
//! set by `nix develop .#testing`) that the cross-sim source list does not name
//! (`tests/fixtures/cross_sim/spice-bench-source.txt`: a deck ngspice fails, or one quarantined
//! as slow, has a row there), filtered as the pair census filters them.
//!
//! A deck whose folder holds a `.spiceinit` is read with it (`Sources::load`, R87a): a statement
//! its compatibility mode rewrites is `Unread` with a code naming the mode (`mode.rs`, R87), never
//! invalid, so the law holds for those decks with no exception list
//! (`tests/dialect_ngspice_modes.rs` checks them against their runs). Every `(deck, code)` the
//! census finds is compared with [`KNOWN_INVALID`], strictly both ways.
//!
//! Oracle: ngspice-45's own checked run of each deck (the cross-sim harness's source list). A
//! missing corpus fails, never skips. Nightly tier: built with `--features corpus`, in release
//! (`cargo test --release`: 1520 s for 4102 decks on 2026-11-13; a debug build reads about 5 s
//! per cmcqa deck, whose includes load whole PDK libraries).
//!
//! Contents: tests only.

#![cfg(feature = "corpus")]

#[path = "common/memcap.rs"]
mod memcap;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use spicerack::dialect::{self, Sources};
use spicerack::ir;

/// `(deck, code)` of the decks that run clean and still hold a statement read as
/// invalid, each with its cause. Strict both ways.
#[rustfmt::skip] // a table: one row per line
const KNOWN_INVALID: &[(&str, &str, &str)] = &[
];

/// The corpus's decks at most: a bound on the walk.
const DECKS_MAX: usize = 16_384;

// ---- private ----

#[test]
fn no_deck_ngspice_runs_clean_holds_a_statement_read_as_invalid() {
    let root = PathBuf::from(
        std::env::var_os("SPICE_BENCH")
            .expect("SPICE_BENCH is set by `nix develop .#testing` (unsupported fails)"),
    );
    let listed = listed();
    let mut decks = Vec::new();
    walk(&root, &mut decks);
    let rel = |p: &Path| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .into_owned()
    };
    decks.retain(|p| {
        let s = p.to_string_lossy();
        !s.contains("/src") && !s.contains("/.") && !listed.contains(&rel(p))
    });
    decks.sort();
    assert!(
        !decks.is_empty(),
        "{}: no deck (unsupported fails)",
        root.display()
    );
    let row = dialect::by_name("ngspice").expect("the ngspice row");
    let reader = row.read.expect("ngspice reads");
    let (mut found, mut lines) = (BTreeSet::new(), Vec::new());
    for deck in &decks {
        let name = rel(deck);
        let src = Sources::load(&[&name], ir::Role::Deck, reader, None, &mut |f| {
            std::fs::read(root.join(f))
                .ok()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        });
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        reader.read(&src, &mut design.builder(), &mut diags);
        for d in diags.rows() {
            if d.item.kind != ir::ItemKind::Opaque || d.item.index == u32::MAX {
                continue;
            }
            let o: ir::item::Opaque = design.row(d.item.index);
            if o.class == ir::OpaqueClass::InvalidInSource
                && found.insert((name.clone(), d.code.to_string()))
            {
                lines.push(format!("{name} {}: {}", d.code, design.text(o.text)));
            }
        }
    }
    println!(
        "census: {} decks run clean, {} (deck, code) rows",
        decks.len(),
        found.len()
    );
    let got: Vec<(&str, &str)> = found
        .iter()
        .map(|(d, c)| (d.as_str(), c.as_str()))
        .collect();
    let want: Vec<(&str, &str)> = KNOWN_INVALID.iter().map(|(d, c, _)| (*d, *c)).collect();
    assert_eq!(
        got,
        want,
        "invalid statements against KNOWN_INVALID\n{}",
        lines.join("\n")
    );
}

/// Every deck the cross-sim source list names (any class).
fn listed() -> BTreeSet<String> {
    let path = format!(
        "{}/tests/fixtures/cross_sim/spice-bench-source.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines()
        .filter_map(|l| l.split('#').next()?.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// Every `.sp` file under `root`.
fn walk(root: &Path, out: &mut Vec<PathBuf>) {
    let mut stack = vec![root.to_path_buf()];
    // Bounded: each directory is entered once, and the decks are capped.
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "sp") {
                out.push(p);
            }
        }
        assert!(
            out.len() <= DECKS_MAX,
            "the corpus has at most DECKS_MAX decks"
        );
    }
}
