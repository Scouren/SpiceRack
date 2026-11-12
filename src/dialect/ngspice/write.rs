//! The writer's spine: one file of a legal design → ngspice text. Line 1 is the deck's title (a
//! blank line when it has none; a library file has no title line), then the file's items in
//! order, each through one exhaustive `match` over `ir::ItemKind` (no `_` arm) to its area
//! function; a deck ends with `.end`. An item that opens a scope has its area function schedule
//! the body (and the line that closes it) on an explicit stack, so nesting costs no recursion.
//!
//! The writer sees only the IR: it never learns where the design came from. The spine
//! (`dialect::write`) hands it only designs whose features are in `FEATURES`; an area whose
//! features are absent is never reached, and asserts so.
//!
//! Cites: `rewrite/DIALECT-NGSPICE.md` §4; `REWRITE-PLAN.md` §2.2 rules 1 and 4, §4.3.
//!
//! Contents: `pub(super) struct Out<'a>`,
//! `pub(super) fn write(&ir::Design, ir::FileId, &mut String, &mut ir::Diags)`,
//! `pub(super) fn admits(&ir::Design, &mut ir::Diags)`,
//! `Out::open(&mut self, ir::ScopeId, String)`, `Out::open_script(&mut self, ir::ScopeId, String)`,
//! `Out::line(&mut self, &str)`, `Out::script(&mut self, &str)`, `Out::scripted(&self) -> bool`
//!
//! A script region (R49, A27): consecutive `Out::script` commands share one `.control` …
//! `.endc` pair; any deck line (`Out::line`) or a frame's tail closes it first, as does the end
//! of the file. A frame scheduled with `Out::open_script` writes its tail as a command instead.

use super::{
    write_control, write_elements, write_models, write_outputs, write_run, write_structure,
};
use crate::ir;

/// The writing state of one file, handed to every area function.
pub(super) struct Out<'a> {
    /// The design, read only.
    pub(super) design: &'a ir::Design,
    /// The text so far: written only through `line`, `script` and frame tails, so a script
    /// region always closes before a deck line.
    out: &'a mut String,
    /// Diagnostics.
    #[allow(
        dead_code,
        reason = "no area writer notes yet; a writer's note goes here"
    )]
    pub(super) diags: &'a mut ir::Diags,
    frames: Vec<Frame<'a>>,
    /// Whether a `.control` line is open.
    region: bool,
    /// Whether this deck's run plan takes the script form (`write_control::needs_script`).
    scripted: bool,
}

/// A body still to write, and the text that closes it (a script command when `script`).
struct Frame<'a> {
    items: &'a [ir::ItemRef],
    at: usize,
    tail: String,
    script: bool,
}

/// Appends `file` of `design` as ngspice text.
pub(super) fn write(
    design: &ir::Design,
    file: ir::FileId,
    out: &mut String,
    diags: &mut ir::Diags,
) {
    debug_assert!(
        ir::features_used(design).minus(super::FEATURES).is_empty(),
        "ngspice write: a design legal for FEATURES (the spine checks it)"
    );
    let f = design.file(file);
    let items = design.items(f.top);
    let deck = f.role == ir::Role::Deck;
    let mut first = 0;
    if deck {
        if let Some(&t) = items.first().filter(|t| t.kind == ir::ItemKind::Title) {
            let title: ir::item::Title = design.row(t.index);
            out.push_str(design.text(title.text));
            first = 1;
        }
        out.push('\n');
    }
    let start = out.len();
    let mut w = Out {
        design,
        out,
        diags,
        frames: Vec::new(),
        region: false,
        scripted: deck && write_control::needs_script(design, f.top),
    };
    let tail = if deck { ".end\n" } else { "" };
    w.frames.push(Frame {
        items: &items[first..],
        at: 0,
        tail: tail.to_string(),
        script: false,
    });
    // Bounded: each step writes one item or closes one frame.
    while let Some(f) = w.frames.last_mut() {
        let Some(&item) = f.items.get(f.at) else {
            let f = w.frames.pop().expect("a frame is open");
            if f.script {
                for cmd in f.tail.lines() {
                    w.script(cmd);
                }
            } else if !f.tail.is_empty() {
                w.close();
                w.out.push_str(&f.tail);
            }
            continue;
        };
        f.at += 1;
        dispatch(&mut w, item);
    }
    w.close();
    assert!(w.out.len() >= start, "the writer only appends");
    debug_assert!(!w.region, "the file ends with no script region open");
}

