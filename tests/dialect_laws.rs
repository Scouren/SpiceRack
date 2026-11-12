//! The laws every dialect passes (REWRITE-PLAN §4.2), run for every row of
//! `dialect::BUILTIN` on the sides it has: L1 totality (any bytes, and the empty text), L2
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
//! Oracles: generated designs (`ir::gen::generate(seed, row's features, size)`, valid and legal
//! for the set by construction) and their canonical IR text (`ir::dump`, which has no spans:
//! `≅`); fixed seeds, each printed in the failing message.
//!
//! Contents: tests only.

use spicerack::dialect::{self, BUILTIN, DialectRow, Policy, Read, Sources, Write};
use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};

/// The seeds every generated law runs (printed on failure).
const SEEDS: std::ops::Range<u64> = 0..64;

/// A moderate design: the merge tier's size (REWRITE-PLAN §7.5).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// The laws a row fails today: (law, row, why). Strict both ways (`check_known`): a row
/// that fails a law it is not listed for fails, and so does a listed row that passes. Each
/// dialect branch lists its own rows here; the `ir` row fails none.
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "l1-empty",
        "ngspice",
        "a blank line 1 is a deck's empty Title (the lead's ruling, follow-up 3 of S3.0); an empty text has none",
    ),
    (
        "l3",
        "ngspice",
        "no admitted design: every seed holds a string value outside a model version (W1006) and most a model parameter, bin or value ngspice lacks (W1202-W1205), see the S3.0 integration report",
    ),
    (
        "l4",
        "ngspice",
        "no admitted design: every seed holds a string value outside a model version (W1006) and most a model parameter, bin or value ngspice lacks (W1202-W1205), see the S3.0 integration report",
    ),
    (
        "l5",
        "ngspice",
        "no admitted design: every seed holds a string value outside a model version (W1006) and most a model parameter, bin or value ngspice lacks (W1202-W1205), see the S3.0 integration report",
    ),
    (
        "l7",
        "ngspice",
        "lacked features with neither a pass nor a citation (Load first; citations.toml not written yet)",
    ),
    (
        "l8",
        "ngspice",
        "no admitted design: every seed holds a string value outside a model version (W1006) and most a model parameter, bin or value ngspice lacks (W1202-W1205), see the S3.0 integration report",
    ),
];

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

#[test]
fn l1_an_empty_text_is_one_file_with_no_item() {
    reader_laws("l1-empty", |row, _| {
        for text in ["", "\n"] {
            let (design, diags) = read_text(row, text);
            let files: Vec<ir::FileId> = design.files().collect();
            if files.len() != 1 || !design.reachable().is_empty() || !diags.is_empty() {
                return Err(format!(
                    "{text:?}: {} files, {} items, rows {:?}",
                    files.len(),
                    design.reachable().len(),
                    diags.rows()
                ));
            }
            if design.str(design.file(files[0]).path) != "law" {
                return Err(format!("{text:?}: the file is not at its source path"));
            }
        }
        Ok(())
    });
}

#[test]
fn l3_one_read_and_four_threads_give_the_same_bytes() {
    laws("l3", |row, writer| {
        let mut admitted = 0;
        for seed in SEEDS.take(8) {
            let Some(design) = admitted_design(seed, row, writer) else {
                continue;
            };
            admitted += 1;
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
                return Err(format!("seed {seed}: the threads differ"));
            }
        }
        some(admitted)
    });
}

#[test]
fn l4_write_is_total_over_a_design_legalize_accepted() {
    laws("l4", |row, writer| {
        let claimed = generated(row, writer);
        let (mut skipped, mut seen) = (Vec::new(), ir::FeatureSet::EMPTY);
        for seed in SEEDS {
            let mut design = generate(seed, &claimed, SIZE);
            let mut diags = ir::Diags::new();
            dialect::legalize(&mut design, row, &mut diags);
            if diags.rows().iter().any(|d| dialect::refuses(d.code)) {
                skipped.push(seed);
                continue;
            }
            seen = seen.union(ir::features_used(&design));
            for file in design.files() {
                let _ = dialect::write(&design, file, row, &mut diags);
            }
            let refusals: Vec<_> = diags
                .rows()
                .iter()
                .filter(|d| dialect::refuses(d.code))
                .collect();
            if !refusals.is_empty() {
                return Err(format!("seed {seed}: {refusals:?}"));
            }
        }
        // The skip cannot make the law vacuous: every item kind the writer claims is admitted in
        // some generated design.
        for &kind in &ir::ItemKind::ALL {
            let f = ir::Feature::Item(kind);
            if claimed.contains(f) && !seen.contains(f) {
                return Err(format!(
                    "no admitted design holds {kind:?} (skipped {skipped:?})"
                ));
            }
        }
        Ok(())
    });
}

