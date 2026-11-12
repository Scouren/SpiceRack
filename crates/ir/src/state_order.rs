//! Where the state items stand: an item's nesting (R26) and the listed order of a deck's items
//! with includes read in place (A14, R24), from which the mid-run state features come. A design
//! in, answers and features out.
//!
//! Cites: `rewrite/AMEND-A12.md` A14; `rewrite/RULINGS.md` R24, R26; `rewrite/IR-RUN.md` §1.
//!
//! Contents: `pub(crate) fn nested`, `pub(crate) fn state_order_features`

use crate::design::{Design, ItemRow};
use crate::feature::{Feature, FeatureSet};
use crate::id::{FileId, ScopeId};
use crate::item::{ItemKind, Role, ScopeKind};
use crate::option::{Applies, Env, NodeSet, OptionRow};
use crate::run_extra::RunExtra;
use crate::run_words::in_scope_kind;

/// Whether an item listed in `scope` sits inside a subcircuit or section body (R26).
pub(crate) fn nested(design: &Design, scope: ScopeId) -> bool {
    in_scope_kind(design, scope, ScopeKind::Subckt)
        || in_scope_kind(design, scope, ScopeKind::Section)
}

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
