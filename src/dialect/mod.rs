//! The contract every dialect implements and the pipeline that calls it: text of one dialect →
//! the IR (`Read`), the IR → text of one dialect (`Write`), one row per language (`BUILTIN`),
//! and the three spines (`read`, `write`, `transpile`) that hold every branch on which work
//! happens. A reader never learns its target and a writer never learns its source: a `Design`
//! holds no source text and no source-dialect tag (REWRITE-PLAN §2.2 rule 1).
//!
//! The caller's code (REWRITE-PLAN §2.4, cases 1 and 2):
//!
//! ```ignore
//! use spicerack::dialect::{self, Policy, Sources};
//! use spicerack::ir;
//!
//! // 1. Text in, text out.
//! let (from, to) = (dialect::by_name("ir").expect("a row"), dialect::by_name("ir").expect("a row"));
//! let out = dialect::transpile(&Sources::one("deck.ir", text), from, to, Policy::Lenient)?;
//! print!("{}", out.text);
//!
//! // 2. A kit. The caller owns file I/O: requests, no callback.
//! let reader = from.read.expect("a reader");
//! let mut src = Sources::new();
//! let mut todo = vec![entry.to_string()];
//! while let Some(path) = todo.pop() {
//!     let text = my_load(&path)?;
//!     todo.extend(reader.references(&text).iter().map(|r| r.resolve(&path)).filter(|p| !src.has(p)));
//!     src.add(&path, &text);
//! }
//! let (mut design, mut diags) = (ir::Design::new(), ir::Diags::new());
//! dialect::read(&src, from, Policy::Lenient, &mut design, &mut diags);
//! for def in design.definitions() { /* kind, name, scope */ }
//! for file in design.files() { let text = dialect::write(&design, file, to, &mut diags); }
//! ```
//!
//! Cites: `REWRITE-PLAN.md` §2.1–§2.4, §3.4, §4.1; `rewrite/SPINE.md` §2–§3.
//!
//! Contents:
//! - `pub trait Read` (`Read::references`, `Read::read`), `pub trait Write` (`Write::features`,
//!   `Write::write`)
//! - `pub struct DialectRow`, `pub static BUILTIN: &[DialectRow]`, `pub const ANY_FAMILY: &str`
//! - `pub enum Policy`, `pub struct Written`, `pub const STRICT_UNREAD: ir::Code`
//! - `pub use sources::{DEPTH_MAX, Reference, Sources}`; `pub mod facade`, `pub mod irtext`
//! - `pub fn by_name(name: &str) -> Option<&'static DialectRow>`
//! - `pub fn read(src: &Sources, from: &DialectRow, policy: Policy, design: &mut ir::Design, diags: &mut ir::Diags)`
//! - `pub fn write(design: &ir::Design, file: ir::FileId, to: &DialectRow, diags: &mut ir::Diags) -> String`
//! - `pub fn transpile(src: &Sources, from: &DialectRow, to: &DialectRow, policy: Policy) -> Result<Written, ir::Diags>`
//! - `pub fn info(code: ir::Code) -> Option<ir::Info>`, `pub fn refuses(code: ir::Code) -> bool`

pub mod facade;
pub mod irtext;
mod sources;

pub use sources::{DEPTH_MAX, Reference, Sources};

use crate::ir;
use crate::ir::ItemRow as _;
use crate::passes;

/// Text of one dialect → the IR.
pub trait Read: Sync {
    /// The files `text` names (includes, library calls, loads), in the order it names them.
    /// No I/O: the caller resolves and loads them (`Reference::resolve`, `Sources::load`).
    fn references(&self, text: &str) -> Vec<Reference>;

    /// Every statement of every file of `src` becomes IR items, or an `Opaque` item with a
    /// coded diagnostic. Total: never panics on input, never drops a statement, never learns a
    /// target. A reader of a SPICE-family dialect creates the design's files in `src`'s order
    /// (the facade anchors diagnostics by it).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags);
}

/// The IR → text of one dialect.
pub trait Write: Sync {
    /// The IR features this dialect spells.
    fn features(&self) -> &'static ir::FeatureSet;

