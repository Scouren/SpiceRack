//! Rule `whole_run_state_first`: state items that hold for the whole run (`Applies::WholeRun`:
//! options, `Env`, node sets, saves) made positional. Three rows:
//! - `whole_run_state_first` removes `Run(WholeRunMidRun)`: every `WholeRun` item after a run
//!   item (an analysis, loop or alter) in listed order moves to just before the first run item,
//!   still `WholeRun`, in its listed order. Exact: a `WholeRun` item holds wherever it stands,
//!   and among items that set one thing the last in listed order holds, which the move keeps
//!   (every moved item stays after every item that stood before the first run item). Requires
//!   `Run(RunInterleaved)`: a moved item may land after a circuit item that follows an earlier
//!   output item (`run_items_together` removes it). An item not listed in its walk's top scope,
//!   or a walk whose first run item is not, keeps the feature with `STATE_ELSEWHERE` (a move
//!   across files or out of a body would change what includes it), and so does a next-only
//!   node set (it holds for the analysis after it: a move would change which).
//! - `whole_run_to_from_here` removes `Run(StateWholeRun)` and `whole_run_save_to_from_here`
//!   removes `Run(SaveWholeRun)`: a `WholeRun` option, `Env` or node set (or save) standing before
//!   every run item becomes `FromHere` (requires `Run(StateFromHere)`, `Run(SaveFromHere)`).
//!   Exact there: it holds for every run item after it, that is for all of them. Left with a
//!   note: one after a run item (`MID_RUN`), one whose class a `Reset` of the walk restores
//!   (`RESET_RESTORES`: a reset drops `FromHere` state, never `WholeRun`), one a `FromHere` item
//!   of the walk sets the same thing as (`FROM_HERE_SAME`: which of the two holds is not
//!   stated by the IR, so the item keeps its mode).
//!
//! Listed order is the IR's (`Applies` docs, `state_order.rs`): per deck file its items depth
//! first, each body right after its item, an `Include` read as the items of the file it names
//! in place; a file no deck includes is walked alone.
//!
//! Cites: `rewrite/AMEND-A12.md` A14, A22, A27, A28; R24, R26; `rewrite/IR-RUN.md` §1.
//!
//! Contents: `pub fn whole_run_state_first(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn whole_run_to_from_here(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn whole_run_save_to_from_here(&mut ir::Design, &mut ir::Diags)`,
//! `pub const STATE_ELSEWHERE: ir::Code`, `pub const MID_RUN: ir::Code`,
//! `pub const RESET_RESTORES: ir::Code`, `pub const FROM_HERE_SAME: ir::Code`; to the group:
//! `pub(super) fn walks(&ir::Design) -> Vec<(ir::ScopeId, Vec<(ir::ItemRef, ir::ScopeId)>)>`

use std::collections::{HashMap, HashSet};

use crate::ir;

/// The note at a mid-run `WholeRun` item the pass cannot move into place.
pub const STATE_ELSEWHERE: ir::Code = ir::Code::new(b'U', 400);
/// The note at a `WholeRun` item after a run item (not made `FromHere` there).
pub const MID_RUN: ir::Code = ir::Code::new(b'U', 401);
/// The note at a `WholeRun` item whose class a `Reset` of its walk restores.
pub const RESET_RESTORES: ir::Code = ir::Code::new(b'U', 402);
/// The note at a `WholeRun` item a `FromHere` item of its walk sets the same thing as.
pub const FROM_HERE_SAME: ir::Code = ir::Code::new(b'U', 403);

/// Moves each walk's mid-run `WholeRun` items to just before its first run item.
pub fn whole_run_state_first(design: &mut ir::Design, diags: &mut ir::Diags) {
    let noted = diags.len();
    for (top, order) in walks(design) {
        let Some(first) = order.iter().position(|&(i, _)| is_run(i.kind)) else {
            continue;
        };
        let in_place = order[first].1 == top;
        let mut moved = Vec::new();
        for &(item, scope) in &order[first + 1..] {
            if holds(design, item) != Some(ir::Applies::WholeRun) {
                continue;
            }
            // a next-only condition holds for the analysis after it: a move would change which
            let next_only = item.kind == ir::ItemKind::NodeSet
                && design.row::<ir::NodeSet>(item.index).next_only;
            match in_place && scope == top && !next_only {
                true => moved.push(item),
                false => diags.push_at(STATE_ELSEWHERE, design, item, 0),
            }
        }
        if moved.is_empty() {
            continue;
        }
        let anchor = order[first].0;
        let mut editor = design.editor();
        let items = editor.items_mut(top);
        let before = items.len();
        items.retain(|i| !moved.contains(i));
        let at = items
            .iter()
            .position(|&i| i == anchor)
            .expect("the first run item is listed");
        items.splice(at..at, moved);
        assert!(items.len() == before, "a move keeps every item");
    }
    // the feature is the design's (`check_legal` reports it at no item): one note there too
    if diags.len() > noted {
        diags.push_at(STATE_ELSEWHERE, design, ir::ItemRef::NONE, 0);
    }
}

