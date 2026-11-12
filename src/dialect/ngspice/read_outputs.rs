//! Output cards → their IR items: `.save` → `Save`, `.print` → `Print`, `.meas`/`.measure` →
//! `Measure` (`read_outputs/meas.rs`), `.plot` and `.width` → an `Inert` opaque (an ASCII picture
//! or a column width on stdout, no number); `.probe` → `Unread` (ngspice-45's `inp_probe` adds
//! sources and a `.save all`, `inpc_probe.c`). A form the IR cannot state yet is `Unread` (`P1350`), one ngspice rejects or
//! fails on is `InvalidInSource` (`P1351`); a card inside a block (`.subckt`, `.if`) is `Unread`.
//!
//! The words of a card are ngspice's: whitespace inside parentheses and around `=` is dropped
//! (`dotcards.c` `gettoks`, `inpcom.c`), quotes and braces hold their blanks. A probe word reads
//! by its card (`Ctx`): `.save v(a,b)` keeps two node voltages (gettoks makes one save per
//! argument), `.print tran v(a,b)` is the difference (`fixem`), a measure has no `v(a,b)`
//! ("no such vector"); in an ac `.print` `vm vp vr vdb` are magnitude, phase in radians (`ph`),
//! real part and dB, in an ac or sp measure `v` and `vr` the real part and `vp` the phase, in
//! radians as everywhere (`com_measure2.c` `correct_vec`, `get_value`: `radtodeg` converts
//! only under `set units=degrees`; run: `ph` = −0.5610 at 100 Hz, the printed `vp`); `vi(` is ngspice's error (gettoks
//! saves `x#branch` and the analysis does not run). `i(x)` and `x#branch` are the current of
//! element `x` (its whole name, R23); `@x[q]` is device quantity `q` as spelled. A dotted name
//! is a path when ngspice's flattening made it: `x1.n` (instances lead with `x`) and
//! `v.x1.v9` (an element's letter, the instances, its name).
//!
//! Every function validates the whole card into local values before the first push.
//!
//! Cites: ngspice-45 `frontend/dotcards.c` (`gettoks`, `fixem`, `ft_cktcoms`),
//! `frontend/inpcom.c` (6870ff), `frontend/com_measure2.c`, `frontend/measure.c`; runs in
//! `tests/fixtures/dialect/ngspice/read/outputs/`; `rewrite/INV-semantics.md` 9.01–9.08.
//!
//! Contents: `pub(super) const FEATURES`, `pub(super) const CODES`, `pub(super) const UNTYPED`,
//! `INVALID`, `INERT`, `DC_WINDOW`, `pub(super) enum No`, `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt)`,
//! `pub(super) fn probe(lw, st, text: &str, analysis: ir::AnalysisKind) -> Result<ir::Probe, Fail>`,
//! `pub(super) fn words(&str) -> Vec<String>`, `pub(super) fn unwrap(&str) -> &str`; the probe
//! words (`Ctx`, `Want`, `probe_word`) are `read_outputs/probe.rs`, the measures
//! `read_outputs/meas.rs`

mod meas;
mod probe;

use probe::build;
pub(super) use probe::{Ctx, Want, probe_word};

use super::codes::Row;
use super::expr_read::Fail;
use super::read::{Lower, Stmt};
use super::tables;
use crate::ir::{self, AnalysisKind, Code, Feature, ItemKind, RunExtra};
use crate::syntax::ast::Tag;

/// The IR rows this area reads (and its writer spells).
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    Feature::Item(ItemKind::Save),
    Feature::Item(ItemKind::Print),
    Feature::Item(ItemKind::Measure),
    Feature::Run(RunExtra::ProbeVdiff),
    Feature::Run(RunExtra::ProbeI),
    Feature::Run(RunExtra::ProbeParam),
    Feature::Run(RunExtra::ProbeHierarchical),
    Feature::Run(RunExtra::ProbeAllV),
    Feature::Run(RunExtra::ProbeAllBranchI),
    Feature::Run(RunExtra::PartRe),
    Feature::Run(RunExtra::PartMag),
    Feature::Run(RunExtra::PartPhaseRad),
    Feature::Run(RunExtra::PartDb),
    Feature::Run(RunExtra::MeasTrigTarg),
    Feature::Run(RunExtra::MeasWhen),
    Feature::Run(RunExtra::MeasFind),
    Feature::Run(RunExtra::MeasStat),
    Feature::Run(RunExtra::MeasParam),
    Feature::Run(RunExtra::MeasLast),
    Feature::Run(RunExtra::MeasDelay),
    Feature::Run(RunExtra::MeasCircuitThreshold),
    Feature::Run(RunExtra::MeasStatAt),
    Feature::Run(RunExtra::MeasUntil),
    Feature::Run(RunExtra::MeasSkipInitial),
]);

