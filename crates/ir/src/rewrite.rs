//! A design and a pass's rule in, the same design with every changed expression root pointing
//! at its rewritten tree out (A23: the passes' door for expressions). Bottom-up: the rule sees a
//! node whose children are already rewritten; a node whose child changed is appended again
//! with its new children (rows are appended, never replaced, so children stay below parents);
//! an unchanged tree keeps its ids; a subtree two roots share is rewritten once.
//!
//! Cites: `rewrite/AMEND-A12.md` A23; `rewrite/IR-KERNEL.md` C11 (explicit stacks).
//!
//! Contents: `pub fn rewrite_exprs(&mut Design, &mut dyn FnMut(&mut Builder<'_>, ExprId) ->
//! Option<ExprId>) -> u32`

use crate::build::Builder;
use crate::design::Design;
use crate::facts::RowFacts;
use crate::feature::{Form, expr_roots};
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
    let items: Vec<ItemRef> = design.reachable().into_iter().map(|(i, _)| i).collect();
    let n = design.expr_count();
    // old id -> its rewrite (itself when unchanged); `NONE` until visited
    let mut memo = vec![ExprId::NONE; n];
    // the roots, as `handles` lists them (`map_exprs` visits the same: the drift property)
    let mut roots = Vec::new();
    let mut handles = Vec::new();
    for &item in &items {
        handles.clear();
        for_kind!(item.kind, R => design.row::<R>(item.index).handles(design, &mut handles));
        roots.extend(expr_roots(design, &handles));
    }
    {
        let mut b = design.builder();
        for root in roots {
            rewrite_tree(&mut b, root, &mut memo, rule);
        }
    }
    // store the changed roots back through each row's own walk
    let mut changed = 0;
    let mut editor = design.editor();
    for item in items {
        editor.map_exprs(item, &mut |root, _| {
            if root.is_none() || root.index() >= n {
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

/// As [`rewrite_exprs`], restricted to the roots of one `form` (A26: `root_form` of each root as
/// it stands before the pass): a root of the other form keeps its id, and a subtree a root of
/// each form shares is rewritten for the one and kept for the other (rows are appended, so both
/// trees exist). Returns how many roots changed.
pub fn rewrite_exprs_in(
    design: &mut Design,
    form: Form,
    rule: &mut dyn FnMut(&mut Builder<'_>, ExprId) -> Option<ExprId>,
) -> u32 {
    todo!()
}

// ---- private ----

/// Rewrites the tree under `root` into `memo`, post-order with an explicit stack. Only rows
/// that existed before the pass (`memo`'s length) are visited; a child not below its parent (a
/// broken design) is kept as it is.
fn rewrite_tree(
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
