//! The HSPICE dialect against its oracle: ESPice 0.3.0 reading HSPICE text (`espice-hspice
//! --tokenizer=hspice`, the flake's pin at rev 5f0b8fd). **ESPice is not HSPICE**: every fact
//! these tests pin rests on ESPice alone unless a recorded HSPICE pin stands beside it
//! (`rewrite/DIALECT-HSPICE.md` on branch `differences`).
//!
//! - The settle decks (`tests/fixtures/dialect/hspice/settle/*.sp`) with ESPice's recorded runs
//!   (`runs.out`, made by `run.sh`): a deck ESPice refuses (exit code not 0) is one the reader
//!   refuses or reads with an `InvalidInSource` statement, and no other.
//! - A deck ESPice runs reads with no `Unread` statement, or is a [`KNOWN_UNREAD`] row.
//! - A deck ESPice runs, read and written back as HSPICE, runs on ESPice to the same printed
//!   table and measures as the deck itself (the same-dialect cell on the settle decks), or is a
//!   [`KNOWN_RUN`] row.
//!
//! - `read/<deck>.sp` + `<deck>.ir`: a settle deck and the IR it reads to; each deck's meaning
//!   is the ESPice run its second line records (the IR text was checked against that line by
//!   hand: the value of each field, the order of each wave's values); its `# codes:` line lists
//!   the notes the read gives. Each is a fixpoint of read → write → read (L6).
//!
//! `espice-hspice` comes from `nix develop .#testing`; a missing simulator fails. Lists are
//! compared by name, both ways.
//!
//! Contents: tests only.

#[path = "common/memcap.rs"]
mod memcap;

use std::collections::BTreeMap;
use std::path::PathBuf;

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// Settle decks ESPice runs whose read still leaves a statement `Unread`, with why.
const KNOWN_UNREAD: &[(&str, &str)] = &[
    (
        "data_sweep.sp",
        "`.data` and `sweep data=`: a table loop around the analysis, no rule yet (P4303)",
    ),
    (
        "expr_logic.sp",
        "`sign(-3,2)`: ESPice gives 0, the manual lead 3; unsettled on HSPICE (P4003)",
    ),
    (
        "expr_power.sp",
        "`pwr(-2,3)`: ESPice gives 8, the manual lead -8; unsettled on HSPICE (P4003)",
    ),
    (
        "temp_list.sp",
        "`.temp 27 50`: a loop around every analysis of the deck, no rule yet (P4308)",
    ),
];

/// Settle decks whose refusal verdict differs from ESPice's (refused by one side only).
const KNOWN_REFUSE: &[(&str, &str)] = &[(
    "alter.sp",
    "`.alter` reruns every analysis with its block's changes: no rule yet, the card is read as \
     InvalidInSource (P4301)",
)];

/// The read fixtures, by name.
const READ: &[&str] = &[
    "ac_poi_sorted",
    "am_form",
    "dc_forms",
    "dc_poi",
    "dc_sweep_param",
    "e_forms",
    "e_max",
    "expr_quotes",
    "expr_random",
    "expr_round",
    "expr_unquoted",
    "func_card",
    "if_param",
    "num_scale",
    "param_string",
    "pulse_zero_rise",
    "sffm_order",
    "subckt_params",
    "timed_op",
];

/// Settle decks ESPice runs whose written text does not run to the deck's own numbers.
const KNOWN_RUN: &[(&str, &str)] = &[];

// ---- private ----

#[test]
fn the_reader_refuses_the_settle_decks_espice_refuses() {
    // Oracle: ESPice's recorded exit code per deck (runs.out; ESPice only).
    let mut got = Vec::new();
    for (deck, rc) in recorded() {
        let (design, diags) = read(&deck);
        let refused =
            transpile(&deck).is_err() || has_opaque(&design, ir::OpaqueClass::InvalidInSource);
        if refused != (rc != 0) {
            eprintln!("{deck}: rc={rc}, read {:?}", codes(&diags));
            got.push(deck);
        }
    }
    assert_eq!(got, known(KNOWN_REFUSE), "refused by one side only");
}

#[test]
fn the_settle_decks_espice_runs_read_typed() {
    // Oracle: ESPice runs each of these decks (runs.out), so every statement has a meaning.
    let mut got = Vec::new();
    for (deck, rc) in recorded() {
        if rc != 0 {
            continue;
        }
        let (design, _) = read(&deck);
        if has_opaque(&design, ir::OpaqueClass::Unread) {
            eprintln!("{deck}: {:?}", codes(&read(&deck).1));
            got.push(deck);
        }
    }
    assert_eq!(got, known(KNOWN_UNREAD), "decks with an Unread statement");
}

