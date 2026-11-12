//! Run items → statements of the one `control` … `endc` block, each in IR order: `options`
//! lines (one per `Option` or `Env` row), `analysis name kind k=v` with its `ic=`/`nodeset=`
//! lists from the `NodeSet` rows just before it, a `Dc` as its axes' `sweep` lines then an `op`,
//! a sweep `Loop` as its `sweep` line then its body, a Monte Carlo `Loop` as `mc … endmc`,
//! `alter`, `elaborate`, `save` (`write_outputs`). Nothing is reordered. The spine writes
//! `control` and `endc` around the span `control_span` gives.
//!
//! An unnamed analysis is named `<kind><n>`, n its place among the deck's analyses in listed
//! order; an unnamed sweep `sweep<n>`, n its place among the deck's sweep lines (DIALECT-VACASK
//! §4; VACASK requires both names, VD cmd-analysis, cmd-sweep).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-overview, cmd-sweep, cmd-analysis-*.
//!
//! Contents: `analysis`, `control_span`, `admits` (crate-private), and for the children
//! `quote`, `name_of`, `fold_alters`, `body`

mod analyses;
mod plan;

use super::read_run::{OPTION_ROW_PRIVATE, RESET_CLASS};
use super::write::Writer;
use super::{fold, write_outputs, write_structure};
use crate::ir;

/// The control commands' head words (VD cmd-overview "Summary").
const COMMANDS: &[&str] = &[
    "analysis",
    "sweep",
    "save",
    "options",
    "var",
    "elaborate",
    "alter",
    "print",
    "postprocess",
    "clear",
    "mc",
    "endmc",
    "abort",
];

/// One run item, its first line at `w.depth` (a loop writes its body below it).
pub(super) fn analysis(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    match at.kind {
        ir::ItemKind::Analysis => analyses::analysis(w, at),
        ir::ItemKind::Loop => plan::loop_(w, at),
        ir::ItemKind::Alter => plan::alter(w, d.row(at.index)),
        ir::ItemKind::Option => plan::option(w, d.row(at.index)),
        ir::ItemKind::Env => plan::env(w, d.row(at.index)),
        ir::ItemKind::Elaborate => plan::elaborate(w, d.row(at.index)),
        ir::ItemKind::Save => write_outputs::save(w, at),
        // `admits` refuses every other class (W2305).
        ir::ItemKind::Reset => {
            w.line();
            w.out.push_str("clear saves\n");
        }
        // Spelled on the analysis it applies to.
        ir::ItemKind::NodeSet => {}
        _ => unreachable!("FEATURES lacks {:?}: legalize refused it", at.kind),
    }
}

/// The first and last index into `design.items(top)` of the control block's items, for the
/// spine's `control` … `endc` (VD cmd-overview: one block per file; run x1): from the first run
/// item to the last, widened over the control-command opaques next to them (a `print` before
/// the first analysis); with no run item, the first and last control-command opaque of class
/// Private, Inert or Hint. A command word alone is no proof: outside a block `print …` is an
/// instance named `print` (another area's opaque, of class Unread or InvalidInSource).
pub(super) fn control_span(design: &ir::Design, top: ir::ScopeId) -> Option<(usize, usize)> {
    let items = design.items(top);
    let command = |k: usize| command_opaque(design, items[k]);
    let ours = |k: usize| command(k).is_some_and(|c| c != ir::OpaqueClass::Unread);
    let comment = |k: usize| items[k].kind == ir::ItemKind::Comment;
    if let Some(mut first) = items.iter().position(|&at| run_item(at)) {
        let mut last = items.iter().rposition(|&at| run_item(at))?;
        // Widen over our command opaques, across comments between them.
        let mut k = first;
        while k > 0 && (ours(k - 1) || comment(k - 1)) {
            k -= 1;
            if ours(k) {
                first = k;
            }
        }
        let mut k = last;
        while k + 1 < items.len() && (ours(k + 1) || comment(k + 1)) {
            k += 1;
            if ours(k) {
                last = k;
            }
        }
        return Some((first, last));
    }
    let own = |k: &usize| {
        command(*k).is_some_and(|c| {
            matches!(
                c,
                ir::OpaqueClass::Private | ir::OpaqueClass::Inert | ir::OpaqueClass::Hint
            )
        }) || command(*k) == Some(ir::OpaqueClass::InvalidInSource) && reserved(design, items[*k])
    };
    let first = (0..items.len()).find(own)?;
    let last = (0..items.len()).rfind(own)?;
    Some((first, last))
}

