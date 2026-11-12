//! The laws every dialect passes (REWRITE-PLAN §4.2), run for every row of
//! `dialect::BUILTIN` on the sides it has: L1 totality (any bytes, and the text of zero bytes), L2
//! validity, L3 determinism (reader); L4 writer totality over what `legalize` accepted, L7
//! feature honesty, L8 isolation (writer); L5 round trip (both); L6 corpus fixpoint (both,
//! `--features corpus` only). L9 belongs to each dialect's own simulator tests.
//!
//! Each law runs per row and its failing rows are compared with [`KNOWN`], strictly both ways:
//! an unexpected failure and an unexpected pass both fail. Every file a writer writes is its own
//! text at its own path, read back as one `Sources` (entry first). A row that is not lossless
//! generates no `Opaque` (an opaque's class is its reader's judgement of the text, which a
//! generated text cannot carry).
//!
//! A generated design the row refuses is shrunk to what it admits, not skipped (A25, R48;
//! `common/laws.rs`): the items its refusals name are dropped, then what `ir::check` no longer
//! accepts, to a fixpoint; a refusal at no item skips the seed, counted by code. The reach list
//! asks every `Item`, `Elem`, `Wave`, `Analysis` and `Family` kind the writer claims to appear
//! in some admitted design; the names never reached are compared with [`KNOWN_REACH`].
//!
//! Oracles: generated designs (`ir::gen::generate(seed, row's features, size)`, valid and legal
//! for the set by construction) and their canonical IR text (`ir::dump`, which has no spans:
//! `≅`); fixed seeds, each printed in the failing message.
//!
//! Contents: tests only.

#[path = "common/known_reach.rs"]
mod known_reach;
#[path = "common/laws.rs"]
mod law_harness;

use std::panic::{AssertUnwindSafe, catch_unwind};

use known_reach::KNOWN_REACH;
use law_harness::{
    PARAMS, SIZE, Tally, admitted, canonical, generated, one_file, read_all, read_text, readers,
    same_meaning, write_all, writers,
};
use spicerack::dialect::{self, DialectRow, Read, Write};
use spicerack::ir;
use spicerack::ir::r#gen::{Params, generate};

/// The seeds every generated law runs (printed on failure).
const SEEDS: std::ops::Range<u64> = 0..64;

/// The laws a row fails today: (law, row, why). Strict both ways (`check_known`): a row
/// that fails a law it is not listed for fails, and so does a listed row that passes. Each
/// dialect branch lists its own rows here; the `ir` row fails none.
const KNOWN: &[(&str, &str, &str)] = &[(
    "l7",
    "ngspice",
    "lacked features with neither a pass nor a citation (Load first; citations.toml not written yet)",
)];

/// The most seeds the shrink self-test searches for its case.
const SEARCH_MAX: u64 = 1024;

/// The seeds the reach list draws from: more than a law's, since it asks for every kind once.
const REACH_SEEDS: std::ops::Range<u64> = 0..512;

/// A writer that spells every feature, refuses every item but elements in `admits`, and panics
/// on a capacitor (the shrink self-test's planted bug).
struct Panicky;

impl Write for Panicky {
    fn features(&self) -> &'static ir::FeatureSet {
        &ir::FeatureSet::ALL
    }

    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, _: &mut ir::Diags) {
        let capacitor = design.reachable().iter().any(|(i, _)| {
            i.kind == ir::ItemKind::Element
                && design.row::<ir::Element>(i.index).kind == ir::ElemKind::Capacitor
        });
        assert!(!capacitor, "the planted bug: a capacitor");
        ir::dump(design, file, out);
    }

    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        for (item, _) in design.reachable() {
            if item.kind != ir::ItemKind::Element {
                diags.push_at(ir::Code::new(b'W', 9001), design, item, 0);
            }
        }
    }
}

// ---- private ----

#[test]
fn l1_l2_any_bytes_read_returns_and_checks_clean() {
    for (row, reader) in readers() {
        for seed in SEEDS {
            let text = noise(seed, 400);
            let (_, diags) = read_text(row, &text);
            let bugs: Vec<_> = diags
                .rows()
                .iter()
                .filter(|d| d.code.letter == b'X')
                .collect();
            assert!(bugs.is_empty(), "{} seed {seed}: X rows {bugs:?}", row.name);
        }
        let _ = reader.references(&noise(1, 400));
    }
}

