//! The run plan rows' facts: `RowFacts` for `Loop` and `Alter`, and the loop details'
//! `DetailFacts` (`SweepLoop`, `TableLoop`, `MonteCarlo`). Rows in; handles, features,
//! violations, IR text and generated rows out.
//!
//! IR text:
//!
//! ```text
//! loop sweep [keep_last] [name=n] <target> <sweep>            body below
//! loop table [keep_last] [name=n] <target> … values=[v, …]    row-major
//! loop monte_carlo [keep_last] [name=n] runs=v [seed=v] [samples=[…]] [mismatch|all]
//!      [latin_hypercube] [nominal]
//! alter <target> value=v
//! ```
//!
//! Cites: `rewrite/IR-RUN.md` §1; `rewrite/IR-KERNEL.md` §7.2; `rewrite/IR-DECISIONS.md` R1–R3,
//! R9, R10; `crates/ir/src/plan.rs` docs.
//!
//! Contents: `RowFacts` for `Loop` and `Alter`; `DetailFacts` for `SweepLoop`, `TableLoop`,
//! `MonteCarlo`

#[cfg(any(test, feature = "gen"))]
use crate::analysis::Analysis;
#[cfg(any(test, feature = "gen"))]
use crate::analysis_facts::gen_of;
use crate::analysis_facts::{
    DetailFacts, analysis_count, dump_of, fault_of, features_of, handles_of, parse_of, report,
};
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::expr::Expr;
#[cfg(any(test, feature = "gen"))]
use crate::expr::ExprTag;
use crate::facts::{Handle, RowFacts};
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::{ExprId, List, NameId};
use crate::id::{ItemRef, Ref, ScopeId};
use crate::item::{DefKind, ItemKind, ScopeKind};
#[cfg(any(test, feature = "gen"))]
use crate::plan::gen_target;
use crate::plan::{
    Alter, LOOP_DEPTH_MAX, Loop, LoopEnd, LoopKind, MonteCarlo, Sampler, SweepLoop, TableLoop,
    TargetUse, Variations, dump_target, parse_target, target_features, target_handles,
    target_valid,
};
use crate::run_extra::RunExtra;
use crate::run_words::{
    get, loop_depth, put_expr, put_exprs, put_flag, put_name, put_word, reachable, take_enum,
    take_expr, take_exprs, take_name, take_word, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{dump_sweep, parse_sweep, sweep_features, sweep_handles, sweep_valid};
use crate::text::Words;

/// Dispatches a generic call over a loop kind's detail type. Exhaustive.
macro_rules! by_loop {
    ($kind:expr, $f:ident ( $($a:expr),* )) => {
        match $kind {
            LoopKind::Sweep => $f::<SweepLoop>($($a),*),
            LoopKind::Table => $f::<TableLoop>($($a),*),
            LoopKind::MonteCarlo => $f::<MonteCarlo>($($a),*),
        }
    };
}

impl RowFacts for Loop {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Name(self.name), Handle::Scope(self.body)]);
        by_loop!(self.kind, handles_of(design, self.detail, out));
    }

    fn map_refs(&mut self, _f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // The loop's targets live in its detail row, out of this hook's reach (run report).
    }

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        let mut add = |x: RunExtra| *set = set.with(Feature::Run(x));
        match self.kind {
            LoopKind::Sweep => {}
            LoopKind::Table => add(RunExtra::LoopTable),
            LoopKind::MonteCarlo => add(RunExtra::LoopMonteCarlo),
        }
        add(match self.after {
            LoopEnd::Restore => RunExtra::LoopRestore,
            LoopEnd::KeepLast => RunExtra::LoopKeepLast,
        });
        if loop_depth(design, self.body) > 1 {
            add(RunExtra::LoopNested);
        }
        if reachable(design, self.body, ItemKind::Analysis) < analysis_count(design) {
            add(RunExtra::LoopScoped);
        }
        by_loop!(self.kind, features_of(design, self.detail, set));
    }

    fn check(&self, design: &Design, at: ItemRef, out: &mut Diags) {
        if let Some(v) = by_loop!(self.kind, fault_of(design, self.detail)) {
            report(design, at, v, out);
        }
        if loop_depth(design, self.body) > LOOP_DEPTH_MAX {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, out: &mut String) {
        out.push_str("loop ");
        out.push_str(&word(self.kind));
        put_flag(out, self.after == LoopEnd::KeepLast, "keep_last");
        put_name(out, design, "name", self.name);
        by_loop!(self.kind, dump_of(design, self.detail, out));
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let Some(kind) = take_enum(words, &LoopKind::ALL, diags) else {
            return ItemRef::NONE;
        };
        let after = if take_word(words, "keep_last") {
            LoopEnd::KeepLast
        } else {
            LoopEnd::Restore
        };
        let name = take_name(words, "name", b, diags);
        let Some(detail) = by_loop!(kind, parse_of(words, scope, b, diags)) else {
            return ItemRef::NONE;
        };
        let span = words.span();
        let row = |body| Loop {
            kind,
            after,
            name,
            detail,
            body,
        };
        b.push_scoped(scope, ScopeKind::Loop, row, span).0
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let has = |x: RunExtra| features.contains(Feature::Run(x));
        let design = b.design();
        let outer = loop_depth(design, scope);
        if (outer > 0 && !has(RunExtra::LoopNested)) || outer >= LOOP_DEPTH_MAX {
            return ItemRef::NONE;
        }
        if !has(RunExtra::LoopScoped) && analysis_count(design) > 0 {
            return ItemRef::NONE;
        }
        let ends: Vec<LoopEnd> = [
            (LoopEnd::Restore, RunExtra::LoopRestore),
            (LoopEnd::KeepLast, RunExtra::LoopKeepLast),
        ]
        .into_iter()
        .filter(|&(_, x)| has(x))
        .map(|(e, _)| e)
        .collect();
        if ends.is_empty() {
            return ItemRef::NONE;
        }
        let after = ends[g.below(ends.len() as u32) as usize];
        let first = g.below(LoopKind::COUNT as u32) as usize;
        let mut detail = None;
        for k in 0..LoopKind::COUNT {
            let kind = LoopKind::ALL[(first + k) % LoopKind::COUNT];
            if let Some(d) = by_loop!(kind, gen_of(g, scope, b, features)) {
                detail = Some((kind, d));
                break;
            }
        }
        let Some((kind, detail)) = detail else {
            return ItemRef::NONE;
        };
        let name = if g.chance(4) {
            g.fresh_name(b)
        } else {
            NameId::NONE
        };
        let row = |body| Loop {
            kind,
            after,
            name,
            detail,
            body,
        };
        let (item, body) = b.push_scoped(scope, ScopeKind::Loop, row, Span::NONE);
        // The body: a few run items while the size budget lasts.
        for _ in 0..1 + g.below(2) {
            if g.budget() == 0 {
                break;
            }
            if g.chance(3) && has(RunExtra::LoopNested) {
                Loop::generate(g, body, b, features);
            } else if g.chance(3) {
                Alter::generate(g, body, b, features);
            } else {
                Analysis::generate(g, body, b, features);
            }
        }
        item
    }
}

