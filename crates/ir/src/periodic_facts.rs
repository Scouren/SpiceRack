//! The periodic and stability analysis details' facts: `DetailFacts` for every row of
//! `periodic.rs`. Detail rows in; handles, features, field faults, IR text and generated rows
//! out. A periodic small-signal row's generator only runs after an `Hb` or `Pss` (R5).
//!
//! IR text (after `analysis <kind> [name=n]`):
//!
//! ```text
//! hb     tones=[v, …] harmonics=[v, …] [intermod=v] truncate=box|diamond|hybrid
//! pss    driven|autonomous [uic] fundamental=v tstab=v [harmonics=v] [points=v] [osc_p=n]
//!        [osc_n=n] [sc_iter=v] [steady_coeff=v]
//! pac    <sweep> [sidebands=v] [relharm=v] [relative]
//! pnoise <probe> source=n <sweep> [sidebands=v] [relharm=v] [relative]
//! pxf    <probe> <sweep> [sidebands=v] [relharm=v] [relative]
//! pstb   probe=n <sweep> [sidebands=v] [relharm=v] [relative]
//! stb    probe=n <sweep>
//! ```
//!
//! Cites: `crates/ir/src/periodic.rs` docs; `rewrite/IR-DECISIONS.md` R5.
//!
//! Contents: `DetailFacts` for `Hb`, `Pss` (the small-signal rows: `periodic_ss_facts.rs`)

use crate::analysis_facts::DetailFacts;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::ScopeId;
#[cfg(any(test, feature = "gen"))]
use crate::id::{ExprId, NameId};
use crate::periodic::{Hb, HbTruncate, Pss, PssMode};
use crate::run_extra::RunExtra;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    domain_unless, flag, put_expr, put_exprs, put_flag, put_name, put_word, take_enum, take_expr,
    take_exprs, take_key, take_name, take_word, word,
};
use crate::text::Words;
use crate::text_expr::Ctx;

impl DetailFacts for Hb {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Exprs(self.tones),
            Handle::Exprs(self.harmonics),
            Handle::Expr(self.intermod),
        ]);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        flag(set, self.tones.len > 1, RunExtra::HbMultiTone);
        flag(set, !self.intermod.is_none(), RunExtra::HbIntermod);
        flag(
            set,
            self.truncate == HbTruncate::Diamond,
            RunExtra::HbDiamond,
        );
        flag(set, self.truncate == HbTruncate::Hybrid, RunExtra::HbHybrid);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        domain_unless(self.tones.len > 0 && self.tones.len == self.harmonics.len)
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_exprs(out, design, ctx, "tones", self.tones);
        put_exprs(out, design, ctx, "harmonics", self.harmonics);
        put_expr(out, design, ctx, "intermod", self.intermod);
        put_word(out, &format!("truncate={}", word(self.truncate)));
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let tones = take_exprs(words, "tones", scope, b, diags);
        let harmonics = take_exprs(words, "harmonics", scope, b, diags);
        let intermod = take_expr(words, "intermod", scope, b, diags);
        if !take_key(words, "truncate") {
            words.error(diags);
            return None;
        }
        let truncate = take_enum(words, &HbTruncate::ALL, diags)?;
        Some(Hb {
            tones,
            harmonics,
            intermod,
            truncate,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let n = if has(features, RunExtra::HbMultiTone) && g.chance(2) {
            2
        } else {
            1
        };
        // Each list right after its elements: the parse appends it there.
        let t: Vec<ExprId> = (0..n)
            .map(|_| g.expr(b, features, Reads::Constant))
            .collect();
        let tones = b.exprs(&t);
        let h: Vec<ExprId> = (0..n)
            .map(|_| g.expr(b, features, Reads::Constant))
            .collect();
        let harmonics = b.exprs(&h);
        let intermod = if has(features, RunExtra::HbIntermod) && g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let mut shapes = vec![HbTruncate::Box];
        for (t, x) in [
            (HbTruncate::Diamond, RunExtra::HbDiamond),
            (HbTruncate::Hybrid, RunExtra::HbHybrid),
        ] {
            if has(features, x) {
                shapes.push(t);
            }
        }
        let truncate = shapes[g.below(shapes.len() as u32) as usize];
        Some(Hb {
            tones,
            harmonics,
            intermod,
            truncate,
        })
    }
}

impl DetailFacts for Pss {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        for e in [self.fundamental, self.tstab, self.harmonics, self.points] {
            out.push(Handle::Expr(e));
        }
        out.extend([Handle::Name(self.osc_p), Handle::Name(self.osc_n)]);
    }
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        flag(set, self.mode == PssMode::Driven, RunExtra::PssDriven);
        flag(
            set,
            self.mode == PssMode::Autonomous,
            RunExtra::PssAutonomous,
        );
        flag(set, self.uic, RunExtra::PssUic);
    }
    fn fault(&self, _design: &Design) -> Option<Violation> {
        let osc = !self.osc_p.is_none() && !self.osc_n.is_none();
        let none = self.osc_p.is_none() && self.osc_n.is_none();
        let mode = match self.mode {
            PssMode::Driven => none,
            PssMode::Autonomous => osc,
        };
        domain_unless(mode && !self.fundamental.is_none() && !self.tstab.is_none())
    }
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        put_word(out, &word(self.mode));
        put_flag(out, self.uic, "uic");
        put_expr(out, design, ctx, "fundamental", self.fundamental);
        put_expr(out, design, ctx, "tstab", self.tstab);
        put_expr(out, design, ctx, "harmonics", self.harmonics);
        put_expr(out, design, ctx, "points", self.points);
        put_name(out, design, "osc_p", self.osc_p);
        put_name(out, design, "osc_n", self.osc_n);
    }
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self> {
        let mode = take_enum(words, &PssMode::ALL, diags)?;
        let uic = take_word(words, "uic");
        let fundamental = take_expr(words, "fundamental", scope, b, diags);
        let tstab = take_expr(words, "tstab", scope, b, diags);
        let harmonics = take_expr(words, "harmonics", scope, b, diags);
        let points = take_expr(words, "points", scope, b, diags);
        let osc_p = take_name(words, "osc_p", b, diags);
        let osc_n = take_name(words, "osc_n", b, diags);
        Some(Pss {
            mode,
            uic,
            fundamental,
            tstab,
            harmonics,
            points,
            osc_p,
            osc_n,
            sc_iter: ExprId::NONE,
            steady_coeff: ExprId::NONE,
        })
    }
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let mut modes = Vec::new();
        for (m, x) in [
            (PssMode::Driven, RunExtra::PssDriven),
            (PssMode::Autonomous, RunExtra::PssAutonomous),
        ] {
            if has(features, x) {
                modes.push(m);
            }
        }
        if modes.is_empty() {
            return None;
        }
        let mode = modes[g.below(modes.len() as u32) as usize];
        let fundamental = g.expr(b, features, Reads::Constant);
        let tstab = g.expr(b, features, Reads::Constant);
        let harmonics = if g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let points = if g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let (osc_p, osc_n) = match mode {
            PssMode::Driven => (NameId::NONE, NameId::NONE),
            PssMode::Autonomous => (g.fresh_name(b), NameId::GROUND),
        };
        let uic = has(features, RunExtra::PssUic) && g.chance(2);
        Some(Pss {
            mode,
            uic,
            fundamental,
            tstab,
            harmonics,
            points,
            osc_p,
            osc_n,
            sc_iter: ExprId::NONE,
            steady_coeff: ExprId::NONE,
        })
    }
}
