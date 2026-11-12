//! Rule `ic_next_only`: initial conditions and node sets that hold beyond the next analysis
//! against ones that hold for the next analysis only (`NodeSet::next_only`; A22). Two rows:
//! - `ic_beyond_next_to_next_only` removes `Run(IcBeyondNext)`: each analysis gets, just before
//!   it, a next-only `FromHere` copy of every beyond-next condition in force there (a
//!   `WholeRun` one: the last of its node; a `FromHere` one: the last before the analysis),
//!   unless a next-only condition of that node already stands for it; the beyond-next items go.
//!   Requires `Run(IcNextOnly)`, `Run(NodeSetFromHere)`, `Run(NodeSetMidRun)` (the copies).
//! - `ic_next_only_to_beyond` removes `Run(IcNextOnly)`: a `FromHere` next-only condition whose
//!   next analysis is the walk's last becomes beyond-next (it then holds for that analysis
//!   alone, as before). Requires `Run(IcBeyondNext)`.
//!
//! Listed order and the walks are the IR's (`ir::run_walks`). Left with a note
//! (`IC_KEPT`), all of a walk's conditions together so no node mixes the two forms: a
//! condition or an analysis of the walk not listed in its top scope (a body or loop: a copy
//! there would be a placement the design may lack), a value holding a draw (a copy per
//! analysis would draw again), a node set both `WholeRun` and `FromHere` (which holds is not
//! stated), and for the second row one whose analysis is not the last.
//!
//! Cites: `rewrite/AMEND-A12.md` A14, A22; R24; `crates/ir/src/option.rs` (`NodeSet`).
//!
//! Contents: `pub fn ic_beyond_next_to_next_only(&mut ir::Design, &mut ir::Diags)`,
//! `pub fn ic_next_only_to_beyond(&mut ir::Design, &mut ir::Diags)`, `pub const IC_KEPT: ir::Code`

use crate::ir;

/// The note at a condition the rule leaves in its form.
pub const IC_KEPT: ir::Code = ir::Code::new(b'U', 406);

/// Replaces each walk's beyond-next conditions by next-only copies before each analysis.
pub fn ic_beyond_next_to_next_only(design: &mut ir::Design, diags: &mut ir::Diags) {
    for (top, order) in ir::run_walks(design) {
        let sets: Vec<(ir::ItemRef, ir::ScopeId)> = order
            .iter()
            .copied()
            .filter(|(i, _)| i.kind == ir::ItemKind::NodeSet)
            .collect();
        let beyond: Vec<ir::ItemRef> = sets
            .iter()
            .map(|s| s.0)
            .filter(|&i| !row(design, i).next_only)
            .collect();
        if beyond.is_empty() {
            continue;
        }
        if !convertible(design, top, &order) {
            for &i in &beyond {
                diags.push_at(IC_KEPT, design, i, 0);
            }
            continue;
        }
        // per analysis, the copies to stand before it
        let mut copies: Vec<(ir::ItemRef, Vec<ir::NodeSet>)> = Vec::new();
        let mut pending: Vec<ir::NodeSet> = Vec::new();
        let mut here: Vec<ir::NodeSet> = Vec::new();
        let whole: Vec<ir::NodeSet> = last_per_node(
            design,
            beyond
                .iter()
                .map(|&i| row(design, i))
                .filter(|n| n.holds == ir::Applies::WholeRun),
        );
        for &(item, _) in &order {
            match item.kind {
                ir::ItemKind::NodeSet => {
                    let n = row(design, item);
                    match n.next_only {
                        true => pending.push(n),
                        false if n.holds == ir::Applies::FromHere => {
                            here.retain(|h| !same_node(design, h, &n));
                            here.push(n);
                        }
                        false => {}
                    }
                }
                ir::ItemKind::Analysis => {
                    let mut add: Vec<ir::NodeSet> = Vec::new();
                    for n in whole.iter().chain(&here) {
                        let taken = pending.iter().chain(&add).any(|p| same_node(design, p, n));
                        if !taken {
                            add.push(ir::NodeSet {
                                next_only: true,
                                holds: ir::Applies::FromHere,
                                ..*n
                            });
                        }
                    }
                    copies.push((item, add));
                    pending.clear();
                }
                _ => {}
            }
        }
        let mut editor = design.editor();
        for (analysis, add) in copies {
            let built: Vec<ir::ItemRef> = add
                .into_iter()
                .map(|n| editor.builder().push(top, n, ir::Span::NONE))
                .collect();
            let items = editor.items_mut(top);
            items.retain(|i| !built.contains(i) && !beyond.contains(i));
            let at = items
                .iter()
                .position(|&i| i == analysis)
                .expect("the analysis is listed");
            items.splice(at..at, built);
        }
        let items = editor.items_mut(top);
        items.retain(|i| !beyond.contains(i));
    }
}

