//! The census law (R87): a deck ngspice-45 runs clean has no statement the ngspice reader calls
//! invalid in its source (an `InvalidInSource` opaque). The decks are spice-bench's (`SPICE_BENCH`,
//! set by `nix develop .#testing`) that the cross-sim source list does not name
//! (`tests/fixtures/cross_sim/spice-bench-source.txt`: a deck ngspice fails, or one quarantined
//! as slow, has a row there), filtered as the pair census filters them.
//!
//! A deck whose folder holds a `.spiceinit` setting `ngbehavior` runs under a compatibility mode,
//! another reading of the text (R87: one refusal at the deck, "no rule yet", once the loader
//! hands the reader that file; settle `spiceinit/`: ngspice reads the deck folder's file). Until
//! then those decks are named in [`KNOWN_MODE`], strictly both ways, and their statements are
//! not counted. Every other `(deck, code)` the census finds is compared with [`KNOWN_INVALID`],
//! strictly both ways.
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

/// The decks under a compatibility mode (a `.spiceinit` with `ngbehavior` in their folder): R87's
/// interim, until the loader passes the file and the reader refuses the deck once.
#[rustfmt::skip] // a table: one row per line
const KNOWN_MODE: &[&str] = &[
    "ngspice/examples/ddt/ddt-test1.sp",
    "ngspice/examples/ddt/ddt-test2.sp",
    "ngspice/examples/ddt/ddt-test3.sp",
    "ngspice/examples/optran/F5TurboV2thermal-optran.sp",
    "ngspice/examples/optran/TLV6001-test.sp",
    "ngspice/examples/optran/TLV9002-test.sp",
    "ngspice/examples/p-to-n-examples/555-timer-2.sp",
    "ngspice/examples/p-to-n-examples/MOS1_out.sp",
    "ngspice/examples/p-to-n-examples/OP_MCP6041.sp",
    "ngspice/examples/p-to-n-examples/logic.sp",
    "ngspice/examples/p-to-n-examples/op-test-adi.sp",
    "ngspice/examples/p-to-n-examples/op-test.sp",
    "ngspice/examples/p-to-n-examples/switch-oscillators.sp",
    "ngspice/examples/p-to-n-examples/switch-oscillators_inc.sp",
    "ngspice/examples/probe/555-timer-2.sp",
    "ngspice/examples/probe/Dual-NMOS-amp.sp",
    "ngspice/examples/probe/F5TurboV2-Probe.sp",
    "ngspice/examples/probe/ac-test.sp",
    "ngspice/examples/probe/cntrl-sources.sp",
    "ngspice/examples/probe/mos-test.sp",
    "ngspice/examples/probe/probe-i-dev.sp",
    "ngspice/examples/probe/switches-manual-p92.sp",
    "ngspice/examples/soa/bjt-soa.sp",
    "ngspice/examples/soa/diode-soa-sh.sp",
    "ngspice/examples/soa/diode-soa-simple.sp",
    "ngspice/examples/soa/nic_soa.sp",
    "ngspice/kicad/sallen_key-bandpass-072/sallen_key_bp.sp",
    "ngspice/paranoia/optran/F5TurboV2thermal-ic-out.sp",
    "ngspice/paranoia/optran/TLV6001-test.sp",
    "ngspice/paranoia/optran/TLV9002-test.sp",
    "ngspice/paranoia/optran/simple-tran.sp",
    "ngspice/paranoia/pton/555-timer-2.sp",
    "ngspice/paranoia/pton/MOS1_out.sp",
    "ngspice/paranoia/pton/OP_MCP6041.sp",
    "ngspice/paranoia/pton/Optimos_out.sp",
    "ngspice/paranoia/pton/op-test-adi.sp",
    "ngspice/paranoia/pton/op-test-test.sp",
    "ngspice/paranoia/pton/op-test.sp",
    "ngspice/paranoia/pton/relax_osc_st.sp",
    "ngspice/paranoia/pton/switch-oscillators.sp",
    "ngspice/paranoia/pton/switch-oscillators_inc.sp",
];

/// `(deck, code)` of the default-mode decks that run clean and still hold a statement read as
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
    let (mut modes, mut found, mut lines) = (Vec::new(), BTreeSet::new(), Vec::new());
    for deck in &decks {
        let name = rel(deck);
        if mode(deck) {
            modes.push(name);
            continue;
        }
        let src = Sources::load(&[&name], ir::Role::Deck, reader, &mut |f| {
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
        "census: {} decks run clean, {} under a mode, {} (deck, code) rows",
        decks.len(),
        modes.len(),
        found.len()
    );
    let want_modes: Vec<&str> = KNOWN_MODE.to_vec();
    let got_modes: Vec<&str> = modes.iter().map(String::as_str).collect();
    assert_eq!(
        got_modes, want_modes,
        "decks under a compatibility mode against KNOWN_MODE"
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

/// Whether `deck`'s folder holds a `.spiceinit` that sets `ngbehavior` (settle `spiceinit/`).
fn mode(deck: &Path) -> bool {
    let init = deck.with_file_name(".spiceinit");
    std::fs::read_to_string(init).is_ok_and(|t| t.contains("ngbehavior"))
}
