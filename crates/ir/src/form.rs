//! The form a root counts its functions, operators and draws in (A26, A28; R14, R51): a design
//! and a root in, `Static`, `Dynamic` or (for a function body) `Both` out. A root counts dynamic
//! when its tree is of class `Temp` or above or its field is a behavioural source's own
//! expression (R66, A40), static otherwise (a passive's constant value too); a function body
//! counts in the forms of the roots that call it, transitively (it has no evaluator of its own).
//! `features_used` and the passes' `rewrite_exprs_in` read the same rule. Split from `feature.rs`.
//!
//! Cites: `rewrite/AMEND-A12.md` A19, A26, A28, A40; `rewrite/RULINGS.md` R14, R38, R51, R66.
//!
//! Contents: `pub enum Form`, `pub fn root_form(&Design, ExprId, ExprUse) -> Form`; crate:
//! `pub(crate) fn body_forms(&Design) -> &FxMap<ExprId, u8>`,
//! `pub(crate) fn form_of(ExprUse, ExprClass) -> Form`,
//! `pub(crate) fn form_feature(Expr, bool) -> Option<Feature>`, `STATIC`, `DYNAMIC`

use crate::class::{ExprClass, def_classes};
use crate::design::Design;
use crate::expr::Expr;
use crate::facts::ExprUse;
use crate::feature::{ExprWalk, Feature, roots_with_use};
use crate::id::ExprId;
use crate::item::ItemKind;

/// Which of a function's or operator's two features a root counts (`Func` or `FuncDyn`, and so
/// on: R14, A19, A26): a pass that removes one feature rewrites only the roots of its form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Form {
    /// The root reads nothing of class `Temp` or above and is no behavioural source's own
    /// expression.
    Static,
    /// The root, or what it reads, is of class `Temp` or above, or it is a behavioural source's
    /// own expression (R66).
    Dynamic,
    /// A function body called from roots of both forms (A28, R51): it counts in both, and no
    /// one-form pass rewrites it.
    Both,
}

/// Bit of a static form in `body_forms`.
pub(crate) const STATIC: u8 = 1;
/// Bit of a dynamic form in `body_forms`.
pub(crate) const DYNAMIC: u8 = 2;

/// The form a root counts its functions and operators in, given how its field reads (`use`, as
/// `RowFacts::map_exprs` reports it): `Dynamic` for a tree of class `Temp` or above (R14) or a
/// behavioural source's own expression (`ExprUse::Behavioural`, R66), else `Static`, a constant
/// root in any other field that may read the circuit included. The one decision: `features_used` makes it
/// through the same rule. O(the tree) once the definitions' classes are cached.
pub fn root_form(design: &Design, root: ExprId, use_: ExprUse) -> Form {
    // A28 (R51): a function body has no evaluator of its own; it is its callers' forms
    if let Some(&bits) = body_forms(design).get(&root) {
        return match (bits & STATIC != 0 || bits == 0, bits & DYNAMIC != 0) {
            (true, true) => Form::Both,
            (false, true) => Form::Dynamic,
            (_, false) => Form::Static,
        };
    }
    form_of(
        use_,
        crate::class::tree_class(design, root, def_classes(design)),
    )
}

/// The forms each function body counts in (A28, R51), by body expression: bit 1 static, bit 2
/// dynamic; a body nothing calls has none and counts static. Cached on the design with the
/// class table.
pub(crate) fn body_forms(design: &Design) -> &crate::names::FxMap<ExprId, u8> {
    design.forms.get_or_init(|| compute_forms(design))
}

/// The form decision itself, shared by `root_form` and `features_used` (A26, R66).
pub(crate) fn form_of(use_: ExprUse, class: ExprClass) -> Form {
    if use_ == ExprUse::Behavioural || class >= ExprClass::Temp {
        Form::Dynamic
    } else {
        Form::Static
    }
}

/// The feature a node counts in one form: its function, operator or draw's distribution, in
/// the static or the dynamic member; `None` for a node with no form (A26, A28).
pub(crate) fn form_feature(e: Expr, dynamic: bool) -> Option<Feature> {
    Some(match e {
        Expr::Call(func, _) if dynamic => Feature::FuncDyn(func),
        Expr::Call(func, _) => Feature::Func(func),
        Expr::Binary(op, ..) if dynamic => Feature::BinaryDyn(op),
        Expr::Binary(op, ..) => Feature::Binary(op),
        Expr::Unary(op, _) if dynamic => Feature::UnaryDyn(op),
        Expr::Unary(op, _) => Feature::Unary(op),
        Expr::Draw(dist, ..) if dynamic => Feature::DistDyn(dist),
        Expr::Draw(dist, ..) => Feature::Dist(dist),
        Expr::Num(_)
        | Expr::Str(_)
        | Expr::Param(_)
        | Expr::UserCall(..)
        | Expr::Ternary(..)
        | Expr::V(..)
        | Expr::I(_)
        | Expr::Time
        | Expr::Temp
        | Expr::Freq
        | Expr::TStep
        | Expr::TStop
        | Expr::Tnom
        | Expr::List(_) => return None,
    })
}

// ---- private ----

/// The forms of every function body: seeded by each listed root that is no body, in its own
/// form, at every call its tree makes; carried from a body to the bodies it calls, by an explicit
/// worklist (each (definition, form) settles once). O(expressions + calls).
fn compute_forms(design: &Design) -> crate::names::FxMap<ExprId, u8> {
    let table = def_classes(design);
    let mut walk = ExprWalk::new(design);
    let mut bits = vec![0u8; design.defs.len()];
    let mut work: Vec<(usize, u8)> = Vec::new();
    let calls = |nodes: &[ExprId], form: u8, work: &mut Vec<(usize, u8)>| {
        for &e in nodes {
            if let Expr::UserCall(r, _) = design.expr(e)
                && let Some(k) = r.def.index_checked(design.defs.len())
                && design.defs[k].kind == crate::item::DefKind::Func
            {
                work.push((k, form));
            }
        }
    };
    let mut handles = Vec::new();
    for (item, _) in design.reachable() {
        if item.kind == ItemKind::Func {
            continue;
        }
        handles.clear();
        design.item_handles(item, &mut handles);
        for (root, use_) in roots_with_use(design, item, &handles) {
            let nodes = walk.tree(design, root).to_vec();
            let class = crate::class::tree_class(design, root, table);
            let form = match form_of(use_, class) {
                Form::Dynamic => DYNAMIC,
                Form::Static | Form::Both => STATIC,
            };
            calls(&nodes, form, &mut work);
        }
    }
    // bounded: each (definition, form) is expanded once; pushes are bounded by the calls
    while let Some((k, form)) = work.pop() {
        if bits[k] & form != 0 {
            continue;
        }
        bits[k] |= form;
        let item = design.defs[k].item;
        if item.kind != ItemKind::Func || item.index as usize >= design.funcs.len() {
            continue;
        }
        let body = design.row::<crate::item::Func>(item.index).body;
        let nodes = walk.tree(design, body).to_vec();
        calls(&nodes, form, &mut work);
    }
    let mut forms = crate::names::FxMap::default();
    for (k, def) in design.defs.iter().enumerate() {
        if def.kind == crate::item::DefKind::Func
            && def.item.kind == ItemKind::Func
            && (def.item.index as usize) < design.funcs.len()
        {
            let body = design.row::<crate::item::Func>(def.item.index).body;
            *forms.entry(body).or_insert(0) |= bits[k];
        }
    }
    forms
}