    /// Spells one file of a design that is legal for `features()` (the spine checked it). Sees
    /// only the IR. Total over legal IR: every item is spelled, or copied when it is this
    /// family's `Opaque` (another family's never reaches it: [`write`] refuses it first).
    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, diags: &mut ir::Diags);
}

/// One language. CDL and Verilog have no reader.
pub struct DialectRow {
    /// The registry name (`ir`, `ngspice`, …): what `by_name`, the route table and the CLI use.
    pub name: &'static str,
    /// The dialect whose `Opaque` text this one's writer may copy (itself, or its parent:
    /// ESPice copies ngspice's); [`ANY_FAMILY`] for a writer that spells every dialect's.
    pub text_family: &'static str,
    /// Its reader, if it has one.
    pub read: Option<&'static dyn Read>,
    /// Its writer, if it has one.
    pub write: Option<&'static dyn Write>,
}

/// Every built-in language: the IR's own text first; each dialect slice appends its row.
pub static BUILTIN: &[DialectRow] = &[DialectRow {
    name: "ir",
    text_family: ANY_FAMILY,
    read: Some(&irtext::IrText),
    write: Some(&irtext::IrText),
}];

/// The `text_family` of a writer whose text holds an `Opaque` of any dialect (the IR text,
/// which is lossless).
pub const ANY_FAMILY: &str = "*";

/// What a read does with a statement its reader has no rule for (`OpaqueClass::Unread`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    /// Refuses the deck (`from_spice`, D26): one [`STRICT_UNREAD`] row per `Unread` item.
    Strict,
    /// Keeps it (kits, corpora): a writer of the same text family copies it, any other
    /// refuses it.
    Lenient,
}

/// A written file and every diagnostic on the way, none of which refuses.
#[derive(Clone, Debug)]
pub struct Written {
    /// The file's text in the target dialect.
    pub text: String,
    /// Notes from the reader, the passes and the writer, in append order.
    pub notes: ir::Diags,
}

/// The refusal a strict read adds at each `Unread` item: `S0101`, `arg` 0. The spine owns
/// the policy, so it owns the code: its catalogue is `S0101..=S0199` ([`info`]).
pub const STRICT_UNREAD: ir::Code = ir::Code::new(b'S', 101);

/// The spine's catalogue rows, in code order.
const SPINE_CODES: &[(ir::Code, ir::Info)] = &[(
    STRICT_UNREAD,
    ir::Info {
        title: "a statement the reader has no rule for, under a strict read",
        cite: "REWRITE-PLAN.md §3.4 (strictness); D26 (`from_spice` is strict)",
        explain: "The reader kept the statement as an `Opaque` of class `Unread` (its own note says which); a strict read refuses the deck instead of carrying text it did not read. Read leniently to keep it, or give the reader a rule.",
        kind: ir::Kind::Refusal,
    },
)];

/// The row named `name`, or `None`.
pub fn by_name(name: &str) -> Option<&'static DialectRow> {
    BUILTIN.iter().find(|row| row.name == name)
}

/// `src` read by `from` into `design`, then checked. A check violation is a bug in the
/// reader: an `X` row, and a debug assertion. Under [`Policy::Strict`] each `Unread` item
/// gets a [`STRICT_UNREAD`] row.
///
/// # Panics
/// When `from` has no reader (the caller checks the row) or `src` holds no file.
pub fn read(
    src: &Sources,
    from: &DialectRow,
    policy: Policy,
    design: &mut ir::Design,
    diags: &mut ir::Diags,
) {
    let reader = from
        .read
        .expect("read: a row with a reader (the caller checks it)");
    assert!(!src.files().is_empty(), "read: sources with a file");
    reader.read(src, &mut design.builder(), diags);
    let checked = diags.len();
    ir::check(design, diags);
    debug_assert!(
        diags.rows()[checked..]
            .iter()
            .all(|d| d.code.letter != b'X'),
        "read: the {} reader made IR that fails ir::check",
        from.name
    );
    if policy == Policy::Strict {
        let table = ir::item::Opaque::table(design);
        for (i, row) in table.rows().iter().enumerate() {
            if row.class == ir::OpaqueClass::Unread {
                diags.push(STRICT_UNREAD, table.span(index(i)), 0);
            }
        }
    }
}