#[test]
fn written_settle_decks_print_espices_numbers() {
    // Oracle: ESPice on the deck as written; the written deck must print the same table and
    // measures (tolerance 1e-9 relative: the same simulator, numbers spelt differently).
    let mut got = Vec::new();
    for (deck, rc) in recorded() {
        if rc != 0 {
            continue;
        }
        let Ok(written) = transpile(&deck) else {
            got.push(deck);
            continue;
        };
        let before = espice(
            &deck,
            &std::fs::read_to_string(settle().join(&deck)).unwrap(),
        );
        let after = espice(&deck, &written);
        if !same(&before, &after) {
            eprintln!("{deck}:\n{written}\n-- before\n{before}\n-- after\n{after}");
            got.push(deck);
        }
    }
    assert_eq!(
        got,
        known(KNOWN_RUN),
        "decks whose written text runs otherwise"
    );
}

#[test]
fn the_read_fixture_list_is_the_files() {
    assert_eq!(names("sp"), READ);
    assert_eq!(names("ir"), READ);
}

#[test]
fn each_read_fixture_reads_to_its_ir_with_its_codes() {
    // Oracle: the IR text checked by hand against each deck's ESPice run (its second line).
    for name in READ {
        let path = format!("{name}.sp");
        let text = std::fs::read_to_string(dir("read").join(&path)).expect("a deck");
        let (got, diags) = read_text(&path, &text);
        let ir = std::fs::read_to_string(dir("read").join(format!("{name}.ir"))).expect("an .ir");
        let line = ir
            .lines()
            .find_map(|l| l.strip_prefix("# codes:"))
            .expect("a codes line");
        let want: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(codes(&diags), want, "{name}");
        let mut design = ir::Design::new();
        let mut d = ir::Diags::new();
        ir::parse(&ir, &mut design.builder(), &mut d);
        assert!(d.is_empty(), "{name}: {d:?}");
        assert!(got.eq_ignoring_spans(&design), "{name}");
    }
}

#[test]
fn each_read_fixture_is_a_fixpoint_of_read_write_read() {
    // Oracle: law L6 on these decks: the written text reads back to the same design, and
    // writing that again gives the same text.
    for name in READ {
        let path = format!("{name}.sp");
        let text = std::fs::read_to_string(dir("read").join(&path)).expect("a deck");
        let (first, _) = read_text(&path, &text);
        let once = write(&first);
        let (second, _) = read_text(&path, &once);
        assert!(first.eq_ignoring_spans(&second), "{name}:\n{once}");
        assert_eq!(write(&second), once, "{name}: the text is a fixpoint");
    }
}

#[test]
fn every_code_the_dialect_defines_has_a_catalogue_row() {
    // Oracle: the dialect's own source, every `Code::new(b'L', N)` it spells, against
    // `dialect::info`: a code with no row is "no catalogue holds it", a refusal by accident.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/dialect/hspice");
    let mut stack = vec![root];
    let mut missing = Vec::new();
    let mut seen = 0usize;
    // Bounded: each directory is entered once.
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("the dialect's source") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file");
            let text = text.split("#[cfg(test)]").next().unwrap_or("");
            for (at, _) in text.match_indices("Code::new(b'") {
                let rest = &text[at + "Code::new(b'".len()..];
                let letter = rest.as_bytes()[0];
                let digits: String = rest[3..]
                    .trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                let Ok(number) = digits.parse::<u16>() else {
                    continue;
                };
                seen += 1;
                assert!(
                    (4000..5000).contains(&number),
                    "{letter}{number} outside 4000..5000"
                );
                if dialect::info(ir::Code::new(letter, number)).is_none() {
                    missing.push(format!("{}{number} ({})", letter as char, path.display()));
                }
            }
        }
    }
    assert!(seen > 50, "the scan found the dialect's codes ({seen})");
    assert!(
        missing.is_empty(),
        "codes with no catalogue row: {missing:?}"
    );
}

/// The hspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("hspice").expect("the hspice row")
}

/// `tests/fixtures/dialect/hspice/settle`.
fn settle() -> PathBuf {
    dir("settle")
}

/// `tests/fixtures/dialect/hspice/<sub>`.
fn dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dialect/hspice")
        .join(sub)
}

/// The sorted names of the `read/*.<ext>` files.
fn names(ext: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir("read"))
        .expect("the read fixtures")
        .filter_map(|e| {
            let p = e.expect("an entry").path();
            (p.extension()? == ext)
                .then(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()))?
        })
        .collect();
    out.sort();
    out
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

/// The entry file written by the hspice row; no refusal.
fn write(design: &ir::Design) -> String {
    let mut diags = ir::Diags::new();
    let text = dialect::write(design, design.entry(), row(), &mut diags);
    let refused: Vec<_> = diags
        .rows()
        .iter()
        .filter(|d| dialect::refuses(d.code))
        .collect();
    assert!(refused.is_empty(), "{refused:?}");
    text
}

