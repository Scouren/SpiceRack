//! The Spectre dialect against its only oracle: **no Spectre runs on this machine**, so every
//! fact rests on the recorded Spectre 25.1 session of 2026-11-07 (RUN-SP,
//! `rewrite/upstream/records/settle-remote.md` on branch `differences`), which ran every
//! `tests/spectre/*.scs` deck unchanged (their outcomes: `tests/spectre/pending.json`,
//! `expected.json`), or on a manual page or the old pipeline's code, each rule saying which
//! (`src/dialect/spectre/`).
//!
//! - Each recorded deck (copied unchanged to `tests/fixtures/dialect/spectre/read/<deck>.scs`)
//!   reads to the IR in `tests/fixtures/dialect/spectre/read/<deck>.ir`,
//!   with the codes its `# codes:` line lists. The IR text was checked by hand against the deck
//!   and the values its run printed (the RUN-SP status lines); `SPICERACK_BLESS=1` rewrites the
//!   files from the reader, for a reviewed diff.
//! - Each recorded deck is a fixpoint of read → write → read (the same-dialect cell, text level:
//!   no simulator), and its read uses only `FEATURES` (the reader's image is the writer's set).
//! - Cross: each SPICE source the recorded decks were written from (`tests/test_pending_real_run.rs`),
//!   read as ngspice and written as Spectre, reads back to the meaning of the deck Spectre ran
//!   to its pinned numbers (titles, comments and inert lines aside), or is a [`KNOWN_CROSS`] row.
//!
//! Lists are compared by name, both ways.
//!
//! Contents: tests only.

use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// The recorded decks whose SPICE source does not convert to the recorded deck's meaning, with
/// why: (deck, why). Strict both ways.
const KNOWN_CROSS: &[(&str, &str)] = &[
    (
        "p_branch_param",
        "ngspice reads no `.param` in an `.if` branch (P1009): the source is Spectre-only",
    ),
    (
        "p_charge_capacitor",
        "ngspice's ceiling min(step, span/50) is written as maxstep=10u; the recorded deck ran at Spectre's span/50 (20u): a tighter step, the same pin (1 V at 1e-3)",
    ),
    (
        "p_func",
        "the source lists `.param rb` before `.func dbl`, the recorded deck the function first: the same definitions in another order",
    ),
    (
        "p_measure_save",
        "refused: ngspice's SIN takes its DC from the wave at t = 0 (Circuit(DcAtZeroSin)) and its `.measure` has no Spectre statement here (Item(Measure)); the recorded deck carries it in a `simulator lang=spice` region",
    ),
    (
        "p_models",
        "refused: ngspice's diode (Family(Diode)) is not Spectre's (DiodeSpectre); RUN-SP matched them at is=10f n=1 only",
    ),
    (
        "p_pwl_repeat",
        "refused: ngspice's `r=` repeat (Circuit(PwlRepeatFrom)) and DC from the wave (Circuit(DcAtZeroPwl)) have no pass to Spectre's `pwlperiod`",
    ),
    (
        "p_tf",
        "refused: `.tf` (Analysis(Tf)) is not Spectre's `xf` (a gain per source over frequency)",
    ),
];

// ---- private ----

#[test]
fn the_read_fixture_list_is_the_recorded_decks() {
    // The fixtures are the recorded decks unchanged (copies, for the dialect-wide mutant law),
    // each with its IR.
    assert_eq!(decks(), irs(), "a .ir for each tests/spectre/*.scs");
    assert_eq!(
        decks(),
        stems(fixtures(), "scs"),
        "a copy of each recorded deck"
    );
    for name in decks() {
        let copy = std::fs::read_to_string(fixtures().join(format!("{name}.scs"))).expect("a copy");
        assert_eq!(
            copy,
            deck_text(&name),
            "{name}: the copy is the recorded deck"
        );
    }
    assert!(decks().len() >= 20, "the recorded decks");
}