/// An output form the IR has no row for yet: `Unread`.
pub(super) const UNTYPED: Code = Code::new(b'P', 1350);
/// An output card ngspice rejects or fails on: `InvalidInSource`.
pub(super) const INVALID: Code = Code::new(b'P', 1351);
/// `.plot`, `.width`: an ASCII picture or a column width, no number: `Inert`.
pub(super) const INERT: Code = Code::new(b'P', 1352);

/// A dc measure window with an expression bound: ngspice orders the two after substitution,
/// and the IR has no window "between the two, in either order": `Unread`, counted on its own.
pub(super) const DC_WINDOW: Code = Code::new(b'P', 1353);

/// This area's codes.
pub(super) const CODES: &[Row] = &[
    note(
        UNTYPED,
        "an output form the IR cannot state yet",
        "rewrite/IR-RUN.md §4",
        ".probe, save alli, to=0 on a crossing, from= or to= on a dc crossing, td= off tran or with from=, td= on a window, par(), a column expression, .print op, a card in a block; kept as ngspice text (Unread).",
    ),
    note(
        INVALID,
        "an output card ngspice rejects or fails on",
        "ngspice-45 com_measure2.c get_measure2, measure.c do_measure, dotcards.c gettoks",
        "deriv and err are not supported, an unknown function or analysis, v(a,b) in a measure, vi(, td= naming a measure (fatal), a print with no output; kept as ngspice text (InvalidInSource).",
    ),
    note(
        DC_WINDOW,
        "a dc measure window with an expression bound",
        "ngspice-45 com_measure2.c measure_parse_stdParams (make m_from always less than m_to)",
        "ngspice swaps the bounds by value at run time; the IR has no window in either order (R18: typed when the census finds one); kept as ngspice text (Unread).",
    ),
    note(
        INERT,
        "a .plot or .width card",
        "ngspice-45 dotcards.c ft_cktcoms (com_asciiplot, the width variable)",
        "an ASCII plot or a column width on standard output moves no number; kept as Inert text.",
    ),
];

/// Why a card is no item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum No {
    /// The IR cannot state it yet (`P1350`).
    Untyped,
    /// ngspice rejects it (`P1351`).
    Invalid,
    /// A dc window whose order is known only at run time (`P1353`).
    DcWindow,
}

/// An output card into `st.scope`, or one opaque with its code.
pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt) {
    let kw = lw.keyword(st.node);
    if kw == ".plot" || kw == ".width" {
        return lw.opaque(st, ir::OpaqueClass::Inert, INERT);
    }
    let result = if lw.ast.tag(st.container) != Tag::Root {
        Err(No::Untyped) // ngspice pulls output cards out of blocks before expanding them
    } else {
        let words = words(&lw.rest(st.node));
        match kw.as_str() {
            ".save" => save(lw, st, &words),
            ".print" => print(lw, st, &words),
            // inp_probe (inpc_probe.c) adds `.save all` to a deck with no save and 0 V and E
            // sources at the devices it names: not a save, not typed
            ".probe" => Err(No::Untyped),
            _ => meas::card(lw, st, &words),
        }
    };
    match result {
        Ok(()) => {}
        Err(No::Untyped) => lw.opaque(st, ir::OpaqueClass::Unread, UNTYPED),
        Err(No::Invalid) => lw.opaque(st, ir::OpaqueClass::InvalidInSource, INVALID),
        Err(No::DcWindow) => lw.opaque(st, ir::OpaqueClass::Unread, DC_WINDOW),
    }
}

