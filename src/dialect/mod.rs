//! The contract every dialect implements and the pipeline that calls it: text of one dialect →
//! the IR (`Read`), the IR → text of one dialect (`Write`), one row per language (`BUILTIN`),
//! and the four spines (`read`, `legalize`, `write`, `transpile`) that hold every branch on which work
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
//! dialect::legalize(&mut design, to, &mut diags);   // once per design and target
//! for file in design.files() { let text = dialect::write(&design, file, to, &mut diags); }
//! ```
//!
//! Cites: `REWRITE-PLAN.md` §2.1–§2.4, §3.4, §4.1; `rewrite/SPINE.md` §2–§3.
//!
//! Contents:
//! - `pub trait Read` (`Read::references`, `Read::read`),
//!   `pub trait Write` (`Write::features`, `Write::write`, `Write::admits`)
//! - `pub struct DialectRow`, `pub static BUILTIN: &[DialectRow]`, `pub const ANY_FAMILY: &str`
//! - `pub enum Policy`, `pub struct Written`, `pub const STRICT_UNREAD: ir::Code`,
//!   `pub const INERT_DROPPED: ir::Code`, `pub const HINT_DROPPED: ir::Code`
//! - `pub use sources::{DEPTH_MAX, Reference, Sources}`; `pub mod facade`, `pub mod irtext`
//! - `pub fn by_name(name: &str) -> Option<&'static DialectRow>`
//! - `pub fn read(src: &Sources, from: &DialectRow, policy: Policy, design: &mut ir::Design, diags: &mut ir::Diags)`
//! - `pub fn legalize(design: &mut ir::Design, to: &DialectRow, diags: &mut ir::Diags)`
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
    /// target. A reader creates a file for every text of `src`, in `src`'s order, the entry
    /// first, even when the text is empty (the facade anchors diagnostics by that order, and
    /// `transpile` writes the entry).
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags);
}

/// The IR → text of one dialect.
pub trait Write: Sync {
    /// The IR features this dialect spells.
    fn features(&self) -> &'static ir::FeatureSet;

    /// Spells one file of a design that is legal for `features()` (the spine checked it). Sees
    /// only the IR. Total over legal IR: every item is spelled, or copied when it is this
    /// family's `Opaque` (another family's never reaches it: [`legalize`] drops or refuses it).
    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, diags: &mut ir::Diags);

    /// Appends a refusal for everything in `design`, legal by `features()`, that this dialect
    /// cannot spell for a reason no feature states (a model parameter its simulator's module does
    /// not have, a name it cannot quote). Never for something a feature could state. Default:
    /// nothing. [`legalize`] calls it last, always (AMEND-A12 S1, R21).
    fn admits(&self, _design: &ir::Design, _diags: &mut ir::Diags) {}
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

/// The note [`legalize`] adds where it drops an `Inert` statement of another dialect: `S0102`,
/// `arg` 0.
pub const INERT_DROPPED: ir::Code = ir::Code::new(b'S', 102);

/// The note [`legalize`] adds where it drops a solver hint of another dialect (`Hint`, R19):
/// `S0103`, `arg` 0.
pub const HINT_DROPPED: ir::Code = ir::Code::new(b'S', 103);

