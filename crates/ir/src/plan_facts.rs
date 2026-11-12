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
//! elaborate (<subckt> …)                                      A12 R1
//! ```
//!
//! Cites: `rewrite/IR-RUN.md` §1; `rewrite/IR-KERNEL.md` §7.2; `rewrite/IR-DECISIONS.md` R1–R3,
//! R9, R10; `crates/ir/src/plan.rs` docs.
//!
//! Contents: `RowFacts` for `Loop`, `Alter` and `Elaborate` (the loop details' `DetailFacts`:
//! `loop_facts.rs`)

use crate::analysis::{Analysis, AnalysisKind};
#[cfg(any(test, feature = "gen"))]
use crate::analysis_facts::gen_of;
use crate::analysis_facts::{
    detail_get, dump_of, fault_of, features_of, handles_of, parse_of, report,
};
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::design::ItemRow;
use crate::diag::Diags;
use crate::expr::Expr;
#[cfg(any(test, feature = "gen"))]
use crate::expr::ExprTag;
use crate::facts::{Handle, RowFacts};
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::NameId;
use crate::id::{ItemRef, Ref, ScopeId};
use crate::item::{DefKind, ItemKind, ScopeKind};
use crate::plan::{
    Alter, Elaborate, LOOP_DEPTH_MAX, Loop, LoopEnd, LoopKind, MonteCarlo, SweepLoop, TableLoop,
    TargetUse, dump_target, parse_target, target_features, target_handles, target_refs,
    target_refs_into, target_valid,
};
#[cfg(any(test, feature = "gen"))]
use crate::plan::{gen_ref, gen_target, target_kinds};
use crate::run_extra::RunExtra;
use crate::run_words::{
    get, loop_depth, pool, put_expr, put_flag, put_name, take_enum, take_expr, take_name,
    take_word, trim_lead, word, write_ref,
};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::Ctx;

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

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // A sweep's target, or a table's columns in order; a Monte Carlo loop holds none.
        let row: Loop = design.row(at.index);
        match row.kind {
            LoopKind::Sweep => {
                if let Some(d) = <SweepLoop as crate::design::DetailRow>::rows_mut(design)
                    .get_mut(row.detail as usize)
                {
                    target_refs(&mut d.target, f);
                }
            }
            LoopKind::Table => {
                let Some(t) = detail_get::<TableLoop>(design, row.detail) else {
                    return;
                };
                let start = t.columns.start as usize;
                let end = (start + t.columns.len as usize).min(design.run.targets.len());
                for target in &mut design.run.targets[start.min(end)..end] {
                    target_refs(target, f);
                }
            }
            LoopKind::MonteCarlo => {}
        }
    }

    fn refs(&self, design: &Design, _at: ItemRef, out: &mut Vec<(Ref, DefKind)>) {
        // `map_refs`'s order
        match self.kind {
            LoopKind::Sweep => {
                if let Some(d) =
                    <SweepLoop as crate::design::DetailRow>::rows(design).get(self.detail as usize)
                {
                    target_refs_into(&d.target, out);
                }
            }
            LoopKind::Table => {
                let Some(t) = detail_get::<TableLoop>(design, self.detail) else {
                    return;
                };
                let start = t.columns.start as usize;
                let end = (start + t.columns.len as usize).min(design.run.targets.len());
                for target in &design.run.targets[start.min(end)..end] {
                    target_refs_into(target, out);
                }
            }
            LoopKind::MonteCarlo => {}
        }
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
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
        // `LoopScoped` is a fact of the whole design: `analysis::design_features`.
        let body = design.items(self.body);
        let over_op = self.kind == LoopKind::Sweep
            && body.len() == 1
            && body[0].kind == ItemKind::Analysis
            && Analysis::table(design).get(body[0].index).kind == AnalysisKind::Op;
        if over_op {
            add(RunExtra::LoopOverOp);
        }
        if !self.name.is_none() {
            add(RunExtra::LoopNamed);
        }
        if self.kind == LoopKind::Sweep && !one_run(design, self.body) {
            add(RunExtra::SweepBodyNotOne);
        }
        by_loop!(self.kind, features_of(design, self.detail, set));
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if let Some(v) = by_loop!(self.kind, fault_of(design, self.detail)) {
            report(design, at, v, out);
        }
        if loop_depth(design, self.body) > LOOP_DEPTH_MAX {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        out.push_str(&word(self.kind));
        put_flag(out, self.after == LoopEnd::KeepLast, "keep_last");
        put_name(out, design, "name", self.name);
        by_loop!(self.kind, dump_of(design, ctx, self.detail, out));
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
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let has = |x: RunExtra| features.contains(Feature::Run(x));
        let design = b.design();
        let outer = loop_depth(design, scope);
        if outer >= LOOP_DEPTH_MAX {
            return ItemRef::NONE;
        }
        if !has(RunExtra::LoopScoped) && !design.analyses.is_empty() {
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
        let targets = !target_kinds(features, TargetUse::Loop).is_empty();
        // the kernel fills the body: without `SweepBodyNotOne` no sweep loop is safe to open
        let kinds: Vec<LoopKind> = [
            (LoopKind::Sweep, targets && has(RunExtra::SweepBodyNotOne)),
            (LoopKind::Table, targets && has(RunExtra::LoopTable)),
            (LoopKind::MonteCarlo, has(RunExtra::LoopMonteCarlo)),
        ]
        .into_iter()
        .filter(|&(_, ok)| ok)
        .map(|(k, _)| k)
        .collect();
        if kinds.is_empty() {
            return ItemRef::NONE;
        }
        let kind = kinds[g.below(kinds.len() as u32) as usize];
        // The name first: the text interns it before the detail's names.
        let name = if has(RunExtra::LoopNamed) && g.chance(4) {
            g.fresh_name(b)
        } else {
            NameId::NONE
        };
        let detail = by_loop!(kind, gen_of(g, scope, b, features))
            .expect("a possible loop kind's detail generates");
        let row = |body| Loop {
            kind,
            after,
            name,
            detail,
            body,
        };
        let (item, body) = b.push_scoped(scope, ScopeKind::Loop, row, Span::NONE);
        // The kernel's generator fills the body (M4). A sweep over one operating point
        // (`LoopOverOp`) needs its body to start with an `Op`: put it there half the time, so
        // the kind does not hang on the kernel picking it (the fill may still add items after).
        let d = b.design();
        let op_ok = features.contains(Feature::Item(ItemKind::Analysis))
            && features.contains(Feature::Analysis(AnalysisKind::Op))
            && (d.analyses.is_empty() || has(RunExtra::SeveralAnalyses))
            && crate::r#gen::placeable(d, body, ItemKind::Analysis, features)
            // as `Analysis::generate`: without `LoopScoped` an analysis is inside every loop
            && (has(RunExtra::LoopScoped) || loop_depth(d, body) == d.loops.len());
        if kind == LoopKind::Sweep
            && has(RunExtra::LoopOverOp)
            && op_ok
            && g.items_left > 1
            && g.chance(2)
        {
            let op = Analysis {
                kind: AnalysisKind::Op,
                name: NameId::NONE,
                detail: u32::MAX,
            };
            b.push(body, op, Span::NONE);
            g.items_left -= 1;
        }
        item
    }
}

impl RowFacts for Alter {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        target_handles(&self.target, out);
        out.push(Handle::Expr(self.value));
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let mut row: Alter = design.row(at.index);
        target_refs(&mut row.target, f);
        design.editor().set(at.index, row);
    }

    fn refs(&self, _design: &Design, _at: ItemRef, out: &mut Vec<(Ref, DefKind)>) {
        target_refs_into(&self.target, out);
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        target_features(&self.target, TargetUse::Alter, set);
        if !self.value.is_none() && matches!(design.expr(self.value), Expr::Str(_)) {
            *set = set.with(Feature::Run(RunExtra::AlterString));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if !target_valid(&self.target) || self.value.is_none() {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let at = out.len();
        dump_target(design, ctx, &self.target, out);
        put_expr(out, design, ctx, "value", self.value);
        trim_lead(out, at);
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
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let Some(target) = gen_target(g, b, features, TargetUse::Alter) else {
            return ItemRef::NONE;
        };
        let string = features.contains(Feature::Run(RunExtra::AlterString))
            && features.contains(Feature::Expr(ExprTag::Str));
        let value = if string && g.chance(3) {
            let text = g.fresh();
            b.string(&text)
        } else if string {
            g.expr(b, features, Reads::Constant)
        } else {
            // No string anywhere in the value, so its root is none (`AlterString`'s).
            let no_str = features.minus(FeatureSet::EMPTY.with(Feature::Expr(ExprTag::Str)));
            g.expr(b, &no_str, Reads::Constant)
        };
        b.push(scope, Alter { target, value }, Span::NONE)
    }
}

impl RowFacts for Elaborate {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        for &r in get(&design.run.refs, self.circuits).unwrap_or(&[]) {
            out.push(Handle::Ref(r));
        }
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let row: Elaborate = design.row(at.index);
        let start = row.circuits.start as usize;
        let end = (start + row.circuits.len as usize).min(design.run.refs.len());
        for r in &mut design.run.refs[start.min(end)..end] {
            f(r, DefKind::Subckt);
        }
    }

    fn refs(&self, design: &Design, _at: ItemRef, out: &mut Vec<(Ref, DefKind)>) {
        let start = self.circuits.start as usize;
        let end = (start + self.circuits.len as usize).min(design.run.refs.len());
        for r in &design.run.refs[start.min(end)..end] {
            out.push((*r, DefKind::Subckt));
        }
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {}

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        match get(&design.run.refs, self.circuits) {
            None => report(design, at, Violation::HandleRange, out),
            Some([]) => report(design, at, Violation::FieldDomain, out),
            Some(_) => {}
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        out.push('(');
        for (k, &r) in design.refs(self.circuits).iter().enumerate() {
            if k > 0 {
                out.push(' ');
            }
            write_ref(out, design, ctx, r, DefKind::Subckt);
        }
        out.push(')');
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        if !words.expect("(", diags) {
            return ItemRef::NONE;
        }
        let mut refs = Vec::new();
        while words.peek().is_some_and(|w| w != ")") {
            refs.push(words.reference(b, diags));
        }
        if !words.expect(")", diags) {
            return ItemRef::NONE;
        }
        let circuits = pool(&mut b.design.run.refs, &refs);
        b.push(scope, Elaborate { circuits }, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        let n = 1 + g.below(2) as usize;
        let refs: Vec<Ref> = (0..n)
            .map(|_| gen_ref(g, b, Some(DefKind::Subckt)))
            .collect();
        let circuits = pool(&mut b.design.run.refs, &refs);
        b.push(scope, Elaborate { circuits }, Span::NONE)
    }
}

// ---- private ----

/// Whether `body` holds, comments aside, exactly one analysis or one sweep loop (VACASK's
/// one analysis per sweep, A22).
fn one_run(design: &Design, body: ScopeId) -> bool {
    let mut runs = design
        .items(body)
        .iter()
        .filter(|i| i.kind != ItemKind::Comment);
    let one = match (runs.next(), runs.next()) {
        (Some(only), None) => Some(*only),
        _ => None,
    };
    one.is_some_and(|i| {
        i.kind == ItemKind::Analysis
            || (i.kind == ItemKind::Loop
                && Loop::table(design).get(i.index).kind == LoopKind::Sweep)
    })
}