/// The text of zero bytes is one file with no item and no row (R46). A text of one line break
/// is each dialect's own fixture: whether a blank line 1 is a statement (ngspice: the deck's
/// title) is its lexicon's call, not the spine's.
#[test]
fn l1_an_empty_text_is_one_file_with_no_item() {
    reader_laws("l1-empty", |row, _| {
        let (design, diags) = read_text(row, "");
        let files: Vec<ir::FileId> = design.files().collect();
        if files.len() != 1 || !design.reachable().is_empty() || !diags.is_empty() {
            return Err(format!(
                "{} files, {} items, rows {:?}",
                files.len(),
                design.reachable().len(),
                diags.rows()
            ));
        }
        match design.str(design.file(files[0]).path) == "law" {
            true => Ok(()),
            false => Err("the file is not at its source path".to_string()),
        }
    });
}

#[test]
fn l3_one_read_and_four_threads_give_the_same_bytes() {
    laws("l3", |row, writer| {
        let mut tally = Tally::new();
        for (seed, params) in cases(SEEDS.take(8)) {
            let design = match admitted(seed, params, row, writer) {
                Ok(d) => d,
                Err(code) => {
                    tally.skip(code);
                    continue;
                }
            };
            tally.admit();
            let files = write_all(&design, row);
            let once = canonical(&read_all(row, &files));
            let threads: Vec<String> = std::thread::scope(|s| {
                let jobs: Vec<_> = (0..4)
                    .map(|_| s.spawn(|| canonical(&read_all(row, &files))))
                    .collect();
                jobs.into_iter()
                    .map(|j| j.join().expect("a reader thread"))
                    .collect()
            });
            if !threads.iter().all(|t| *t == once) {
                return Err(format!("seed {seed} {params:?}: the threads differ"));
            }
        }
        tally.some()
    });
}

#[test]
fn l4_write_is_total_over_a_design_legalize_accepted() {
    laws("l4", |row, writer| l4_on(row, writer, SEEDS));
}

#[test]
fn l5_read_of_write_is_the_design() {
    laws("l5", |row, writer| {
        if row.read.is_none() {
            return Ok(());
        }
        let mut tally = Tally::new();
        for (seed, params) in cases(SEEDS) {
            let design = match admitted(seed, params, row, writer) {
                Ok(d) => d,
                Err(code) => {
                    tally.skip(code);
                    continue;
                }
            };
            tally.admit();
            let back = read_all(row, &write_all(&design, row));
            // By meaning (R28), in normal form (each side through the IR text): real literals
            // equal by value, ids in canonical order; the IR text is the failure's evidence.
            if !same_meaning(&back, &design) {
                return Err(format!(
                    "seed {seed} {params:?}:\n{}\nread back as\n{}",
                    canonical(&design),
                    canonical(&back)
                ));
            }
        }
        tally.some()
    });
}

#[cfg(feature = "corpus")]
#[test]
fn l6_a_corpus_text_is_a_fixpoint_of_read_write_read() {
    for (row, _) in writers() {
        if row.read.is_none() {
            continue;
        }
        let texts = corpus(row.name);
        assert!(
            !texts.is_empty(),
            "{}: no corpus (unsupported fails)",
            row.name
        );
        for (k, text) in texts.iter().enumerate() {
            let once = read_text(row, text).0;
            let twice = read_all(row, &write_all(&once, row));
            assert!(
                same_meaning(&twice, &once),
                "{} corpus text {k} (by meaning, R28):\n{}\nread back as\n{}",
                row.name,
                canonical(&once),
                canonical(&twice)
            );
        }
    }
}

