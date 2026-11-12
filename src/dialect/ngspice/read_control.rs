//! Control scripts → run items: every `.control` … `.endc` block and `*#` line of a deck is one
//! script, run after the parse in file order (`rewrite/S3-CONTROL.md` O1); each statement's
//! commands become items of `st.scope` at its place, in script order (R49 (2)). Analysis
//! commands are the run area's (`read_run::read_analysis`), `pre_osdi` a `Load`, `alter`,
//! `altermod` and an `alterparam` (at the `reset` that applies it) `Alter`s, a script
//! `option`/`set` of a simulator option a `FromHere` state item, a `foreach` over constant
//! words a `Loop` when it is exactly the IR loop's law and else unrolled, a `repeat n` unrolled;
//! a display command an `Inert` opaque of its own `.control` pair (blocks concatenate, so the
//! pair is exact where the writer puts it). A command the IR cannot state is an opaque of its
//! own at its place; data-dependent flow, a typed command reading a reprinted variable, or a
//! state change in a deck with dot-card analyses (R49 (3)) makes the whole script one `Script`.
//! A `foreach` variable named `sr<depth>` at that loop depth is read as an unnamed loop: it is
//! the variable the writer makes up for one (R42's pattern; a user's own `sr1` at depth 1
//! round-trips to the same text).
//!
//! The function validates the whole statement before it pushes anything, binds references by
//! ngspice's rules (R3) and never learns a target. Codes: `P1400–P1499`.
//!
//! Cites: `rewrite/S3-CONTROL.md` (the command table, the settle decks); `rewrite/RULINGS.md`
//! R49; ngspice-45 `inp.c:742-859` (the script), `control.c` (loops), `variable.c`
//! (substitution), `com_option.c`, `options.c` (`option` = `set`), `device.c` (`alter`),
//! `inp.c:1735-1857` (`alterparam`), `runcoms2.c:172-240` (`reset`).
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! the codes `pub(super) const FLOW`, `READS`, `EFFECT`, `STATE_WITH_CARDS`, `UNROLL`, `DISPLAY`,
//! `NO_RESET`, `FORM`, `OWN`, `IN_BRANCH`, `NO_COMMAND`, `REJECTED`,
//! `pub(super) fn control(lw: &mut Lower<'_, '_>, st: Stmt)`,
//! `pub(super) fn script_line(lw: &mut Lower<'_, '_>, scope: ir::ScopeId, index: usize, span: ir::Span)`

mod args;
mod plan;
mod push;
mod script;
mod table;

use super::codes::Row;
use super::read::{Lower, Stmt};
use super::tables;
use crate::ir::{self, Feature, ItemKind, RunExtra, ScopeKind};

/// What this area's reader produces (the image its writer spells, L5).
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    Feature::Item(ItemKind::Alter),
    Feature::Item(ItemKind::Loop),
    Feature::Item(ItemKind::Load),
    Feature::Item(ItemKind::Reset),
    Feature::Run(RunExtra::ResetValues),
    Feature::Run(RunExtra::ResetOptions),
    Feature::Run(RunExtra::ResetTemp),
    Feature::Run(RunExtra::ResetTnom),
    Feature::Run(RunExtra::ResetSaves),
    Feature::Placed(ScopeKind::Loop, ItemKind::Reset),
    Feature::Run(RunExtra::LoopKeepLast),
    Feature::Run(RunExtra::LoopNamed),
    // a `foreach` whose variable is the writer's made-up `sr<depth>` (A35)
    Feature::Run(RunExtra::LoopUnnamed),
    Feature::Run(RunExtra::LoopOverOp),
    Feature::Run(RunExtra::LoopScoped),
    Feature::Run(RunExtra::LoopParam),
    Feature::Run(RunExtra::LoopSourceDc),
    Feature::Run(RunExtra::LoopElementValue),
    Feature::Run(RunExtra::LoopDeviceParam),
    Feature::Run(RunExtra::LoopModelParam),
    Feature::Run(RunExtra::SweepLoopList),
    Feature::Run(RunExtra::AlterParam),
    Feature::Run(RunExtra::AlterSourceDc),
    Feature::Run(RunExtra::AlterElementValue),
    Feature::Run(RunExtra::AlterDeviceParam),
    Feature::Run(RunExtra::AlterModelParam),
    // a script `option`/`set` holds from its place; no script command sets a node's condition
    // (`commands.c`), so `NodeSetFromHere` is no member
    Feature::Run(RunExtra::OptionFromHere),
    Feature::Run(RunExtra::EnvFromHere),
    Feature::Run(RunExtra::OptionMidRun),
    Feature::Run(RunExtra::EnvMidRun),
    Feature::Run(RunExtra::SweepBodyNotOne),
    Feature::Placed(ScopeKind::Loop, ItemKind::Analysis),
    Feature::Placed(ScopeKind::Loop, ItemKind::Alter),
    Feature::Placed(ScopeKind::Loop, ItemKind::Loop),
    Feature::Placed(ScopeKind::Loop, ItemKind::Option),
    Feature::Placed(ScopeKind::Loop, ItemKind::Env),
    Feature::Placed(ScopeKind::Loop, ItemKind::Opaque),
]);

