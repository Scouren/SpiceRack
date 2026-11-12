//! The measure rows' facts: `RowFacts` for `Measure`, `DetailFacts` for the six measure
//! details, and the event facts they share. Rows in; handles, features, violations, IR text and
//! generated rows out. A measure is a definition (`DefKind::Measure`, A7): its parse and its
//! generator define it.
//!
//! IR text:
//!
//! ```text
//! measure <name> <analysis kind> trig_targ <event> <event>
//! measure <name> <analysis kind> when <event>
//! measure <name> <analysis kind> find|deriv <probe> <event>
//! measure <name> <analysis kind> stat avg|min|max|pp|rms|integ|min_at|max_at <probe> <event>
//!         <event>
//! measure <name> <analysis kind> param value=v
//! event = (cross rise|fall|cross [last] [skip_initial] <probe> value=v [count=v] [delay=v]
//!         [until=v]) | (at value=v)
//!       | (bound)
//! ```
//!
//! Cites: `crates/ir/src/measure.rs` docs; `rewrite/IR-DECISIONS.md` R14, A7.
//!
//! Contents: `RowFacts` for `Measure`; `DetailFacts` for `MeasureTrigTarg`, `MeasureWhen`,
//! `MeasureFind`, `MeasureDeriv`, `MeasureStat`, `MeasureParam`

use crate::analysis::AnalysisKind;
#[cfg(any(test, feature = "gen"))]
use crate::analysis_facts::gen_of;
use crate::analysis_facts::{
    DetailFacts, dump_of, fault_of, features_of, handles_of, parse_of, report,
};
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, ItemRef, NameId, Ref, ScopeId};
use crate::item::DefKind;
#[cfg(any(test, feature = "gen"))]
use crate::measure::gen_event;
use crate::measure::{
    MeasureDeriv, MeasureFind, MeasureKind, MeasureParam, MeasureStat, MeasureTrigTarg,
    MeasureWhen, Stat, dump_event, event_features, event_handles, event_valid, parse_event, point,
};
use crate::output::Measure;
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{dump_probe, parse_probe, probe_features, probe_handles, probe_valid};
use crate::run_exprs::Roots;
use crate::run_exprs::exprs_of;
use crate::run_extra::RunExtra;
use crate::run_names::Names;
use crate::run_names::names_of;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{domain_unless, flag, put_expr, put_word, take_enum, take_expr, word};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::Ctx;

/// Dispatches a generic call over a measure kind's detail type. Exhaustive.
macro_rules! by_measure {
    ($kind:expr, $f:ident ( $($a:expr),* )) => {
        match $kind {
            MeasureKind::TrigTarg => $f::<MeasureTrigTarg>($($a),*),
            MeasureKind::When => $f::<MeasureWhen>($($a),*),
            MeasureKind::Find => $f::<MeasureFind>($($a),*),
            MeasureKind::Deriv => $f::<MeasureDeriv>($($a),*),
            MeasureKind::Stat => $f::<MeasureStat>($($a),*),
            MeasureKind::Param => $f::<MeasureParam>($($a),*),
        }
    };
}

