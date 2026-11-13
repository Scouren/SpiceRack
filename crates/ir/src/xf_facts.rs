//! The output-only DC and small-signal analysis details' facts: `DetailFacts` for `DcXf`,
//! `Sens`, `Xf` and `SensAc` (split from `dc_ac_facts.rs`, whose header shows their IR text).
//! Detail rows in; handles, features, field faults, IR text and generated rows out.
//!
//! Cites: `crates/ir/src/dc_ac.rs` docs; `rewrite/IR-RUN.md` §2.
//!
//! Contents: `DetailFacts` for `DcXf`, `Sens`, `Xf`, `SensAc`

use crate::analysis_facts::DetailFacts;
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{DcXf, Sens, SensAc, Xf};
use crate::dc_ac_facts::output_valid;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::ScopeId;
use crate::probe::{dump_probe, parse_probe, probe_features, probe_handles};
use crate::run_exprs::Roots;
use crate::run_names::Names;
use crate::run_words::domain_unless;
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, dump_sweep, parse_sweep, sweep_features, sweep_handles, sweep_valid};
use crate::text::Words;
use crate::text_expr::Ctx;

impl DetailFacts for DcXf {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        Some(DcXf {
            output: parse_probe(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        Some(DcXf {
            output: crate::dc_ac_facts::gen_output(g, b, features),
        })
    }
}

impl DetailFacts for Sens {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        Some(Sens {
            output: parse_probe(words, scope, b, diags)?,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        Some(Sens {
            output: crate::dc_ac_facts::gen_output(g, b, features),
        })
    }
}

impl DetailFacts for Xf {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        sweep_handles(&self.sweep, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
        r.sweep(&mut self.sweep);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output) && sweep_valid(&self.sweep))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
        dump_sweep(design, ctx, &self.sweep, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let output = parse_probe(words, scope, b, diags)?;
        Some(Xf {
            output,
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
        let output = crate::dc_ac_facts::gen_output(g, b, features);
        Some(Xf {
            output,
            sweep: gen_sweep(g, b, features, SweepUse::Freq),
        })
    }
}

impl DetailFacts for SensAc {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        sweep_handles(&self.sweep, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
        r.sweep(&mut self.sweep);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output) && sweep_valid(&self.sweep))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
        dump_sweep(design, ctx, &self.sweep, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let output = parse_probe(words, scope, b, diags)?;
        Some(SensAc {
            output,
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
        let output = crate::dc_ac_facts::gen_output(g, b, features);
        Some(SensAc {
            output,
            sweep: gen_sweep(g, b, features, SweepUse::Freq),
        })
    }
}