#[test]
fn each_recorded_deck_reads_to_its_ir_with_its_codes() {
    // Oracle: the IR text, checked by hand against the deck and its RUN-SP outcome.
    let bless = std::env::var_os("SPICERACK_BLESS").is_some();
    for name in decks() {
        let (got, diags) = read_deck(&name);
        let path = fixtures().join(format!("{name}.ir"));
        if bless {
            let mut text = format!(
                "# tests/spectre/{name}.scs read by the spectre row; checked by hand against the deck and its RUN-SP outcome (tests/spectre/pending.json, expected.json).\n# codes: {}\n",
                codes(&diags).join(" ")
            );
            for f in got.files() {
                ir::dump(&got, f, &mut text);
            }
            std::fs::write(&path, text).expect("a fixture");
        }
        let ir_text = std::fs::read_to_string(&path).expect("an .ir");
        let line = ir_text
            .lines()
            .find_map(|l| l.strip_prefix("# codes:"))
            .expect("a codes line");
        let want: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(codes(&diags), want, "{name}");
        let mut design = ir::Design::new();
        let mut d = ir::Diags::new();
        ir::parse(&ir_text, &mut design.builder(), &mut d);
        assert!(d.is_empty(), "{name}: {d:?}");
        assert!(normal(&got).eq_meaning(&normal(&design)), "{name}");
    }
}

#[test]
fn each_recorded_deck_is_a_fixpoint_of_read_write_read() {
    // Oracle: the round trip (law L6 on these decks): the written text reads back to the same
    // design, and writing that again gives the same text.
    for name in decks() {
        let (first, _) = read_deck(&name);
        let once = write(&first);
        let (second, _) = read_text(&format!("{name}.scs"), &once);
        assert!(
            normal(&first).eq_meaning(&normal(&second)),
            "{name}:\n{once}"
        );
        assert_eq!(write(&second), once, "{name}: the text is a fixpoint");
    }
}

#[test]
fn each_recorded_deck_reads_into_the_writers_features() {
    // Oracle: `FEATURES` is the reader's image (law L5's premise): nothing read from a recorded
    // deck is outside it.
    for name in decks() {
        let (design, _) = read_deck(&name);
        let outside = ir::features_used(&design).minus(dialect::spectre::FEATURES);
        assert!(
            outside.is_empty(),
            "{name}: {:?}",
            outside.iter().collect::<Vec<_>>()
        );
    }
}

#[test]
fn spice_sources_convert_to_the_meaning_spectre_ran() {
    // Oracle: the recorded deck, which Spectre 25.1 ran to its pinned numbers (RUN-SP): the
    // ngspice source written as Spectre reads back to that deck's meaning.
    let ngspice = dialect::by_name("ngspice").expect("the ngspice row");
    let mut differ = Vec::new();
    for (name, source) in sources() {
        let src = Sources::one(&format!("{name}.sp"), &source);
        let same = match dialect::transpile(&src, ngspice, row(), Policy::Lenient) {
            Ok(w) => {
                let (mut ours, _) = read_text("deck.scs", &w.text);
                let (mut ran, _) = read_text("deck.scs", &deck_text(&name));
                strip(&mut ours);
                strip(&mut ran);
                let same = normal(&ours).eq_meaning(&normal(&ran));
                if !same && std::env::var_os("SPECTRE_SHOW").is_some() {
                    let (mut a, mut b) = (String::new(), String::new());
                    ir::dump(&ours, ours.entry(), &mut a);
                    ir::dump(&ran, ran.entry(), &mut b);
                    eprintln!("== {name}\n{a}-- ran\n{b}");
                }
                same
            }
            Err(_) => false,
        };
        if !same {
            differ.push(name);
        }
    }
    let known: Vec<String> = KNOWN_CROSS.iter().map(|(n, _)| n.to_string()).collect();
    assert_eq!(
        differ, known,
        "the sources that do not convert, against KNOWN_CROSS"
    );
}