/// Makes a next-only condition before a walk's last analysis beyond-next.
pub fn ic_next_only_to_beyond(design: &mut ir::Design, diags: &mut ir::Diags) {
    let mut change = Vec::new();
    for (top, order) in ir::run_walks(design) {
        let next: Vec<ir::ItemRef> = order
            .iter()
            .map(|p| p.0)
            .filter(|&i| i.kind == ir::ItemKind::NodeSet && row(design, i).next_only)
            .collect();
        if next.is_empty() {
            continue;
        }
        let last = order
            .iter()
            .rposition(|(i, _)| i.kind == ir::ItemKind::Analysis);
        let ok = convertible(design, top, &order)
            && next.iter().all(|&i| {
                let k = order.iter().position(|p| p.0 == i).expect("listed");
                // its next analysis is the last one: no analysis between it and the last
                row(design, i).holds == ir::Applies::FromHere
                    && last.is_some_and(|l| {
                        l > k
                            && !order[k..l]
                                .iter()
                                .any(|p| p.0.kind == ir::ItemKind::Analysis)
                    })
            });
        for i in next {
            match ok {
                true => change.push(i),
                false => diags.push_at(IC_KEPT, design, i, 0),
            }
        }
    }
    let mut editor = design.editor();
    for item in change {
        let mut n = row(editor.design(), item);
        n.next_only = false;
        editor.set(item.index, n);
    }
}

// ---- private ----

fn row(design: &ir::Design, item: ir::ItemRef) -> ir::NodeSet {
    design.row::<ir::NodeSet>(item.index)
}

/// Whether a walk's conditions may change form: every condition and analysis listed in its
/// top, no value holding a draw, no node set in both modes.
fn convertible(
    design: &ir::Design,
    top: ir::ScopeId,
    order: &[(ir::ItemRef, ir::ScopeId)],
) -> bool {
    let kinds = [ir::ItemKind::NodeSet, ir::ItemKind::Analysis];
    let placed = order
        .iter()
        .all(|&(i, s)| !kinds.contains(&i.kind) || s == top);
    let sets: Vec<ir::NodeSet> = order
        .iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::NodeSet)
        .map(|&(i, _)| row(design, i))
        .collect();
    let draws = sets.iter().any(|n| holds_draw(design, n.value));
    // two modes on one node, or a next-only condition beside a whole-run one: which holds
    // there is not stated
    let differ = |a: &ir::NodeSet, b: &ir::NodeSet| {
        a.holds != b.holds && (!a.next_only && !b.next_only || a.holds == ir::Applies::WholeRun)
    };
    let mixed = sets
        .iter()
        .any(|a| sets.iter().any(|b| differ(a, b) && same_node(design, a, b)));
    placed && !draws && !mixed
}

/// Whether two conditions set one node's: kind, scope, nodes and reference.
fn same_node(design: &ir::Design, a: &ir::NodeSet, b: &ir::NodeSet) -> bool {
    a.kind == b.kind
        && a.applies == b.applies
        && design.names(a.node) == design.names(b.node)
        && design.names(a.reference) == design.names(b.reference)
}

/// The last condition per node, in the order of their last appearance.
fn last_per_node(design: &ir::Design, sets: impl Iterator<Item = ir::NodeSet>) -> Vec<ir::NodeSet> {
    let mut out: Vec<ir::NodeSet> = Vec::new();
    for n in sets {
        // ponytail: O(conditions²) per walk; a map by node when decks hold thousands
        out.retain(|o| !same_node(design, o, &n));
        out.push(n);
    }
    out
}

/// Whether the tree under `root` holds a draw. Explicit stack; a tree's rows lie below it.
// ponytail: a draw inside a called user function's body is not seen; walk the body when a
// deck puts one in a condition's value.
fn holds_draw(design: &ir::Design, root: ir::ExprId) -> bool {
    let mut stack = vec![root];
    // bounded: each pop visits one row of the tree (a shared row once per path)
    while let Some(e) = stack.pop() {
        if e.is_none() {
            continue;
        }
        match design.expr(e) {
            ir::Expr::Draw(..) => return true,
            ir::Expr::Call(_, l) | ir::Expr::UserCall(_, l) | ir::Expr::List(l) => {
                stack.extend_from_slice(design.exprs(l))
            }
            ir::Expr::Unary(_, a) => stack.push(a),
            ir::Expr::Binary(_, a, b) => stack.extend([a, b]),
            ir::Expr::Ternary(c, a, b) => stack.extend([c, a, b]),
            _ => {}
        }
    }
    false
}
