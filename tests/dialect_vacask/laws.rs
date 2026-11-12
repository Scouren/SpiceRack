//! Law L5 for the `vacask` row on generated designs, until the shared harness can run it: the
//! generator makes `Opaque` rows of other dialects only (`gen_rows.rs`), which `legalize` refuses
//! for every target, so `tests/dialect_laws.rs` skips every design for `vacask`. Here the set is
//! VACASK's `FEATURES` without `Item(Opaque)`, and without `Item(DeviceLibrary)` until A15's
//! `Family` feature lets `FEATURES` name the families VACASK ships (the generator draws any, and
//! `admits` refuses the rest); the oracle is the canonical IR text.
//!
//! Contents: tests only.

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};

/// The harness's merge-tier size (`tests/dialect_laws.rs`).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

// ---- private ----

/// The canonical IR text of every file.
fn canonical(design: &ir::Design) -> String {
    let mut out = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut out);
    }
    out
}

#[test]
// marker: the generator's designs are almost never VACASK-spellable until these land, so the
// law admits too few (it fails, and goes red as an unexpected pass when they do): the kernel's
// `DeckWithoutTitle` and the spine's `deck_title` pass; A15's `Placed` (a `global`, `include`,
// `load` only where VACASK allows it) and `Family`; a feature for a formal with no default or a
// default of `$temp` (W2003; run c2); a feature for a scale letter VACASK lacks (`c`, `peta`).
#[should_panic(expected = "designs admitted")]
fn l5_read_of_write_is_the_design_without_foreign_opaques() {
    let row = dialect::by_name("vacask").expect("the vacask row");
    let set = row.write.expect("a writer").features().minus(
        ir::FeatureSet::EMPTY
            .with(ir::Feature::Item(ir::ItemKind::Opaque))
            .with(ir::Feature::Item(ir::ItemKind::DeviceLibrary)),
    );
    let mut failed = Vec::new();
    let mut admitted = 0;
    let mut refused = Vec::new();
    for seed in 0..64 {
        let mut design = generate(seed, &set, SIZE);
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row, &mut diags);
        if let Some(d) = diags.rows().iter().find(|d| dialect::refuses(d.code)) {
            refused.push(format!("{:?} arg {}", d.code, d.arg));
            continue;
        }
        // ponytail: a deck with no title until the kernel's `DeckWithoutTitle` (VACASK lacks it)
        // and the spine's `deck_title` pass land: VACASK's line 1 is always a title.
        let entry_top = design.file(design.entry()).top;
        let titled = design
            .items(entry_top)
            .iter()
            .any(|i| i.kind == ir::ItemKind::Title);
        if design.file(design.entry()).role == ir::Role::Deck && !titled {
            refused.push("no title".to_string());
            continue;
        }
        admitted += 1;
        let entry = design.entry();
        let mut src = Sources::new();
        for f in std::iter::once(entry).chain(design.files().filter(|&f| f != entry)) {
            let text = dialect::write(&design, f, row, &mut diags);
            src.add(design.str(design.file(f).path), &text);
        }
        src.set_form(match design.file(entry).role {
            ir::Role::Deck => ir::Role::Deck,
            _ => ir::Role::Library,
        });
        let (mut back, mut d2) = (ir::Design::new(), ir::Diags::new());
        dialect::read(&src, row, Policy::Lenient, &mut back, &mut d2);
        if canonical(&back) != canonical(&design) {
            let texts: String = src
                .files()
                .iter()
                .map(|(p, t)| format!("== {p}\n{t}"))
                .collect();
            let (b, d) = (canonical(&back), canonical(&design));
            let first = b.lines().zip(d.lines()).find(|(x, y)| x != y);
            failed.push(format!(
                "seed {seed}: first difference {first:?}\n--- written\n{texts}"
            ));
        }
    }
    let firsts: Vec<&str> = failed
        .iter()
        .map(|f| f.lines().next().unwrap_or(""))
        .collect();
    assert!(
        failed.is_empty(),
        "{} failed:\n{}\n{}",
        failed.len(),
        firsts.join("\n"),
        failed[0]
    );
    // ponytail: most designs hold a formal with no default, which `admits` refuses until the
    // kernel has a feature for it (W2003); the law is not vacuous while some are admitted.
    assert!(
        admitted >= 4,
        "only {admitted} of 64 designs admitted: {refused:?}"
    );
}
