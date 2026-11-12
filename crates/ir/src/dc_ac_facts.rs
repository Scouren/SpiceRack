//! The DC and small-signal analysis details' facts: `DetailFacts` for every row of `dc_ac.rs`.
//! Detail rows in; handles, features, field faults, IR text and generated rows out.
//!
//! IR text (after `analysis <kind> [name=n]`; fields in declaration order):
//!
//! ```text
//! dc       <target> <sweep> [name=n] [afresh] { <target> <sweep> [name=n] [afresh] }
//! ac       <sweep>
//! noise    <probe> source=n <sweep> [points_per_summary=v] [ac_sweep] [source_ac]
//! tf       <probe> source=n          dc_xf <probe>          xf <probe> <sweep>
//! sens     <probe>                   sens_ac <probe> <sweep>
//! pz       nodes|source voltage|current poles|zeros|both [in_p=n] [in_n=n] out_p=n out_n=n
//!          [source=n]
//! sp       <sweep> [ports=(n …)] [noise]
//! net      <probe> source=n [rin=v] [rout=v]
//! disto    <sweep> [f2_over_f1=v]
//! timed_op times=[v, …]
//! ```
//!
//! Cites: `crates/ir/src/dc_ac.rs` docs; `rewrite/IR-RUN.md` §2.
//!
//! Contents: `DetailFacts` for `Dc`, `Ac`, `Noise`, `Tf` (`DcXf`, `Xf`, `Sens`, `SensAc`:
//! `xf_facts.rs`; `Pz`, `Sp`, `Net`, `Disto`, `TimedOp`: `network_facts.rs`);
//! `pub(crate) fn output_valid`, `pub(crate) fn gen_output` (gen)

use crate::analysis_facts::DetailFacts;
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{Ac, Dc, DcAxis, Noise, NoiseRef, Tf};
use crate::design::Design;
use crate::feature::FeatureSet;
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
use crate::plan::{TargetUse, target_features, target_valid};
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{AcPart, Probe, ProbeKind, probe_features, probe_valid};
use crate::run_extra::RunExtra;
use crate::run_fields::{N, NN, PROBE, SWEEP};
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{domain_unless, flag, get};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, sweep_features, sweep_valid};

/// Whether `p` is a valid analysis output: a node voltage, a voltage between nodes or a branch
/// current, with no part (R68/A42: the dB of a noise or transfer output means nothing on any
/// simulator we run).
pub(crate) fn output_valid(p: &Probe) -> bool {
    probe_valid(p)
        && matches!(p.kind, ProbeKind::V | ProbeKind::Vdiff | ProbeKind::I)
        && p.part == AcPart::None
}

/// A generated analysis output: a node voltage, a voltage between nodes or a branch current
/// the set allows, with no part (`output_valid`).
#[cfg(any(test, feature = "gen"))]
pub(crate) fn gen_output(g: &mut Gen, b: &mut Builder<'_>, features: &FeatureSet) -> Probe {
    let mut p = gen_probe(g, b, features, true);
    p.part = AcPart::None;
    p
}

run_choices!(NoiseRef);

run_fields! {
    Dc { axes: DcAxes("") = List::EMPTY }
    DcAxis { target: Target("") = Target::TEMP, sweep: Sweep("") = SWEEP, name: Name = NN, afresh: Word = false }
    Ac { sweep: Sweep("") = SWEEP }
    Noise {
        output: Probe("") = PROBE, source: Element = NN, sweep: Sweep("") = SWEEP,
        points_per_summary: Expr = N, ac_sweep: Word = false, input_ref: Optional("") = NoiseRef::Unit,
    }
    Tf { output: Probe("") = PROBE, source: Element = NN }
}

