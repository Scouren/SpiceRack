//! The dynamic class of an expression (R14, A11): a design and an expression in, `Constant`,
//! `Temp` or `Circuit` out. An expression is `Circuit` when it, or a parameter or user function
//! it reads (transitively), holds `V`, `I`, `Time`, `Freq`, `Ddt` or `Idt`; else `Temp` when one
//! of them holds `Temp`. The `*Dyn` features are `class >= Temp` (`features_used`); `check`'s
//! `ProbeContext` stays the direct test (making it transitive is open, IR-DECISIONS).
//!
//! One class per definition, computed once per design state and cached in the design (cleared by
//! every `Builder` and `Editor` mutation): each `Param` and `Func` definition's own expression is
//! walked once, then the classes flow along its reads in dependency order (Kahn's worklist, no
//! recursion). A definition left over by the worklist lies on or behind a reference cycle and
//! is `Circuit`. A formal-argument read is not a dependency: its value is the call's argument,
//! which the calling expression walks itself.
//!
//! Cites: `rewrite/RULINGS.md` R14; `rewrite/IR-KERNEL.md` §7.4 (A11).
//!
//! Contents: `pub enum ExprClass`, `pub fn expr_class(&Design, ExprId) -> ExprClass`; crate:
//! `pub(crate) fn def_classes(&Design) -> &[ExprClass]`,
//! `pub(crate) fn tree_class(&Design, ExprId, &[ExprClass]) -> ExprClass`,
//! `pub(crate) fn tree_class_with(&Design, ExprId, &dyn Fn(DefId) -> ExprClass) -> ExprClass`,
//! `pub(crate) fn class_nodes(&Design, ExprId, &mut Vec<ExprId>)`,
//! `pub(crate) fn tree_nodes(&Design, ExprId, &mut Vec<ExprId>)`,
//! `pub(crate) fn formals_read(&Design, DefId) -> Option<Vec<bool>>`,
//! `pub(crate) fn cycle_defs(&Design) -> Vec<usize>`,

use crate::design::Design;
use crate::expr::{Expr, Func};
use crate::id::{DefId, ExprId, NameId};
use crate::item::{DefKind, ItemKind};

/// The dynamic class of an expression (R14, A11): `Circuit` when it, or a parameter or user
/// function it reads (transitively), holds `V`, `I`, `Time`, `Freq`, `Ddt` or `Idt`; else `Temp`
/// when one of them holds `Temp`; else `Constant`. Ordered: the `*Dyn` features are
/// `class >= Temp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ExprClass {
    /// Reads nothing that varies in a run.
    Constant,
    /// Reads the temperature (directly or through a definition), nothing of the circuit.
    Temp,
    /// Reads the circuit or the time (directly or through a definition).
    Circuit,
}

/// The class of `e` (see the module header). O(its tree) once the design's definition classes
/// are cached; the first call after a mutation computes them, O(design).
///
/// An unbound parameter read counts as `Constant` by assumption, not knowledge (A33): the reader
/// that left it unbound asserts a parameter will bind it (R4), and its class is known only once
/// one does. The one signal carried today is `Extra::FormalNotConstant`, for a subcircuit
/// default holding such a read.
pub fn expr_class(design: &Design, e: ExprId) -> ExprClass {
    tree_class(design, e, def_classes(design))
}

/// The class of the tree under `root` given the definitions' classes: O(its tree), with no
/// table sized by the design (so a row may call it per expression). An out-of-range or `NONE`
/// root is `Constant`. Explicit stack (C11).
pub(crate) fn tree_class(design: &Design, root: ExprId, table: &[ExprClass]) -> ExprClass {
    tree_class_with(design, root, &|d| table[d.index()])
}

/// As [`tree_class`], with each definition's class from `def_class` (the generator's own table).
pub(crate) fn tree_class_with(
    design: &Design,
    root: ExprId,
    def_class: &dyn Fn(DefId) -> ExprClass,
) -> ExprClass {
    let mut nodes = Vec::new();
    class_nodes(design, root, &mut nodes);
    nodes
        .iter()
        .map(|&e| {
            let read = dependency(design, e).map_or(ExprClass::Constant, def_class);
            leaf_class(design, e).max(read)
        })
        .max()
        .unwrap_or(ExprClass::Constant)
}

/// The rows of the tree under `root` its class reads, each once: every row but the arguments
/// of a user call whose formal the function's body never reads (the lead's ruling on A28's
/// finding a: a call is its body with each formal read taking its argument's class, so an
/// unread argument adds nothing). Explicit stack (C11); descends only to lower ids.
pub(crate) fn class_nodes(design: &Design, root: ExprId, out: &mut Vec<ExprId>) {
    walk_small(design, root, out, true);
}

