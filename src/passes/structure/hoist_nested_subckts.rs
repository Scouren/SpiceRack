//! Pass `hoist_nested_subckts` (rows `hoist_nested_subckts` and `hoist_enclosing_refs`): a
//! design in, the same design out with every subcircuit listed in another subcircuit's body
//! moved one level out per step (into the scope listing its enclosing subcircuit, right after
//! it) until none is nested. It removes `Placed(Subckt, Subckt)` and, with it,
//! `Extra::EnclosingRef` (VACASK resolves a name in the own subcircuit, then the top, never an
//! enclosing subcircuit; A12 K2). A read of an enclosing subcircuit's parameter becomes a new
//! formal of the same name, and each instance passes that name, bound to the enclosing
//! definition; a name taken where the subcircuit lands is renamed (`<name>_<n>`), its
//! instances following.
//!
//! The added formal's default (R13: no invented value): the rows `hoist_nested_subckts` and
//! `hoist_enclosing_refs` leave it without one and require `Extra(FormalWithoutDefault)`, the
//! exact statement that every instance passes it; their `_default` twins, for a target without
//! that feature, give it the literal `0` and put a `HOIST_DEFAULT` note at the subcircuit
//! saying the default is the hoist's and no instance reads it. `legalize` takes the first
//! usable row, so a target that can say "no default" gets the clean form.
//!
//! Left with a note: a subcircuit whose body reads an enclosing model, function or subcircuit,
//! or an enclosing parameter one of its own formals is named as (`READS_ENCLOSING`), or one defined in a branch, loop or section inside a subcircuit
//! (`NOT_IN_BODY`).
//!
//! Cites: `rewrite/AMEND-A12.md` K2, A15 K1, A32; R27; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub fn hoist_nested_subckts(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub fn hoist_nested_subckts_default(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub fn hoist_enclosing_refs(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub fn hoist_enclosing_refs_default(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const READS_ENCLOSING: ir::Code`, `pub const NOT_IN_BODY: ir::Code`,
//! `pub const HOIST_DEFAULT: ir::Code`

use std::collections::HashMap;

use crate::ir;

/// The note at a nested subcircuit whose body reads an enclosing model, function or subcircuit.
pub const READS_ENCLOSING: ir::Code = ir::Code::new(b'U', 303);

/// The note at a subcircuit defined in a branch, loop or section inside a subcircuit.
pub const NOT_IN_BODY: ir::Code = ir::Code::new(b'U', 304);

/// The note at a hoisted subcircuit whose added formals got the literal default `0`.
pub const HOIST_DEFAULT: ir::Code = ir::Code::new(b'U', 307);

/// Hoists every nested subcircuit one level per step until none is nested; added formals have
/// no default.
pub fn hoist_nested_subckts(design: &mut ir::Design, diags: &mut ir::Diags) {
    hoist(design, diags, false, false);
}

/// As [`hoist_nested_subckts`], the added formals defaulting to `0`, with a `HOIST_DEFAULT` note.
pub fn hoist_nested_subckts_default(design: &mut ir::Design, diags: &mut ir::Diags) {
    hoist(design, diags, true, false);
}

/// As [`hoist_nested_subckts`], only the subcircuits whose subtree reads an enclosing body
/// (`EnclosingRef`); every item still reading one gets a `READS_ENCLOSING` note.
pub fn hoist_enclosing_refs(design: &mut ir::Design, diags: &mut ir::Diags) {
    hoist(design, diags, false, true);
}

/// As [`hoist_enclosing_refs`], the added formals defaulting to `0`, with a `HOIST_DEFAULT`
/// note.
pub fn hoist_enclosing_refs_default(design: &mut ir::Design, diags: &mut ir::Diags) {
    hoist(design, diags, true, true);
}

// ---- private ----