/// Makes each walk's leading `WholeRun` options, `Env`s and node sets `FromHere`.
pub fn whole_run_to_from_here(design: &mut ir::Design, diags: &mut ir::Diags) {
    to_from_here(
        design,
        diags,
        &[
            ir::ItemKind::Option,
            ir::ItemKind::Env,
            ir::ItemKind::NodeSet,
        ],
    );
}

/// Makes each walk's leading `WholeRun` saves `FromHere`.
pub fn whole_run_save_to_from_here(design: &mut ir::Design, diags: &mut ir::Diags) {
    to_from_here(design, diags, &[ir::ItemKind::Save]);
}

/// Every walk: (its top scope, its items in listed order with their listing scopes). Decks
/// first, then each file no walk reached. Explicit stack; each scope entered once per walk.
pub(super) fn walks(design: &ir::Design) -> Vec<(ir::ScopeId, Vec<(ir::ItemRef, ir::ScopeId)>)> {
    let files: Vec<ir::FileId> = design.files().collect();
    let deck = |f: &ir::FileId| design.file(*f).role == ir::Role::Deck;
    let starts: Vec<ir::FileId> = files
        .iter()
        .copied()
        .filter(deck)
        .chain(files.iter().copied().filter(|f| !deck(f)))
        .collect();
    // each item's body scopes, in the order `reachable` lists them (branch order for an `If`)
    let mut bodies: HashMap<ir::ItemRef, Vec<ir::ScopeId>> = HashMap::new();
    let mut listed: HashSet<ir::ScopeId> = HashSet::new();
    for (_, s) in design.reachable() {
        let owner = design.scope(s).owner;
        if owner != ir::ItemRef::NONE && listed.insert(s) {
            bodies.entry(owner).or_default().push(s);
        }
    }
    let mut reached = vec![false; files.len()];
    let mut out = Vec::new();
    for start in starts {
        if reached[start.index()] && !deck(&start) {
            continue;
        }
        reached[start.index()] = true;
        let top = design.file(start).top;
        let mut order = Vec::new();
        let mut entered = HashSet::from([top]);
        let mut stack = vec![(top, 0usize)];
        // bounded: each scope is entered once, each of its items read once
        while let Some((scope, k)) = stack.pop() {
            let Some(&item) = design.items(scope).get(k) else {
                continue;
            };
            stack.push((scope, k + 1));
            order.push((item, scope));
            let mut next: Vec<ir::ScopeId> = bodies.get(&item).cloned().unwrap_or_default();
            if item.kind == ir::ItemKind::Include {
                let path = design.row::<ir::item::Include>(item.index).path;
                let target = design.find_file(design.str(path));
                if !target.is_none() {
                    reached[target.index()] = true;
                    next.push(design.file(target).top);
                }
            }
            for &s in next.iter().rev() {
                if entered.insert(s) {
                    stack.push((s, 0));
                }
            }
        }
        out.push((top, order));
    }
    out
}

// ---- private ----

/// Whether the item is a run item for listed order (A14): an analysis, loop or alter.
fn is_run(kind: ir::ItemKind) -> bool {
    matches!(
        kind,
        ir::ItemKind::Analysis | ir::ItemKind::Loop | ir::ItemKind::Alter
    )
}

/// The mode of a state item or save, `None` for any other item.
fn holds(design: &ir::Design, item: ir::ItemRef) -> Option<ir::Applies> {
    match item.kind {
        ir::ItemKind::Option => Some(design.row::<ir::OptionRow>(item.index).holds),
        ir::ItemKind::Env => Some(design.row::<ir::Env>(item.index).holds),
        ir::ItemKind::NodeSet => Some(design.row::<ir::NodeSet>(item.index).holds),
        ir::ItemKind::Save => Some(design.row::<ir::Save>(item.index).holds),
        _ => None,
    }
}

