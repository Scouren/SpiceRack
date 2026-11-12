//! The writer's spine: one file of a legal design → VACASK text. The title first (line 1 of an
//! entry deck: VACASK reads line 1 as the title whatever it holds, so a deck with none gets an
//! empty line), then the file's items in order, each body below its item and indented two
//! spaces per level (explicit stack, C11), the run items of the top scope in one `control …
//! endc` block at the end (S4.3). One exhaustive `match` over `ir::ItemKind`, no `_` arm: each
//! arm is an area function. An `Opaque` is copied when its dialect is `vacask` (the spine's
//! `legalize` refused every other).
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; REWRITE-PLAN §4.3 (one exhaustive match).
//!
//! Contents: `Writer` (`Writer::line`, `Writer::head`), `write` (crate-private)

use super::name::{self, Place};
use super::{write_elements, write_models, write_run, write_sources, write_structure};
use crate::ir;

/// The text being written and where it is.
pub(super) struct Writer<'d, 'o> {
    /// The legal design.
    pub(super) design: &'d ir::Design,
    /// The text so far.
    pub(super) out: &'o mut String,
    /// The current depth: two spaces of indent each.
    pub(super) depth: usize,
    /// The directory of the file being written (`""` for none), which its paths are relative to.
    pub(super) dir: String,
}

/// One step of the walk: an item, a branch head of an `If`, or a block's closing line.
#[derive(Clone, Copy)]
enum Task {
    Item(usize, ir::ItemRef),
    Branch(usize, usize, ir::item::Branch),
    Close(usize, fn(&mut Writer<'_, '_>)),
}

impl Writer<'_, '_> {
    /// Starts a line at the current depth.
    pub(super) fn line(&mut self) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
    }

    /// Starts an instance statement: `name (nodes) master` (no master for a behavioural
    /// source), each name spelled so that VACASK reads it back; the area appends its parameters
    /// and the newline.
    #[allow(
        dead_code,
        reason = "write_elements (S4.1) and write_sources (S4.2) call it"
    )]
    pub(super) fn head(&mut self, row: &ir::Element) {
        let d = self.design;
        self.line();
        name::spell(d.str(row.name), Place::Other, self.out);
        self.out.push_str(" (");
        for (k, &t) in d.names(row.terminals).iter().enumerate() {
            if k > 0 {
                self.out.push(' ');
            }
            name::spell(d.str(t), Place::Node, self.out);
        }
        self.out.push(')');
        if !row.model.name.is_none() {
            self.out.push(' ');
            name::spell(d.str(row.model.name), Place::Other, self.out);
        }
    }
}

/// Appends file `file` of `design` (legal for `FEATURES`) to `out`.
pub(super) fn write(
    design: &ir::Design,
    file: ir::FileId,
    out: &mut String,
    diags: &mut ir::Diags,
) {
    let _ = diags;
    let top = design.file(file).top;
    let path = design.str(design.file(file).path);
    let dir = std::path::Path::new(path)
        .parent()
        .map_or(String::new(), |d| d.to_string_lossy().into_owned());
    let mut w = Writer {
        design,
        out,
        depth: 0,
        dir,
    };
    if design.file(file).role == ir::Role::Deck {
        let title = design
            .items(top)
            .iter()
            .find(|at| at.kind == ir::ItemKind::Title)
            .map(|at| design.row::<ir::item::Title>(at.index));
        write_structure::title(&mut w, title);
    }
    let mut todo: Vec<Task> = top_items(design, top);
    // Bounded: each item and closer is pushed once (scopes form a tree, `ir::check`).
    while let Some(task) = todo.pop() {
        match task {
            Task::Close(depth, close) => {
                w.depth = depth;
                close(&mut w);
            }
            Task::Branch(depth, k, branch) => {
                w.depth = depth;
                write_structure::branch(&mut w, k, branch);
                todo.extend(items(design, branch.body, depth + 1));
            }
            Task::Item(depth, at) => {
                w.depth = depth;
                item(&mut w, &mut todo, at);
            }
        }
    }
}

// ---- private ----

/// Whether the sources area writes an element of `kind` (the kinds of `tables::SOURCE_MODULES`'
/// modules), the devices area every other.
fn source_like(kind: ir::ElemKind) -> bool {
    use ir::ElemKind as K;
    matches!(
        kind,
        K::VSource
            | K::ISource
            | K::Vcvs
            | K::Vccs
            | K::Ccvs
            | K::Cccs
            | K::Coupling
            | K::LosslessLine
    )
}