/// The worklist: each step hoists the first nested subcircuit not yet refused, one level.
/// Bounded: a step lowers one subcircuit's nesting depth, so the steps are at most the
/// subcircuits times the scope depth; the loop asserts that bound.
fn hoist(design: &mut ir::Design, diags: &mut ir::Diags, with_default: bool, enclosing: bool) {
    let mut refused: Vec<ir::ItemRef> = Vec::new();
    let mut last_moved: HashMap<ir::ItemRef, ir::ItemRef> = HashMap::new();
    let subckts = design
        .reachable()
        .iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Subckt)
        .count();
    let bound = (subckts + 1) * (subckts + 1);
    for step in 0.. {
        assert!(step <= bound, "hoist: the steps do not settle");
        let next = design.reachable().into_iter().find(|&(i, s)| {
            i.kind == ir::ItemKind::Subckt && !refused.contains(&i) && in_subckt(design, s)
        });
        let Some((s2, l2)) = next else { break };
        let reads = || {
            let c = carrying(design, ir::Feature::Extra(ir::Extra::EnclosingRef));
            subtree(design, s2).iter().any(|p| c.contains(&p.0))
        };
        if enclosing && !reads() {
            refused.push(s2); // nothing in it reads an enclosing body: this row leaves it
            continue;
        }
        if design.scope(l2).kind != ir::ScopeKind::Subckt {
            if !enclosing {
                diags.push_at(NOT_IN_BODY, design, s2, 0);
            }
            refused.push(s2);
            continue;
        }
        match plan(design, s2, l2) {
            Some(formals) => {
                let s1 = design.scope(l2).owner;
                let after = last_moved.get(&s1).copied().unwrap_or(s1);
                hoist_one(design, s2, l2, after, &formals, with_default);
                if with_default && !formals.is_empty() {
                    diags.push_at(HOIST_DEFAULT, design, s2, 0);
                }
                last_moved.insert(s1, s2);
            }
            None => {
                if !enclosing {
                    diags.push_at(READS_ENCLOSING, design, s2, 0);
                }
                refused.push(s2);
            }
        }
    }
    if enclosing {
        for item in carrying(design, ir::Feature::Extra(ir::Extra::EnclosingRef)) {
            diags.push_at(READS_ENCLOSING, design, item, 0);
        }
    }
}

/// The items that carry `feature`: `check_legal` against every feature but it.
fn carrying(design: &ir::Design, feature: ir::Feature) -> Vec<ir::ItemRef> {
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    let rows = rows.rows().iter();
    rows.filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index)
        .map(|d| d.item)
        .collect()
}

/// Whether `scope` lies in a subcircuit's body (its chain holds a `Subckt` scope).
fn in_subckt(design: &ir::Design, scope: ir::ScopeId) -> bool {
    let mut at = scope;
    // bounded by the scope tree's depth
    while !at.is_none() {
        let s = design.scope(at);
        if s.kind == ir::ScopeKind::Subckt {
            return true;
        }
        at = s.parent;
    }
    false
}

/// Whether `scope` is `body` or lies inside it.
fn within(design: &ir::Design, scope: ir::ScopeId, body: ir::ScopeId) -> bool {
    let mut at = scope;
    // bounded by the scope tree's depth
    while !at.is_none() {
        if at == body {
            return true;
        }
        at = design.scope(at).parent;
    }
    false
}

/// The items of `s2`'s subtree: itself (its head defaults) and every item listed in its body.
fn subtree(design: &ir::Design, s2: ir::ItemRef) -> Vec<(ir::ItemRef, ir::ScopeId)> {
    let body = design.row::<ir::item::Subckt>(s2.index).body;
    let reachable = design.reachable();
    let home = reachable
        .iter()
        .find(|p| p.0 == s2)
        .map(|p| p.1)
        .expect("listed");
    let mut out = vec![(s2, home)];
    out.extend(
        reachable
            .into_iter()
            .filter(|&(_, s)| within(design, s, body)),
    );
    out
}