impl RowFacts for Measure {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.name));
        by_measure!(self.kind, handles_of(design, self.detail, out));
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let row: Measure = design.row(at.index);
        by_measure!(row.kind, exprs_of(design, row.detail, f));
    }

    fn map_element_names(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut [NameId])) {
        let row: Measure = design.row(at.index);
        by_measure!(row.kind, names_of(design, row.detail, f));
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        let x = match self.kind {
            MeasureKind::TrigTarg => RunExtra::MeasTrigTarg,
            MeasureKind::When => RunExtra::MeasWhen,
            MeasureKind::Find => RunExtra::MeasFind,
            MeasureKind::Deriv => RunExtra::MeasDeriv,
            MeasureKind::Stat => RunExtra::MeasStat,
            MeasureKind::Param => RunExtra::MeasParam,
        };
        flag(set, true, x);
        by_measure!(self.kind, features_of(design, self.detail, set));
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if let Some(v) = by_measure!(self.kind, fault_of(design, self.detail)) {
            report(design, at, v, out);
        }
        if self.name.is_none() {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        crate::text_expr::write_name(design.str(self.name), out);
        put_word(out, &word(self.analysis));
        put_word(out, &word(self.kind));
        by_measure!(self.kind, dump_of(design, ctx, self.detail, out));
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let name = words.name(b, diags);
        let Some(analysis) = take_enum(words, &AnalysisKind::ALL, diags) else {
            return ItemRef::NONE;
        };
        let Some(kind) = take_enum(words, &MeasureKind::ALL, diags) else {
            return ItemRef::NONE;
        };
        let Some(detail) = by_measure!(kind, parse_of(words, scope, b, diags)) else {
            return ItemRef::NONE;
        };
        let item = b.push(
            scope,
            Measure {
                name,
                analysis,
                kind,
                detail,
            },
            words.span(),
        );
        define(b, name, scope, item);
        item
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
        let feature = |k: MeasureKind| match k {
            MeasureKind::TrigTarg => RunExtra::MeasTrigTarg,
            MeasureKind::When => RunExtra::MeasWhen,
            MeasureKind::Find => RunExtra::MeasFind,
            MeasureKind::Deriv => RunExtra::MeasDeriv,
            MeasureKind::Stat => RunExtra::MeasStat,
            MeasureKind::Param => RunExtra::MeasParam,
        };
        let kinds: Vec<MeasureKind> = MeasureKind::ALL
            .into_iter()
            .filter(|&k| has(features, feature(k)))
            .collect();
        if kinds.is_empty() {
            return ItemRef::NONE;
        }
        let kind = kinds[g.below(kinds.len() as u32) as usize];
        // The name first: the text interns it before the detail's names.
        let name = g.fresh_name(b);
        let analysis = AnalysisKind::ALL[g.below(AnalysisKind::COUNT as u32) as usize];
        let detail = by_measure!(kind, gen_of(g, scope, b, features))
            .expect("a measure kind with its feature generates");
        let item = b.push(
            scope,
            Measure {
                name,
                analysis,
                kind,
                detail,
            },
            Span::NONE,
        );
        define(b, name, scope, item);
        item
    }
}

