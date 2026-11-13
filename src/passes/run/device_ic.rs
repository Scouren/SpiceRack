//! Rule `device_ic_to_tran_ic`: a capacitor's `ic` applied only under `uic`
//! (`IcMode::UicOnly`, `Circuit(IcUicOnly)`) moved into the initial conditions of every
//! transient that runs under `uic` (approved as proposed by the team lead, after the runs below), the form a
//! target with no instance `ic` states (VACASK's `ic=` list under `icmode="uic"`; its
//! `sp_capacitor` has no `ic` parameter: "Parameter 'ic' not found").
//!
//! The two simulators start a `uic` transient from different things. SPICE takes each storage
//! element's own state (a capacitor's `ic`, zero when it has none) and solves no node voltage
//! at t = 0; the target takes node voltages and every element's state follows from them. The
//! **assignment** the list gives (pinned by VACASK runs, the `settle_*` decks beside the fixtures): a
//! single entry sets its node; a lone differential entry `a − b = v` sets `a = v`, `b = 0`
//! (run va3); a differential beside a single entry on either of its nodes sets that node and
//! the other by the difference (runs d1, d2 in either order, d4); every unlisted node is 0. A
//! node in two single entries, in two differential entries, or a differential with a single
//! entry on both nodes is refused (not run).
//!
//! **Exact when**, under that assignment: every capacitor at the top level has across it
//! exactly its `ic` (or 0 when it has none); no element with a state of its own (a diode, a
//! transistor, a behavioural, charge or flux element, a line, an instance of a subcircuit) and
//! no voltage source touches a node whose assigned voltage is not 0 (a resistor, an inductor,
//! a coupling or a current source may: an inductor's state is a current, zero on both); every
//! moved capacitor is listed at a file's top level (not in a subcircuit: one state per
//! instance) and its `ic` is a literal; and the design holds no other initial condition or
//! node set. Otherwise every capacitor keeps its `ic` with `DEVICE_IC_KEPT` (all or nothing:
//! the assignment is one for the whole circuit). A transient without `uic` gets nothing:
//! there the `ic` is never read.
//!
//! Each moved capacitor gets `DEVICE_IC_MOVED`: the transient starts from stated conditions.
//! The point at t = 0 of a `uic` transient is no solution of the circuit on either simulator
//! (VACASK stores the unsolved start); comparing it is the harness's question.
//!
//! Removes `Circuit(IcUicOnly)`, requires `Item(NodeSet)` and the next-only, differential and
//! from-here forms of the condition it writes. An inductor's or a semiconductor's `ic` keeps
//! the feature with the note.
//!
//! Cites: R13, R79; the runs in `tests/fixtures/passes/run/device_ic_to_tran_ic/`;
//! `crates/ir/src/option.rs` (`NodeSet`), `passive.rs` (`IcMode`).
//!
//! Contents: `pub fn device_ic_to_tran_ic(&mut ir::Design, &mut ir::Diags)`,
//! `pub const DEVICE_IC_MOVED: ir::Code`, `pub const DEVICE_IC_KEPT: ir::Code`

use std::collections::HashMap;

use crate::ir;

/// The note at a capacitor whose `ic` now starts every `uic` transient.
pub const DEVICE_IC_MOVED: ir::Code = ir::Code::new(b'U', 442);
/// The note at an element whose `ic` stays (the module header's conditions).
pub const DEVICE_IC_KEPT: ir::Code = ir::Code::new(b'U', 443);

/// One entry of the list: a node, its reference node (`NONE`: ground), the value's literal and
/// expression.
type Entry = (ir::NameId, ir::NameId, f64, ir::ExprId);

/// Moves every capacitor's `uic` initial condition into the `uic` transients' lists when the
/// module header's conditions hold; notes every element that keeps one.
pub fn device_ic_to_tran_ic(design: &mut ir::Design, diags: &mut ir::Diags) {
    let reachable = design.reachable();
    let held: Vec<(ir::ItemRef, ir::Element)> = reachable
        .iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Element)
        .map(|&(i, _)| (i, design.row::<ir::Element>(i.index)))
        .filter(|(_, e)| e.kind.detail() == ir::DetailKind::Passive)
        .filter(|(_, e)| design.passive(e).ic_mode == ir::passive::IcMode::UicOnly)
        .collect();
    let some = !held.is_empty();
    if let Some(entries) = entries(design, &reachable, &held).filter(|_| some) {
        write(design, diags, &reachable, &held, &entries);
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::IcUicOnly);
    let legal = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    let mut rows = ir::Diags::new();
    ir::check_legal(design, &legal, &mut rows);
    let index = u32::from(feature.index());
    for d in rows
        .rows()
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED && d.arg == index)
    {
        diags.push_at(DEVICE_IC_KEPT, design, d.item, index);
    }
}

// ---- private ----