/// The parameters of `l2` (the enclosing body) that `s2`'s subtree reads, in first-read order,
/// or `None` when it reads any other definition there (a model, function, subcircuit).
fn plan(design: &ir::Design, s2: ir::ItemRef, l2: ir::ScopeId) -> Option<Vec<ir::DefId>> {
    let mut formals: Vec<ir::DefId> = Vec::new();
    for (item, _) in subtree(design, s2) {
        let mut refs = Vec::new();
        design.item_refs(item, &mut refs);
        let mut defs: Vec<ir::DefId> = refs.iter().map(|r| r.0.def).collect();
        defs.extend(expr_reads(design, item));
        for def in defs.into_iter().filter(|d| !d.is_none()) {
            let d = design.def(def);
            if d.scope != l2 {
                continue;
            }
            if d.kind != ir::DefKind::Param {
                return None;
            }
            // a formal of that name already (a head default reading the outer one): refused
            let body = design.row::<ir::item::Subckt>(s2.index).body;
            let own = design.lookup(body, ir::DefKind::Param, d.name);
            if !own.is_none() && design.def(own).scope == body {
                return None;
            }
            if !formals.contains(&def) {
                formals.push(def);
            }
        }
    }
    Some(formals)
}

/// The definitions the expressions of `item` read (`Param` and `UserCall` nodes), tree order.
fn expr_reads(design: &ir::Design, item: ir::ItemRef) -> Vec<ir::DefId> {
    let mut handles = Vec::new();
    design.item_handles(item, &mut handles);
    let mut stack: Vec<ir::ExprId> = Vec::new();
    for h in &handles {
        match *h {
            ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => stack.push(e),
            ir::Handle::Exprs(l) => stack.extend_from_slice(design.exprs(l)),
            ir::Handle::Params(l) => stack.extend(design.params(l).iter().map(|p| p.1)),
            _ => {}
        }
    }
    stack.reverse();
    let mut out = Vec::new();
    // bounded: each pop reads one row of a tree (a shared row once per path); children are
    // pushed last first, so reads come out in tree order
    while let Some(e) = stack.pop() {
        if e.is_none() {
            continue;
        }
        match design.expr(e) {
            ir::Expr::Param(r) => out.push(r.def),
            ir::Expr::UserCall(r, l) => {
                out.push(r.def);
                stack.extend(design.exprs(l).iter().rev());
            }
            ir::Expr::Call(_, l) | ir::Expr::Draw(_, _, _, l) | ir::Expr::List(l) => {
                stack.extend(design.exprs(l).iter().rev())
            }
            ir::Expr::Unary(_, a) => stack.push(a),
            ir::Expr::Binary(_, a, b) => stack.extend([b, a]),
            ir::Expr::Ternary(c, a, b) => stack.extend([b, a, c]),
            _ => {}
        }
    }
    out
}