#[test]
fn l5_read_of_write_is_the_design() {
    laws("l5", |row, writer| {
        if row.read.is_none() {
            return Ok(());
        }
        let mut admitted = 0;
        for seed in SEEDS {
            let Some(design) = admitted_design(seed, row, writer) else {
                continue;
            };
            admitted += 1;
            let back = read_all(row, &write_all(&design, row));
            if canonical(&back) != canonical(&design) {
                return Err(format!(
                    "seed {seed}:\n{}\nread back as\n{}",
                    canonical(&design),
                    canonical(&back)
                ));
            }
        }
        some(admitted)
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
            assert_eq!(
                canonical(&twice),
                canonical(&once),
                "{} corpus text {k}",
                row.name
            );
        }
    }
}

#[test]
fn l7_legalized_designs_use_only_the_writers_features() {
    let all = dialect::irtext::IrText.features();
    laws("l7", |_row, writer| {
        for f in all.minus(*writer.features()).iter() {
            let removed = spicerack::passes::PASSES.iter().any(|p| p.removes == f);
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
        let mut admitted = 0;
        for seed in SEEDS {
            let Some(design) = admitted_design(seed, row, writer) else {
                continue;
            };
            admitted += 1;
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
                return Err(format!("seed {seed}: the file is written otherwise"));
            }
        }
        some(admitted)
    });
}

/// Runs `law` on every row with a writer and compares the rows it fails on with the known
/// failures of that law ([`KNOWN`]), strictly: an unexpected failure and an unexpected pass both
/// fail, each with its message.
fn laws(name: &str, law: impl Fn(&DialectRow, &dyn Write) -> Result<(), String>) {
    let mut failing = Vec::new();
    let mut messages = Vec::new();
    for (row, writer) in writers() {
        if let Err(m) = law(row, writer) {
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
        if let Err(m) = law(row, reader) {
            failing.push(row.name);
            messages.push(format!("{}: {m}", row.name));
        }
    }
    check_known(name, &failing, &messages);
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

/// `Ok` when at least one generated design was admitted (else the law ran on nothing).
fn some(admitted: usize) -> Result<(), String> {
    if admitted == 0 {
        return Err("no generated design was admitted: the law ran on nothing".to_string());
    }
    Ok(())
}

/// The set a law generates over: the writer's features, without `Opaque`
/// for a row that is not lossless (an opaque's class is its reader's judgement of the text,
/// which a generated text cannot carry).
fn generated(row: &DialectRow, writer: &dyn Write) -> ir::FeatureSet {
    let set = *writer.features();
    if row.text_family == dialect::ANY_FAMILY {
        return set;
    }
    set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Opaque)))
}

/// Design `seed` over [`generated`], legalized for `row`; `None` when `legalize` refuses it.
fn admitted_design(seed: u64, row: &DialectRow, writer: &dyn Write) -> Option<ir::Design> {
    let mut design = generate(seed, &generated(row, writer), SIZE);
    let mut diags = ir::Diags::new();
    dialect::legalize(&mut design, row, &mut diags);
    (!diags.rows().iter().any(|d| dialect::refuses(d.code))).then_some(design)
}

/// Every file of `design` written by `row` as one source set: each file its own text at the
/// path the design gives it, the entry first (its role setting the form), then file order.
fn write_all(design: &ir::Design, row: &DialectRow) -> Sources {
    let mut diags = ir::Diags::new();
    let entry = design.entry();
    let mut src = Sources::new();
    for f in std::iter::once(entry).chain(design.files().filter(|&f| f != entry)) {
        let path = design.str(design.file(f).path).to_string();
        src.add(&path, &dialect::write(design, f, row, &mut diags));
    }
    src.set_form(match design.file(entry).role {
        ir::Role::Deck => ir::Role::Deck,
        ir::Role::Library | ir::Role::VerilogA | ir::Role::Missing => ir::Role::Library,
    });
    src
}

/// `src` read back by `row`, leniently.
fn read_all(row: &DialectRow, src: &Sources) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    design
}

/// The rows with a reader.
fn readers() -> impl Iterator<Item = (&'static DialectRow, &'static dyn Read)> {
    let rows = BUILTIN.iter().filter_map(|r| r.read.map(|read| (r, read)));
    assert!(
        BUILTIN.iter().any(|r| r.read.is_some()),
        "a row with a reader"
    );
    rows
}

/// The rows with a writer.
fn writers() -> impl Iterator<Item = (&'static DialectRow, &'static dyn Write)> {
    let rows = BUILTIN
        .iter()
        .filter_map(|r| r.write.map(|write| (r, write)));
    assert!(
        BUILTIN.iter().any(|r| r.write.is_some()),
        "a row with a writer"
    );
    rows
}

/// `text` read by `row`, leniently, as one file.
fn read_text(row: &DialectRow, text: &str) -> (ir::Design, ir::Diags) {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one("law", text),
        row,
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    (design, diags)
}

/// One file as IR text.
fn one_file(design: &ir::Design, file: ir::FileId) -> String {
    let mut out = String::new();
    ir::dump(design, file, &mut out);
    out
}

/// `≅`: the canonical IR text of every file (no spans in it).
fn canonical(design: &ir::Design) -> String {
    design.files().map(|f| one_file(design, f)).collect()
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
