//! `Save` rows → `save` lines: `AllV` followed by `AllBranchI` is `default` (run e5), `V` is
//! `v(node)`, `I` is `i(instance)`, `Param` is `p(instance, quantity)`, hierarchical names
//! quoted `'x1:n'` (VD cmd-save). `NodeSet` rows are spelled on the analysis they apply to
//! (`write_run`). The inverse of `read_outputs::save`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §4; VD cmd-save.
//!
//! Contents: `admits`, `save` (crate-private)

use super::name::{self, Place};
use super::read_run::SAVE_CATEGORY;
use super::write::Writer;
use crate::ir;

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
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    for (at, _) in design.reachable() {
        if at.kind != ir::ItemKind::Save {
            continue;
        }
        let row: ir::Save = design.row(at.index);
        let kinds: Vec<ir::ProbeKind> = design.probes(row.probes).iter().map(|p| p.kind).collect();
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
        let param = kinds.contains(&ir::ProbeKind::Param);
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
}