/// The rows of the tree under `root`, each once, with no table sized by the design (so a row
/// may call it per expression: O(its tree)). Explicit stack (C11); descends only to lower ids.
pub(crate) fn tree_nodes(design: &Design, root: ExprId, out: &mut Vec<ExprId>) {
    walk_small(design, root, out, false);
}

/// The class of every definition, by `DefId` (`Constant` for kinds an expression does not read),
/// computed once per design state.
pub(crate) fn def_classes(design: &Design) -> &[ExprClass] {
    design.classes.get_or_init(|| compute(design))
}

/// The definitions on a reference cycle (A28): those neither a deps-first nor a readers-first
/// worklist can settle (a definition between two cycles counts too). O(definitions + reads).
pub(crate) fn cycle_defs(design: &Design) -> Vec<usize> {
    let (_, deps, readers) = graph(design);
    let forward = kahn(&deps, &readers);
    let backward = kahn(&readers, &deps);
    (0..deps.len())
        .filter(|&k| !forward[k] && !backward[k])
        .collect()
}

/// Whether the body of user function `def` reads each of its formals, by position; `None` when
/// `def` is no function this design defines (then every argument counts).
pub(crate) fn formals_read(design: &Design, def: DefId) -> Option<Vec<bool>> {
    let d = design.defs[def.index_checked(design.defs.len())?];
    if d.kind != DefKind::Func
        || d.item.kind != ItemKind::Func
        || d.item.index as usize >= design.funcs.len()
    {
        return None;
    }
    let f: crate::item::Func = design.row(d.item.index);
    let mut nodes = Vec::new();
    tree_nodes(design, f.body, &mut nodes);
    let reads: Vec<NameId> = nodes
        .iter()
        .filter_map(|&e| match design.expr(e) {
            Expr::Param(r) if r.def == def => Some(r.name),
            _ => None,
        })
        .collect();
    Some(
        design
            .names(f.args)
            .iter()
            .map(|n| reads.contains(n))
            .collect(),
    )
}

// ---- private ----

/// [`class_nodes`] (`read_only`: a user call's unread arguments skipped) or [`tree_nodes`]. Its
/// seen-set grows with the tree, never with the design: a design-sized table here made every
/// call's `formals_read` O(expressions), and `check` quadratic.
fn walk_small(design: &Design, root: ExprId, out: &mut Vec<ExprId>, read_only: bool) {
    let mut seen: std::collections::HashSet<u32, std::hash::BuildHasherDefault<crate::names::Fx>> =
        Default::default();
    let mut stack = vec![root];
    let mut kids = Vec::new();
    while let Some(e) = stack.pop() {
        if e.is_none() || e.index() >= design.expr_count() || !seen.insert(e.0) {
            continue;
        }
        out.push(e);
        kids.clear();
        design.children(e, &mut kids);
        if read_only
            && let Expr::UserCall(r, _) = design.expr(e)
            && let Some(read) = formals_read(design, r.def)
        {
            // only the arguments whose formal the body reads (an argument past the formals is
            // `check`'s arity violation, and reads as unread)
            let mut k = 0;
            kids.retain(|_| {
                k += 1;
                read.get(k - 1).copied().unwrap_or(false)
            });
        }
        // children have lower ids, so the walk ends on any table contents
        stack.extend(kids.iter().copied().filter(|k| *k < e));
    }
}

/// Each definition's direct class, the definitions it reads (one entry per read), and its
/// readers.
fn graph(design: &Design) -> (Vec<ExprClass>, Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let n = design.defs.len();
    let mut direct = vec![ExprClass::Constant; n];
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut nodes = Vec::new();
    for k in 0..n {
        for root in roots(design, DefId(k as u32)) {
            nodes.clear();
            class_nodes(design, root, &mut nodes);
            for &e in &nodes {
                direct[k] = direct[k].max(leaf_class(design, e));
                if let Some(d) = dependency(design, e) {
                    deps[k].push(d.index());
                }
            }
        }
    }
    let mut readers: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, ds) in deps.iter().enumerate() {
        for &d in ds {
            readers[d].push(k);
        }
    }
    (direct, deps, readers)
}

/// Which nodes a worklist settles when a node is ready once every node it waits on (`on`) is
/// settled (`waiters` the reverse edges): Kahn's algorithm, bounded by the edges.
fn kahn(on: &[Vec<usize>], waiters: &[Vec<usize>]) -> Vec<bool> {
    let n = on.len();
    let mut waiting: Vec<usize> = on.iter().map(Vec::len).collect();
    let mut done = vec![false; n];
    let mut ready: Vec<usize> = (0..n).filter(|&k| waiting[k] == 0).collect();
    while let Some(k) = ready.pop() {
        done[k] = true;
        for &u in &waiters[k] {
            waiting[u] -= 1;
            if waiting[u] == 0 {
                ready.push(u);
            }
        }
    }
    done
}

