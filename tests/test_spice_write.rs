//! The SPICE-family writers' conformance gaps (`render/spice/`), each
//! against an oracle the writer does not compute: real ngspice running the
//! written deck to a closed form, or the target manual's clause.
//!
//! Unsupported fails, never skips: no ngspice on PATH fails the run tests
//! (`nix develop .#testing`).
//!
//! Cites: per test.
//!
//! Contents: none (tests only).
#![cfg(feature = "backend")]

use spicerack::backend::Dialect;
use spicerack::syntax::render::render_text;

// ---- private ----

/// The deck `text` read as `from`, written for `to`; panics on a refusal.
fn write(text: &str, from: Dialect, to: Dialect) -> String {
    render_text(text, from, "t", to)
        .unwrap_or_else(|e| panic!("{}\n{text}", e.messages))
        .text
}

/// `deck` run on real ngspice: the operating point's `v(name)`.
fn ngspice_op(deck: &str, name: &str) -> f64 {
    let raw = Dialect::Ngspice
        .run(deck)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{deck}"));
    let want = format!("v({name})");
    let i = raw
        .variables
        .iter()
        .position(|v| v.name.eq_ignore_ascii_case(&want) || v.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no {want} in {:?}", raw.variables));
    raw.real_data[i][0]
}

/// A file in the temporary directory holding `text`: its path.
fn scratch(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("spice-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

// ── LTspice `.lib file` (no section) ──

/// LTspice help (`.lib`): `.lib file` with no section reads the whole file.
/// ngspice §2.7 takes `.lib file section` only (ngspice-45 stops on the
/// sectionless card: "unimplemented dot command '.lib'"); §2.6 `.include
/// file` reads the whole file. Real ngspice on the written deck: the
/// divider 1 V · 3k/(1k+3k) = 0.75 V, so the file was read.
#[test]
fn an_ltspice_whole_file_lib_is_an_include_on_the_spice_targets() {
    let lib = scratch("half.lib", "R2 out 0 3k\n");
    let deck = format!(
        "* t\nV1 in 0 1\nR1 in out 1k\n.lib {}\n.op\n.end\n",
        lib.display()
    );
    let ng = write(&deck, Dialect::Ltspice, Dialect::Ngspice);
    let v = ngspice_op(&ng, "out");
    assert!((v - 0.75).abs() < 1e-9, "v(out) = {v}\n{ng}");
    // HSPICE `.INCLUDE`, Xyce RG `.INCLUDE`: the same card.
    for to in [Dialect::Espice, Dialect::Hspice, Dialect::Xyce] {
        let out = write(&deck, Dialect::Ltspice, to);
        let card = out.lines().find(|l| l.contains("half.lib")).unwrap();
        assert!(card.starts_with(".include "), "{to:?}: {card}");
    }
    // A `.lib file section` stays a section call.
    let sec = deck.replace("half.lib\n", "half.lib tt\n");
    let ng = write(&sec, Dialect::Ltspice, Dialect::Ngspice);
    assert!(ng.contains("half.lib tt\n"), "{ng}");
}