impl RowFacts for Alter {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        target_handles(&self.target, out);
        out.push(Handle::Expr(self.value));
    }

    fn map_refs(&mut self, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        if let Some(kind) = self.target.kind.def_kind() {
            f(&mut self.target.a, kind);
        }
    }

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        target_features(&self.target, TargetUse::Alter, set);
        if !self.value.is_none() && matches!(design.expr(self.value), Expr::Str(_)) {
            *set = set.with(Feature::Run(RunExtra::AlterString));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, out: &mut Diags) {
        if !target_valid(&self.target) || self.value.is_none() {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, out: &mut String) {
        out.push_str("alter");
        dump_target(design, &self.target, out);
        put_expr(out, design, "value", self.value);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let Some(target) = parse_target(words, b, diags) else {
            return ItemRef::NONE;
        };
        let value = take_expr(words, "value", scope, b, diags);
        b.push(scope, Alter { target, value }, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        let Some(target) = gen_target(g, b, features, TargetUse::Alter) else {
            return ItemRef::NONE;
        };
        let string = features.contains(Feature::Run(RunExtra::AlterString))
            && features.contains(Feature::Expr(ExprTag::Str));
        let value = if string && g.chance(3) {
            let w = g.fresh_name(b);
            let text = b.design().str(w).to_owned();
            b.string(&text)
        } else {
            g.expr(b, features, Reads::Constant)
        };
        b.push(scope, Alter { target, value }, Span::NONE)
    }
}

impl DetailFacts for SweepLoop {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        target_handles(&self.target, out);
        sweep_handles(&self.sweep, out);
    }

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        target_features(&self.target, TargetUse::Loop, set);
        sweep_features(&self.sweep, set);
    }

    fn fault(&self, _design: &Design) -> Option<Violation> {
        (!target_valid(&self.target) || !sweep_valid(&self.sweep)).then_some(Violation::FieldDomain)
    }

    fn dump(&self, design: &Design, out: &mut String) {
        dump_target(design, &self.target, out);
        dump_sweep(design, &self.sweep, out);
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

    fn dump(&self, design: &Design, out: &mut String) {
        for t in design.targets(self.columns) {
            dump_target(design, t, out);
        }
        put_exprs(out, design, "values", self.values);
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

    fn dump(&self, design: &Design, out: &mut String) {
        put_expr(out, design, "runs", self.runs);
        put_expr(out, design, "seed", self.seed);
        put_exprs(out, design, "samples", self.samples);
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

// ---- private ----
