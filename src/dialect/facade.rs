//! The seam where the old pipeline and the new one meet: a (source, target) pair in → which
//! pipeline writes it (`route`), and a routed call's result in today's types out
//! (`render_text`, `render_kit`, `diags`). `syntax::render::render_text` and `render_kit` ask
//! `route` first and keep their signatures, so the Python binding, the CLI, `golden` and every
//! test keep calling the old entry points while pairs flip one by one (the strangler).
//!
//! A pair goes to the new pipeline when it is in [`ROUTES`] (empty in slice S2), or when
//! `SPICERACK_PIPELINE=new` and both rows have the side it needs; `SPICERACK_PIPELINE=old`
//! sends every pair to the old one. A new diagnostic code is shown as today's through
//! [`OLD_CODE`] (a row exists only where a routed test asserts an old code), else as
//! [`IR_REFUSAL`] or [`IR_NOTE`] by its verdict, its message carrying the new code and title.
//!
//! Cites: `rewrite/SPINE.md` §5; `REWRITE-PLAN.md` §7.2.
//!
//! Contents:
//! - `pub enum Pipeline`, `pub const ENV: &str`, `pub const ROUTES: &[(&str, &str)]`,
//!   `pub struct OldCode`, `pub static OLD_CODE: &[OldCode]`, `pub const IR_REFUSAL: Code`,
//!   `pub const IR_NOTE: Code`
//! - `pub fn route(from: &str, to: &str) -> Option<(&'static DialectRow, &'static DialectRow)>`
//! - `pub fn route_with(from: &str, to: &str, pipeline: Option<Pipeline>) -> Option<(&'static DialectRow, &'static DialectRow)>`
//! - `pub fn render_text(text: &str, name: &str, from: &DialectRow, to: &DialectRow) -> Result<render::Written, render::Diags>`
//! - `pub fn render_kit(kit: &Kit, file: FileId, name: &str, from: &DialectRow, to: &DialectRow) -> Result<render::Written, render::Diags>`
//! - `pub fn diags(rows: &[ir::Diag], src: &Sources, name: &str, target: &str, citations: &[Citation]) -> render::Diags`
//! - `pub fn old_code(d: &ir::Diag, target: &str) -> Code`

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use super::{Citation, DialectRow, Policy, Roots, Sources, by_name, transpile, transpile_with};
use crate::ir;
use crate::syntax::diag::{self, Code};
use crate::syntax::render;
use crate::syntax::resolve::{FileId, Kit, Role};

/// Which pipeline writes a pair, when the harness forces one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pipeline {
    /// `syntax::render`, today's.
    Old,
    /// `dialect::transpile`, where both rows have the side the pair needs.
    New,
}

/// The variable that forces a pipeline: `old` or `new`; unset, the route table decides.
pub const ENV: &str = "SPICERACK_PIPELINE";

/// The (source, target) pairs the new pipeline writes. A pair is listed only when both rows
/// are in `BUILTIN` with the side it needs (`tests/test_spine.rs` checks it).
pub const ROUTES: &[(&str, &str)] = &[];

/// One mapping of a new code to today's. `None` in `feature` or `target` matches any.
#[derive(Clone, Copy, Debug)]
pub struct OldCode {
    /// The new code.
    pub code: ir::Code,
    /// For a `U` code: the feature in its `arg`.
    pub feature: Option<ir::Feature>,
    /// The target dialect's name.
    pub target: Option<&'static str>,
    /// Today's code it is shown as.
    pub old: Code,
}

/// The code map the dialect slices fill as they flip pairs; the first matching row wins.
pub static OLD_CODE: &[OldCode] = &[];

/// How a refusing new code with no [`OLD_CODE`] row is shown: `W0203 <code> <title>`.
pub const IR_REFUSAL: Code = Code::W0203;

/// How a new note with no [`OLD_CODE`] row is shown: `W0204 <code> <title>`.
pub const IR_NOTE: Code = Code::W0204;

/// [`route_with`] under `SPICERACK_PIPELINE` (read once per process).
pub fn route(from: &str, to: &str) -> Option<(&'static DialectRow, &'static DialectRow)> {
    static FORCED: OnceLock<Option<Pipeline>> = OnceLock::new();
    route_with(from, to, *FORCED.get_or_init(forced))
}

/// The rows of `from` (a reader) and `to` (a writer) when the new pipeline writes the pair:
/// never under `Old`; whenever both rows have the side under `New` (else the old one runs, said
/// once on stderr); when [`ROUTES`] lists it under `None`.
pub fn route_with(
    from: &str,
    to: &str,
    pipeline: Option<Pipeline>,
) -> Option<(&'static DialectRow, &'static DialectRow)> {
    let pair = by_name(from)
        .zip(by_name(to))
        .filter(|(f, t)| f.read.is_some() && t.write.is_some());
    match pipeline {
        Some(Pipeline::Old) => None,
        Some(Pipeline::New) => {
            static NOTED: AtomicBool = AtomicBool::new(false);
            if pair.is_none() && !NOTED.swap(true, Ordering::Relaxed) {
                eprintln!("spicerack: the new pipeline has no {from} -> {to}; the old one runs");
            }
            pair
        }
        None => pair.filter(|_| ROUTES.contains(&(from, to))),
    }
}

/// `text` (one deck named `name`) through `transpile`, leniently, in today's types.
///
/// # Errors
/// The refusal, as today's `Diags`.
pub fn render_text(
    text: &str,
    name: &str,
    from: &DialectRow,
    to: &DialectRow,
) -> Result<render::Written, render::Diags> {
    let src = Sources::one(name, text);
    finish(transpile(&src, from, to, Policy::Lenient), &src, name, to)
}

