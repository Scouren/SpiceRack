//! The laws every dialect passes (REWRITE-PLAN §4.2), run for every row of
//! `dialect::BUILTIN` on the sides it has: L1 totality, L2 validity, L3 determinism (reader);
//! L4 writer totality over what `legalize` accepted, L7 feature honesty, L8 isolation (writer);
//! L5 round trip (both); L6 corpus fixpoint (both, `--features corpus` only). L9 belongs to each dialect's own simulator
//! tests. In slice S2 the only row is `ir`.
//!
//! Each file a writer writes is its own text at its own path, read back as one `Sources`
//! (entry first); a reader whose one text names its files (`Read::names_its_files`, the IR
//! text) reads them concatenated.
//!
//! Oracles: generated designs (`ir::gen::generate(seed, row's features, size)`, valid and legal
//! for the set by construction) and their canonical IR text (`ir::dump`, which has no spans:
//! `≅`); fixed seeds, each printed in the failing assertion.
//!
//! Each law runs on the writer's whole feature set.
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

/// The IR text read one file per text (`names_its_files` false): the harness's per-file path,
/// exercised before a dialect with that shape lands.
struct PerFile;

static PER_FILE: DialectRow = DialectRow {
    name: "ir-per-file",
    text_family: dialect::ANY_FAMILY,
    read: Some(&PerFile),
    write: Some(&PerFile),
};

impl Read for PerFile {
    fn references(&self, text: &str) -> Vec<dialect::Reference> {
        dialect::irtext::IrText.references(text)
    }

    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        dialect::irtext::IrText.read(src, out, diags);
    }
}

impl Write for PerFile {
    fn features(&self) -> &'static ir::FeatureSet {
        &ir::FeatureSet::ALL
    }

    fn write(
        &self,
        design: &ir::Design,
        file: ir::FileId,
        out: &mut String,
        diags: &mut ir::Diags,
    ) {
        dialect::irtext::IrText.write(design, file, out, diags);
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

#[test]
fn l1_an_empty_text_is_one_file_with_no_item() {
    for (row, _) in readers() {
        for text in ["", "\n"] {
            let (design, diags) = read_text(row, text);
            let files: Vec<ir::FileId> = design.files().collect();
            assert_eq!(files.len(), 1, "{}: {text:?} is one file", row.name);
            assert!(
                design.reachable().is_empty(),
                "{}: {text:?} holds no item",
                row.name
            );
            assert!(
                diags.is_empty(),
                "{}: {text:?}: {:?}",
                row.name,
                diags.rows()
            );
            assert_eq!(design.str(design.file(files[0]).path), "law");
        }
    }
}

#[test]
fn l3_one_read_and_four_threads_give_the_same_bytes() {
    for (row, writer) in writers() {
        for seed in SEEDS.take(8) {
            let src = written(&generate(seed, writer.features(), SIZE), row);
            let bytes = |d: &ir::Design| written(d, row).files().to_vec();
            let once = bytes(&read_src(row, &src));
            let threads: Vec<Vec<(String, String)>> = std::thread::scope(|s| {
                let jobs: Vec<_> = (0..4)
                    .map(|_| s.spawn(|| bytes(&read_src(row, &src))))
                    .collect();
                jobs.into_iter()
                    .map(|j| j.join().expect("a reader thread"))
                    .collect()
            });
            assert!(
                threads.iter().all(|t| *t == once),
                "{} seed {seed}",
                row.name
            );
        }
    }
}

#[test]
fn l4_write_is_total_over_a_design_legalize_accepted() {
    for (row, writer) in writers() {
        let claimed = *writer.features();
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
            assert!(
                refusals.is_empty(),
                "{} seed {seed}: {refusals:?}",
                row.name
            );
        }
        // The skip cannot make the law vacuous: every item kind the writer claims is admitted in
        // some generated design.
        for &kind in &ir::ItemKind::ALL {
            let f = ir::Feature::Item(kind);
            assert!(
                !claimed.contains(f) || seen.contains(f),
                "{}: no admitted design holds {kind:?} (skipped seeds {skipped:?})",
                row.name
            );
        }
    }
}

#[test]
fn l5_read_of_write_is_the_design() {
    for (row, writer) in writers() {
        if row.read.is_none() {
            continue;
        }
        for seed in SEEDS {
            let design = generate(seed, writer.features(), SIZE);
            let back = read_src(row, &written(&design, row));
            assert_eq!(
                canonical(&back),
                canonical(&design),
                "{} seed {seed}",
                row.name
            );
        }
    }
}

#[test]
fn l5_per_file_sources_carry_each_file_at_its_own_path() {
    for seed in SEEDS {
        let design = generate(seed, &ir::FeatureSet::ALL, SIZE);
        let src = written(&design, &PER_FILE);
        assert_eq!(
            src.files().len(),
            design.files().count(),
            "seed {seed}: one text per file"
        );
        let entry = design.str(design.file(design.entry()).path);
        assert_eq!(src.entries()[0].0, entry, "seed {seed}: the entry first");
        let back = read_src(&PER_FILE, &src);
        // The entry comes first, so compare file by file under each path.
        for f in design.files() {
            let path = design.str(design.file(f).path);
            let again = back.find_file(path);
            assert_eq!(
                one_file(&back, again),
                one_file(&design, f),
                "seed {seed}: {path}"
            );
        }
    }
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
            let twice = read_src(row, &written(&once, row));
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
    for (row, writer) in writers() {
        for f in all.minus(*writer.features()).iter() {
            let removed = spicerack::passes::PASSES.iter().any(|p| p.removes == f);
            assert!(
                removed,
                "{} lacks {f:?}: no pass removes it, and no citation",
                row.name
            );
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
            assert_eq!(
                left, refused,
                "{} seed {seed}: what is left is what is refused",
                row.name
            );
        }
    }
}

#[test]
fn l8_a_file_is_written_the_same_whatever_the_other_files() {
    let ir_row = dialect::by_name("ir").expect("the ir row");
    for (row, writer) in writers() {
        for seed in SEEDS {
            let design = generate(seed, writer.features(), SIZE);
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
            assert_eq!(again, first, "{} seed {seed}", row.name);
        }
    }
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

/// Every file of `design` written by `row`, as the sources one read takes back: each file its
/// own text at the path the design gives it, the entry first (in its role); or, for a reader
/// whose one text names its files (`Read::names_its_files`), the texts concatenated.
fn written(design: &ir::Design, row: &DialectRow) -> Sources {
    let mut diags = ir::Diags::new();
    let entry = design.entry();
    let order = std::iter::once(entry).chain(design.files().filter(|&f| f != entry));
    let texts: Vec<(String, String)> = order
        .map(|f| {
            (
                design.str(design.file(f).path).to_string(),
                dialect::write(design, f, row, &mut diags),
            )
        })
        .collect();
    if row.read.is_some_and(|r| r.names_its_files()) {
        return Sources::one(
            "law",
            &texts.iter().map(|(_, t)| t.as_str()).collect::<String>(),
        );
    }
    let mut src = Sources::new();
    for (path, text) in &texts {
        src.add(path, text);
    }
    src.set_form(match design.file(entry).role {
        ir::Role::Deck => ir::Role::Deck,
        ir::Role::Library | ir::Role::VerilogA | ir::Role::Missing => ir::Role::Library,
    });
    src
}

/// `src` read by `row`, leniently.
fn read_src(row: &DialectRow, src: &Sources) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    design
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
