//! The network, pole-zero, distortion and timed-report analysis details' facts:
//! `DetailFacts` for `Pz`, `Sp`, `Net`, `Disto`, `TimedOp` of `dc_ac.rs`. Detail rows in;
//! handles, features, field faults, IR text and generated rows out. Their IR text is in
//! `dc_ac_facts.rs`'s header.
//!
//! Cites: `crates/ir/src/dc_ac.rs` docs; `rewrite/IR-RUN.md` §2.
//!
//! Contents: `DetailFacts` for `Pz`, `Sp`, `Net`, `Disto`, `TimedOp`

use crate::analysis_facts::DetailFacts;
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{Disto, Net, Pz, PzFind, PzInput, PzTransfer, Sp, TimedOp};
use crate::dc_ac_facts::output_valid;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::{ExprId, List};
use crate::id::{NameId, ScopeId};
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{dump_probe, parse_probe, probe_features, probe_handles};
use crate::run_exprs::Roots;
use crate::run_extra::RunExtra;
use crate::run_names::Names;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    domain_unless, flag, put_expr, put_exprs, put_flag, put_name, put_names, put_word, take_enum,
    take_expr, take_exprs, take_name, take_names, take_word, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::gen_sweep;
use crate::sweep::{SweepUse, dump_sweep, parse_sweep, sweep_features, sweep_handles, sweep_valid};
use crate::text::Words;
use crate::text_expr::Ctx;

impl DetailFacts for Pz {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        for n in [self.in_p, self.in_n, self.out_p, self.out_n, self.source] {
            out.push(Handle::Name(n));
        }
    }

    fn map_exprs(&mut self, _r: &mut Roots<'_>) {
        // no expression field
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.one(&mut self.source); // the nodes are nets, flat
    }
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
    fn dump(&self, design: &Design, _ctx: &Ctx, out: &mut String) {
        put_word(out, &word(self.input));
        put_word(out, &word(self.transfer));
        put_word(out, &word(self.find));
        put_name(out, design, "in_p", self.in_p);
        put_name(out, design, "in_n", self.in_n);
        put_name(out, design, "out_p", self.out_p);
        put_name(out, design, "out_n", self.out_n);
        put_name(out, design, "source", self.source);
    }
    fn parse(
        words: &mut Words<'_>,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let input = take_enum(words, &PzInput::ALL, diags)?;
        let transfer = take_enum(words, &PzTransfer::ALL, diags)?;
        let find = take_enum(words, &PzFind::ALL, diags)?;
        let in_p = take_name(words, "in_p", b, diags);
        let in_n = take_name(words, "in_n", b, diags);
        let out_p = take_name(words, "out_p", b, diags);
        let out_n = take_name(words, "out_n", b, diags);
        let source = take_name(words, "source", b, diags);
        Some(Pz {
            input,
            transfer,
            find,
            in_p,
            in_n,
            out_p,
            out_n,
            source,
        })
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
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        sweep_handles(&self.sweep, out);
        out.push(Handle::Names(self.ports));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
    }

    fn map_names(&mut self, r: &mut Names<'_>) {
        r.each(self.ports);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        sweep_features(&self.sweep, SweepUse::Freq, set);
        flag(set, self.ports.len > 0, RunExtra::SpPorts);
        flag(set, self.noise, RunExtra::SpNoise);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(sweep_valid(&self.sweep))
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        dump_sweep(design, ctx, &self.sweep, out);
        put_names(out, design, "ports", self.ports);
        put_flag(out, self.noise, "noise");
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let sweep = parse_sweep(words, scope, b, diags)?;
        let ports = take_names(words, "ports", b, diags);
        Some(Sp {
            sweep,
            ports,
            noise: take_word(words, "noise"),
        })
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
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        probe_handles(&self.output, out);
        out.extend([
            Handle::Name(self.source),
            Handle::Expr(self.rin),
            Handle::Expr(self.rout),
        ]);
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.probe(&mut self.output);
        r.expr(&mut self.rin);
        r.expr(&mut self.rout);
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
        put_expr(out, design, ctx, "rin", self.rin);
        put_expr(out, design, ctx, "rout", self.rout);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let output = parse_probe(words, scope, b, diags)?;
        let source = take_name(words, "source", b, diags);
        let rin = take_expr(words, "rin", scope, b, diags);
        let rout = take_expr(words, "rout", scope, b, diags);
        Some(Net {
            output,
            source,
            rin,
            rout,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let output = gen_probe(g, b, features, true);
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
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        sweep_handles(&self.sweep, out);
        out.push(Handle::Expr(self.f2_over_f1));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.sweep(&mut self.sweep);
        r.expr(&mut self.f2_over_f1);
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
        put_expr(out, design, ctx, "f2_over_f1", self.f2_over_f1);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let sweep = parse_sweep(words, scope, b, diags)?;
        Some(Disto {
            sweep,
            f2_over_f1: take_expr(words, "f2_over_f1", scope, b, diags),
        })
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
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Exprs(self.times));
    }

    fn map_exprs(&mut self, r: &mut Roots<'_>) {
        r.exprs(self.times);
    }

    fn map_names(&mut self, _r: &mut Names<'_>) {
        // no element name
    }
    fn features(&self, _design: &Design, _set: &mut FeatureSet) {}
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(self.times.len > 0)
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_exprs(out, design, ctx, "times", self.times);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        Some(TimedOp {
            times: take_exprs(words, "times", scope, b, diags),
        })
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