#[test]
fn l7_legalized_designs_use_only_the_writers_features() {
    let all = dialect::irtext::IrText.features();
    laws("l7", |_row, writer| {
        for f in all.minus(*writer.features()).iter() {
            let removed = spicerack::passes::all().any(|p| p.removes == f);
            if !removed {
                return Err(format!("lacks {f:?}: no pass removes it, and no citation"));
            }
        }
        for seed in SEEDS {
            let mut design = generate(seed, all, SIZE);
            let mut diags = ir::Diags::new();
            spicerack::passes::legalize(&mut design, writer.features(), &mut diags);
            let refused: Vec<u32> = diags
                .rows()
                .iter()
                .filter(|d| d.code == ir::diag::UNSUPPORTED)
                .map(|d| d.arg)
                .collect();
            let left: Vec<u32> = ir::features_used(&design)
                .minus(*writer.features())
                .iter()
                .map(|f| u32::from(f.index()))
                .collect();
            if left != refused {
                return Err(format!(
                    "seed {seed}: what is left {left:?} is not what is refused {refused:?}"
                ));
            }
        }
        Ok(())
    });
}

#[test]
fn l8_a_file_is_written_the_same_whatever_the_other_files() {
    let ir_row = dialect::by_name("ir").expect("the ir row");
    laws("l8", |row, writer| {
        let mut tally = Tally::new();
        for (seed, params) in cases(SEEDS) {
            let design = match admitted(seed, params, row, writer) {
                Ok(d) => d,
                Err(code) => {
                    tally.skip(code);
                    continue;
                }
            };
            tally.admit();
            let entry = design.entry();
            let path = design.str(design.file(entry).path).to_string();
            let mut alone = ir::Diags::new();
            let first = dialect::write(&design, entry, row, &mut alone);
            // The same files in reverse order, rebuilt through the IR text.
            let mut files: Vec<String> = design.files().map(|f| one_file(&design, f)).collect();
            files.reverse();
            let permuted = read_text(ir_row, &files.concat()).0;
            let mut diags = ir::Diags::new();
            let again = dialect::write(&permuted, permuted.find_file(&path), row, &mut diags);
            if again != first {
                return Err(format!(
                    "seed {seed} {params:?}: the file is written otherwise"
                ));
            }
        }
        tally.some()
    });
}

/// Every `Item`, `Elem`, `Wave`, `Analysis` and `Family` feature a row's writer claims is
/// used by at least one design the laws run on (shrunk to admitted, over every seed); the
/// names never reached are compared with [`KNOWN_REACH`], strictly both ways.
#[test]
fn reach_every_kind_the_row_claims() {
    let mut unreached: Vec<(String, String)> = Vec::new();
    for (row, writer) in writers() {
        let mut seen = ir::FeatureSet::EMPTY;
        for (seed, params) in cases(REACH_SEEDS) {
            if let Ok(design) = admitted(seed, params, row, writer) {
                seen = seen.union(ir::features_used(&design));
            }
        }
        let claimed = generated(row, writer).minus(seen);
        for f in claimed.iter().filter(|f| kind(*f)) {
            unreached.push((row.name.to_string(), format!("{f:?}")));
        }
    }
    let known: Vec<(String, String)> = KNOWN_REACH
        .iter()
        .map(|(r, f, _, _)| (r.to_string(), f.to_string()))
        .collect();
    assert_eq!(
        unreached, known,
        "kinds claimed and never reached, against KNOWN_REACH"
    );
    let uncovered = uncovered(KNOWN_REACH, &test_sources());
    assert!(
        uncovered.is_empty(),
        "KNOWN_REACH rows with no covering property:\n{}",
        uncovered.join("\n")
    );
    // Interim rows (a fixture fixpoint standing in for the seeded property) pass here and are
    // printed on every run; the dialect gates (S3.5, S4.5) fail on any.
    for (row, kind, name, _) in KNOWN_REACH.iter().filter(|r| r.3.starts_with("interim")) {
        eprintln!("interim covering property: {row} {kind}: {name}");
    }
}

/// The covering-property check's self-test: an empty name and a name no test file defines are
/// refused; a name that is defined passes.
#[test]
fn a_known_unreached_kind_names_a_covering_property_that_exists() {
    let sources = test_sources();
    let rows: &[(&str, &str, &str, &str)] = &[
        ("x", "Item(Loop)", "", "no name"),
        (
            "x",
            "Item(Save)",
            "no_such_covering_property_anywhere",
            "missing",
        ),
        (
            "x",
            "Item(Env)",
            "reach_every_kind_the_row_claims",
            "defined",
        ),
    ];
    let found = uncovered(rows, &sources);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].contains("Item(Loop)") && found[1].contains("Item(Save)"),
        "{found:?}"
    );
}