/// Each settle deck `runs.out` records, with ESPice's exit code, in file order.
fn recorded() -> Vec<(String, i32)> {
    let runs = std::fs::read_to_string(settle().join("runs.out")).expect("runs.out");
    let out: Vec<(String, i32)> = runs
        .lines()
        .filter_map(|l| l.strip_prefix("== "))
        .filter_map(|l| {
            let (deck, rc) = l.split_once(" rc=")?;
            Some((deck.to_string(), rc.trim().parse().ok()?))
        })
        .collect();
    assert!(
        out.len() > 50,
        "runs.out lists the settle decks ({})",
        out.len()
    );
    out
}

/// The deck's sources: the deck and every file in the settle folder it names.
fn sources(deck: &str) -> Sources {
    let dir = settle();
    Sources::load(
        &[deck],
        ir::Role::Deck,
        row().read.expect("a reader"),
        &mut |p| std::fs::read_to_string(dir.join(p)).ok(),
    )
}

/// `deck` read by the hspice row, leniently.
fn read(deck: &str) -> (ir::Design, ir::Diags) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &sources(deck),
        row(),
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    (design, diags)
}

/// The codes of `diags`, in order.
fn codes(diags: &ir::Diags) -> Vec<String> {
    diags.rows().iter().map(|d| d.code.to_string()).collect()
}

/// Whether `design` holds an `Opaque` of `class`.
fn has_opaque(design: &ir::Design, class: ir::OpaqueClass) -> bool {
    design.reachable().into_iter().any(|(item, _)| {
        item.kind == ir::ItemKind::Opaque
            && design.row::<ir::item::Opaque>(item.index).class == class
    })
}

/// `deck` read and written by the hspice row, or its diagnostics when one refuses.
fn transpile(deck: &str) -> Result<String, ir::Diags> {
    dialect::transpile(&sources(deck), row(), row(), Policy::Lenient).map(|w| w.text)
}

/// ESPice's print table and measures for `text` run as `name` in a copy of the settle folder
/// (its library files beside it), or its exit code and errors when it fails.
fn espice(name: &str, text: &str) -> String {
    let dir = tempfile::tempdir().expect("a temp dir");
    for e in std::fs::read_dir(settle()).expect("the settle folder") {
        let p = e.expect("an entry").path();
        std::fs::copy(&p, dir.path().join(p.file_name().unwrap())).expect("a copy");
    }
    std::fs::write(dir.path().join(name), text).expect("the deck is written");
    let out = std::process::Command::new("espice-hspice")
        .current_dir(dir.path())
        .args([
            "--tokenizer=hspice",
            "--format=print",
            "-r",
            "out.txt",
            "--measures=m.json",
        ])
        .arg(name)
        .output()
        .expect("espice-hspice runs (nix develop .#testing provides it; unsupported fails)");
    let file = |f: &str| std::fs::read_to_string(dir.path().join(f)).unwrap_or_default();
    if !out.status.success() {
        return format!(
            "rc={:?} {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    format!("{}\n{}", file("out.txt"), measures(&file("m.json")))
}

/// The `"name": …, "value": …` pairs of ESPice's measures JSON, one per line.
fn measures(json: &str) -> String {
    json.lines()
        .filter(|l| l.contains("\"name\""))
        .map(|l| {
            let field = |k: &str| {
                l.split(&format!("\"{k}\": ")).nth(1).map_or("", |r| {
                    r.split([',', '}']).next().unwrap_or("").trim_matches('"')
                })
            };
            format!("{} {}", field("name"), field("value"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether two ESPice outputs agree: the same words, numbers within 1e-9 relative.
fn same(a: &str, b: &str) -> bool {
    let (wa, wb): (Vec<&str>, Vec<&str>) = (
        a.split_whitespace().collect(),
        b.split_whitespace().collect(),
    );
    wa.len() == wb.len()
        && !a.starts_with("rc=")
        && wa
            .iter()
            .zip(&wb)
            .all(|(x, y)| match (x.parse::<f64>(), y.parse::<f64>()) {
                (Ok(p), Ok(q)) => (p - q).abs() <= 1e-9 * p.abs().max(q.abs()).max(1e-30),
                _ => x == y,
            })
}

/// The names of `rows`, in `recorded` order.
fn known(rows: &[(&str, &str)]) -> Vec<String> {
    let order: BTreeMap<String, usize> = recorded()
        .into_iter()
        .enumerate()
        .map(|(k, (d, _))| (d, k))
        .collect();
    let mut names: Vec<String> = rows.iter().map(|r| r.0.to_string()).collect();
    names.sort_by_key(|n| order.get(n).copied().unwrap_or(usize::MAX));
    names
}
