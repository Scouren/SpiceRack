//! The laws every dialect passes (REWRITE-PLAN §4.2), run for every row of
//! `dialect::BUILTIN` on the sides it has: L1 totality, L2 validity, L3 determinism (reader);
//! L4 writer totality, L7 feature honesty, L8 isolation (writer); L5 round trip (both); L6
//! corpus fixpoint (both, `--features corpus` only). L9 belongs to each dialect's own simulator
//! tests. In slice S2 the only row is `ir`.
//!
//! Oracles: generated designs (`ir::gen::generate(seed, row's features, size)`, valid and legal
//! for the set by construction) and their canonical IR text (`ir::dump`, which has no spans:
//! `≅`); fixed seeds, each printed in the failing assertion.
//!
//! Until the circuit and run owners' rows land, designs are generated over the kernel's item
//! kinds only (`KERNEL_KINDS`); then `kernel()` goes and each law runs on the writer's whole set.
//!
//! Contents: tests only.

use spicerack::dialect::{self, BUILTIN, DialectRow, Policy, Read, Sources, Write};
use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};

/// The seeds every generated law runs (printed on failure).
const SEEDS: std::ops::Range<u64> = 0..64;

/// The item kinds whose rows the kernel owns (bodies landed, M1–M4); the circuit and run
/// owners' kinds join when their rows land, then this set is the writer's whole set.
const KERNEL_KINDS: [ir::ItemKind; 11] = [
    ir::ItemKind::Title,
    ir::ItemKind::Comment,
    ir::ItemKind::Include,
    ir::ItemKind::Global,
    ir::ItemKind::Load,
    ir::ItemKind::Opaque,
    ir::ItemKind::Subckt,
    ir::ItemKind::Section,
    ir::ItemKind::If,
    ir::ItemKind::Param,
    ir::ItemKind::Func,
];

/// A moderate design: the merge tier's size (REWRITE-PLAN §7.5).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

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
fn l3_one_read_and_four_threads_give_the_same_bytes() {
    for (row, writer) in writers() {
        for seed in SEEDS.take(8) {
            let text = all_files(&generate(seed, &kernel(writer.features()), SIZE), row);
            let once = all_files(&read_text(row, &text).0, row);
            let threads: Vec<String> = std::thread::scope(|s| {
                let jobs: Vec<_> = (0..4)
                    .map(|_| s.spawn(|| all_files(&read_text(row, &text).0, row)))
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
fn l4_a_writer_refuses_nothing_it_claims() {
    for (row, writer) in writers() {
        for seed in SEEDS {
            let design = generate(seed, &kernel(writer.features()), SIZE);
            let mut diags = ir::Diags::new();
            let _ = dialect::write(&design, design.entry(), row, &mut diags);
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
    }
}

#[test]
fn l5_read_of_write_is_the_design() {
    for (row, writer) in writers() {
        if row.read.is_none() {
            continue;
        }
        for seed in SEEDS {
            let design = generate(seed, &kernel(writer.features()), SIZE);
            let text = all_files(&design, row);
            let back = read_text(row, &text).0;
            assert_eq!(
                canonical(&back),
                canonical(&design),
                "{} seed {seed}",
                row.name
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
            let twice = read_text(row, &all_files(&once, row)).0;
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
            let mut design = generate(seed, &kernel(all), SIZE);
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
            let design = generate(seed, &kernel(writer.features()), SIZE);
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

/// `set` without the item kinds the kernel does not own yet (what `generate` may build today).
fn kernel(set: &ir::FeatureSet) -> ir::FeatureSet {
    let others = ir::ItemKind::ALL
        .iter()
        .filter(|k| !KERNEL_KINDS.contains(k));
    let others = others.fold(ir::FeatureSet::EMPTY, |acc, &k| {
        acc.with(ir::Feature::Item(k))
    });
    set.minus(others)
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

/// Every file of `design` written by `row`, concatenated (the form one read takes back).
fn all_files(design: &ir::Design, row: &DialectRow) -> String {
    let mut diags = ir::Diags::new();
    design
        .files()
        .map(|f| dialect::write(design, f, row, &mut diags))
        .collect()
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
        // The kernel's kinds; the worked example of `text.rs` (elements, an analysis) joins
        // when the circuit and run owners' rows land.
        "ir" => vec![
            "file \"kit.sp\" deck\n  title \"kernel kinds\"\n  global vdd vss\n  param w = 2u\n  subckt cell (a b) r=w\n    param k = r * 2\n  if (w > 1u)\n    param m = 1\n  else\n    param m = 3\nfile \"models.lib\" library\n  section tt\n    param vth = 0.4\n".to_string(),
        ],
        _ => Vec::new(),
    }
}