/// Appends a refusal for each row of this area VACASK cannot spell for a reason no feature
/// states (R21): an option row of a private key (W2304; VACASK's own options are Hint or
/// Private opaques).
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    for (at, _) in design.reachable() {
        if at.kind == ir::ItemKind::Option
            && design.row::<ir::OptionRow>(at.index).key == ir::OptionKey::Private
        {
            diags.push_at(OPTION_ROW_PRIVATE, design, at, 0);
        }
        let saves = ir::ResetSet::of(&[ir::Restorable::Saves]);
        if at.kind == ir::ItemKind::Reset && design.row::<ir::Reset>(at.index).restores != saves {
            diags.push_at(RESET_CLASS, design, at, 0);
        }
    }
}

/// `text` as a VACASK string (`"…"`, `"` and `\` escaped).
pub(super) fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
}

/// The hierarchical name of `path` (`x1:r1`).
pub(super) fn name_of(design: &ir::Design, path: ir::NameList) -> String {
    let names: Vec<&str> = design.names(path).iter().map(|&n| design.str(n)).collect();
    names.join(":")
}

/// R42's framing before analysis `at` (a top-level one): for each `Given` source with a wave
/// (`fold::given`, item order), `alter instance("<name>") type="<word>"` before a transient
/// and `type="dc"` before a DC-kind analysis, when the side differs from the previous top-level
/// analysis's (the line starts at dc). Contract: S4-AREAS.md "dc plus type alters".
pub(super) fn fold_alters(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let reached = d.reachable();
    let Some(&(_, scope)) = reached.iter().find(|(item, _)| *item == at) else {
        return;
    };
    let top = d.file(d.scope(scope).file).top;
    if scope != top {
        return;
    }
    let sources = fold::given(d, top);
    if sources.is_empty() {
        return;
    }
    let wave = |a: ir::ItemRef| d.row::<ir::Analysis>(a.index).kind == ir::AnalysisKind::Tran;
    let items = d.items(top);
    let k = items
        .iter()
        .position(|&i| i == at)
        .expect("listed in its scope");
    let before = items[..k]
        .iter()
        .rev()
        .find(|i| i.kind == ir::ItemKind::Analysis)
        .is_some_and(|&a| wave(a));
    let now = wave(at);
    if before == now {
        return;
    }
    for (source, word) in sources {
        let row: ir::Element = d.row(source.index);
        w.line();
        w.out.push_str("alter instance(");
        quote(d.str(row.name), w.out);
        w.out.push_str(") type=");
        quote(if now { word } else { "dc" }, w.out);
        w.out.push('\n');
    }
}

/// Every item of `scope` (a loop's body), one level deeper.
pub(super) fn body(w: &mut Writer<'_, '_>, scope: ir::ScopeId) {
    let d = w.design;
    w.depth += 1;
    for &at in d.items(scope) {
        match at.kind {
            ir::ItemKind::Comment => write_structure::comment(w, d.row(at.index)),
            ir::ItemKind::Opaque => write_structure::opaque(w, d.row(at.index)),
            _ => analysis(w, at),
        }
    }
    w.depth -= 1;
}

// ---- private ----

/// Whether `at` is a run item.
fn run_item(at: ir::ItemRef) -> bool {
    matches!(
        at.kind,
        ir::ItemKind::Analysis
            | ir::ItemKind::Loop
            | ir::ItemKind::Alter
            | ir::ItemKind::Option
            | ir::ItemKind::Env
            | ir::ItemKind::NodeSet
            | ir::ItemKind::Save
            | ir::ItemKind::Print
            | ir::ItemKind::Measure
            | ir::ItemKind::Elaborate
            | ir::ItemKind::Reset
    )
}

/// The class of `at` when it is a `vacask` opaque whose head word is a control command.
fn command_opaque(design: &ir::Design, at: ir::ItemRef) -> Option<ir::OpaqueClass> {
    if at.kind != ir::ItemKind::Opaque {
        return None;
    }
    let row: ir::item::Opaque = design.row(at.index);
    let text = design.text(row.text);
    let head = text
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or("");
    (design.str(row.dialect) == "vacask" && COMMANDS.contains(&head)).then_some(row.class)
}

/// Whether opaque `at` starts with a word VACASK reads as a control statement only (`analysis`,
/// `sweep`, `save` are reserved in a control block, VD input-reserved; `options`, `mc`,
/// `endmc`, `alter`, `elaborate` are no instance names an area reads as invalid).
fn reserved(design: &ir::Design, at: ir::ItemRef) -> bool {
    let row: ir::item::Opaque = design.row(at.index);
    let head = design
        .text(row.text)
        .split_whitespace()
        .next()
        .unwrap_or("");
    matches!(
        head,
        "analysis" | "sweep" | "save" | "options" | "mc" | "endmc" | "alter" | "elaborate"
    )
}
