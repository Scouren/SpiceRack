//! The dialect laws' harness (`tests/dialect_laws.rs` includes it by path; `common/mod.rs` does
//! not): the rows, the set a law generates over, a generated design shrunk to what the row
//! admits, the tally of what a law ran on, and the write/read helpers every law shares. A
//! module of its own for the 500-line cap.
//!
//! Shrink to admitted (A25, R48): a generated design the row refuses is not skipped. The items
//! its refusals name (`Diag::item`) are dropped from their listing scopes, then the items
//! `ir::check` no longer accepts, to a fixpoint bounded by the item count, and the law runs on
//! the rest. A refusal that names no item, or names one the design does not list (a pass made
//! it), skips the seed; the tally counts skips by code for the failure text.
//!
//! Contents: `pub const SIZE: Size`, `pub struct Tally`, `Tally::new() -> Tally`,
//! `Tally::admit(&mut self)`, `Tally::skip(&mut self, String)`,
//! `Tally::some(&self) -> Result<(), String>`,
//! `pub fn generated(row: &DialectRow, writer: &dyn Write) -> ir::FeatureSet`,
//! `pub const PARAMS: [Params; 2]`,
//! `pub fn admitted(seed: u64, params: Params, row: &DialectRow, writer: &dyn Write) -> Result<ir::Design, String>`,
//! `pub fn write_all(design: &ir::Design, row: &DialectRow) -> Sources`,
//! `pub fn read_all(row: &DialectRow, src: &Sources) -> ir::Design`,
//! `pub fn read_text(row: &DialectRow, text: &str) -> (ir::Design, ir::Diags)`,
//! `pub fn readers() -> impl Iterator`, `pub fn writers() -> impl Iterator`,
//! `pub fn one_file(design: &ir::Design, file: ir::FileId) -> String`,
//! `pub fn canonical(design: &ir::Design) -> String`,
//! `pub fn same_meaning(a: &ir::Design, b: &ir::Design) -> bool`

use std::collections::BTreeMap;

use spicerack::dialect::{self, BUILTIN, DialectRow, Policy, Read, Sources, Write};
use spicerack::ir;
use spicerack::ir::r#gen::{Params, Size, generate_with};

/// A moderate design: the merge tier's size (REWRITE-PLAN §7.5).
pub const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// Both generator settings (A28), the laws' outer loop: untyped parameters drawn, and none.
pub const PARAMS: [Params; 2] = [Params::Untyped, Params::TypedOnly];

/// What a law ran on: the designs admitted, and the seeds skipped by the code that skipped
/// them.
#[derive(Debug, Default)]
pub struct Tally {
    admitted: usize,
    skipped: BTreeMap<String, usize>,
}

impl Tally {
    /// Nothing yet.
    pub fn new() -> Tally {
        Tally::default()
    }

    /// One design the law ran on.
    pub fn admit(&mut self) {
        self.admitted += 1;
    }

    /// One seed skipped, by the code that skipped it.
    pub fn skip(&mut self, code: String) {
        *self.skipped.entry(code).or_default() += 1;
    }

    /// `Ok` when the law ran on at least one design; else the skips by code.
    pub fn some(&self) -> Result<(), String> {
        match self.admitted {
            0 => Err(format!(
                "the law ran on nothing; seeds skipped by code: {:?}",
                self.skipped
            )),
            _ => Ok(()),
        }
    }
}

/// The set a law generates over: the writer's features, without `Opaque` for a row that is not
/// lossless (an opaque's class is its reader's judgement of the text, which a generated text
/// cannot carry).
pub fn generated(row: &DialectRow, writer: &dyn Write) -> ir::FeatureSet {
    let set = *writer.features();
    if row.text_family == dialect::ANY_FAMILY {
        return set;
    }
    set.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Opaque)))
}

