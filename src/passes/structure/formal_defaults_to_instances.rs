//! Pass `formal_defaults_to_instances`: a design in, the same design out with every instance
//! that omits the actual of a formal whose default is not a constant passing that default as
//! its actual. It removes `Extra::FormalNotConstant` (VACASK lets an instance override only a
//! formal whose default folds to a constant; A12 K2, A25).
//!
//! The formal's own default then (R13: no invented value): the row
//! `formal_defaults_to_instances` drops it and requires `Extra(FormalWithoutDefault)`, the exact
//! statement that every instance passes it; its twin `formal_defaults_to_instances_default`,
//! for a target without that feature, makes it the literal `0` and puts a `PASS_DEFAULT` note
//! at the subcircuit saying the default is the pass's and no instance reads it.
//!
//! ngspice's rule exactly (R53, A29 and its addendum): in the copy each instance gets, an
//! unbound parameter read is looked up from the instance's scope, a read of a sibling formal is
//! that instance's actual for it (or the sibling's default, expanded the same way), and a
//! function call stays bound to the definition's function.
//!
//! Left with a note: a default holding a draw (`DRAW_IN_DEFAULT`: one draw per copy would not
//! be the draw the default made), and a subcircuit an element names that no definition binds
//! (`UNKNOWN_INSTANCES`: an instance the pass cannot see would read the new default).
//!
//! Cites: R53; `rewrite/AMEND-A12.md` A29, K2, A25; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub fn formal_defaults_to_instances(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub fn formal_defaults_to_instances_default(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const DRAW_IN_DEFAULT: ir::Code`, `pub const UNKNOWN_INSTANCES: ir::Code`,
//! `pub const PASS_DEFAULT: ir::Code`

use crate::ir;

/// The note at a subcircuit whose non-constant default holds a draw.
pub const DRAW_IN_DEFAULT: ir::Code = ir::Code::new(b'U', 305);

/// The note at a subcircuit some element names without a binding.
pub const UNKNOWN_INSTANCES: ir::Code = ir::Code::new(b'U', 306);

/// The note at a subcircuit whose formals' replaced defaults are the pass's literal `0`.
pub const PASS_DEFAULT: ir::Code = ir::Code::new(b'U', 308);

/// One formal: its name, its default, its definition.
type Formal = (ir::NameId, ir::ExprId, ir::DefId);

/// Passes every non-constant default at each instance that omits it; the default goes.
pub fn formal_defaults_to_instances(design: &mut ir::Design, diags: &mut ir::Diags) {
    defaults(design, diags, false);
}

/// As [`formal_defaults_to_instances`], the replaced defaults the literal `0`, with a
/// `PASS_DEFAULT` note.
pub fn formal_defaults_to_instances_default(design: &mut ir::Design, diags: &mut ir::Diags) {
    defaults(design, diags, true);
}

// ---- private ----

fn defaults(design: &mut ir::Design, diags: &mut ir::Diags, with_default: bool) {
    let feature = ir::Feature::Extra(ir::Extra::FormalNotConstant);
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let subckts: Vec<ir::ItemRef> = rows
        .rows()
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index)
        .map(|d| d.item)
        .collect();
    let reachable = design.reachable();
    for s in subckts {
        let row = design.row::<ir::item::Subckt>(s.index);
        let formals: Vec<Formal> = design
            .params(row.params)
            .iter()
            .map(|&(n, v)| (n, v, design.lookup(row.body, ir::DefKind::Param, n)))
            .collect();
        let open: Vec<usize> = (0..formals.len())
            .filter(|&k| not_constant(design, formals[k].1))
            .collect();
        if open.iter().any(|&k| holds_draw(design, formals[k].1)) {
            diags.push_at(DRAW_IN_DEFAULT, design, s, 0);
            continue;
        }
        let name = design.str(row.name).to_ascii_lowercase();
        let unknown = reachable.iter().any(|&(i, _)| {
            i.kind == ir::ItemKind::Element && {
                let e = design.row::<ir::Element>(i.index);
                e.model.name != ir::NameId::NONE
                    && !e.model.is_resolved()
                    && design.str(e.model.name).eq_ignore_ascii_case(&name)
            }
        });
        if unknown {
            diags.push_at(UNKNOWN_INSTANCES, design, s, 0);
            continue;
        }
        let instances: Vec<(ir::ItemRef, ir::ScopeId)> = reachable
            .iter()
            .copied()
            .filter(|&(i, _)| {
                i.kind == ir::ItemKind::Element && {
                    let e = design.row::<ir::Element>(i.index);
                    e.model.is_resolved() && design.def(e.model.def).item == s
                }
            })
            .collect();
        let mut editor = design.editor();
        for (inst, scope) in instances {
            let e = editor.design().row::<ir::Element>(inst.index);
            let given: Vec<(ir::NameId, ir::ExprId)> = editor.design().params(e.params).to_vec();
            let mut actuals: Vec<(String, ir::ExprId)> = given
                .iter()
                .map(|&(n, v)| (editor.design().str(n).to_string(), v))
                .collect();
            let mut b = editor.builder();
            for &k in &open {
                if given.iter().any(|g| g.0 == formals[k].0) {
                    continue;
                }
                let value = expand(&mut b, &formals, &given, scope, k, 0);
                actuals.push((b.design().str(formals[k].0).to_string(), value));
            }
            let pairs: Vec<(&str, ir::ExprId)> =
                actuals.iter().map(|(n, v)| (n.as_str(), *v)).collect();
            let mut r = e;
            r.params = b.params(&pairs);
            editor.set(inst.index, r);
        }
        // the replaced defaults: none, or the pass's 0
        let d = editor.design();
        let mut head: Vec<(String, ir::ExprId)> = formals
            .iter()
            .map(|f| (d.str(f.0).to_string(), f.1))
            .collect();
        let mut b = editor.builder();
        for &k in &open {
            head[k].1 = if with_default {
                b.num(0.0)
            } else {
                ir::ExprId::NONE
            };
        }
        let pairs: Vec<(&str, ir::ExprId)> = head.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        let mut r = row;
        r.params = b.params(&pairs);
        editor.set(s.index, r);
        if with_default && !open.is_empty() {
            diags.push_at(PASS_DEFAULT, design, s, 0);
        }
    }
}

