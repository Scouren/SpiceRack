//! The periodic small-signal and stability details' facts: `DetailFacts` for `Pac`, `Pnoise`,
//! `Pxf`, `Pstb`, `Stb` of `periodic.rs`. Detail rows in; handles, features, field faults, IR
//! text and generated rows out. Their IR text is in `periodic_facts.rs`'s header; a
//! periodic small-signal row's generator only runs after an `Hb` or `Pss` (R5).
//!
//! Cites: `crates/ir/src/periodic.rs` docs; `rewrite/IR-DECISIONS.md` R5.
//!
//! Contents: `DetailFacts` for `Pac`, `Pnoise`, `Pxf`, `Pstb`, `Stb`

use crate::analysis_facts::DetailFacts;
#[cfg(any(test, feature = "gen"))]
use crate::analysis_facts::base_just_before;
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac_facts::output_valid;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, ScopeId};
use crate::periodic::{FreqAxis, Pac, Pnoise, Pstb, Pxf, Stb};
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{dump_probe, parse_probe, probe_features, probe_handles};
use crate::run_exprs::Roots;
use crate::run_extra::RunExtra;
use crate::run_names::Names;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    domain_unless, flag, put_expr, put_flag, put_name, take_expr, take_name, take_word,
};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, dump_sweep, parse_sweep, sweep_features, sweep_handles, sweep_valid};
use crate::text::Words;
use crate::text_expr::Ctx;

