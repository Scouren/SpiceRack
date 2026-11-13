//! The loop details' facts: `DetailFacts` for `SweepLoop`, `TableLoop` and `MonteCarlo`. Detail
//! rows in; handles, features, field faults, IR text and generated rows out. Their IR text is
//! in `plan_facts.rs`'s header.
//!
//! Cites: `rewrite/IR-RUN.md` §1; `crates/ir/src/plan.rs` docs.
//!
//! Contents: `DetailFacts` for `SweepLoop`, `TableLoop`, `MonteCarlo`

use crate::analysis_facts::DetailFacts;
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::ExprId;
use crate::id::List;
#[cfg(any(test, feature = "gen"))]
use crate::id::ScopeId;
use crate::plan::Target;
#[cfg(any(test, feature = "gen"))]
use crate::plan::gen_target;
use crate::plan::{
    MonteCarlo, Sampler, SweepLoop, TableLoop, TargetUse, Variations, target_features, target_valid,
};
use crate::run_extra::RunExtra;
use crate::run_fields::{EL, N, SWEEP};
use crate::run_words::get;
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, sweep_features, sweep_valid};

run_choices!(Variations, Sampler);

run_fields! {
    SweepLoop { target: Target("") = Target::TEMP, sweep: Sweep("") = SWEEP }
    TableLoop { columns: Targets("") = List::EMPTY, values: Exprs = EL }
    MonteCarlo {
        runs: Expr = N, seed: Expr = N, samples: Exprs = EL,
        variations: Optional("") = Variations::Process, sampler: Optional("") = Sampler::Random,
        nominal: Word = false,
    }
}

impl DetailFacts for SweepLoop {
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        target_features(&self.target, TargetUse::Loop, set);
        sweep_features(&self.sweep, SweepUse::Loop, set);
    }

    fn fault(&self, design: &Design) -> Option<Violation> {
        (!target_valid(&self.target) || !sweep_valid(design, &self.sweep, SweepUse::Loop))
            .then_some(Violation::FieldDomain)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let target = gen_target(g, b, features, TargetUse::Loop)?;
        let sweep = gen_sweep(g, b, features, SweepUse::Loop);
        Some(SweepLoop { target, sweep })
    }
}

impl DetailFacts for TableLoop {
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