/// Data-dependent flow (`if`, `while`, `dowhile`, `break`, `continue`, `label`, `goto`, an
/// unclosed or stray `end`, a `foreach` over words that are not constants): `Script`.
pub(super) const FLOW: ir::Code = ir::Code::new(b'P', 1401);
/// A typed command reading a `set` variable or a vector, both reprinted with six digits
/// (`variable.c:41-56`, `:1045`): `Script`.
pub(super) const READS: ir::Code = ir::Code::new(b'P', 1402);
/// A command whose effect the IR cannot state (`source`, `shell`, `setcirc`, `load`, `stop`, …):
/// `Script`.
pub(super) const EFFECT: ir::Code = ir::Code::new(b'P', 1403);
/// A script that changes state in a deck with a dot-card analysis (R49 (3)): `Script`.
pub(super) const STATE_WITH_CARDS: ir::Code = ir::Code::new(b'P', 1404);
/// A loop unrolled past [`plan::UNROLL_MAX`] commands: `Script`.
pub(super) const UNROLL: ir::Code = ir::Code::new(b'P', 1405);
/// A display command: `Inert`.
pub(super) const DISPLAY: ir::Code = ir::Code::new(b'P', 1406);
/// An `alterparam` with no `reset` after it: it changes nothing (S2): `Inert`.
pub(super) const NO_RESET: ir::Code = ir::Code::new(b'P', 1407);
/// A command form the IR cannot state yet (a vector or expression value, a subcircuit path,
/// `alterparam <sub>`, a `reset` with state to restore, `save`/`meas`/`fourier` outside their
/// exact case): `Unread`.
pub(super) const FORM: ir::Code = ir::Code::new(b'P', 1408);
/// ngspice's own (`setseed`, `optran`, `codemodel`, an unclassified variable): `Private`.
pub(super) const OWN: ir::Code = ir::Code::new(b'P', 1409);
/// A block in an `.if` branch: it runs whatever the condition (R49, lead's Q8): `Private`.
pub(super) const IN_BRANCH: ir::Code = ir::Code::new(b'P', 1410);
/// A word that is no command, or `osdi` after the parse: `InvalidInSource`.
pub(super) const NO_COMMAND: ir::Code = ir::Code::new(b'P', 1411);
/// A command ngspice rejects (`alter` with no value, an `alterparam` with no `=`):
/// `InvalidInSource`.
pub(super) const REJECTED: ir::Code = ir::Code::new(b'P', 1412);

/// This area's codes, `P1400–P1499`.
pub(super) const CODES: &[Row] = &[
    note(
        FLOW,
        "a script with data-dependent flow",
        "ngspice-45 control.c; S3-CONTROL §2",
        "if, while, dowhile, break, continue, label, goto: what runs depends on results; the whole script is kept as text (Script).",
    ),
    note(
        READS,
        "a script command reading a reprinted variable",
        "ngspice-45 variable.c:41-56, :1045; S3-CONTROL V2, V3",
        "a set number or a vector is substituted with six significant digits; the whole script is kept as text (Script).",
    ),
    note(
        EFFECT,
        "a script command with effects the IR cannot state",
        "S3-CONTROL §2",
        "source, shell, setcirc, load, stop and the like change what runs; the whole script is kept as text (Script).",
    ),
    note(
        STATE_WITH_CARDS,
        "a script that changes state in a deck with dot-card analyses",
        "rewrite/RULINGS.md R49 (3); S3-CONTROL O3, O4",
        "the cards run at each run and after the script on its final state; the whole script is kept as text (Script).",
    ),
    note(
        UNROLL,
        "a script loop past the unroll capacity",
        "S3-CONTROL §2 (foreach, repeat)",
        "the unrolled commands would pass the capacity; the whole script is kept as text (Script).",
    ),
    note(
        DISPLAY,
        "a display command",
        "S3-CONTROL §2",
        "it moves no number; kept as its own .control pair (Inert).",
    ),
    note(
        NO_RESET,
        "an alterparam with no reset after it",
        "ngspice-45 inp.c:1735-1857; S3-CONTROL S2",
        "alterparam changes the deck only at the next reset; with none it changes nothing (Inert).",
    ),
    note(
        FORM,
        "a script command form the IR cannot state yet",
        "rewrite/RULINGS.md R49 (5)",
        "kept as its own .control pair (Unread).",
    ),
    note(
        OWN,
        "a script command of ngspice's own",
        "rewrite/RULINGS.md R49 (5)",
        "setseed, optran, codemodel or an unclassified variable; kept as its own .control pair (Private).",
    ),
    note(
        IN_BRANCH,
        "a control block in an .if branch",
        "ngspice-45 inp.c:742-812; S3-CONTROL O1",
        "ngspice runs it whatever the condition; kept as text (Private).",
    ),
    note(
        NO_COMMAND,
        "a script word ngspice has no command for",
        "ngspice-45 commands.c; S3-CONTROL §2, L1",
        "ngspice says 'no such command' (or loads osdi too late for the deck); kept as text (InvalidInSource).",
    ),
    note(
        REJECTED,
        "a script command ngspice rejects",
        "ngspice-45 device.c:1249-1420, inp.c:1735-1857",
        "a missing value or assignment; kept as text (InvalidInSource).",
    ),
];

