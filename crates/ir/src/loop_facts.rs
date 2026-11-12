//! The loop details' facts: `DetailFacts` for `SweepLoop`, `TableLoop` and `MonteCarlo`. Detail
//! rows in; handles, features, field faults, IR text and generated rows out. Their IR text is
//! in `plan_facts.rs`'s header.
//!
//! Cites: `rewrite/IR-RUN.md` §1; `crates/ir/src/plan.rs` docs.
//!
//! Contents: `DetailFacts` for `SweepLoop`, `TableLoop`, `MonteCarlo`

use crate::analysis_facts::DetailFacts;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::ScopeId;
#[cfg(any(test, feature = "gen"))]
use crate::id::{ExprId, List};
#[cfg(any(test, feature = "gen"))]
use crate::plan::gen_target;
use crate::plan::{
    MonteCarlo, Sampler, SweepLoop, TableLoop, TargetUse, Variations, dump_target, parse_target,
    target_features, target_handles, target_valid,
};
use crate::run_exprs::Roots;
use crate::run_extra::RunExtra;
use crate::run_words::{
    get, put_expr, put_exprs, put_flag, put_word, take_expr, take_exprs, take_word, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{dump_sweep, parse_sweep, sweep_features, sweep_handles, sweep_valid};
use crate::text::Words;
use crate::text_expr::Ctx;

impl DetailFacts for SweepLoop {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        target_handles(&self.target, out);
        sweep_handles(&self.sweep, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
    }

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        target_features(&self.target, TargetUse::Loop, set);
        sweep_features(&self.sweep, set);
    }

    fn fault(&self, _design: &Design) -> Option<Violation> {
        (!target_valid(&self.target) || !sweep_valid(&self.sweep)).then_some(Violation::FieldDomain)
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_target(design, ctx, &self.target, out);
        dump_sweep(design, ctx, &self.sweep, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let target = parse_target(words, b, diags)?;
        let sweep = parse_sweep(words, scope, b, diags)?;
        Some(SweepLoop { target, sweep })
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let target = gen_target(g, b, features, TargetUse::Loop)?;
        let sweep = gen_sweep(g, b, features);
        Some(SweepLoop { target, sweep })
    }
}

impl DetailFacts for TableLoop {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        for t in get(&design.run.targets, self.columns).unwrap_or(&[]) {
            target_handles(t, out);
        }
        out.push(Handle::Exprs(self.values));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.exprs(self.values);
    }

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        for t in get(&design.run.targets, self.columns).unwrap_or(&[]) {
            target_features(t, TargetUse::Loop, set);
        }
    }

    fn fault(&self, design: &Design) -> Option<Violation> {
        let Some(columns) = get(&design.run.targets, self.columns) else {
            return Some(Violation::HandleRange);
        };
        let n = columns.len() as u32;
        let rows_ok = n > 0 && self.values.len >= n && self.values.len.is_multiple_of(n);
        (!rows_ok || !columns.iter().all(target_valid)).then_some(Violation::FieldDomain)
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        for t in design.targets(self.columns) {
            dump_target(design, ctx, t, out);
        }
        put_exprs(out, design, ctx, "values", self.values);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let mut columns = Vec::new();
        while words.peek() == Some("(") {
            columns.push(parse_target(words, b, diags)?);
        }
        let values = take_exprs(words, "values", scope, b, diags);
        let columns = b.targets(&columns);
        Some(TableLoop { columns, values })
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        if !features.contains(Feature::Run(RunExtra::LoopTable)) {
            return None;
        }
        let n = 1 + g.below(2) as usize;
        let mut columns = Vec::with_capacity(n);
        for _ in 0..n {
            columns.push(gen_target(g, b, features, TargetUse::Loop)?);
        }
        let rows = 1 + g.below(3) as usize;
        let values: Vec<ExprId> = (0..rows * n)
            .map(|_| g.expr(b, features, Reads::Constant))
            .collect();
        let values = b.exprs(&values);
        let columns = b.targets(&columns);
        Some(TableLoop { columns, values })
    }
}

impl DetailFacts for MonteCarlo {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Expr(self.runs),
            Handle::Expr(self.seed),
            Handle::Exprs(self.samples),
        ]);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.expr(&mut self.runs);
        r.expr(&mut self.seed);
        r.exprs(self.samples);
    }

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        let mut add = |on: bool, x: RunExtra| {
            if on {
                *set = set.with(Feature::Run(x));
            }
        };
        add(!self.seed.is_none(), RunExtra::McSeed);
        add(self.samples.len > 0, RunExtra::McSampleList);
        add(
            self.sampler == Sampler::LatinHypercube,
            RunExtra::McLatinHypercube,
        );
        add(self.nominal, RunExtra::McNominal);
        add(self.variations != Variations::Process, RunExtra::McMismatch);
    }

    fn fault(&self, _design: &Design) -> Option<Violation> {
        self.runs.is_none().then_some(Violation::FieldDomain)
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_expr(out, design, ctx, "runs", self.runs);
        put_expr(out, design, ctx, "seed", self.seed);
        put_exprs(out, design, ctx, "samples", self.samples);
        if self.variations != Variations::Process {
            put_word(out, &word(self.variations));
        }
        put_flag(
            out,
            self.sampler == Sampler::LatinHypercube,
            "latin_hypercube",
        );
        put_flag(out, self.nominal, "nominal");
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let runs = take_expr(words, "runs", scope, b, diags);
        let seed = take_expr(words, "seed", scope, b, diags);
        let samples = take_exprs(words, "samples", scope, b, diags);
        let variations = if take_word(words, "mismatch") {
            Variations::Mismatch
        } else if take_word(words, "all") {
            Variations::All
        } else {
            Variations::Process
        };
        let sampler = if take_word(words, "latin_hypercube") {
            Sampler::LatinHypercube
        } else {
            Sampler::Random
        };
        let nominal = take_word(words, "nominal");
        Some(MonteCarlo {
            runs,
            seed,
            samples,
            variations,
            sampler,
            nominal,
        })
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let has = |x: RunExtra| features.contains(Feature::Run(x));
        if !has(RunExtra::LoopMonteCarlo) {
            return None;
        }
        let runs = g.expr(b, features, Reads::Constant);
        let seed = if has(RunExtra::McSeed) && g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let samples = if has(RunExtra::McSampleList) && g.chance(2) {
            let s = [g.expr(b, features, Reads::Constant)];
            b.exprs(&s)
        } else {
            List::EMPTY
        };
        let variations = if has(RunExtra::McMismatch) && g.chance(2) {
            [Variations::Mismatch, Variations::All][g.below(2) as usize]
        } else {
            Variations::Process
        };
        let sampler = if has(RunExtra::McLatinHypercube) && g.chance(2) {
            Sampler::LatinHypercube
        } else {
            Sampler::Random
        };
        let nominal = has(RunExtra::McNominal) && g.chance(2);
        Some(MonteCarlo {
            runs,
            seed,
            samples,
            variations,
            sampler,
            nominal,
        })
    }
}