/// Design `seed` under `params` over [`generated`], shrunk to what `row` admits and legalized for it; `Err`
/// with the code of a refusal that names no item the design lists.
pub fn admitted(
    seed: u64,
    params: Params,
    row: &DialectRow,
    writer: &dyn Write,
) -> Result<ir::Design, String> {
    let mut design = generate_with(seed, &generated(row, writer), SIZE, params);
    let bound = design.reachable().len() + 1;
    for _ in 0..=bound {
        let mut trial = design.clone();
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut trial, row, &mut diags);
        let refusing: Vec<ir::Diag> = diags
            .rows()
            .iter()
            .filter(|d| dialect::refuses(d.code))
            .copied()
            .collect();
        if refusing.is_empty() {
            return Ok(trial);
        }
        drop_named(&mut design, &refusing)?;
        // Then what `check` no longer accepts, to a fixpoint (each round drops at least one).
        for _ in 0..=bound {
            let mut found = ir::Diags::new();
            ir::check(&design, &mut found);
            if found.is_empty() {
                break;
            }
            drop_named(&mut design, found.rows())?;
        }
    }
    Err("no fixpoint within the item count".to_string())
}

/// Every file of `design` written by `row` as one source set: each file its own text at the
/// path the design gives it, the entry first (its role setting the form), then file order.
pub fn write_all(design: &ir::Design, row: &DialectRow) -> Sources {
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
pub fn read_all(row: &DialectRow, src: &Sources) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(src, row, Policy::Lenient, &mut design, &mut diags);
    design
}

/// `text` read by `row`, leniently, as one file.
pub fn read_text(row: &DialectRow, text: &str) -> (ir::Design, ir::Diags) {
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

/// The rows with a reader.
pub fn readers() -> impl Iterator<Item = (&'static DialectRow, &'static dyn Read)> {
    assert!(
        BUILTIN.iter().any(|r| r.read.is_some()),
        "a row with a reader"
    );
    BUILTIN.iter().filter_map(|r| r.read.map(|read| (r, read)))
}

/// The rows with a writer.
pub fn writers() -> impl Iterator<Item = (&'static DialectRow, &'static dyn Write)> {
    assert!(
        BUILTIN.iter().any(|r| r.write.is_some()),
        "a row with a writer"
    );
    BUILTIN
        .iter()
        .filter_map(|r| r.write.map(|write| (r, write)))
}

/// One file as IR text.
pub fn one_file(design: &ir::Design, file: ir::FileId) -> String {
    let mut out = String::new();
    ir::dump(design, file, &mut out);
    out
}

/// `≅`: the canonical IR text of every file (no spans in it).
pub fn canonical(design: &ir::Design) -> String {
    design.files().map(|f| one_file(design, f)).collect()
}

/// `≅` for L5/L6 (R28): both designs in normal form (written as IR text and read back, so
/// their ids follow the canonical order whatever order a reader pushed in), then
/// `Design::eq_meaning` (real literals equal by value).
pub fn same_meaning(a: &ir::Design, b: &ir::Design) -> bool {
    let ir_row = dialect::by_name("ir").expect("the ir row");
    let normal = |d: &ir::Design| read_all(ir_row, &write_all(d, ir_row));
    normal(a).eq_meaning(&normal(b))
}

// ---- private ----

/// Drops the items `rows` name from their listing scopes (`Editor::remove_item`, so their
/// definitions leave the index too); `Err` with the code of a row that
/// names none, or one `design` does not list.
fn drop_named(design: &mut ir::Design, rows: &[ir::Diag]) -> Result<(), String> {
    let listing = design.reachable();
    let mut drop = Vec::new();
    for d in rows {
        let at = listing.iter().find(|(i, _)| *i == d.item);
        match at {
            Some(&found) if d.item.index != u32::MAX => {
                // Two rows may name one item: it is removed once.
                if !drop.contains(&found) {
                    drop.push(found);
                }
            }
            _ => return Err(d.code.to_string()),
        }
    }
    assert!(
        !drop.is_empty(),
        "drop_named: at least one row names an item"
    );
    let mut editor = design.editor();
    for (item, scope) in &drop {
        editor.remove_item(*scope, *item);
    }
    Ok(())
}