/// The top scope's items, reversed for the stack, with the one `control` … `endc` block around
/// the range `write_run::control_span` names (VACASK takes one block per file, anywhere at the top:
/// vc-run's runs), its items one level deeper.
fn top_items(design: &ir::Design, top: ir::ScopeId) -> Vec<Task> {
    let span = write_run::control_span(design, top);
    let mut out = Vec::new();
    for (k, &at) in design.items(top).iter().enumerate() {
        if at.kind == ir::ItemKind::Title {
            continue;
        }
        let inside = span.is_some_and(|(first, last)| (first..=last).contains(&k));
        if span.is_some_and(|(first, _)| first == k) {
            out.push(Task::Close(0, control_open));
        }
        out.push(Task::Item(usize::from(inside), at));
        if span.is_some_and(|(_, last)| last == k) {
            out.push(Task::Close(0, control_close));
        }
    }
    out.reverse();
    out
}

/// `control`.
fn control_open(w: &mut Writer<'_, '_>) {
    w.line();
    w.out.push_str("control\n");
}

/// `endc`.
fn control_close(w: &mut Writer<'_, '_>) {
    w.line();
    w.out.push_str("endc\n");
}

/// The items of `scope` at `depth`, reversed for the stack (the title is line 1's).
fn items(design: &ir::Design, scope: ir::ScopeId, depth: usize) -> Vec<Task> {
    design
        .items(scope)
        .iter()
        .rev()
        .filter(|at| at.kind != ir::ItemKind::Title)
        .map(|&at| Task::Item(depth, at))
        .collect()
}

/// Item `at`: its first line, and its body and closer pushed (one arm per kind, no `_` arm).
fn item(w: &mut Writer<'_, '_>, todo: &mut Vec<Task>, at: ir::ItemRef) {
    let d = w.design;
    let depth = w.depth;
    match at.kind {
        ir::ItemKind::Title => {}
        ir::ItemKind::Comment => write_structure::comment(w, d.row(at.index)),
        ir::ItemKind::Include => write_structure::include(w, d.row(at.index)),
        ir::ItemKind::Global => write_structure::global(w, d.row(at.index)),
        ir::ItemKind::Load => write_structure::load(w, at),
        ir::ItemKind::DeviceLibrary => write_structure::device_library(w, d.row(at.index)),
        ir::ItemKind::Opaque => write_structure::opaque(w, d.row(at.index)),
        ir::ItemKind::Param => write_structure::param(w, d.row(at.index)),
        ir::ItemKind::Subckt => {
            let row: ir::item::Subckt = d.row(at.index);
            write_structure::subckt(w, row);
            todo.push(Task::Close(depth, write_structure::subckt_end));
            todo.extend(items(d, row.body, depth + 1));
        }
        ir::ItemKind::Section => {
            let row: ir::item::Section = d.row(at.index);
            write_structure::section(w, row);
            todo.push(Task::Close(depth, write_structure::section_end));
            todo.extend(items(d, row.body, depth + 1));
        }
        ir::ItemKind::If => {
            let row: ir::item::If = d.row(at.index);
            todo.push(Task::Close(depth, write_structure::if_end));
            for (k, &branch) in d.branches(row.branches).iter().enumerate().rev() {
                todo.push(Task::Branch(depth, k, branch));
            }
        }
        ir::ItemKind::Model => write_models::model(w, at),
        ir::ItemKind::Element => {
            let row: ir::Element = d.row(at.index);
            match source_like(row.kind) {
                true => write_sources::source(w, at),
                false => write_elements::element(w, at),
            }
        }
        ir::ItemKind::Func | ir::ItemKind::Statistics => {
            unreachable!("FEATURES lacks {:?}: legalize refused it", at.kind)
        }
        ir::ItemKind::Analysis
        | ir::ItemKind::Loop
        | ir::ItemKind::Alter
        | ir::ItemKind::Option
        | ir::ItemKind::Env
        | ir::ItemKind::NodeSet
        | ir::ItemKind::Save
        | ir::ItemKind::Print
        | ir::ItemKind::Measure
        | ir::ItemKind::Elaborate => write_run::analysis(w, at),
    }
}
