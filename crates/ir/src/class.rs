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
//! `pub(crate) fn node_class(&Design, ExprId, &[ExprClass]) -> ExprClass`

use crate::design::Design;
use crate::expr::{Expr, Func};
use crate::feature::ExprWalk;
use crate::id::{DefId, ExprId};
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
pub fn expr_class(design: &Design, e: ExprId) -> ExprClass {
    let table = def_classes(design);
    let mut walk = ExprWalk::new(design);
    walk.tree(design, e)
        .iter()
        .map(|&n| node_class(design, n, table))
        .max()
        .unwrap_or(ExprClass::Constant)
}

/// The class of every definition, by `DefId` (`Constant` for kinds an expression does not read),
/// computed once per design state.
pub(crate) fn def_classes(design: &Design) -> &[ExprClass] {
    design.classes.get_or_init(|| compute(design))
}

/// The class one expression row contributes: its own leaf or function, or the definition it
/// reads (a formal-argument read contributes nothing).
pub(crate) fn node_class(design: &Design, e: ExprId, table: &[ExprClass]) -> ExprClass {
    let read = dependency(design, e).map_or(ExprClass::Constant, |d| table[d.index()]);
    leaf_class(design, e).max(read)
}

// ---- private ----

/// The class table: each definition's direct class from its own expressions, then the
/// dependency-ordered maximum over what it reads; what the worklist cannot order is `Circuit`.
fn compute(design: &Design) -> Vec<ExprClass> {
    let n = design.defs.len();
    let mut direct = vec![ExprClass::Constant; n];
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut walk = ExprWalk::new(design);
    for k in 0..n {
        for root in roots(design, DefId(k as u32)) {
            for &e in walk.tree(design, root) {
                direct[k] = direct[k].max(leaf_class(design, e));
                if let Some(d) = dependency(design, e) {
                    deps[k].push(d.index());
                }
            }
        }
    }
    // Kahn: a definition is final once every definition it reads is
    let mut waiting: Vec<usize> = deps.iter().map(Vec::len).collect();
    let mut readers: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, ds) in deps.iter().enumerate() {
        for &d in ds {
            readers[d].push(k);
        }
    }
    let mut class = direct.clone();
    let mut done = vec![false; n];
    let mut ready: Vec<usize> = (0..n).filter(|&k| waiting[k] == 0).collect();
    while let Some(k) = ready.pop() {
        done[k] = true;
        class[k] = deps[k].iter().fold(direct[k], |c, &d| c.max(class[d]));
        for &u in &readers[k] {
            waiting[u] -= 1;
            if waiting[u] == 0 {
                ready.push(u);
            }
        }
    }
    for k in 0..n {
        if !done[k] {
            class[k] = ExprClass::Circuit; // on or behind a reference cycle
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
