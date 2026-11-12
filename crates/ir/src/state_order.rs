//! Where the run items stand, and what holds where. A design in; its walks in listed order, the
//! state item in force at a run item, and the order features out:
//! - the listed order of a deck's items, with includes read in place (A14, R24): the walks the
//!   mid-run state features come from, and the order the run passes move items in;
//! - which item's value holds at a point of a walk, `WholeRun` against `FromHere` (A28);
//! - the order of a file top's items, from which `RunInterleaved` comes (A22).
//!
//! Cites: `rewrite/AMEND-A12.md` A14, A22, A28; `rewrite/RULINGS.md` R24, R26, R43, R49;
//! `rewrite/IR-RUN.md` §1; `ng-control`'s `S3-CONTROL.md` S1, S3.
//!
//! Contents: `pub enum Listed`, `pub const fn listed(ItemKind) -> Listed`, `pub enum Setting`,
//! `pub fn run_walks(&Design) -> Vec<(ScopeId, Vec<(ItemRef, ScopeId)>)>`,
//! `pub fn in_force(&Design, &[(ItemRef, ScopeId)], usize, Setting) -> ItemRef`; crate:
//! `pub(crate) fn state_order_features`, `pub(crate) fn interleaved_features`,
//! `pub(crate) fn run_append_ok` (gen)

use crate::design::{Design, ItemRow};
use crate::feature::Feature;
#[cfg(any(test, feature = "gen"))]
use crate::feature::FeatureSet;
use crate::id::{FileId, ItemRef, NameId, ScopeId};
#[cfg(any(test, feature = "gen"))]
use crate::item::ScopeKind;
use crate::item::{ItemKind, Role};
use crate::option::{Applies, Env, NodeSet, OptionRow};
use crate::option_key::OptionKey;
use crate::output::Save;
use crate::reset::{Reset, Restorable};
use crate::run_extra::RunExtra;

/// How an item counts in a file top's order (`RunInterleaved`, A22).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listed {
    /// A run owner's statement kind: analysis, loop, alter, option, env, node set, save,
    /// print, measure, elaborate, reset.
    Run,
    /// A comment or an opaque: stands anywhere.
    Neutral,
    /// Anything else: between two run items, it splits them.
    Other,
}

/// One thing a state item sets, for [`in_force`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    /// A portable option key (never `OptionKey::Private`).
    Option(OptionKey),
    /// A private option: its name and its dialect, as `OptionRow` holds them.
    Private(NameId, NameId),
    /// The temperature (`Env::temp`).
    Temp,
    /// The nominal temperature (`Env::tnom`).
    Tnom,
}

/// The longest run of items `run_append_ok` reads back before it refuses (O(1) per call).
#[cfg(any(test, feature = "gen"))]
const SCAN_MAX: usize = 32;

/// Each item kind's `Listed` class.
pub const fn listed(kind: ItemKind) -> Listed {
    match kind {
        ItemKind::Analysis
        | ItemKind::Loop
        | ItemKind::Alter
        | ItemKind::Option
        | ItemKind::Env
        | ItemKind::NodeSet
        | ItemKind::Save
        | ItemKind::Print
        | ItemKind::Measure
        | ItemKind::Elaborate
        | ItemKind::Reset => Listed::Run,
        ItemKind::Comment | ItemKind::Opaque => Listed::Neutral,
        ItemKind::Title
        | ItemKind::Include
        | ItemKind::Global
        | ItemKind::Load
        | ItemKind::Subckt
        | ItemKind::Section
        | ItemKind::If
        | ItemKind::Param
        | ItemKind::Func
        | ItemKind::Model
        | ItemKind::DeviceLibrary
        | ItemKind::Statistics
        | ItemKind::Element => Listed::Other,
    }
}

