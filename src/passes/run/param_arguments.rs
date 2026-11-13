//! Rule `analysis_param_args_to_values`: a parameter read in an analysis's own argument (a DC
//! axis's range included) or a loop's range, written out as the parameter's value where nothing
//! can change it. It removes `Run(AnalysisParamArgument)` (R68/A42: ngspice substitutes the
//! parameter there; VACASK rejects a circuit parameter in an analysis argument) and requires
//! nothing: the value's expression already stands in the design, as the parameter's.
//!
//! A read is replaced by the parameter's own value expression (shared, as the IR allows) when
//! the parameter is a top-level `Param` whose value is a constant (no read, no draw: it
//! evaluates with nothing bound), no alter, sweep loop or table loop in the design targets it,
//! and the design holds no statistics block (which may vary it). Then the field evaluates to the
//! same value before and after (`tests/passes_run/args.rs`, the reference evaluator's value of
//! every field, the parameter bound by its definition before, nothing bound after). A read in a
//! function's body the field calls is not rewritten. An item still reading a parameter in such
//! a field keeps the feature with `PARAM_ARG_KEPT`.
//!
//! Cites: R68, AMEND-A12 A42; `crates/ir/src/analysis_facts.rs` (`run_arguments_read_param`).
//!
//! Contents: `pub fn analysis_param_args_to_values(&mut ir::Design, &mut ir::Diags)`,
//! `pub const PARAM_ARG_KEPT: ir::Code`

use super::sweep_forms::Nothing;
use crate::ir;
use ir::plan::{Alter, Loop, LoopKind, SweepLoop, TableLoop};

/// The note at an analysis or loop whose argument still reads a parameter.
pub const PARAM_ARG_KEPT: ir::Code = ir::Code::new(b'U', 425);

/// Every fixed parameter read in an analysis or loop argument replaced by its value; the items
/// left reading one noted.
pub fn analysis_param_args_to_values(design: &mut ir::Design, diags: &mut ir::Diags) {
    let feature = ir::Feature::Run(ir::RunExtra::AnalysisParamArgument);
    let carriers = carrying(design, feature);
    if carriers.is_empty() {
        return;
    }
    let reachable = design.reachable();
    let stats = reachable
        .iter()
        .any(|(i, _)| i.kind == ir::ItemKind::Statistics);
    let mut touched: Vec<ir::DefId> = Vec::new();
    for &(i, _) in &reachable {
        let targets = match i.kind {
            ir::ItemKind::Alter => vec![design.row::<Alter>(i.index).target],
            ir::ItemKind::Loop => {
                let l = design.row::<Loop>(i.index);
                match l.kind {
                    LoopKind::Sweep => vec![design.detail::<SweepLoop>(l.detail).target],
                    LoopKind::Table => {
                        let t = design.detail::<TableLoop>(l.detail);
                        design.targets(t.columns).to_vec()
                    }
                    LoopKind::MonteCarlo => Vec::new(),
                }
            }
            _ => Vec::new(),
        };
        touched.extend(
            targets
                .iter()
                .filter(|t| t.a.is_resolved())
                .map(|t| t.a.def),
        );
    }
    let fixed = |design: &ir::Design, r: ir::Ref| -> Option<ir::ExprId> {
        if stats || !r.is_resolved() || touched.contains(&r.def) {
            return None;
        }
        let def = design.def(r.def);
        let top = design.scope(def.scope).kind == ir::ScopeKind::File;
        if def.kind != ir::DefKind::Param || def.item.kind != ir::ItemKind::Param || !top {
            return None;
        }
        let value = design.row::<ir::item::Param>(def.item.index).value;
        ir::eval(design, value, &Nothing).is_ok().then_some(value)
    };
    for item in carriers {
        let mut roots: Vec<(ir::ExprId, ir::ExprUse)> = Vec::new();
        design
            .editor()
            .map_exprs(item, &mut |e, u| roots.push((*e, u)));
        let mut new: Vec<ir::ExprId> = Vec::with_capacity(roots.len());
        let mut b = design.builder();
        for &(root, use_) in &roots {
            if use_ != ir::ExprUse::Constant {
                new.push(root);
                continue;
            }
            new.push(ir::rewrite_tree(
                &mut b,
                root,
                &mut |b, e| match b.design().expr(e) {
                    ir::Expr::Param(r) => fixed(b.design(), r),
                    _ => None,
                },
            ));
        }
        let mut k = 0;
        design.editor().map_exprs(item, &mut |e, _| {
            *e = new[k];
            k += 1;
        });
        assert!(k == roots.len(), "the same roots in the same order");
    }
    for item in carrying(design, feature) {
        diags.push_at(PARAM_ARG_KEPT, design, item, 0);
    }
}

// ---- private ----

/// The reachable items carrying `feature` (`ir::check_legal` against every other feature).
fn carrying(design: &ir::Design, feature: ir::Feature) -> Vec<ir::ItemRef> {
    let mut rows = ir::Diags::new();
    let others = ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature));
    ir::check_legal(design, &others, &mut rows);
    let mut out: Vec<ir::ItemRef> = rows
        .rows()
        .iter()
        .map(|d| d.item)
        .filter(|&i| i != ir::ItemRef::NONE)
        .collect();
    out.sort_by_key(|i| (i.kind as u8, i.index));
    out.dedup();
    out
}