/// The settled definitions in an order where each follows what it reads.
fn order(deps: &[Vec<usize>], readers: &[Vec<usize>]) -> Vec<usize> {
    let n = deps.len();
    let mut waiting: Vec<usize> = deps.iter().map(Vec::len).collect();
    let mut out = Vec::new();
    let mut ready: Vec<usize> = (0..n).filter(|&k| waiting[k] == 0).collect();
    while let Some(k) = ready.pop() {
        out.push(k);
        for &u in &readers[k] {
            waiting[u] -= 1;
            if waiting[u] == 0 {
                ready.push(u);
            }
        }
    }
    out
}

/// The class table: each definition's direct class from its own expressions, then the
/// dependency-ordered maximum over what it reads. A definition on or behind a reference cycle
/// (which `check` reports as `DefinitionCycle`, A28) takes the most its direct class and the
/// classes it reads give, propagated by a worklist (each class rises at most twice), so a cycle
/// does not make every reader `Circuit`.
fn compute(design: &Design) -> Vec<ExprClass> {
    let (direct, deps, readers) = graph(design);
    let n = direct.len();
    let done = kahn(&deps, &readers);
    let mut class = direct.clone();
    // the settled ones in dependency order
    for k in order(&deps, &readers) {
        class[k] = deps[k].iter().fold(direct[k], |c, &d| c.max(class[d]));
    }
    // the rest: rise to the fixpoint
    let mut work: Vec<usize> = (0..n).filter(|&k| !done[k]).collect();
    while let Some(k) = work.pop() {
        let next = deps[k].iter().fold(direct[k], |c, &d| c.max(class[d]));
        if next > class[k] {
            class[k] = next;
            work.extend(readers[k].iter().copied().filter(|&u| !done[u]));
        }
    }
    class
}

/// The expressions a definition's value or body is: a `Param`'s value (or a subcircuit head
/// default's), a `Func`'s body; nothing for other kinds.
fn roots(design: &Design, def: DefId) -> Vec<ExprId> {
    let d = design.def(def);
    let in_range = (d.item.index as usize)
        < for_kind!(d.item.kind, R => <R as crate::design::ItemRow>::table(design).len());
    if !in_range {
        return Vec::new();
    }
    match (d.kind, d.item.kind) {
        (DefKind::Param, ItemKind::Param) => {
            vec![design.row::<crate::item::Param>(d.item.index).value]
        }
        (DefKind::Param, ItemKind::Subckt) => {
            let row: crate::item::Subckt = design.row(d.item.index);
            design
                .params(row.params)
                .iter()
                .filter(|p| p.0 == d.name && !p.1.is_none())
                .map(|p| p.1)
                .collect()
        }
        (DefKind::Func, ItemKind::Func) => vec![design.row::<crate::item::Func>(d.item.index).body],
        _ => Vec::new(),
    }
}

/// The class a row is by itself (a probe, the time, a time-history function, the temperature).
fn leaf_class(design: &Design, e: ExprId) -> ExprClass {
    match design.expr(e) {
        Expr::V(..) | Expr::I(_) | Expr::Time | Expr::Freq => ExprClass::Circuit,
        Expr::Call(Func::Ddt | Func::Idt, _) => ExprClass::Circuit,
        Expr::Temp => ExprClass::Temp,
        Expr::Num(_)
        | Expr::Str(_)
        | Expr::Param(_)
        | Expr::Call(..)
        | Expr::UserCall(..)
        | Expr::Unary(..)
        | Expr::Binary(..)
        | Expr::Ternary(..)
        | Expr::TStep
        | Expr::TStop
        | Expr::Tnom
        | Expr::Draw(..)
        | Expr::List(_) => ExprClass::Constant,
    }
}

/// The definition a read depends on: a parameter read's `Param` or `Measure`, a user call's
/// `Func`. A formal-argument read (a `Param` expression bound to a `Func`) is none: its value is
/// the call's argument, which the caller walks.
fn dependency(design: &Design, e: ExprId) -> Option<DefId> {
    let (r, call) = match design.expr(e) {
        Expr::Param(r) => (r, false),
        Expr::UserCall(r, _) => (r, true),
        Expr::Num(_)
        | Expr::Str(_)
        | Expr::Call(..)
        | Expr::Unary(..)
        | Expr::Binary(..)
        | Expr::Ternary(..)
        | Expr::V(..)
        | Expr::I(_)
        | Expr::Time
        | Expr::Temp
        | Expr::Freq
        | Expr::TStep
        | Expr::TStop
        | Expr::Tnom
        | Expr::Draw(..)
        | Expr::List(_) => return None,
    };
    let k = r.def.index_checked(design.defs.len())?;
    let kind = design.defs[k].kind;
    let reads = if call {
        kind == DefKind::Func
    } else {
        matches!(kind, DefKind::Param | DefKind::Measure)
    };
    reads.then_some(r.def)
}