/// A control block: the deck's script read once, this statement's part pushed.
pub(super) fn control(lw: &mut Lower<'_, '_>, st: Stmt) {
    if in_branch(lw, st.scope) {
        return whole(lw, st, ir::OpaqueClass::Private, IN_BRANCH);
    }
    let me = lw.script_index(st);
    let ops = match planned(lw, me) {
        Err(code) => return whole(lw, st, ir::OpaqueClass::Script, code),
        Ok(ops) => ops,
    };
    if ops.is_empty() {
        return lw.framing();
    }
    let ready = push::ready(lw, st, &ops);
    push::push(lw, st, st.scope, &ready);
}

/// A `*#` line outside a block (R49): statement `index` of `lw.deck_script()`, into `scope` at
/// `span`. It has no CST node, so only what needs none is typed (a `Load`, an opaque); a typed
/// command that reads a value is `Unread` here.
// ponytail: an analysis or alter on a `*#` line stays Unread (no corpus deck has one); type it
// when `read_analysis` takes a span instead of a statement.
pub(super) fn script_line(
    lw: &mut Lower<'_, '_>,
    scope: ir::ScopeId,
    index: usize,
    span: ir::Span,
) {
    let text = lw.deck_script()[index].clone();
    if in_branch(lw, scope) {
        return lw.opaque_text(scope, &text, span, ir::OpaqueClass::Private, IN_BRANCH);
    }
    let ops = match planned(lw, index) {
        Err(code) => return lw.opaque_text(scope, &text, span, ir::OpaqueClass::Script, code),
        Ok(ops) => ops,
    };
    if ops.is_empty() {
        return lw.opaque_text(scope, &text, span, ir::OpaqueClass::Inert, DISPLAY);
    }
    for op in &ops {
        match op {
            plan::Op::Load { path, .. } => {
                lw.b.load(scope, ir::LoadKind::Osdi, path, span);
            }
            plan::Op::Opaque { class, code, cmd } => {
                lw.opaque_text(scope, &cmd.opaque_text(), span, *class, *code);
            }
            _ => lw.opaque_text(scope, &text, span, ir::OpaqueClass::Unread, FORM),
        }
    }
}

// ---- private ----

/// The whole block as one opaque. A block the file ends before closing (ngspice reads it to
/// the end of the file, as the CST does) gets its `.endc`: the same script, and a deck line
/// written after it stays outside it.
fn whole(lw: &mut Lower<'_, '_>, st: Stmt, class: ir::OpaqueClass, code: ir::Code) {
    let mut text = lw.raw(st.node);
    let last = text
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if !last.to_ascii_lowercase().starts_with(".endc") {
        text.push_str("\n.endc");
    }
    let span = lw.span(st.node);
    lw.opaque_text(st.scope, &text, span, class, code);
}

/// `Info` of a note.
const fn note(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
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

/// Whether `scope` is an `.if` branch or inside one.
fn in_branch(lw: &Lower<'_, '_>, scope: ir::ScopeId) -> bool {
    let d = lw.b.design();
    let mut s = scope;
    // Bounded: each step goes one scope outward; a file's top scope has no parent.
    while !s.is_none() {
        let sc = d.scope(s);
        if sc.kind == ir::ScopeKind::Branch {
            return true;
        }
        s = sc.parent;
    }
    false
}

/// The operations of script statement `me`, the deck's whole script read (R49 (2)), or the
/// whole script's `Script` code.
// ponytail: the deck is this file's script statements (`Lower::deck_script`), until the
// declare step inlines includes: a block in an included file is read alone.
fn planned(lw: &Lower<'_, '_>, me: usize) -> Result<Vec<plan::Op>, ir::Code> {
    let texts = lw.deck_script();
    assert!(
        me < texts.len(),
        "control: the statement is one of the deck's script"
    );
    let blocks: Vec<script::Block> = texts.iter().map(|t| script::block(t)).collect();
    let cards = lw.deck_card_analyses();
    debug_assert_eq!(
        lw.deck_has_card_analysis(),
        !cards.is_empty(),
        "the declare step's two card facts agree"
    );
    let mut by_block = plan::plan(&blocks, cards)?;
    Ok(std::mem::take(&mut by_block[me]))
}