/// Every walk of the design in listed order (A14, R24): its top scope, and its items with the
/// scope listing each. A walk starts at a deck's top, then at each file no walk reached; it
/// reads its items depth first, each body right after its item, and an `Include` as the items
/// of the file it names, in place (each scope once per walk). This is the run order the IR
/// defines, which the run passes walk. O(items + scopes) per walk; explicit stack (C11).
pub fn run_walks(design: &Design) -> Vec<(ScopeId, Vec<(ItemRef, ScopeId)>)> {
    let bodies = design.bodies();
    let files: Vec<FileId> = design.files().collect();
    let mut reached = vec![false; files.len()];
    let decks = files.iter().filter(|&&f| design.file(f).role == Role::Deck);
    let rest = files.iter().filter(|&&f| design.file(f).role != Role::Deck);
    let mut out = Vec::new();
    for &start in decks.chain(rest) {
        if reached[start.index()] && design.file(start).role != Role::Deck {
            continue;
        }
        reached[start.index()] = true;
        let mut entered = vec![false; design.scopes.len()];
        let top = design.file(start).top;
        entered[top.index()] = true;
        let mut order = Vec::new();
        let mut stack: Vec<(ScopeId, usize)> = vec![(top, 0)];
        while let Some((scope, k)) = stack.pop() {
            let Some(&item) = design.items(scope).get(k) else {
                continue;
            };
            stack.push((scope, k + 1));
            order.push((item, scope));
            if item.kind == ItemKind::Include {
                let path = design.str(crate::item::Include::table(design).get(item.index).path);
                let target = design.find_file(path);
                if !target.is_none() {
                    reached[target.index()] = true;
                    let t = design.file(target).top;
                    if !std::mem::replace(&mut entered[t.index()], true) {
                        stack.push((t, 0));
                    }
                }
            }
            for &body in bodies
                .get(&item)
                .map_or(&[][..], |v| v.as_slice())
                .iter()
                .rev()
            {
                if !std::mem::replace(&mut entered[body.index()], true) {
                    stack.push((body, 0));
                }
            }
        }
        out.push((top, order));
    }
    out
}

/// The state item whose value of `what` holds at position `at` of a walk (`run_walks`), or
/// `ItemRef::NONE` when none sets it there (the simulator's default holds). The precedence
/// (A28, by ngspice's runs: a script `option` over an `.options` card, `reset` dropping the
/// script's and keeping the card's): the last `FromHere` item before `at` that sets `what`,
/// unless a `Reset` of its class stands between it and `at`; else the last `WholeRun` item of
/// the whole walk that sets it, wherever it stands. This reads one pass: in a loop body's later
/// passes, the items listed after `at` in the body have also run before it. Asserts `at` is
/// in the walk. O(walk length).
pub fn in_force(
    design: &Design,
    order: &[(ItemRef, ScopeId)],
    at: usize,
    what: Setting,
) -> ItemRef {
    assert!(at < order.len(), "a position in the walk");
    let class = match what {
        Setting::Option(_) | Setting::Private(..) => Restorable::Options,
        Setting::Temp => Restorable::Temp,
        Setting::Tnom => Restorable::Tnom,
    };
    let mut from_here = ItemRef::NONE;
    for &(item, _) in &order[..at] {
        if item.kind == ItemKind::Reset
            && Reset::table(design)
                .get(item.index)
                .restores
                .contains(class)
        {
            from_here = ItemRef::NONE;
        } else if sets(design, item, what) == Some(Applies::FromHere) {
            from_here = item;
        }
    }
    if from_here != ItemRef::NONE {
        return from_here;
    }
    order
        .iter()
        .rev()
        .map(|&(item, _)| item)
        .find(|&item| sets(design, item, what) == Some(Applies::WholeRun))
        .unwrap_or(ItemRef::NONE)
}

