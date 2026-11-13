//! The output-only DC and small-signal analysis details' facts: `DetailFacts` for `DcXf`,
//! `Sens`, `Xf` and `SensAc` (split from `dc_ac_facts.rs`, whose header shows their IR text).
//! Detail rows in; handles, features, field faults, IR text and generated rows out.
//!
//! Cites: `crates/ir/src/dc_ac.rs` docs; `rewrite/IR-RUN.md` §2.
//!
//! Contents: `DetailFacts` for `DcXf`, `Sens`, `Xf`, `SensAc`

use crate::analysis_facts::DetailFacts;
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{DcXf, Sens, SensAc, Xf};
use crate::dc_ac_facts::output_valid;
use crate::design::Design;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
#[cfg(any(test, feature = "gen"))]
use crate::id::ScopeId;
use crate::probe::probe_features;
use crate::run_fields::{PROBE, SWEEP};
use crate::run_words::domain_unless;
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, sweep_features, sweep_valid};

run_fields! {
    DcXf { output: Probe("") = PROBE }
    Sens { output: Probe("") = PROBE }
    Xf { output: Probe("") = PROBE, sweep: Sweep("") = SWEEP }
    SensAc { output: Probe("") = PROBE, sweep: Sweep("") = SWEEP }
}

impl DetailFacts for DcXf {
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output))
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
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output))
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
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, design: &Design) -> Option<Violation> {
        domain_unless(
            output_valid(&self.output) && sweep_valid(design, &self.sweep, SweepUse::Freq),
        )
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
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, design: &Design) -> Option<Violation> {
        domain_unless(
            output_valid(&self.output) && sweep_valid(design, &self.sweep, SweepUse::Freq),
        )
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