/// The rows of `rows` whose covering property is empty or not defined (`fn <name>` in no
/// source of `sources`: a text search, enough to name a test by its function).
fn uncovered(rows: &[(&str, &str, &str, &str)], sources: &[String]) -> Vec<String> {
    let defined = |name: &str| sources.iter().any(|t| t.contains(&format!("fn {name}(")));
    rows.iter()
        .filter(|(_, _, name, _)| name.is_empty() || !defined(name))
        .map(|(row, kind, name, _)| format!("{row} {kind}: covering property `{name}` not found"))
        .collect()
}

/// The text of every `.rs` file under `tests/`.
fn test_sources() -> Vec<String> {
    let mut out = Vec::new();
    let mut todo = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests")];
    // Bounded by the tree: each directory is read once.
    while let Some(dir) = todo.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.extend(std::fs::read_to_string(&p).ok());
            }
        }
    }
    assert!(!out.is_empty(), "test_sources: the tests/ tree");
    out
}

/// The self-test of shrink to admitted (A25): a row whose `admits` refuses every item but
/// elements refuses every design as generated, and its writer panics on a capacitor; on a seed
/// (searched, bounded, so the test does not depend on the generator's stream) whose shrunk
/// design keeps a capacitor, L4 runs and sees the panic.
#[test]
fn shrink_lets_l4_see_a_writer_bug_in_a_row_that_admits_nothing() {
    let row: &'static DialectRow = Box::leak(Box::new(DialectRow {
        name: "panicky",
        text_family: dialect::ANY_FAMILY,
        read: None,
        write: Some(&Panicky),
    }));
    // Independent of the generator's stream: the first seed, within a bound, whose design the
    // row refuses as generated and whose shrunk design keeps a capacitor.
    let refused_as_generated = |seed: u64| {
        let mut design = generate(seed, &generated(row, &Panicky), SIZE);
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row, &mut diags);
        diags.rows().iter().any(|d| dialect::refuses(d.code))
    };
    let keeps_a_capacitor = |seed: u64| {
        admitted(seed, Params::Untyped, row, &Panicky).is_ok_and(|d| {
            d.reachable().iter().any(|(i, _)| {
                i.kind == ir::ItemKind::Element
                    && d.row::<ir::Element>(i.index).kind == ir::ElemKind::Capacitor
            })
        })
    };
    let seed = (0..SEARCH_MAX)
        .find(|&s| refused_as_generated(s) && keeps_a_capacitor(s))
        .expect("within SEARCH_MAX seeds, a refused design whose shrink keeps a capacitor");
    let found = l4_on(row, &Panicky, seed..seed + 1).expect_err("L4 sees the writer's panic");
    assert!(found.contains("panicked"), "{found}");
}

/// L4 on one row: each seed shrunk to admitted, every file written; a refusal or a writer's
/// panic is the failure (with its seed).
fn l4_on(row: &DialectRow, writer: &dyn Write, seeds: std::ops::Range<u64>) -> Result<(), String> {
    let mut tally = Tally::new();
    for (seed, params) in cases(seeds) {
        let design = match admitted(seed, params, row, writer) {
            Ok(d) => d,
            Err(code) => {
                tally.skip(code);
                continue;
            }
        };
        tally.admit();
        let mut diags = ir::Diags::new();
        let wrote = catch_unwind(AssertUnwindSafe(|| {
            for file in design.files() {
                let _ = dialect::write(&design, file, row, &mut diags);
            }
        }));
        if wrote.is_err() {
            return Err(format!("seed {seed} {params:?}: the writer panicked"));
        }
        let refusals: Vec<_> = diags
            .rows()
            .iter()
            .filter(|d| dialect::refuses(d.code))
            .collect();
        if !refusals.is_empty() {
            return Err(format!("seed {seed} {params:?}: {refusals:?}"));
        }
    }
    tally.some()
}