/// The spine's catalogue rows, in code order.
const SPINE_CODES: &[(ir::Code, ir::Info)] = &[
    (
        STRICT_UNREAD,
        ir::Info {
            title: "a statement the reader has no rule for, under a strict read",
            cite: "REWRITE-PLAN.md §3.4 (strictness); D26 (`from_spice` is strict)",
            explain: "The reader kept the statement as an `Opaque` of class `Unread` (its own note says which); a strict read refuses the deck instead of carrying text it did not read. Read leniently to keep it, or give the reader a rule.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        INERT_DROPPED,
        ir::Info {
            title: "an inert statement of another dialect was dropped",
            cite: "rewrite/RULINGS.md R11; REWRITE-PLAN.md §3.4 (`Inert` changes no number)",
            explain: "The statement is another dialect's text that its own simulator runs to the same numbers without; the target cannot spell it, so it is left out. The deck is still written.",
            kind: ir::Kind::Note,
        },
    ),
    (
        HINT_DROPPED,
        ir::Info {
            title: "a solver hint of another dialect was dropped",
            cite: "rewrite/RULINGS.md R19, R11; AMEND-A12 (`OpaqueClass::Hint`)",
            explain: "The statement tunes another simulator's solver (tolerances, iteration limits, a method); it changes no meaning of the circuit, so it is left out. The numbers may move within the target solver's own tolerance. The deck is still written.",
            kind: ir::Kind::Note,
        },
    ),
];

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

/// `design` made legal for `to`, once per (design, target), in place (clone first to keep the
/// neutral design or to write a second target). Every `Opaque` of another text family is
/// dropped from its scope with an [`INERT_DROPPED`] note when its class is `Inert` (it moves no
/// number), with a [`HINT_DROPPED`] note when it is `Hint` (solver tuning: numbers may move
/// within the solver's tolerance), and refused otherwise (`U0001`, `arg` = `Feature::Item(Opaque)`, at the item); a
/// writer of [`ANY_FAMILY`] skips that step. Then `passes::legalize` with the writer's features,
/// whose `ir::check_legal` reports what no pass removed. Last, always, the writer's own
/// [`Write::admits`], so the caller sees every refusal at once. Legality is of the whole design.
///
/// # Panics
/// When `to` has no writer (the caller checks the row).
pub fn legalize(design: &mut ir::Design, to: &DialectRow, diags: &mut ir::Diags) {
    let writer = to
        .write
        .expect("legalize: a row with a writer (the caller checks it)");
    let opaque = u32::from(ir::Feature::Item(ir::ItemKind::Opaque).index());
    let mut dropped = Vec::new();
    for (item, scope) in foreign(design, to) {
        let row: ir::item::Opaque = design.row(item.index);
        let span = design.span_of(item);
        match row.class {
            ir::OpaqueClass::Inert => {
                diags.push(INERT_DROPPED, span, 0);
                dropped.push((item, scope));
            }
            ir::OpaqueClass::Hint => {
                diags.push(HINT_DROPPED, span, 0);
                dropped.push((item, scope));
            }
            ir::OpaqueClass::Unread
            | ir::OpaqueClass::Private
            | ir::OpaqueClass::InvalidInSource
            | ir::OpaqueClass::Script => diags.push(ir::diag::UNSUPPORTED, span, opaque),
        }
    }
    if !dropped.is_empty() {
        let mut editor = design.editor();
        for (item, scope) in &dropped {
            editor.items_mut(*scope).retain(|i| i != item);
        }
    }
    passes::legalize(design, writer.features(), diags);
    writer.admits(design, diags);
}

/// `file` of a design [`legalize`] accepted for `to`, written by `to`'s writer; nothing else
/// (no clone, no second legalization). A design that is not legal for `to` is the caller's bug.
///
/// # Panics
/// When `to` has no writer (the caller checks the row); in debug builds, when `design` uses a
/// feature the writer lacks, holds a foreign `Opaque`, or is refused by the writer's `admits`.
pub fn write(
    design: &ir::Design,
    file: ir::FileId,
    to: &DialectRow,
    diags: &mut ir::Diags,
) -> String {
    let writer = to
        .write
        .expect("write: a row with a writer (the caller checks it)");
    debug_assert!(
        ir::features_used(design)
            .minus(*writer.features())
            .is_empty(),
        "write: a design legalize accepted for {}",
        to.name
    );
    debug_assert!(
        foreign(design, to).is_empty(),
        "write: no foreign Opaque for {}",
        to.name
    );
    #[cfg(debug_assertions)]
    {
        let mut refused = ir::Diags::new();
        writer.admits(design, &mut refused);
        assert!(refused.is_empty(), "write: {} admits the design", to.name);
    }
    let mut out = String::new();
    writer.write(design, file, &mut out, diags);
    out
}

/// `src` read by `from`, legalized for `to` and its entry file written, with the verdict: any
/// row whose code [`refuses`] refuses the deck, and no step runs after a refusal.
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
    legalize(&mut design, to, &mut diags);
    if refused(&diags) {
        return Err(diags);
    }
    let entry = design.entry();
    assert!(
        entry != ir::FileId::NONE,
        "transpile: the {} reader made no file",
        from.name
    );
    let text = write(&design, entry, to, &mut diags);
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

/// Every reachable `Opaque` whose dialect is not `to`'s text family, with its listing scope;
/// none for [`ANY_FAMILY`].
fn foreign(design: &ir::Design, to: &DialectRow) -> Vec<(ir::ItemRef, ir::ScopeId)> {
    if to.text_family == ANY_FAMILY {
        return Vec::new();
    }
    let mut out = design.reachable();
    out.retain(|(item, _)| {
        item.kind == ir::ItemKind::Opaque
            && design.str(design.row::<ir::item::Opaque>(item.index).dialect) != to.text_family
    });
    out
}

/// A table position as the IR's `u32` row index.
fn index(i: usize) -> u32 {
    u32::try_from(i).expect("an IR table holds fewer than 2^32 rows (ROWS_MAX)")
}