/// Appends, at the state item, the features of state items that come after a run item (an
/// analysis, loop or alter) in a walk's listed order (A14): a `FromHere` item there is
/// `OptionMidRun`, `EnvMidRun`, `NodeSetMidRun` or `SaveMidRun`; a `WholeRun` one is
/// `WholeRunMidRun`. O(items + scopes) per walk.
pub(crate) fn state_order_features(design: &Design, out: &mut Vec<(Feature, ItemRef)>) {
    for (_, order) in run_walks(design) {
        let mut seen_run = false;
        for (item, _) in order {
            let holds = match item.kind {
                ItemKind::Option => Some((
                    OptionRow::table(design).get(item.index).holds,
                    RunExtra::OptionMidRun,
                )),
                ItemKind::Env => Some((
                    Env::table(design).get(item.index).holds,
                    RunExtra::EnvMidRun,
                )),
                ItemKind::NodeSet => Some((
                    NodeSet::table(design).get(item.index).holds,
                    RunExtra::NodeSetMidRun,
                )),
                ItemKind::Save => Some((
                    Save::table(design).get(item.index).holds,
                    RunExtra::SaveMidRun,
                )),
                ItemKind::Title
                | ItemKind::Comment
                | ItemKind::Include
                | ItemKind::Global
                | ItemKind::Load
                | ItemKind::Opaque
                | ItemKind::Subckt
                | ItemKind::Section
                | ItemKind::If
                | ItemKind::Param
                | ItemKind::Func
                | ItemKind::Model
                | ItemKind::DeviceLibrary
                | ItemKind::Statistics
                | ItemKind::Element
                | ItemKind::Analysis
                | ItemKind::Loop
                | ItemKind::Alter
                | ItemKind::Print
                | ItemKind::Measure
                | ItemKind::Elaborate
                | ItemKind::Reset => None,
            };
            if let Some((holds, from_here)) = holds
                && seen_run
            {
                let x = match holds {
                    Applies::FromHere => from_here,
                    Applies::WholeRun => RunExtra::WholeRunMidRun,
                };
                out.push((Feature::Run(x), item));
            }
            seen_run |= matches!(
                item.kind,
                ItemKind::Analysis | ItemKind::Loop | ItemKind::Alter
            );
        }
    }
}

/// Appends `RunInterleaved` at each item of a file top that is no run item, comment or opaque
/// and is listed between two run items there (A22). O(items of the file tops).
pub(crate) fn interleaved_features(design: &Design, out: &mut Vec<(Feature, ItemRef)>) {
    for f in design.files() {
        let mut seen_run = false;
        // the other items since the last run item; each counts when a run item follows
        let mut between: Vec<ItemRef> = Vec::new();
        for &item in design.items(design.file(f).top) {
            match listed(item.kind) {
                Listed::Run => {
                    out.extend(
                        between
                            .drain(..)
                            .map(|i| (Feature::Run(RunExtra::RunInterleaved), i)),
                    );
                    seen_run = true;
                }
                Listed::Other if seen_run => between.push(item),
                Listed::Other | Listed::Neutral => {}
            }
        }
    }
}

/// Whether a run item appended to `scope` keeps a design without `RunInterleaved` without it:
/// always when the set has the feature or `scope` is no file top; else when no other item
/// follows the top's last run item. Reads back at most `SCAN_MAX` items and refuses when that
/// does not decide. For a run row's generator, before it pushes anything.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn run_append_ok(design: &Design, scope: ScopeId, features: &FeatureSet) -> bool {
    if features.contains(Feature::Run(RunExtra::RunInterleaved))
        || design.scope(scope).kind != ScopeKind::File
    {
        return true;
    }
    let items = design.items(scope);
    let mut other = false;
    for item in items.iter().rev().take(SCAN_MAX) {
        match listed(item.kind) {
            Listed::Run => return !other,
            Listed::Other => other = true,
            Listed::Neutral => {}
        }
    }
    items.len() <= SCAN_MAX // read to the start: no run item before
}

// ---- private ----

/// The mode of `item` when it is a state item that sets `what`, else `None`.
fn sets(design: &Design, item: ItemRef, what: Setting) -> Option<Applies> {
    match (item.kind, what) {
        (ItemKind::Option, Setting::Option(key)) => {
            let row = OptionRow::table(design).get(item.index);
            (row.key == key).then_some(row.holds)
        }
        (ItemKind::Option, Setting::Private(name, dialect)) => {
            let row = OptionRow::table(design).get(item.index);
            let same =
                row.key == OptionKey::Private && row.private == name && row.dialect == dialect;
            same.then_some(row.holds)
        }
        (ItemKind::Env, Setting::Temp) => {
            let row = Env::table(design).get(item.index);
            (!row.temp.is_none()).then_some(row.holds)
        }
        (ItemKind::Env, Setting::Tnom) => {
            let row = Env::table(design).get(item.index);
            (!row.tnom.is_none()).then_some(row.holds)
        }
        _ => None, // else: only an option row or an env sets one of these
    }
}