impl DetailFacts for Pac {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        sweep_handles(&self.sweep, out);
        out.extend([Handle::Expr(self.sidebands), Handle::Expr(self.relharm)]);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
        r.expr(&mut self.sidebands);
        r.expr(&mut self.relharm);
    }

    fn map_names(&mut self, _r: &mut Names<'_>) {
        // no element name
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(
            set,
            self.axis == FreqAxis::Relative,
            RunExtra::PeriodicRelative,
        );
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(sweep_valid(&self.sweep) && axis_ok(self.axis, self.relharm))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_sweep(design, ctx, &self.sweep, out);
        dump_side(design, ctx, self.sidebands, self.relharm, self.axis, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let sweep = parse_sweep(words, scope, b, diags)?;
        let (sidebands, relharm, axis) = parse_side(words, scope, b, diags);
        Some(Pac {
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        after_base(b, scope)?;
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let (sidebands, relharm, axis) = gen_side(g, b, features);
        Some(Pac {
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
}

impl DetailFacts for Pnoise {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        out.push(Handle::Name(self.source));
        sweep_handles(&self.sweep, out);
        out.extend([Handle::Expr(self.sidebands), Handle::Expr(self.relharm)]);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
        r.sweep(&mut self.sweep);
        r.expr(&mut self.sidebands);
        r.expr(&mut self.relharm);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
        r.one(&mut self.source);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(
            set,
            self.axis == FreqAxis::Relative,
            RunExtra::PeriodicRelative,
        );
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        let ok = output_valid(&self.output) && !self.source.is_none();
        domain_unless(ok && sweep_valid(&self.sweep) && axis_ok(self.axis, self.relharm))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
        put_name(out, design, "source", self.source);
        dump_sweep(design, ctx, &self.sweep, out);
        dump_side(design, ctx, self.sidebands, self.relharm, self.axis, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let output = parse_probe(words, scope, b, diags)?;
        let source = take_name(words, "source", b, diags);
        let sweep = parse_sweep(words, scope, b, diags)?;
        let (sidebands, relharm, axis) = parse_side(words, scope, b, diags);
        Some(Pnoise {
            output,
            source,
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        after_base(b, scope)?;
        let output = gen_probe(g, b, features, true);
        let source = g.fresh_name(b);
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let (sidebands, relharm, axis) = gen_side(g, b, features);
        Some(Pnoise {
            output,
            source,
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
}

impl DetailFacts for Pxf {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        sweep_handles(&self.sweep, out);
        out.extend([Handle::Expr(self.sidebands), Handle::Expr(self.relharm)]);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
        r.sweep(&mut self.sweep);
        r.expr(&mut self.sidebands);
        r.expr(&mut self.relharm);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(
            set,
            self.axis == FreqAxis::Relative,
            RunExtra::PeriodicRelative,
        );
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        let ok = output_valid(&self.output) && sweep_valid(&self.sweep);
        domain_unless(ok && axis_ok(self.axis, self.relharm))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
        dump_sweep(design, ctx, &self.sweep, out);
        dump_side(design, ctx, self.sidebands, self.relharm, self.axis, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let output = parse_probe(words, scope, b, diags)?;
        let sweep = parse_sweep(words, scope, b, diags)?;
        let (sidebands, relharm, axis) = parse_side(words, scope, b, diags);
        Some(Pxf {
            output,
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        after_base(b, scope)?;
        let output = gen_probe(g, b, features, true);
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let (sidebands, relharm, axis) = gen_side(g, b, features);
        Some(Pxf {
            output,
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
}

impl DetailFacts for Pstb {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.probe));
        sweep_handles(&self.sweep, out);
        out.extend([Handle::Expr(self.sidebands), Handle::Expr(self.relharm)]);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
        r.expr(&mut self.sidebands);
        r.expr(&mut self.relharm);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.one(&mut self.probe);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(
            set,
            self.axis == FreqAxis::Relative,
            RunExtra::PeriodicRelative,
        );
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        let ok = !self.probe.is_none() && sweep_valid(&self.sweep);
        domain_unless(ok && axis_ok(self.axis, self.relharm))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_name(out, design, "probe", self.probe);
        dump_sweep(design, ctx, &self.sweep, out);
        dump_side(design, ctx, self.sidebands, self.relharm, self.axis, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let probe = take_name(words, "probe", b, diags);
        let sweep = parse_sweep(words, scope, b, diags)?;
        let (sidebands, relharm, axis) = parse_side(words, scope, b, diags);
        Some(Pstb {
            probe,
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        after_base(b, scope)?;
        let probe = g.fresh_name(b);
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let (sidebands, relharm, axis) = gen_side(g, b, features);
        Some(Pstb {
            probe,
            sweep,
            sidebands,
            relharm,
            axis,
        })
    }
}

impl DetailFacts for Stb {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.probe));
        sweep_handles(&self.sweep, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.one(&mut self.probe);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(!self.probe.is_none() && sweep_valid(&self.sweep))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_name(out, design, "probe", self.probe);
        dump_sweep(design, ctx, &self.sweep, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let probe = take_name(words, "probe", b, diags);
        Some(Stb {
            probe,
            sweep: parse_sweep(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let probe = g.fresh_name(b);
        Some(Stb {
            probe,
            sweep: gen_sweep(g, b, features, SweepUse::Freq),
        })
    }
}

// ---- private ----

/// A relative axis names its harmonic; an absolute one none.
fn axis_ok(axis: FreqAxis, relharm: ExprId) -> bool {
    (axis == FreqAxis::Relative) != relharm.is_none()
}

fn dump_side(
    design: &Design,
    ctx: &Ctx,
    sidebands: ExprId,
    relharm: ExprId,
    axis: FreqAxis,
    out: &mut String,
) {
    put_expr(out, design, ctx, "sidebands", sidebands);
    put_expr(out, design, ctx, "relharm", relharm);
    put_flag(out, axis == FreqAxis::Relative, "relative");
}

fn parse_side(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> (ExprId, ExprId, FreqAxis) {
    let sidebands = take_expr(words, "sidebands", scope, b, diags);
    let relharm = take_expr(words, "relharm", scope, b, diags);
    let axis = if take_word(words, "relative") {
        FreqAxis::Relative
    } else {
        FreqAxis::Absolute
    };
    (sidebands, relharm, axis)
}

#[cfg(any(test, feature = "gen"))]
fn gen_side(g: &mut Gen, b: &mut Builder<'_>, features: &FeatureSet) -> (ExprId, ExprId, FreqAxis) {
    let sidebands = if g.chance(2) {
        g.expr(b, features, Reads::Constant)
    } else {
        ExprId::NONE
    };
    if has(features, RunExtra::PeriodicRelative) && g.chance(2) {
        (
            sidebands,
            g.expr(b, features, Reads::Constant),
            FreqAxis::Relative,
        )
    } else {
        (sidebands, ExprId::NONE, FreqAxis::Absolute)
    }
}

/// `Some` when an `Hb` or `Pss` comes before an item appended to `scope` now.
#[cfg(any(test, feature = "gen"))]
fn after_base(b: &Builder<'_>, scope: ScopeId) -> Option<()> {
    let design = b.design();
    base_just_before(design, scope).then_some(())
}