/// `file` of `design` written by `to`. A design that uses only the target's features goes to
/// the writer as it is; any other is cloned and legalized (`passes::legalize`, whose
/// `ir::check_legal` reports what is left). An `Opaque` of another text family is refused here (a `U0001` row for
/// `Feature::Item(Opaque)` at the item). On any refusal the writer is not called and the text
/// is empty.
///
/// # Panics
/// When `to` has no writer (the caller checks the row).
pub fn write(
    design: &ir::Design,
    file: ir::FileId,
    to: &DialectRow,
    diags: &mut ir::Diags,
) -> String {
    let writer = to
        .write
        .expect("write: a row with a writer (the caller checks it)");
    let target = writer.features();
    let mut out = String::new();
    let foreign = refuse_foreign(design, file, to, diags);
    if ir::features_used(design).minus(*target).is_empty() {
        if foreign == 0 {
            writer.write(design, file, &mut out, diags);
        }
        return out;
    }
    let mut legal = design.clone();
    passes::legalize(&mut legal, target, diags);
    if foreign == 0 && ir::features_used(&legal).minus(*target).is_empty() {
        writer.write(&legal, file, &mut out, diags);
    }
    out
}

/// `src` read by `from` and its entry file written by `to`, with the verdict: any row whose
/// code [`refuses`] refuses the deck. A read that refused is not written.
///
/// # Errors
/// Every diagnostic, when one refuses.
pub fn transpile(
    src: &Sources,
    from: &DialectRow,
    to: &DialectRow,
    policy: Policy,
) -> Result<Written, ir::Diags> {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    read(src, from, policy, &mut design, &mut diags);
    if refused(&diags) {
        return Err(diags);
    }
    let text = write(&design, design.entry(), to, &mut diags);
    if refused(&diags) {
        return Err(diags);
    }
    Ok(Written { text, notes: diags })
}

/// The catalogue row of `code`: the spine's own, else the IR crate's; `None` for a code no
/// catalogue holds (each dialect slice adds its table here when it lands).
pub fn info(code: ir::Code) -> Option<ir::Info> {
    let spine = SPINE_CODES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, i)| *i);
    spine.or_else(|| ir::diag::info(code))
}

/// Whether `code` refuses a deck: its row is a refusal, or no catalogue holds it (a missing
/// row fails loudly, never silently).
pub fn refuses(code: ir::Code) -> bool {
    info(code).is_none_or(|info| info.kind == ir::Kind::Refusal)
}

// ---- private ----

/// Whether any row of `diags` refuses.
fn refused(diags: &ir::Diags) -> bool {
    diags.rows().iter().any(|d| refuses(d.code))
}

/// Pushes a `U0001` row at each `Opaque` of `file` whose dialect is not `to`'s text family;
/// returns how many. An item with no span (built) counts as `file`'s.
fn refuse_foreign(
    design: &ir::Design,
    file: ir::FileId,
    to: &DialectRow,
    diags: &mut ir::Diags,
) -> usize {
    if to.text_family == ANY_FAMILY {
        return 0;
    }
    let feature = u32::from(ir::Feature::Item(ir::ItemKind::Opaque).index());
    let table = ir::item::Opaque::table(design);
    let mut refused = 0;
    for (i, row) in table.rows().iter().enumerate() {
        let span = table.span(index(i));
        let here = span.file == file || span.file.is_none();
        if here && design.str(row.dialect) != to.text_family {
            diags.push(ir::diag::UNSUPPORTED, span, feature);
            refused += 1;
        }
    }
    assert!(refused <= table.len(), "at most one refusal per opaque row");
    refused
}

/// A table position as the IR's `u32` row index.
fn index(i: usize) -> u32 {
    u32::try_from(i).expect("an IR table holds fewer than 2^32 rows (ROWS_MAX)")
}