/// The default of formal `k` as instance `given` reads it from `scope` (R53, A29): an unbound
/// read looked up from `scope`, a sibling read the instance's actual or the sibling's default
/// expanded the same way, a call kept. Bounded: a sibling chain longer than the formals is a
/// cycle, which `check` refuses (asserted).
fn expand(
    b: &mut ir::Builder<'_>,
    formals: &[Formal],
    given: &[(ir::NameId, ir::ExprId)],
    scope: ir::ScopeId,
    k: usize,
    depth: usize,
) -> ir::ExprId {
    assert!(depth <= formals.len(), "a cycle among defaults");
    let root = formals[k].1;
    assert!(!root.is_none(), "an omitted actual has a default");
    ir::rewrite_tree(b, root, &mut |b, e| {
        let ir::Expr::Param(r) = b.design().expr(e) else {
            return None;
        };
        if r.def.is_none() {
            let def = b.design().lookup(scope, ir::DefKind::Param, r.name);
            return (!def.is_none()).then(|| b.param_ref_to(def));
        }
        let sibling = formals.iter().position(|f| f.2 == r.def)?;
        match given.iter().find(|g| g.0 == formals[sibling].0) {
            Some(g) => Some(g.1),
            None => Some(expand(b, formals, given, scope, sibling, depth + 1)),
        }
    })
}

/// Whether a default is not a constant: it reads a parameter, `$tnom` or the temperature.
fn not_constant(design: &ir::Design, root: ir::ExprId) -> bool {
    if root.is_none() {
        return false;
    }
    let mut stack = vec![root];
    // bounded: each pop reads one row of the tree (a shared row once per path)
    while let Some(e) = stack.pop() {
        if e.is_none() {
            continue;
        }
        match design.expr(e) {
            ir::Expr::Param(_) | ir::Expr::Tnom | ir::Expr::Temp => return true,
            ir::Expr::Call(_, l)
            | ir::Expr::UserCall(_, l)
            | ir::Expr::Draw(_, _, _, l)
            | ir::Expr::List(l) => stack.extend_from_slice(design.exprs(l)),
            ir::Expr::Unary(_, a) => stack.push(a),
            ir::Expr::Binary(_, a, c) => stack.extend([a, c]),
            ir::Expr::Ternary(c, a, d) => stack.extend([c, a, d]),
            _ => {}
        }
    }
    ir::expr_class(design, root) != ir::ExprClass::Constant
}

/// Whether the tree under `root` holds a draw.
fn holds_draw(design: &ir::Design, root: ir::ExprId) -> bool {
    let mut stack = vec![root];
    // bounded: each pop reads one row of the tree (a shared row once per path)
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
            ir::Expr::Binary(_, a, c) => stack.extend([a, c]),
            ir::Expr::Ternary(c, a, d) => stack.extend([c, a, d]),
            _ => {}
        }
    }
    false
}