/// The list's entries when every condition of the module header holds; `None` otherwise.
fn entries(
    design: &ir::Design,
    reachable: &[(ir::ItemRef, ir::ScopeId)],
    held: &[(ir::ItemRef, ir::Element)],
) -> Option<Vec<Entry>> {
    if reachable
        .iter()
        .any(|(i, _)| i.kind == ir::ItemKind::NodeSet)
    {
        return None;
    }
    let scope_of: HashMap<ir::ItemRef, ir::ScopeId> = reachable.iter().copied().collect();
    let mut out = Vec::new();
    for (item, e) in held {
        let p = design.passive(e);
        let top = design.scope(scope_of[item]).owner == ir::ItemRef::NONE;
        let v = literal(design, p.ic)?;
        if e.kind != ir::ElemKind::Capacitor || !top {
            return None;
        }
        let [pos, neg] = <[ir::NameId; 2]>::try_from(design.names(e.terminals)).ok()?;
        out.push(
            match (pos == ir::NameId::GROUND, neg == ir::NameId::GROUND) {
                (false, true) => (pos, ir::NameId::NONE, v, p.ic),
                (true, false) => (neg, ir::NameId::NONE, -v, ir::ExprId::NONE),
                (false, false) => (pos, neg, v, p.ic),
                (true, true) => return None,
            },
        );
    }
    let volts = assignment(&out)?;
    let at = |n: ir::NameId| volts.get(&n).copied().unwrap_or(0.0);
    // every element against the assignment
    for &(item, scope) in reachable {
        if item.kind != ir::ItemKind::Element || design.scope(scope).owner != ir::ItemRef::NONE {
            continue;
        }
        let e = design.row::<ir::Element>(item.index);
        let nodes = design.names(e.terminals);
        match e.kind {
            ir::ElemKind::Capacitor => {
                let p = design.passive(&e);
                let want = match p.ic_mode {
                    ir::passive::IcMode::UicOnly => literal(design, p.ic)?,
                    _ => 0.0,
                };
                let [a, b] = <[ir::NameId; 2]>::try_from(nodes).ok()?;
                if at(a) - at(b) != want {
                    return None;
                }
            }
            ir::ElemKind::Resistor
            | ir::ElemKind::Inductor
            | ir::ElemKind::Coupling
            | ir::ElemKind::ISource => {}
            _ if nodes.iter().any(|&n| at(n) != 0.0) => return None,
            _ => {}
        }
    }
    Some(out)
}

/// The node voltages the list states (the module header's assignment); `None` for a
/// combination no run pinned.
fn assignment(entries: &[Entry]) -> Option<HashMap<ir::NameId, f64>> {
    let mut single: HashMap<ir::NameId, f64> = HashMap::new();
    for &(n, r, v, _) in entries {
        if r == ir::NameId::NONE && single.insert(n, v).is_some() {
            return None;
        }
    }
    let mut volts = single.clone();
    let mut in_diff: HashMap<ir::NameId, u8> = HashMap::new();
    for &(n, r, v, _) in entries.iter().filter(|e| e.1 != ir::NameId::NONE) {
        for x in [n, r] {
            *in_diff.entry(x).or_default() += 1;
        }
        let (pos, neg) = match (single.get(&n), single.get(&r)) {
            (Some(_), Some(_)) => return None,
            (Some(&a), None) => (a, a - v),
            (None, Some(&b)) => (b + v, b),
            (None, None) => (v, 0.0),
        };
        volts.insert(n, pos);
        volts.insert(r, neg);
    }
    in_diff.values().all(|&k| k == 1).then_some(volts)
}

/// Writes the entries before every `uic` transient and takes each capacitor's `ic` out.
fn write(
    design: &mut ir::Design,
    diags: &mut ir::Diags,
    reachable: &[(ir::ItemRef, ir::ScopeId)],
    held: &[(ir::ItemRef, ir::Element)],
    entries: &[Entry],
) {
    let trans: Vec<(ir::ItemRef, ir::ScopeId)> = reachable
        .iter()
        .copied()
        .filter(|(i, _)| i.kind == ir::ItemKind::Analysis)
        .filter(|(i, _)| {
            let a = design.row::<ir::Analysis>(i.index);
            a.kind == ir::AnalysisKind::Tran && design.detail::<ir::tran::Tran>(a.detail).uic
        })
        .collect();
    for (tran, scope) in trans {
        for &(n, r, v, e) in entries {
            let mut editor = design.editor();
            let mut b = editor.builder();
            let value = match e.is_none() {
                true => b.literal(ir::Literal::from_f64(v)),
                false => e,
            };
            let reference = match r == ir::NameId::NONE {
                true => ir::NameList::EMPTY,
                false => b.name_ids(&[r]),
            };
            let row = ir::option::NodeSet {
                kind: ir::option::NodeSetKind::Ic,
                applies: ir::option::IcScope::Transient,
                next_only: true,
                node: b.name_ids(&[n]),
                reference,
                value,
                holds: ir::option::Applies::FromHere,
            };
            let item = b.push(scope, row, ir::Span::NONE);
            let at = editor.design().items(scope).iter().position(|&i| i == tran);
            let at = at.expect("the transient is listed in its scope");
            editor.move_item(scope, item, scope, at);
        }
    }
    for (item, e) in held {
        let mut p = design.passive(e);
        p.ic = ir::ExprId::NONE;
        p.ic_mode = ir::passive::IcMode::None;
        design.editor().set_detail(e.detail, p);
        diags.push_at(DEVICE_IC_MOVED, design, *item, 0);
    }
}

/// The value of a literal, or of a literal under negations; `None` otherwise.
fn literal(design: &ir::Design, e: ir::ExprId) -> Option<f64> {
    let (mut sign, mut at) = (1.0, e);
    // bounded by the expression table
    for _ in 0..=design.expr_count_all() {
        if at.is_none() {
            return None;
        }
        match design.expr(at) {
            ir::Expr::Num(n) => return Some(sign * design.literal_value(design.num(n))),
            ir::Expr::Unary(ir::UnOp::Neg, a) => (sign, at) = (-sign, a),
            _ => return None,
        }
    }
    unreachable!("a chain of negations is shorter than the table")
}