/// One level: `s2` gets a formal per read parameter (its reads rebound to it), each instance
/// passes the enclosing parameter, and `s2` moves to just after `after` in the scope listing
/// its enclosing subcircuit, renamed first when that scope defines its name.
fn hoist_one(
    design: &mut ir::Design,
    s2: ir::ItemRef,
    l2: ir::ScopeId,
    after: ir::ItemRef,
    formals: &[ir::DefId],
    with_default: bool,
) {
    let s1 = design.scope(l2).owner;
    let l = design
        .reachable()
        .into_iter()
        .find(|p| p.0 == s1)
        .map(|p| p.1)
        .expect("listed");
    let row = design.row::<ir::item::Subckt>(s2.index);
    let own = design.lookup(l2, ir::DefKind::Subckt, row.name);
    assert!(
        design.def(own).item == s2,
        "the subcircuit defines its name"
    );
    let instances: Vec<ir::ItemRef> = design
        .reachable()
        .into_iter()
        .map(|(i, _)| i)
        .filter(|i| {
            i.kind == ir::ItemKind::Element && design.row::<ir::Element>(i.index).model.def == own
        })
        .collect();
    let inside = subtree(design, s2);
    let mut editor = design.editor();
    // the formals: head entries, definitions in the body, and the reads rebound to them
    let mut map: Vec<(ir::DefId, ir::DefId)> = Vec::new();
    {
        let d = editor.design();
        let mut head: Vec<(String, ir::ExprId)> = d
            .params(row.params)
            .iter()
            .map(|&(n, v)| (d.str(n).to_string(), v))
            .collect();
        let mut b = editor.builder();
        for &old in formals {
            let value = if with_default {
                b.num(0.0)
            } else {
                ir::ExprId::NONE
            };
            head.push((b.design().str(b.design().def(old).name).to_string(), value));
        }
        let pairs: Vec<(&str, ir::ExprId)> = head.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        let params = b.params(&pairs);
        for &old in formals {
            let name = b.design().def(old).name;
            let new = b.define(ir::DefKind::Param, name, row.body, s2);
            map.push((old, new));
        }
        let mut r = row;
        r.params = params;
        editor.set(s2.index, r);
    }
    let lookup = |d: ir::DefId| map.iter().find(|m| m.0 == d).map(|m| m.1);
    for &(item, _) in &inside {
        editor.map_refs(item, &mut |r, _| {
            if let Some(new) = lookup(r.def) {
                r.def = new;
            }
        });
        let mut roots = Vec::new();
        editor.map_exprs(item, &mut |e, _| roots.push(*e));
        let mut b = editor.builder();
        let new: Vec<ir::ExprId> = roots
            .iter()
            .map(|&root| match root.is_none() {
                true => root,
                false => ir::rewrite_tree(&mut b, root, &mut |b, e| match b.design().expr(e) {
                    ir::Expr::Param(r) => lookup(r.def).map(|d| b.param_ref_to(d)),
                    _ => None,
                }),
            })
            .collect();
        let mut k = 0;
        editor.map_exprs(item, &mut |e, _| {
            *e = new[k];
            k += 1;
        });
    }
    // each instance passes the enclosing parameters by their own definitions
    for &inst in &instances {
        let d = editor.design();
        let e = d.row::<ir::Element>(inst.index);
        let mut actuals: Vec<(String, ir::ExprId)> = d
            .params(e.params)
            .iter()
            .map(|&(n, v)| (d.str(n).to_string(), v))
            .collect();
        let mut b = editor.builder();
        for &old in formals {
            let read = b.param_ref_to(old);
            actuals.push((b.design().str(b.design().def(old).name).to_string(), read));
        }
        let pairs: Vec<(&str, ir::ExprId)> =
            actuals.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        let params = b.params(&pairs);
        let mut row = e;
        row.params = params;
        editor.set(inst.index, row);
    }
    // a name the landing scope already defines (in any case) is renamed, its references follow
    let taken = |d: &ir::Design, n: &str| {
        d.definitions().iter().any(|x| {
            x.scope == l && x.kind == ir::DefKind::Subckt && d.str(x.name).eq_ignore_ascii_case(n)
        })
    };
    let base = editor.design().str(row.name).to_string();
    if taken(editor.design(), &base) {
        let bound = editor.design().definitions().len() + 1;
        let fresh = (1..=bound)
            .map(|n| format!("{base}_{n}"))
            .find(|n| !taken(editor.design(), n))
            .expect("fewer definitions than candidates");
        let name = editor.builder().name(&fresh);
        editor.rename_def(own, name);
        let mut r = editor.design().row::<ir::item::Subckt>(s2.index);
        r.name = name;
        editor.set(s2.index, r);
        let all: Vec<ir::ItemRef> = editor
            .design()
            .reachable()
            .into_iter()
            .map(|p| p.0)
            .collect();
        for item in all {
            editor.map_refs(item, &mut |r, _| {
                if r.def == own {
                    r.name = name;
                }
            });
        }
    }
    let at = editor
        .design()
        .items(l)
        .iter()
        .position(|&i| i == after)
        .expect("listed")
        + 1;
    editor.move_item(l2, s2, l, at);
}
