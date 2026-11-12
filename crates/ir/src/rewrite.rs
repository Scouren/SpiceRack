//! A design and a pass's rule in, the same design with every changed expression root pointing
//! at its rewritten tree out (A23: the passes' door for expressions). Bottom-up: the rule sees a
//! node whose children are already rewritten; a node whose child changed is appended again
//! with its new children (rows are appended, never replaced, so children stay below parents);
//! an unchanged tree keeps its ids; a subtree two roots share is rewritten once.
//!
//! Cites: `rewrite/AMEND-A12.md` A23; `rewrite/IR-KERNEL.md` C11 (explicit stacks).
//!
//! Contents: `pub fn rewrite_exprs(&mut Design, &mut dyn FnMut(&mut Builder<'_>, ExprId) ->
//! Option<ExprId>) -> u32`, `pub fn rewrite_exprs_in(&mut Design, Form, …) -> u32`,
//! `pub fn rewrite_tree(&mut Builder<'_>, ExprId, …) -> ExprId`

use crate::build::Builder;
use crate::design::Design;
use crate::facts::RowFacts;
use crate::feature::{Form, root_form, roots_with_use};
use crate::id::{ExprId, ItemRef};

/// Rewrites every expression root of every listed item by `rule` and returns how many roots
/// changed (a root two fields hold counts in each). `rule(b, node)` returns `Some(new)` to
/// replace `node`, a tree it builds through `b`; nodes `rule` appends are not visited again.
/// Roots are reached through `RowFacts::map_exprs`, so a row's part of the walk is its own.
/// O(expressions + items); the walk's depth is an explicit stack.
///
/// ```
/// use spicerack_ir::{Design, Diags, Expr, Func, dump, parse, rewrite_exprs};
///
/// let mut design = Design::new();
/// let text = "file \"p.sp\" deck\n  param p = pow_abs(-2, 3)\n";
/// parse(text, &mut design.builder(), &mut Diags::new());
/// // the whole of a pass: pow_abs(x, y) is pow(abs(x), y)
/// let changed = rewrite_exprs(&mut design, &mut |b, e| {
///     let Expr::Call(Func::PowAbs, args) = b.design().expr(e) else { return None };
///     let [x, y] = <[_; 2]>::try_from(b.design().exprs(args)).unwrap();
///     let abs = b.call(Func::Abs, &[x]);
///     Some(b.call(Func::Pow, &[abs, y]))
/// });
/// assert_eq!(changed, 1);
/// let mut out = String::new();
/// dump(&design, design.entry(), &mut out);
/// assert_eq!(out, "file \"p.sp\" deck\n  param p = pow(abs(-2), 3)\n");
/// ```
pub fn rewrite_exprs(
    design: &mut Design,
    rule: &mut dyn FnMut(&mut Builder<'_>, ExprId) -> Option<ExprId>,
) -> u32 {
    rewrite_where(design, None, rule)
}

/// As [`rewrite_exprs`], restricted to the roots of one `form` (A26: `root_form` of each root as
/// it stands before the pass): a root of the other form keeps its id, and a subtree a root of
/// each form shares is rewritten for the one and kept for the other (rows are appended, so both
/// trees exist). A function body counts in its callers' forms (A28): one of both forms is left
/// to neither call. Returns how many roots changed.
///
/// # Panics
/// On `Form::Both`: a pass rewrites one form.
pub fn rewrite_exprs_in(
    design: &mut Design,
    form: Form,
    rule: &mut dyn FnMut(&mut Builder<'_>, ExprId) -> Option<ExprId>,
) -> u32 {
    assert!(form != Form::Both, "a pass rewrites the roots of one form");
    rewrite_where(design, Some(form), rule)
}

/// Rewrites the one tree under `root` bottom-up by `rule` and returns its new root (`root`
/// itself when nothing changed): a per-root twin of `rewrite_exprs` for a pass whose rule
/// depends on where the copy goes (R53: a default copied to each instance). Its own memo; rows
/// are appended, so the old tree and every root pointing at it are untouched; nothing is
/// stored back. Explicit stack.
pub fn rewrite_tree(
    b: &mut Builder<'_>,
    root: ExprId,
    rule: &mut dyn FnMut(&mut Builder<'_>, ExprId) -> Option<ExprId>,
) -> ExprId {
    let n = b.design().expr_count();
    if root.is_none() || root.index() >= n {
        return root;
    }
    let mut memo = vec![ExprId::NONE; n];
    rewrite_into(b, root, &mut memo, rule);
    memo[root.index()]
}

// ---- private ----

/// The walk of `rewrite_exprs` (`only` `None`) and `rewrite_exprs_in`: roots read through
/// `handles` with their use, each root's form decided before anything is appended, the chosen
/// roots rewritten into a memo, and stored back through each row's `map_exprs` in the same order.
fn rewrite_where(
    design: &mut Design,
    only: Option<Form>,
    rule: &mut dyn FnMut(&mut Builder<'_>, ExprId) -> Option<ExprId>,
) -> u32 {
    let items: Vec<ItemRef> = design.reachable().into_iter().map(|(i, _)| i).collect();
    let n = design.expr_count();
    // old id -> its rewrite (itself when unchanged); `NONE` until visited
    let mut memo = vec![ExprId::NONE; n];
    // per item, whether each root (in `map_exprs` order) is of the chosen form
    let mut chosen: Vec<Vec<bool>> = Vec::with_capacity(items.len());
    let mut roots = Vec::new();
    let mut handles = Vec::new();
    for &item in &items {
        handles.clear();
        for_kind!(item.kind, R => design.row::<R>(item.index).handles(design, &mut handles));
        let mut flags = Vec::new();
        for (root, use_) in roots_with_use(design, &handles) {
            let pick = only.is_none_or(|f| root.is_none() || root_form(design, root, use_) == f);
            flags.push(pick);
            if pick {
                roots.push(root);
            }
        }
        chosen.push(flags);
    }
    {
        let mut b = design.builder();
        for root in roots {
            rewrite_into(&mut b, root, &mut memo, rule);
        }
    }
    // store the chosen roots back through each row's own walk
    let mut changed = 0;
    let mut editor = design.editor();
    for (item, flags) in items.into_iter().zip(&chosen) {
        let mut k = 0;
        editor.map_exprs(item, &mut |root, _| {
            let pick = flags.get(k).copied().unwrap_or(false);
            k += 1;
            if !pick || root.is_none() || root.index() >= n {
                return;
            }
            let new = memo[root.index()];
            if !new.is_none() && new != *root {
                *root = new;
                changed += 1;
            }
        });
    }
    changed
}

/// Rewrites the tree under `root` into `memo`, post-order with an explicit stack. Only rows
/// that existed before the pass (`memo`'s length) are visited; a child not below its parent (a
/// broken design) is kept as it is.
fn rewrite_into(
    b: &mut Builder<'_>,
    root: ExprId,
    memo: &mut [ExprId],
    rule: &mut dyn FnMut(&mut Builder<'_>, ExprId) -> Option<ExprId>,
) {
    let n = memo.len();
    if root.is_none() || root.index() >= n || !memo[root.index()].is_none() {
        return;
    }
    let mut stack = vec![(root, false)];
    let mut kids = Vec::new();
    // each row is pushed at most twice (before and after its children)
    for _ in 0..2 * n + 2 {
        let Some((e, ready)) = stack.pop() else { break };
        if !memo[e.index()].is_none() {
            continue;
        }
        kids.clear();
        b.design().children(e, &mut kids);
        if !ready {
            stack.push((e, true));
            for &k in kids.iter().rev() {
                if k < e && memo[k.index()].is_none() {
                    stack.push((k, false));
                }
            }
            continue;
        }
        let new: Vec<ExprId> = kids
            .iter()
            .map(|&k| if k < e { memo[k.index()] } else { k })
            .collect();
        let base = if new == kids { e } else { b.rebuild(e, &new) };
        memo[e.index()] = rule(b, base).unwrap_or(base);
    }
    assert!(stack.is_empty(), "the walk visits each row at most twice");
}