impl DetailFacts for Dc {
    table_facts!();

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        let axes = get(&design.run.dc_axes, self.axes).unwrap_or(&[]);
        flag(set, axes.len() > 1, RunExtra::DcNested);
        if let Some((inner, outer)) = axes.split_first() {
            flag(set, inner.afresh, RunExtra::DcRestart);
            for axis in outer {
                flag(set, axis.afresh, RunExtra::DcOuterAfresh);
                flag(set, !axis.afresh, RunExtra::DcOuterCarries);
            }
        }
        for axis in axes {
            flag(set, !axis.name.is_none(), RunExtra::DcAxisNamed);
            flag(set, axis.name.is_none(), RunExtra::DcAxisUnnamed);
            target_features(&axis.target, TargetUse::Dc, set);
            sweep_features(&axis.sweep, SweepUse::Dc, set);
        }
    }

    fn fault(&self, design: &Design) -> Option<Violation> {
        let Some(axes) = get(&design.run.dc_axes, self.axes) else {
            return Some(Violation::HandleRange);
        };
        let ok = !axes.is_empty()
            && axes
                .iter()
                .all(|a| target_valid(&a.target) && sweep_valid(&a.sweep));
        domain_unless(ok)
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let outer = [RunExtra::DcOuterCarries, RunExtra::DcOuterAfresh].map(|x| has(features, x));
        let n = if has(features, RunExtra::DcNested) && outer.contains(&true) && g.chance(3) {
            2
        } else {
            1
        };
        let mut axes = Vec::with_capacity(n);
        for k in 0..n {
            let target = gen_target(g, b, features, TargetUse::Dc)?;
            let sweep = gen_sweep(g, b, features, SweepUse::Dc);
            // after the sweep: the text interns the name after the sweep's values
            let name = crate::run_words::gen_name(
                g,
                b,
                features,
                RunExtra::DcAxisNamed,
                RunExtra::DcAxisUnnamed,
                3,
            )?;
            let afresh = match (k, outer) {
                (0, _) => has(features, RunExtra::DcRestart) && g.chance(2),
                (_, [true, true]) => g.chance(2),
                (_, [carries, _]) => !carries,
            };
            axes.push(DcAxis {
                target,
                sweep,
                name,
                afresh,
            });
        }
        Some(Dc {
            axes: b.dc_axes(&axes),
        })
    }
}

impl DetailFacts for Ac {
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
        Some(Ac {
            sweep: gen_sweep(g, b, features, SweepUse::Freq),
        })
    }
}

impl DetailFacts for Noise {
    table_facts!();

    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(
            set,
            !self.points_per_summary.is_none(),
            RunExtra::NoiseSummary,
        );
        flag(set, self.ac_sweep, RunExtra::NoiseAcSweep);
        flag(
            set,
            self.input_ref == NoiseRef::SourceAc,
            RunExtra::NoiseRefSourceAc,
        );
        flag(
            set,
            self.output.kind == ProbeKind::I,
            RunExtra::NoiseCurrentOutput,
        );
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(
            output_valid(&self.output) && !self.source.is_none() && sweep_valid(&self.sweep),
        )
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let mut output = gen_output(g, b, features);
        if output.kind == ProbeKind::I && !has(features, RunExtra::NoiseCurrentOutput) {
            output.kind = ProbeKind::V;
        }
        let source = g.fresh_name(b);
        let sweep = gen_sweep(g, b, features, SweepUse::Freq);
        let points_per_summary = if has(features, RunExtra::NoiseSummary) && g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let ac_sweep = has(features, RunExtra::NoiseAcSweep) && g.chance(2);
        let source_ac = has(features, RunExtra::NoiseRefSourceAc) && g.chance(2);
        let input_ref = if source_ac {
            NoiseRef::SourceAc
        } else {
            NoiseRef::Unit
        };
        Some(Noise {
            output,
            source,
            sweep,
            points_per_summary,
            ac_sweep,
            input_ref,
        })
    }
}

impl DetailFacts for Tf {
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
        let output = gen_output(g, b, features);
        Some(Tf {
            output,
            source: g.fresh_name(b),
        })
    }
}