/// The words of a card's text: blanks inside parentheses or brackets and around `=` dropped,
/// blanks inside quotes or braces kept.
pub(super) fn words(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let (mut depth, mut brace, mut quote) = (0i32, 0i32, false);
    let chars: Vec<char> = text.chars().collect();
    for (k, &c) in chars.iter().enumerate() {
        match c {
            '\'' => quote = !quote,
            '{' if !quote => brace += 1,
            '}' if !quote => brace -= 1,
            '(' | '[' if !quote && brace == 0 => depth += 1,
            ')' | ']' if !quote && brace == 0 => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && !quote && brace == 0 {
            let next = chars[k + 1..].iter().find(|c| !c.is_whitespace());
            let joins = depth > 0 || cur.ends_with('=') || next == Some(&'=');
            if !joins && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `text` without one `'…'` or `{…}` around all of it.
pub(super) fn unwrap(text: &str) -> &str {
    let t = text.trim();
    for (open, close) in [('\'', '\''), ('{', '}')] {
        if let Some(inner) = t.strip_prefix(open).and_then(|r| r.strip_suffix(close))
            && !inner.contains([open, close])
        {
            return inner;
        }
    }
    t
}

/// One output named inside another card (`.four 1k v(a)`, `.tf v(out) vin`, `.noise v(out)
/// vin`): `v(a)`, `v(a,b)` (the difference) or `i(e)`, no part. The run area calls it so one
/// reader spells probes; `analysis` is the analysis the output belongs to. `Err` is the class and
/// code of the `Opaque` the calling statement becomes. Appends only names.
#[allow(dead_code, reason = "the run area calls it (S3.3a)")]
pub(super) fn probe(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    text: &str,
    analysis: ir::AnalysisKind,
) -> Result<ir::Probe, Fail> {
    let _ = (st, analysis); // one spelling in every analysis output
    let fail = |no| match no {
        No::Untyped => Fail {
            code: UNTYPED,
            class: ir::OpaqueClass::Unread,
        },
        No::Invalid => Fail {
            code: INVALID,
            class: ir::OpaqueClass::InvalidInSource,
        },
        No::DcWindow => Fail {
            code: DC_WINDOW,
            class: ir::OpaqueClass::Unread,
        },
    };
    let mut wants =
        probe_word(&text.split_whitespace().collect::<String>(), Ctx::Output).map_err(fail)?;
    if wants.len() != 1 {
        return Err(fail(No::Untyped));
    }
    Ok(build(lw, &wants.remove(0)))
}

// ---- private ----

/// `.save word …`: one `Save` for every analysis.
fn save(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String]) -> Result<(), No> {
    let wants = probes(words, Ctx::Save)?;
    let probes: Vec<ir::Probe> = wants.iter().map(|w| build(lw, w)).collect();
    let span = lw.span(st.node);
    lw.b.save(st.scope, None, &probes, span);
    Ok(())
}

/// `.print kind word …`: one `Print` of `Main` view.
fn print(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String]) -> Result<(), No> {
    let (kind, rest) = words.split_first().ok_or(No::Invalid)?;
    let kind = match kind.to_ascii_lowercase().as_str() {
        "tran" => AnalysisKind::Tran,
        "ac" => AnalysisKind::Ac,
        "dc" => AnalysisKind::Dc,
        _ => return Err(No::Untyped),
    };
    let wants = probes(rest, Ctx::Print(kind))?;
    let probes: Vec<ir::Probe> = wants.iter().map(|w| build(lw, w)).collect();
    let span = lw.span(st.node);
    lw.b.print(st.scope, kind, ir::PrintView::Main, &probes, span);
    Ok(())
}

/// Every word's probes; a card with none is ngspice's "no nodes given".
fn probes(words: &[String], ctx: Ctx) -> Result<Vec<Want>, No> {
    let mut out = Vec::new();
    for w in words {
        out.extend(probe_word(w, ctx)?);
    }
    if out.is_empty() {
        return Err(No::Invalid);
    }
    Ok(out)
}

/// A note row.
const fn note(code: Code, title: &'static str, cite: &'static str, explain: &'static str) -> Row {
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind: ir::Kind::Note,
        },
    )
}
