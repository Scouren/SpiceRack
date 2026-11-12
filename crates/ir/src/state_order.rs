//! Where the run items stand: the listed order of a deck's items with includes read in place
//! (A14, R24), from which the mid-run state features come, and the order of a file top's items,
//! from which `RunInterleaved` comes (A22). A design in, features out.
//!
//! Cites: `rewrite/AMEND-A12.md` A14, A22; `rewrite/RULINGS.md` R24, R26, R43;
//! `rewrite/IR-RUN.md` §1.
//!
//! Contents: `pub(crate) fn state_order_features`, `pub(crate) fn interleaved_features`,
//! `pub(crate) fn run_append_ok` (gen)

use crate::design::{Design, ItemRow};
use crate::feature::{Feature, FeatureSet};
use crate::id::{FileId, ScopeId};
#[cfg(any(test, feature = "gen"))]
use crate::item::ScopeKind;
use crate::item::{ItemKind, Role};
use crate::option::{Applies, Env, NodeSet, OptionRow};
use crate::run_extra::RunExtra;

/// How a file top's item counts for `RunInterleaved`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Listed {
    /// A run owner's statement kind.
    Run,
    /// A comment or an opaque: stands anywhere.
    Neutral,
    /// Anything else: between two run items, it splits them.
    Other,
}

/// The longest run of items `run_append_ok` reads back before it refuses (O(1) per call).
#[cfg(any(test, feature = "gen"))]
const SCAN_MAX: usize = 32;

/// Adds the features of state items that come after a run item (an analysis, loop or alter)
/// in listed order (A14): per deck file, its items depth first with each body right after its
/// item, an `Include` read as the items of the file it names, in place (once per walk). A
/// `FromHere` item there is `OptionMidRun`, `EnvMidRun` or `NodeSetMidRun`; a `WholeRun` one is
/// `WholeRunMidRun`. Files no deck includes are walked alone. Explicit stack (C11);
/// O(items + scopes) per walk.
pub(crate) fn state_order_features(design: &Design, set: &mut FeatureSet) {
    let bodies = design.bodies();
    let files: Vec<FileId> = design.files().collect();
    let mut reached = vec![false; files.len()];
    let decks = files.iter().filter(|&&f| design.file(f).role == Role::Deck);
    let rest = files.iter().filter(|&&f| design.file(f).role != Role::Deck);
    for &start in decks.chain(rest) {
        if reached[start.index()] && design.file(start).role != Role::Deck {
            continue;
        }
        reached[start.index()] = true;
        let mut entered = vec![false; design.scopes.len()];
        let mut seen_run = false;
        let top = design.file(start).top;
        entered[top.index()] = true;
        let mut stack: Vec<(ScopeId, usize)> = vec![(top, 0)];
        while let Some((scope, k)) = stack.pop() {
            let Some(&item) = design.items(scope).get(k) else {
                continue;
            };
            stack.push((scope, k + 1));
            let holds = if item.kind == ItemKind::Option {
                Some((
                    OptionRow::table(design).get(item.index).holds,
                    RunExtra::OptionMidRun,
                ))
            } else if item.kind == ItemKind::Env {
                Some((
                    Env::table(design).get(item.index).holds,
                    RunExtra::EnvMidRun,
                ))
            } else if item.kind == ItemKind::NodeSet {
                Some((
                    NodeSet::table(design).get(item.index).holds,
                    RunExtra::NodeSetMidRun,
                ))
            } else {
                None
            };
            if let Some((holds, from_here)) = holds
                && seen_run
            {
                let x = match holds {
                    Applies::FromHere => from_here,
                    Applies::WholeRun => RunExtra::WholeRunMidRun,
                };
                *set = set.with(Feature::Run(x));
            }
            seen_run |= matches!(
                item.kind,
                ItemKind::Analysis | ItemKind::Loop | ItemKind::Alter
            );
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
    }
}

/// Adds `RunInterleaved` when a file top lists an item that is no run item, comment or opaque
/// between two run items (A22). O(items of the file tops).
pub(crate) fn interleaved_features(design: &Design, set: &mut FeatureSet) {
    for f in design.files() {
        // 0: no run item yet; 1: a run item; 2: a run item, then another item
        let mut state = 0u8;
        for item in design.items(design.file(f).top) {
            state = match (listed(item.kind), state) {
                (Listed::Run, 2) => {
                    *set = set.with(Feature::Run(RunExtra::RunInterleaved));
                    return;
                }
                (Listed::Run, _) => 1,
                (Listed::Other, 1) => 2,
                (Listed::Neutral | Listed::Other, s) => s,
            };
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

/// Each item kind's `Listed` class.
const fn listed(kind: ItemKind) -> Listed {
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
        | ItemKind::Elaborate => Listed::Run,
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