/// `Write::admits` (R21): a refusal for each reachable item ngspice cannot spell for a reason no
/// feature states, by the item's area (one exhaustive dispatch, as `write`'s).
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    // a string or list anywhere but a model (W1006): one walk over every item
    write_structure::admits(design, diags);
    for (item, scope) in design.reachable() {
        match item.kind {
            ir::ItemKind::Title
            | ir::ItemKind::Comment
            | ir::ItemKind::Include
            | ir::ItemKind::Global
            | ir::ItemKind::Opaque
            | ir::ItemKind::Subckt
            | ir::ItemKind::Section
            | ir::ItemKind::If
            | ir::ItemKind::Param
            | ir::ItemKind::Func => {}
            ir::ItemKind::Element => write_elements::admits(design, item, diags),
            ir::ItemKind::Model => write_models::admits(design, item, scope, diags),
            ir::ItemKind::Statistics
            | ir::ItemKind::Analysis
            | ir::ItemKind::Option
            | ir::ItemKind::Env
            | ir::ItemKind::NodeSet => write_run::admits(design, item, diags),
            ir::ItemKind::Save | ir::ItemKind::Print | ir::ItemKind::Measure => {
                write_outputs::admits(design, item, diags);
            }
            ir::ItemKind::Load | ir::ItemKind::Loop | ir::ItemKind::Alter | ir::ItemKind::Reset => {
                write_control::admits(design, item, diags);
            }
            // ngspice spells none of these (FEATURES lacks them): nothing to refuse beyond the
            // feature
            ir::ItemKind::DeviceLibrary | ir::ItemKind::Elaborate => {}
        }
    }
}

impl<'a> Out<'a> {
    /// Schedules `scope`'s items after the item being written, then `tail` (its closing lines).
    pub(super) fn open(&mut self, scope: ir::ScopeId, tail: String) {
        let items = self.design.items(scope);
        self.frames.push(Frame {
            items,
            at: 0,
            tail,
            script: false,
        });
    }

    /// [`Out::open`] for a script body (a `foreach`): `tail`'s lines are commands of the open
    /// region, not deck lines.
    pub(super) fn open_script(&mut self, scope: ir::ScopeId, tail: String) {
        let items = self.design.items(scope);
        self.frames.push(Frame {
            items,
            at: 0,
            tail,
            script: true,
        });
    }

    /// Appends `text` and a line break, a deck line: an open script region closes first.
    pub(super) fn line(&mut self, text: &str) {
        self.close();
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// Appends the script command `cmd`, opening a `.control` region when none is open.
    pub(super) fn script(&mut self, cmd: &str) {
        debug_assert!(!cmd.contains('\n'), "one command per call");
        if !self.region {
            self.out.push_str(".control\n");
            self.region = true;
        }
        self.out.push_str(cmd);
        self.out.push('\n');
    }

    /// Whether this deck's run plan takes the script form (R49, A27): run items are spelled as
    /// commands through [`Out::script`].
    pub(super) fn scripted(&self) -> bool {
        self.scripted
    }
}

// ---- private ----

impl Out<'_> {
    /// Closes an open script region with `.endc`.
    fn close(&mut self) {
        if std::mem::replace(&mut self.region, false) {
            self.out.push_str(".endc\n");
        }
    }
}

/// The one dispatch: an item's kind → its area function.
fn dispatch(w: &mut Out<'_>, item: ir::ItemRef) {
    match item.kind {
        ir::ItemKind::Title => write_structure::title(w, item),
        ir::ItemKind::Comment => write_structure::comment(w, item),
        ir::ItemKind::Include => write_structure::include(w, item),
        ir::ItemKind::Global => write_structure::global(w, item),
        ir::ItemKind::Load => write_control::load(w, item),
        ir::ItemKind::Opaque => write_structure::opaque(w, item),
        ir::ItemKind::Subckt => write_structure::subckt(w, item),
        ir::ItemKind::Section => write_structure::section(w, item),
        ir::ItemKind::If => write_structure::if_(w, item),
        ir::ItemKind::Param => write_structure::param(w, item),
        ir::ItemKind::Func => write_structure::func(w, item),
        ir::ItemKind::Model => write_models::model(w, item),
        ir::ItemKind::Statistics => write_run::statistics(w, item),
        ir::ItemKind::Element => write_elements::element(w, item),
        ir::ItemKind::Analysis => write_run::analysis(w, item),
        ir::ItemKind::Loop => write_control::loop_(w, item),
        ir::ItemKind::Alter => write_control::alter(w, item),
        ir::ItemKind::Reset => write_control::reset(w, item),
        ir::ItemKind::Option => write_run::option(w, item),
        ir::ItemKind::Env => write_run::env(w, item),
        ir::ItemKind::NodeSet => write_run::node_set(w, item),
        ir::ItemKind::Save => write_outputs::save(w, item),
        ir::ItemKind::Print => write_outputs::print(w, item),
        ir::ItemKind::Measure => write_outputs::measure(w, item),
        ir::ItemKind::DeviceLibrary | ir::ItemKind::Elaborate => unspelled(item),
    }
}

/// A kind ngspice spells nothing for (a VACASK device library, an `elaborate`; A12): the spine
/// never hands one here, since `FEATURES` lacks its item feature; asserts so.
fn unspelled(item: ir::ItemRef) {
    let feature = ir::Feature::Item(item.kind);
    assert!(
        !super::FEATURES.contains(feature),
        "ngspice write: FEATURES has {:?} but nothing spells it",
        item.kind
    );
    unreachable!("ngspice write: no {:?} is legal for ngspice", item.kind)
}