/// Every (seed, setting) of `seeds` under both generator settings (A28: untyped parameters
/// drawn, and none); a law and the reach list run over all of them.
fn cases(seeds: impl Iterator<Item = u64> + Clone) -> impl Iterator<Item = (u64, Params)> {
    PARAMS
        .into_iter()
        .flat_map(move |p| seeds.clone().map(move |s| (s, p)))
}

/// Whether `f` is a kind the reach list covers.
fn kind(f: ir::Feature) -> bool {
    matches!(
        f,
        ir::Feature::Item(_)
            | ir::Feature::Elem(_)
            | ir::Feature::Wave(_)
            | ir::Feature::Analysis(_)
            | ir::Feature::Family(_)
    )
}

/// Runs `law` on every row with a writer and compares the rows it fails on with the known
/// failures of that law ([`KNOWN`]), strictly: an unexpected failure and an unexpected pass both
/// fail, each with its message.
fn laws(name: &str, law: impl Fn(&DialectRow, &dyn Write) -> Result<(), String>) {
    let mut failing = Vec::new();
    let mut messages = Vec::new();
    for (row, writer) in writers() {
        if let Err(m) = caught(|| law(row, writer)) {
            failing.push(row.name);
            messages.push(format!("{}: {m}", row.name));
        }
    }
    check_known(name, &failing, &messages);
}

/// [`laws`] over the rows with a reader.
fn reader_laws(name: &str, law: impl Fn(&DialectRow, &dyn Read) -> Result<(), String>) {
    let mut failing = Vec::new();
    let mut messages = Vec::new();
    for (row, reader) in readers() {
        if let Err(m) = caught(|| law(row, reader)) {
            failing.push(row.name);
            messages.push(format!("{}: {m}", row.name));
        }
    }
    check_known(name, &failing, &messages);
}

/// `law()`, a panic in it (a reader's or a writer's) turned into the row's failure, so one row
/// that panics fails as a row instead of aborting the law for every row.
fn caught(law: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    match catch_unwind(AssertUnwindSafe(law)) {
        Ok(result) => result,
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned();
            let msg = msg.or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            Err(format!("panicked: {}", msg.unwrap_or_default()))
        }
    }
}

/// The rows that failed law `name` against its [`KNOWN`] rows, in `BUILTIN` order.
fn check_known(name: &str, failing: &[&str], messages: &[String]) {
    let known: Vec<&str> = KNOWN
        .iter()
        .filter(|(law, _, _)| *law == name)
        .map(|(_, row, _)| *row)
        .collect();
    assert_eq!(
        failing,
        known,
        "{name}: failing rows against KNOWN\n{}",
        messages.join("\n")
    );
}

/// `len` pseudo-random bytes of seed `seed` (splitmix64), as text: mostly IR-text bytes, some
/// arbitrary.
fn noise(seed: u64, len: usize) -> String {
    const ALPHABET: &[u8] =
        b"file \"deck\" library  element resistor r1 (a b) value=1k\n#=()[],?`\\";
    let mut state = seed;
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        bytes.push(if z.is_multiple_of(8) {
            z as u8 >> 1
        } else {
            ALPHABET[(z >> 8) as usize % ALPHABET.len()]
        });
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A dialect's corpus for L6.
#[cfg(feature = "corpus")]
fn corpus(name: &str) -> Vec<String> {
    match name {
        // The kernel's kinds, and the worked example of `crates/ir/src/text.rs`.
        "ir" => vec![
            "file \"rc.sp\" deck\n  title \"rc divider\"\n  param rval = 1k\n  subckt div (in out gnd) r=rval\n    element resistor r1 (in out) value=r\n    element resistor r2 (out gnd) value=(r / 2)\n  element instance x1 (a b 0) model=div\n  element vsource v1 (a 0) dc=1.8\n  analysis op\n".to_string(),
            "file \"kit.sp\" deck\n  title \"kernel kinds\"\n  global vdd vss\n  param w = 2u\n  subckt cell (a b) r=w\n    param k = r * 2\n  if (w > 1u)\n    param m = 1\n  else\n    param m = 3\nfile \"models.lib\" library\n  section tt\n    param vth = 0.4\n".to_string(),
        ],
        _ => Vec::new(),
    }
}