/// The spectre row.
fn row() -> &'static DialectRow {
    dialect::by_name("spectre").expect("the spectre row")
}

/// `tests/spectre`.
fn recorded() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/spectre")
}

/// `tests/fixtures/dialect/spectre/read`.
fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dialect/spectre/read")
}

/// The sorted stems of the files of `dir` with extension `ext`.
fn stems(dir: PathBuf, ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .expect("a directory")
        .filter_map(|e| {
            let p = e.expect("an entry").path();
            (p.extension()? == ext)
                .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))?
        })
        .collect();
    out.sort();
    out
}

/// The recorded decks' names.
fn decks() -> Vec<String> {
    stems(recorded(), "scs")
}

/// The read fixtures' names.
fn irs() -> Vec<String> {
    if std::env::var_os("SPICERACK_BLESS").is_some() {
        return decks();
    }
    stems(fixtures(), "ir")
}

/// Recorded deck `name`'s text.
fn deck_text(name: &str) -> String {
    std::fs::read_to_string(recorded().join(format!("{name}.scs"))).expect("a recorded deck")
}

/// Recorded deck `name`, read.
fn read_deck(name: &str) -> (ir::Design, ir::Diags) {
    read_text(&format!("{name}.scs"), &deck_text(name))
}

/// `text` read as the one file `path`, leniently.
fn read_text(path: &str, text: &str) -> (ir::Design, ir::Diags) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one(path, text),
        row(),
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    (design, diags)
}

/// The entry file written by the spectre row; no refusal.
fn write(design: &ir::Design) -> String {
    let mut d = design.clone();
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut d, row(), &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|r| dialect::refuses(r.code))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    dialect::write(&d, d.entry(), row(), &mut diags)
}

/// `design` in normal form: dumped as IR text and parsed back, so its ids follow the text's
/// order whatever order the reader pushed in (the laws' `same_meaning`, R28).
fn normal(design: &ir::Design) -> ir::Design {
    let mut text = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut text);
    }
    let mut out = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(&text, &mut out.builder(), &mut diags);
    assert!(diags.is_empty(), "the dump parses: {diags:?}");
    out
}

/// Each code of `diags`, in order.
fn codes(diags: &ir::Diags) -> Vec<String> {
    diags.rows().iter().map(|d| d.code.to_string()).collect()
}

/// `design` without its title, comments and inert statements (what moves no number).
fn strip(design: &mut ir::Design) {
    let gone: Vec<(ir::ItemRef, ir::ScopeId)> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| match i.kind {
            ir::ItemKind::Title | ir::ItemKind::Comment => true,
            ir::ItemKind::Opaque => {
                design.row::<ir::item::Opaque>(i.index).class == ir::OpaqueClass::Inert
            }
            _ => false,
        })
        .collect();
    let mut editor = design.editor();
    for (item, scope) in gone {
        editor.remove_item(scope, item);
    }
}

/// The SPICE source of each recorded `p_*` deck, by name, from the test that wrote the decks
/// (`tests/test_pending_real_run.rs`: `stem: "…"` then `src: "…"`).
fn sources() -> Vec<(String, String)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test_pending_real_run.rs");
    let text = std::fs::read_to_string(path).expect("the source test");
    let mut out = Vec::new();
    let mut rest = text.as_str();
    // Bounded: each step consumes one `stem:`.
    while let Some(at) = rest.find("stem: \"") {
        rest = &rest[at + 7..];
        let name = rest[..rest.find('"').expect("a stem's end")].to_string();
        let Some(src) = rest.find("src: \"") else {
            break;
        };
        let body = &rest[src + 6..];
        let end = body.find("\",\n").expect("a source's end");
        out.push((name, body[..end].replace("\\n", "\n")));
        rest = &body[end..];
    }
    out.sort();
    assert!(out.len() >= 10, "the p_* sources");
    out
}
