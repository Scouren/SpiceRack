//! `analysis_param_args_to_values` by the reference evaluator: over generated designs holding
//! a parameter read in an analysis's or loop's argument, every constant field of every analysis
//! and loop evaluates to the same value (bit for bit, or to no value both times) before and
//! after the rule, each parameter read bound by its definition's value; and some read is
//! written out (the law runs on something).
//!
//! Contents: `pub fn param_arguments_keep_every_field_value(Range<u64>, Size)`

use std::cell::Cell;

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::passes::run::param_arguments::{PARAM_ARG_KEPT, analysis_param_args_to_values};

/// The fewest fields the rule must rewrite over the seeds for the same-bits check to mean
/// something: the built designs alone give six a seed (96 seeds: 576); the run on 0f4625f1's
/// tree gave 586 with the generated ones.
const REWRITTEN_FLOOR: usize = 576;

/// A parameter read is its definition's value, evaluated; nothing else is bound.
struct Follow<'d> {
    design: &'d ir::Design,
    depth: Cell<u32>,
}

impl ir::Bindings for Follow<'_> {
    fn param(&self, p: ir::Ref) -> Option<f64> {
        if !p.is_resolved() || self.depth.get() > 64 {
            return None;
        }
        let def = self.design.def(p.def);
        if def.kind != ir::DefKind::Param || def.item.kind != ir::ItemKind::Param {
            return None;
        }
        let value = self.design.row::<ir::item::Param>(def.item.index).value;
        self.depth.set(self.depth.get() + 1);
        let v = ir::eval(self.design, value, self).ok();
        self.depth.set(self.depth.get() - 1);
        v
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        None
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        None
    }
    fn time(&self) -> Option<f64> {
        None
    }
    fn temp(&self) -> Option<f64> {
        None
    }
    fn freq(&self) -> Option<f64> {
        None
    }
    fn tstep(&self) -> Option<f64> {
        None
    }
    fn tstop(&self) -> Option<f64> {
        None
    }
    fn tnom(&self) -> Option<f64> {
        None
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, _: &[f64]) -> Option<f64> {
        None
    }
}

/// Each field before and after, and a count of the items converted.
pub fn param_arguments_keep_every_field_value(seeds: std::ops::Range<u64>, size: Size) {
    let feature = ir::Feature::Run(ir::RunExtra::AnalysisParamArgument);
    let (mut ran, mut converted, mut rewritten) = (0, 0, 0);
    // the generator reads a fresh name in an analysis's argument far more often than a defined
    // parameter, so the designs the rule converts are also built here, one a seed: constant
    // top-level parameters read by a transient's, a DC axis's and a sweep loop's arguments
    let sets = [ir::FeatureSet::ALL, crate::gen_sets::PARAM_ARGS];
    let generated = sets
        .iter()
        .flat_map(|s| seeds.clone().map(move |k| (k, generate(k, s, size))));
    let built = seeds.clone().map(|k| (k, convertible(k)));
    for (seed, mut design) in generated.chain(built) {
        if !ir::features_used(&design).contains(feature) {
            continue;
        }
        ran += 1;
        let before = fields(&mut design);
        let mut diags = ir::Diags::new();
        analysis_param_args_to_values(&mut design, &mut diags);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "seed {seed}: {:?}", found.rows());
        let after = fields(&mut design);
        let values =
            |f: &[(ir::ItemRef, ir::ExprId, Option<u64>)]| -> Vec<(ir::ItemRef, Option<u64>)> {
                f.iter().map(|&(i, _, v)| (i, v)).collect()
            };
        assert_eq!(
            values(&before),
            values(&after),
            "seed {seed}: a field's value changed"
        );
        rewritten += before
            .iter()
            .zip(&after)
            .filter(|(b, a)| b.1 != a.1)
            .count();
        let kept = diags
            .rows()
            .iter()
            .filter(|d| d.code == PARAM_ARG_KEPT)
            .count();
        converted += usize::from(!ir::features_used(&design).contains(feature) || kept == 0);
    }
    eprintln!(
        "analysis_param_args_to_values: {ran} designs, {converted} fully written out, {rewritten} fields rewritten"
    );
    assert!(ran > 0 && converted > 0, "the law ran on nothing converted");
    assert!(
        rewritten >= REWRITTEN_FLOOR,
        "{rewritten} fields rewritten, under the floor {REWRITTEN_FLOOR}: the oracle sees too little"
    );
}

// ---- private ----

/// A design the rule converts: two constant top-level parameters (values drawn from `seed`)
/// read in a transient's, a DC axis's and a sweep loop's arguments, through operators.
fn convertible(seed: u64) -> ir::Design {
    let a = 1 + seed % 9;
    let b = 1 + seed % 4;
    let text = format!(
        "file \"a.sp\" deck\n  title \"t\"\n  param p1 = {a}u\n  param p2 = {b}n\n  param q = 1\n  element vsource v1 (in 0) dc=1\n  analysis tran stop=(p1 * 2) print_step=p2 cap=none\n  analysis dc (source_dc v1) (lin start=0 stop=p1 step=p2)\n  loop sweep (param q) (list values=[p1, (p2 + 1n)])\n    analysis op\n"
    );
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(&text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "seed {seed}: {:?}\n{text}", diags.rows());
    design
}

/// Every constant root of every reachable analysis and loop, with its value as bits.
fn fields(design: &mut ir::Design) -> Vec<(ir::ItemRef, ir::ExprId, Option<u64>)> {
    let items: Vec<ir::ItemRef> = design
        .reachable()
        .into_iter()
        .map(|(i, _)| i)
        .filter(|i| matches!(i.kind, ir::ItemKind::Analysis | ir::ItemKind::Loop))
        .collect();
    let mut out = Vec::new();
    for item in items {
        let mut roots = Vec::new();
        design
            .editor()
            .map_exprs(item, &mut |e, u| roots.push((*e, u)));
        for (root, use_) in roots {
            if use_ == ir::ExprUse::Constant && !root.is_none() {
                let env = Follow {
                    design,
                    depth: Cell::new(0),
                };
                let v = ir::eval(design, root, &env).ok().map(f64::to_bits);
                out.push((item, root, v));
            }
        }
    }
    out
}
