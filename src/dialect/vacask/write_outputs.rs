//! `Save` rows → `save` lines: `AllV` followed by `AllBranchI` is `default` (run e5), `V` is
//! `v(node)`, `I` is `i(instance)`, or `p(instance, i)` for an isource, resistor, vccs or cccs
//! (run F13: they have no `i()`), `Param` is `p(instance, quantity)`, hierarchical names
//! quoted `'x1:n'` (VD cmd-save). `NodeSet` rows are spelled on the analysis they apply to
//! (`write_run`). The inverse of `read_outputs::save`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-save.
//!
//! Contents: `admit`, `save` (crate-private); private `Current`, `current`

use super::name::{self, Place};
use super::read_outputs::{ELEMENT_CURRENT, I_CURRENT, P_CURRENT};
use super::read_run::SAVE_CATEGORY;
use super::write::Writer;
use crate::ir;

/// How an `I` probe is saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Current {
    /// `i(x)`: a branch kind, a path, or a name no element of the save's scope has.
    I,
    /// `p(x, i)`: an isource, resistor, vccs or cccs named before the save.
    P,
    /// No spelling (`ELEMENT_CURRENT`).
    None,
}

/// One `Save` row as its `save` line.
pub(super) fn save(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let row: ir::Save = d.row(at.index);
    let probes = d.probes(row.probes);
    w.line();
    w.out.push_str("save");
    let mut k = 0;
    // Bounded: each pass takes one or two probes.
    while k < probes.len() {
        let p = probes[k];
        let next = probes.get(k + 1).map(|q| q.kind);
        w.out.push(' ');
        let path = |list: ir::NameList| {
            let names: Vec<&str> = d.names(list).iter().map(|&n| d.str(n)).collect();
            names.join(":")
        };
        match p.kind {
            ir::ProbeKind::AllV if next == Some(ir::ProbeKind::AllBranchI) => {
                w.out.push_str("default");
                k += 1;
            }
            ir::ProbeKind::I if current(d, at, p.a) == Current::P => {
                w.out.push_str("p(");
                name::spell(&path(p.a), Place::Node, w.out);
                w.out.push_str(", i)");
            }
            ir::ProbeKind::V | ir::ProbeKind::I => {
                w.out.push_str(if p.kind == ir::ProbeKind::V {
                    "v("
                } else {
                    "i("
                });
                name::spell(&path(p.a), Place::Node, w.out);
                w.out.push(')');
            }
            ir::ProbeKind::Param => {
                w.out.push_str("p(");
                name::spell(&path(p.a), Place::Node, w.out);
                w.out.push_str(", ");
                name::spell(&path(p.b), Place::Other, w.out);
                w.out.push(')');
            }
            // `admits` refuses every other probe.
            _ => w.out.push_str("default"),
        }
        k += 1;
    }
    w.out.push('\n');
}

/// Appends a refusal for each `Save` whose probes VACASK cannot spell for a reason no feature
/// states: an `AllV` not followed by an `AllBranchI`, an `AllBranchI` alone (VACASK's only
/// category is `default`, both together; run e5), or a set of analyses other than the one a
/// `save` line governs (op, its sweeps and tran; with `default` and no p() also ac and dcinc).
pub(super) fn admit(design: &ir::Design, at: ir::ItemRef, diags: &mut ir::Diags) {
    let row: ir::Save = design.row(at.index);
    let probes = design.probes(row.probes);
    let kinds: Vec<ir::ProbeKind> = probes.iter().map(|p| p.kind).collect();
    let currents: Vec<Current> = probes
        .iter()
        .filter(|p| p.kind == ir::ProbeKind::I)
        .map(|p| current(design, at, p.a))
        .collect();
    if currents.contains(&Current::None) {
        diags.push_at(ELEMENT_CURRENT, design, at, 0);
    }
    let mut k = 0;
    let mut spelled = true;
    // Bounded: each pass takes one or two probes.
    while k < kinds.len() {
        spelled &= match kinds[k] {
            ir::ProbeKind::AllV => kinds.get(k + 1) == Some(&ir::ProbeKind::AllBranchI),
            ir::ProbeKind::V | ir::ProbeKind::I | ir::ProbeKind::Param => true,
            _ => false,
        };
        k += 1 + usize::from(kinds[k] == ir::ProbeKind::AllV);
    }
    // A `save` line governs op, its sweeps and tran; with `default` and no p() also ac and
    // dcinc (run e5): every other set has no spelling.
    use ir::AnalysisKind as K;
    let base = ir::AnalysisKinds::of(&[K::Op, K::Dc, K::Tran]);
    let own = ir::AnalysisKinds::of(&[K::Op, K::Dc, K::Tran, K::Ac, K::DcInc]);
    let default = kinds.contains(&ir::ProbeKind::AllV);
    let param = kinds.contains(&ir::ProbeKind::Param) || currents.contains(&Current::P);
    let own_set = default && !param;
    spelled &= match own_set {
        true => row.analyses == own,
        false => row.analyses == base,
    };
    // Runs sv1, sv2: a VACASK save holds from where it stands.
    spelled &= row.holds == ir::Applies::FromHere;
    if !spelled {
        diags.push_at(SAVE_CATEGORY, design, at, 0);
    }
}

// ---- private ----

/// The spelling of the current of element path `path` saved by `save`: the kind of the first
/// element of that whole name in the save's scope decides; `p(x, i)` reads back as a current
/// only after the element's line (`read_outputs::save`).
// ponytail: one walk of the design per I probe; an index when saves of many currents matter.
fn current(design: &ir::Design, save: ir::ItemRef, path: ir::NameList) -> Current {
    let names = design.names(path);
    if names.len() != 1 {
        return Current::I;
    }
    let items = design.reachable();
    let Some(&(_, scope)) = items.iter().find(|(i, _)| *i == save) else {
        return Current::I;
    };
    let mut before = true;
    for &(item, s) in &items {
        before &= item != save;
        if s != scope || item.kind != ir::ItemKind::Element {
            continue;
        }
        let e: ir::Element = design.row(item.index);
        if e.name != names[0] {
            continue;
        }
        return match (P_CURRENT.contains(&e.kind), before) {
            (true, true) => Current::P,
            (true, false) => Current::None,
            (false, _) if I_CURRENT.contains(&e.kind) => Current::I,
            (false, _) => Current::None,
        };
    }
    Current::I
}
