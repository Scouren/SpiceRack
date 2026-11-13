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
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac_facts::output_valid;
use crate::design::Design;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::ExprId;
#[cfg(any(test, feature = "gen"))]
use crate::id::ScopeId;
use crate::periodic::{FreqAxis, Pac, Pnoise, Pstb, Pxf, Stb};
use crate::probe::probe_features;
use crate::run_extra::RunExtra;
use crate::run_fields::{N, NN, PROBE, SWEEP};
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{domain_unless, flag};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, sweep_features, sweep_valid};

run_choices!(FreqAxis);

run_fields! {
    Pac { sweep: Sweep("") = SWEEP, sidebands: Expr = N, relharm: Expr = N, axis: Optional("") = FreqAxis::Absolute }
    Pnoise {
        output: Probe("") = PROBE, source: Element = NN, sweep: Sweep("") = SWEEP,
        sidebands: Expr = N, relharm: Expr = N, axis: Optional("") = FreqAxis::Absolute,
    }
    Pxf {
        output: Probe("") = PROBE, sweep: Sweep("") = SWEEP, sidebands: Expr = N, relharm: Expr = N,
        axis: Optional("") = FreqAxis::Absolute,
    }
    Pstb {
        probe: Element = NN, sweep: Sweep("") = SWEEP, sidebands: Expr = N, relharm: Expr = N,
        axis: Optional("") = FreqAxis::Absolute,
    }
    Stb { probe: Element = NN, sweep: Sweep("") = SWEEP }
}

impl DetailFacts for Pac {
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
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        after_base(b, scope)?;
        let output = crate::dc_ac_facts::gen_output(g, b, features);
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
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        after_base(b, scope)?;
        let output = crate::dc_ac_facts::gen_output(g, b, features);
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
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(!self.probe.is_none() && sweep_valid(&self.sweep))
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