/// The `WholeRun` items of `kinds` before each walk's first run item made `FromHere`, or noted.
fn to_from_here(design: &mut ir::Design, diags: &mut ir::Diags, kinds: &[ir::ItemKind]) {
    let mut change = Vec::new();
    for (_, order) in walks(design) {
        let first = order
            .iter()
            .position(|&(i, _)| is_run(i.kind))
            .unwrap_or(order.len());
        let resets: Vec<ir::ResetSet> = order
            .iter()
            .filter(|(i, _)| i.kind == ir::ItemKind::Reset)
            .map(|(i, _)| design.row::<ir::Reset>(i.index).restores)
            .collect();
        let mut verdicts: Vec<(ir::ItemRef, Option<ir::Code>)> = Vec::new();
        for (k, &(item, _)) in order.iter().enumerate() {
            if !kinds.contains(&item.kind) || holds(design, item) != Some(ir::Applies::WholeRun) {
                continue;
            }
            let restored = classes(design, item)
                .iter()
                .any(|&c| resets.iter().any(|r| r.contains(c)));
            let same = order.iter().any(|&(other, _)| {
                holds(design, other) == Some(ir::Applies::FromHere)
                    && same_thing(design, item, other)
            });
            let code = if k > first {
                Some(MID_RUN)
            } else if restored {
                Some(RESET_RESTORES)
            } else if same {
                Some(FROM_HERE_SAME)
            } else {
                None
            };
            verdicts.push((item, code));
        }
        // an item converts only with every `WholeRun` item of its thing (else the two modes
        // would set one thing); bounded: each round fixes one more item or stops
        for _ in 0..verdicts.len() {
            let kept: Vec<ir::ItemRef> = verdicts
                .iter()
                .filter(|v| v.1.is_some())
                .map(|v| v.0)
                .collect();
            let mut more = false;
            for v in verdicts.iter_mut().filter(|v| v.1.is_none()) {
                if kept.iter().any(|&o| same_thing(design, v.0, o)) {
                    v.1 = Some(FROM_HERE_SAME);
                    more = true;
                }
            }
            if !more {
                break;
            }
        }
        for (item, code) in verdicts {
            match code {
                Some(c) => diags.push_at(c, design, item, 0),
                None if !change.contains(&item) => change.push(item),
                None => {}
            }
        }
    }
    let mut editor = design.editor();
    for item in change {
        let d = editor.design();
        match item.kind {
            ir::ItemKind::Option => {
                let mut row = d.row::<ir::OptionRow>(item.index);
                row.holds = ir::Applies::FromHere;
                editor.set(item.index, row);
            }
            ir::ItemKind::Env => {
                let mut row = d.row::<ir::Env>(item.index);
                row.holds = ir::Applies::FromHere;
                editor.set(item.index, row);
            }
            ir::ItemKind::NodeSet => {
                let mut row = d.row::<ir::NodeSet>(item.index);
                row.holds = ir::Applies::FromHere;
                editor.set(item.index, row);
            }
            ir::ItemKind::Save => {
                let mut row = d.row::<ir::Save>(item.index);
                row.holds = ir::Applies::FromHere;
                editor.set(item.index, row);
            }
            _ => unreachable!("only state items and saves are changed"),
        }
    }
}

/// The `Reset` classes that would restore the item were it `FromHere` (`Restorable` docs).
fn classes(design: &ir::Design, item: ir::ItemRef) -> Vec<ir::Restorable> {
    match item.kind {
        ir::ItemKind::Option => vec![ir::Restorable::Options],
        ir::ItemKind::Env => {
            let e = design.row::<ir::Env>(item.index);
            let mut out = Vec::new();
            if !e.temp.is_none() {
                out.push(ir::Restorable::Temp);
            }
            if !e.tnom.is_none() {
                out.push(ir::Restorable::Tnom);
            }
            out
        }
        ir::ItemKind::Save => vec![ir::Restorable::Saves],
        _ => Vec::new(), // a node set is no class a reset restores
    }
}

/// Whether two state items set one thing: an option key (with its private name and dialect),
/// `temp` or `tnom`, one node condition (kind, scope, nodes). Saves add up: never.
fn same_thing(design: &ir::Design, a: ir::ItemRef, b: ir::ItemRef) -> bool {
    if a.kind != b.kind {
        return false;
    }
    match a.kind {
        ir::ItemKind::Option => {
            let (x, y) = (
                design.row::<ir::OptionRow>(a.index),
                design.row::<ir::OptionRow>(b.index),
            );
            x.key == y.key && x.private == y.private && x.dialect == y.dialect
        }
        ir::ItemKind::Env => {
            let (x, y) = (
                design.row::<ir::Env>(a.index),
                design.row::<ir::Env>(b.index),
            );
            !x.temp.is_none() && !y.temp.is_none() || !x.tnom.is_none() && !y.tnom.is_none()
        }
        ir::ItemKind::NodeSet => {
            let (x, y) = (
                design.row::<ir::NodeSet>(a.index),
                design.row::<ir::NodeSet>(b.index),
            );
            x.kind == y.kind
                && x.applies == y.applies
                && design.names(x.node) == design.names(y.node)
                && design.names(x.reference) == design.names(y.reference)
        }
        _ => false,
    }
}