impl DetailFacts for MeasureTrigTarg {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        event_handles(&self.trig, out);
        event_handles(&self.targ, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.event(&mut self.trig);
        r.event(&mut self.targ);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.event(&self.trig);
        r.event(&self.targ);
    }
    fn features(&self, design: &Design, set: &mut FeatureSet) {
        event_features(design, &self.trig, set);
        event_features(design, &self.targ, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(point(&self.trig) && point(&self.targ))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_event(design, ctx, &self.trig, out);
        dump_event(design, ctx, &self.targ, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let trig = parse_event(words, scope, b, diags)?;
        Some(MeasureTrigTarg {
            trig,
            targ: parse_event(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        if !has(features, RunExtra::MeasTrigTarg) {
            return None;
        }
        let trig = gen_event(g, b, features, false);
        Some(MeasureTrigTarg {
            trig,
            targ: gen_event(g, b, features, false),
        })
    }
}

impl DetailFacts for MeasureWhen {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        event_handles(&self.at, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.event(&mut self.at);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.event(&self.at);
    }
    fn features(&self, design: &Design, set: &mut FeatureSet) {
        event_features(design, &self.at, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(point(&self.at))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_event(design, ctx, &self.at, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        Some(MeasureWhen {
            at: parse_event(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        has(features, RunExtra::MeasWhen).then(|| MeasureWhen {
            at: gen_event(g, b, features, false),
        })
    }
}

impl DetailFacts for MeasureFind {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.find, out);
        event_handles(&self.at, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.find);
        r.event(&mut self.at);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.find);
        r.event(&self.at);
    }
    fn features(&self, design: &Design, set: &mut FeatureSet) {
        probe_features(&self.find, set);
        event_features(design, &self.at, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(probe_valid(&self.find) && point(&self.at))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.find, out);
        dump_event(design, ctx, &self.at, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let find = parse_probe(words, scope, b, diags)?;
        Some(MeasureFind {
            find,
            at: parse_event(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        if !has(features, RunExtra::MeasFind) {
            return None;
        }
        let find = gen_probe(g, b, features, true);
        Some(MeasureFind {
            find,
            at: gen_event(g, b, features, false),
        })
    }
}

impl DetailFacts for MeasureDeriv {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.signal, out);
        event_handles(&self.at, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.signal);
        r.event(&mut self.at);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.signal);
        r.event(&self.at);
    }
    fn features(&self, design: &Design, set: &mut FeatureSet) {
        probe_features(&self.signal, set);
        event_features(design, &self.at, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(probe_valid(&self.signal) && point(&self.at))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.signal, out);
        dump_event(design, ctx, &self.at, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let signal = parse_probe(words, scope, b, diags)?;
        Some(MeasureDeriv {
            signal,
            at: parse_event(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        if !has(features, RunExtra::MeasDeriv) {
            return None;
        }
        let signal = gen_probe(g, b, features, true);
        Some(MeasureDeriv {
            signal,
            at: gen_event(g, b, features, false),
        })
    }
}

impl DetailFacts for MeasureStat {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.signal, out);
        event_handles(&self.from, out);
        event_handles(&self.to, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.signal);
        r.event(&mut self.from);
        r.event(&mut self.to);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.signal);
        r.event(&self.from);
        r.event(&self.to);
    }
    fn features(&self, design: &Design, set: &mut FeatureSet) {
        flag(
            set,
            matches!(self.stat, Stat::MinAt | Stat::MaxAt),
            RunExtra::MeasStatAt,
        );
        probe_features(&self.signal, set);
        event_features(design, &self.from, set);
        event_features(design, &self.to, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(probe_valid(&self.signal) && event_valid(&self.from) && event_valid(&self.to))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_word(out, &word(self.stat));
        dump_probe(design, ctx, &self.signal, out);
        dump_event(design, ctx, &self.from, out);
        dump_event(design, ctx, &self.to, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let stat = take_enum(words, &Stat::ALL, diags)?;
        let signal = parse_probe(words, scope, b, diags)?;
        let from = parse_event(words, scope, b, diags)?;
        Some(MeasureStat {
            stat,
            signal,
            from,
            to: parse_event(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        if !has(features, RunExtra::MeasStat) {
            return None;
        }
        let n = if has(features, RunExtra::MeasStatAt) {
            Stat::COUNT
        } else {
            6
        };
        let stat = Stat::ALL[g.below(n as u32) as usize]; // the last two are `MeasStatAt`'s
        let signal = gen_probe(g, b, features, true);
        let from = gen_event(g, b, features, true);
        Some(MeasureStat {
            stat,
            signal,
            from,
            to: gen_event(g, b, features, true),
        })
    }
}

impl DetailFacts for MeasureParam {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Expr(self.value));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.expr(&mut self.value);
    }

    fn map_names(&mut self, _r: &mut Names<'_>) {
        // no element name
    }
    fn features(&self, _design: &Design, _set: &mut FeatureSet) {}
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(!self.value.is_none())
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_expr(out, design, ctx, "value", self.value);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        Some(MeasureParam {
            value: take_expr(words, "value", scope, b, diags),
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        has(features, RunExtra::MeasParam).then(|| MeasureParam {
            value: g.expr(b, features, Reads::Constant),
        })
    }
}

// ---- private ----

/// Defines measure `name` in `scope` unless a measure of that name is already there (a
/// malformed text: `check` reports the duplicate through the kernel).
fn define(b: &mut Builder<'_>, name: crate::id::NameId, scope: ScopeId, item: ItemRef) {
    if !name.is_none()
        && !b
            .design()
            .def_index
            .contains_key(&(scope, DefKind::Measure, name))
    {
        b.define(DefKind::Measure, name, scope, item);
    }
}
