//! The DC and small-signal analysis details' facts: `DetailFacts` for every row of `dc_ac.rs`.
//! Detail rows in; handles, features, field faults, IR text and generated rows out.
//!
//! IR text (after `analysis <kind> [name=n]`; fields in declaration order):
//!
//! ```text
//! dc       <target> <sweep> [name=n] { <target> <sweep> [name=n] } [restart]
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
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{Ac, Dc, DcAxis, Noise, NoiseRef, Tf};
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::ExprId;
use crate::id::ScopeId;
#[cfg(any(test, feature = "gen"))]
use crate::plan::gen_target;
use crate::plan::{
    TargetUse, dump_target, parse_target, target_features, target_handles, target_valid,
};
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{
    AcPart, Probe, ProbeKind, dump_probe, parse_probe, probe_features, probe_handles, probe_valid,
};
use crate::run_exprs::Roots;
use crate::run_extra::RunExtra;
use crate::run_names::Names;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    domain_unless, flag, get, put_expr, put_flag, put_name, take_expr, take_name, take_word,
};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, dump_sweep, parse_sweep, sweep_features, sweep_handles, sweep_valid};
use crate::text::Words;
use crate::text_expr::Ctx;

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

impl DetailFacts for Dc {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        for axis in get(&design.run.dc_axes, self.axes).unwrap_or(&[]) {
            target_handles(&axis.target, out);
            sweep_handles(&axis.sweep, out);
            out.push(Handle::Name(axis.name));
        }
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.dc_axes(self.axes);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.dc_axes(self.axes);
    }

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        let axes = get(&design.run.dc_axes, self.axes).unwrap_or(&[]);
        flag(set, axes.len() > 1, RunExtra::DcNested);
        flag(set, self.restart, RunExtra::DcRestart);
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

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        for axis in design.dc_axes(self.axes) {
            dump_target(design, ctx, &axis.target, out);
            dump_sweep(design, ctx, &axis.sweep, out);
            put_name(out, design, "name", axis.name);
        }
        put_flag(out, self.restart, "restart");
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let mut axes = Vec::new();
        while words.peek() == Some("(") {
            let target = parse_target(words, b, diags)?;
            let sweep = parse_sweep(words, scope, b, diags)?;
            let name = take_name(words, "name", b, diags);
            axes.push(DcAxis {
                target,
                sweep,
                name,
            });
        }
        let restart = take_word(words, "restart");
        Some(Dc {
            axes: b.dc_axes(&axes),
            restart,
        })
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let n = if has(features, RunExtra::DcNested) && g.chance(3) {
            2
        } else {
            1
        };
        let mut axes = Vec::with_capacity(n);
        for _ in 0..n {
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
            axes.push(DcAxis {
                target,
                sweep,
                name,
            });
        }
        let restart = has(features, RunExtra::DcRestart) && g.chance(2);
        Some(Dc {
            axes: b.dc_axes(&axes),
            restart,
        })
    }
}

impl DetailFacts for Ac {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        sweep_handles(&self.sweep, out);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
    }

    fn map_names(&mut self, _r: &mut Names<'_>) {
        // no element name
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(sweep_valid(&self.sweep))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_sweep(design, ctx, &self.sweep, out);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        Some(Ac {
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
        Some(Ac {
            sweep: gen_sweep(g, b, features, SweepUse::Freq),
        })
    }
}

impl DetailFacts for Noise {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        out.push(Handle::Name(self.source));
        sweep_handles(&self.sweep, out);
        out.push(Handle::Expr(self.points_per_summary));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
        r.sweep(&mut self.sweep);
        r.expr(&mut self.points_per_summary);
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
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
        put_name(out, design, "source", self.source);
        dump_sweep(design, ctx, &self.sweep, out);
        put_expr(
            out,
            design,
            ctx,
            "points_per_summary",
            self.points_per_summary,
        );
        put_flag(out, self.ac_sweep, "ac_sweep");
        put_flag(out, self.input_ref == NoiseRef::SourceAc, "source_ac");
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
        let points_per_summary = take_expr(words, "points_per_summary", scope, b, diags);
        let ac_sweep = take_word(words, "ac_sweep");
        let input_ref = if take_word(words, "source_ac") {
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
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        out.push(Handle::Name(self.source));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.probe(&self.output);
        r.one(&mut self.source);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        probe_features(&self.output, set);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(output_valid(&self.output) && !self.source.is_none())
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_probe(design, ctx, &self.output, out);
        put_name(out, design, "source", self.source);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let output = parse_probe(words, scope, b, diags)?;
        Some(Tf {
            output,
            source: take_name(words, "source", b, diags),
        })
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