/// `file` of `kit` (its entry) and every kit file it reaches through `from`'s references,
/// through `transpile`, leniently and with every definition a root (a kit: `Roots::Every`,
/// R80a), in today's types: one read and one `legalize` of the kit per call (today's
/// `render_kit` writes one file per call; a caller of the spine writes them all after one
/// `legalize`).
///
/// # Errors
/// The refusal, as today's `Diags`.
///
/// # Panics
/// When `file` is not a deck or library the kit read, or `from` has no reader.
pub fn render_kit(
    kit: &Kit,
    file: FileId,
    name: &str,
    from: &DialectRow,
    to: &DialectRow,
) -> Result<render::Written, render::Diags> {
    let entry = kit.file(file);
    let form = match entry.role {
        Role::Deck => ir::Role::Deck,
        Role::Library => ir::Role::Library,
        Role::VerilogA | Role::Unread(_) => panic!("render_kit: a file the kit read"),
    };
    let reader = from.read.expect("render_kit: a row with a reader");
    let mut loader = |path: &str| {
        let found = kit
            .files()
            .iter()
            .find(|f| f.path.to_string_lossy() == path);
        found
            .filter(|f| matches!(f.role, Role::Deck | Role::Library))
            .map(|f| f.text.clone())
    };
    let path = entry.path.to_string_lossy();
    let src = Sources::load(&[&path], form, reader, None, &mut loader);
    assert!(
        src.entries().len() == 1,
        "render_kit: the kit's own file loads"
    );
    let written = transpile_with(&src, from, to, Policy::Lenient, Roots::Every);
    finish(written, &src, name, to)
}

/// `rows` in today's form: each code through [`old_code`], sorted by position (file, byte,
/// code), messages `name:line:column: CODE title` (for [`IR_REFUSAL`] and [`IR_NOTE`]: `CODE
/// <new code> <new title>`) with the line and byte column (from 1) computed from the span in
/// `src`'s file of that position; `0:0` for a row with no span or a file `src` lacks. Today's
/// `Diag::token` holds the span's first byte. A `U0001` row whose feature `citations` (the
/// target writer's, `Write::citations`) names ends with `: <evidence>` (A34).
pub fn diags(
    rows: &[ir::Diag],
    src: &Sources,
    name: &str,
    target: &str,
    citations: &[Citation],
) -> render::Diags {
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|d| (d.span.file, d.span.start, d.code));
    let mut out = render::Diags {
        diags: Vec::with_capacity(sorted.len()),
        messages: String::new(),
    };
    for d in &sorted {
        let old = old_code(d, target);
        let text = (d.span.file != ir::FileId::NONE)
            .then(|| src.files().get(usize::from(d.span.file.0)))
            .flatten();
        let (line, column) = text.map_or((0, 0), |(_, t)| location(t, d.span.start));
        out.diags.push(diag::Diag::new(old, d.span.start, 0));
        let mut title = match old {
            IR_REFUSAL | IR_NOTE => {
                let new = super::info(d.code).map_or("a code no catalogue holds", |i| i.title);
                format!("{}{:04} {new}", char::from(d.code.letter), d.code.number)
            }
            _ => diag::info(old).title.to_string(),
        };
        let cited = citations
            .iter()
            .find(|c| u32::from(c.feature.index()) == d.arg);
        if d.code == ir::diag::UNSUPPORTED
            && let Some(c) = cited
        {
            title = format!("{title}: {}", c.evidence);
        }
        out.messages += &format!("{name}:{line}:{column}: {old:?} {title}\n");
    }
    assert!(out.diags.len() == rows.len(), "one old row per new row");
    out
}

/// Today's code for `d` on `target`: the first [`OLD_CODE`] row that matches, else
/// [`IR_REFUSAL`] or [`IR_NOTE`] by the new code's verdict (`dialect::refuses`), so a refusal
/// stays a refusal.
pub fn old_code(d: &ir::Diag, target: &str) -> Code {
    let row = OLD_CODE.iter().find(|r| {
        r.code == d.code
            && r.target.is_none_or(|t| t == target)
            && r.feature.is_none_or(|f| u32::from(f.index()) == d.arg)
    });
    match row {
        Some(r) => r.old,
        None if super::refuses(d.code) => IR_REFUSAL,
        None => IR_NOTE,
    }
}

// ---- private ----

/// `SPICERACK_PIPELINE` read: `old`, `new`, or unset; any other value is said on stderr and
/// treated as unset.
fn forced() -> Option<Pipeline> {
    match std::env::var(ENV).ok()?.as_str() {
        "old" => Some(Pipeline::Old),
        "new" => Some(Pipeline::New),
        other => {
            eprintln!("spicerack: {ENV}={other}: not old or new; the route table decides");
            None
        }
    }
}

/// A routed result in today's types.
fn finish(
    result: Result<super::Written, ir::Diags>,
    src: &Sources,
    name: &str,
    to: &DialectRow,
) -> Result<render::Written, render::Diags> {
    match result {
        Ok(w) => Ok(render::Written {
            text: w.text,
            notes: diags(w.notes.rows(), src, name, to.name, cited(to)),
        }),
        Err(d) => Err(diags(d.rows(), src, name, to.name, cited(to))),
    }
}

/// The target writer's citations, none for a row with no writer.
fn cited(to: &DialectRow) -> &'static [Citation] {
    to.write.map_or(&[], |w| w.citations())
}

/// The line and byte column (both from 1) of byte `at` of `text`; past the end, the end's.
fn location(text: &str, at: u32) -> (usize, usize) {
    let at = (at as usize).min(text.len());
    let before = &text.as_bytes()[..at];
    let line = 1 + before.iter().filter(|&&b| b == b'\n').count();
    let start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    (line, at - start + 1)
}
