//! The network, pole-zero, distortion and timed-report analysis details' facts:
//! `DetailFacts` for `Pz`, `Sp`, `Net`, `Disto`, `TimedOp` of `dc_ac.rs`. Detail rows in;
//! handles, features, field faults, IR text and generated rows out. Their IR text is in
//! `dc_ac_facts.rs`'s header.
//!
//! Cites: `crates/ir/src/dc_ac.rs` docs; `rewrite/IR-RUN.md` §2.
//!
//! Contents: `DetailFacts` for `Pz`, `Sp`, `Net`, `Disto`, `TimedOp`

use crate::analysis_facts::DetailFacts;
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{Disto, Net, Pz, PzFind, PzInput, PzTransfer, Sp, TimedOp};
use crate::dc_ac_facts::output_valid;
use crate::design::Design;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::NameId;
#[cfg(any(test, feature = "gen"))]
use crate::id::ScopeId;
#[cfg(any(test, feature = "gen"))]
use crate::id::{ExprId, List};
use crate::probe::probe_features;
use crate::run_extra::RunExtra;
use crate::run_fields::{EL, N, NL, NN, PROBE, SWEEP};
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{domain_unless, flag};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, sweep_features, sweep_valid};

run_choices!(PzInput, PzTransfer, PzFind);

run_fields! {
    Pz {
        input: Choice("") = PzInput::Nodes, transfer: Choice("") = PzTransfer::Voltage,
        find: Choice("") = PzFind::Poles, in_p: Name = NN, in_n: Name = NN, out_p: Name = NN,
        out_n: Name = NN, source: Element = NN,
    }
    Sp { sweep: Sweep("") = SWEEP, ports: Elements = NL, noise: Word = false }
    Net { output: Probe("") = PROBE, source: Element = NN, rin: Expr = N, rout: Expr = N }
    Disto { sweep: Sweep("") = SWEEP, f2_over_f1: Expr = N }
    TimedOp { times: Exprs = EL }
}

impl DetailFacts for Pz {
    table_facts!();

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        flag(set, self.input == PzInput::Source, RunExtra::PzSourceInput);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        let set = |n: NameId| !n.is_none();
        let input = match self.input {
            PzInput::Nodes => set(self.in_p) && set(self.in_n) && !set(self.source),
            PzInput::Source => !set(self.in_p) && !set(self.in_n) && set(self.source),
        };
        domain_unless(input && set(self.out_p) && set(self.out_n))
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let source_input = has(features, RunExtra::PzSourceInput) && g.chance(2);
        let transfer = PzTransfer::ALL[g.below(2) as usize];
        let find = PzFind::ALL[g.below(3) as usize];
        // Names in the text's order: in_p, in_n, out_p, out_n, source.
        let in_p = if source_input {
            NameId::NONE
        } else {
            g.fresh_name(b)
        };
        let in_n = if source_input {
            NameId::NONE
        } else {
            NameId::GROUND
        };
        let out_p = g.fresh_name(b);
        let source = if source_input {
            g.fresh_name(b)
        } else {
            NameId::NONE
        };
        let input = if source_input {
            PzInput::Source
        } else {
            PzInput::Nodes
        };
        let row = Pz {
            input,
            transfer,
            find,
            in_p,
            in_n,
            out_p,
            out_n: NameId::GROUND,
            source,
        };
        Some(row)
    }
}

impl DetailFacts for Sp {
    table_facts!();

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(set, self.ports.len > 0, RunExtra::SpPorts);
        flag(set, self.noise, RunExtra::SpNoise);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(sweep_valid(&self.sweep))
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let ports = if has(features, RunExtra::SpPorts) && g.chance(2) {
            let p = g.fresh_name(b);
            b.name_ids(&[p])
        } else {
            List::EMPTY
        };
        Some(Sp {
            sweep,
            ports,
            noise: has(features, RunExtra::SpNoise) && g.chance(2),
        })
    }
}

impl DetailFacts for Net {
    table_facts!();

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output) && !self.source.is_none())
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let output = crate::dc_ac_facts::gen_output(g, b, features);
        let source = g.fresh_name(b);
        let rin = if g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let rout = if g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        Some(Net {
            output,
            source,
            rin,
            rout,
        })
    }
}

impl DetailFacts for Disto {
    table_facts!();

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(sweep_valid(&self.sweep))
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let f = if g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        Some(Disto {
            sweep,
            f2_over_f1: f,
        })
    }
}

impl DetailFacts for TimedOp {
    table_facts!();

    fn features(&self, _design: &Design, _set: &mut FeatureSet) {}
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(self.times.len > 0)
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let t = [g.expr(b, features, Reads::Constant)];
        Some(TimedOp { times: b.exprs(&t) })
    }
}
