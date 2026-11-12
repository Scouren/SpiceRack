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
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::ExprId;
use crate::id::NameId;
#[cfg(any(test, feature = "gen"))]
use crate::id::ScopeId;
use crate::periodic::{Hb, HbTruncate, Pss, PssMode};
use crate::run_extra::RunExtra;
use crate::run_fields::{EL, N, NN};
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{domain_unless, flag};

run_choices!(HbTruncate, PssMode);

run_fields! {
    Hb { tones: Exprs = EL, harmonics: Exprs = EL, intermod: Expr = N, truncate: Choice = HbTruncate::Box }
    Pss {
        mode: Choice("") = PssMode::Driven, uic: Word = false, fundamental: Expr = N,
        tstab: Expr = N, harmonics: Expr = N, points: Expr = N, osc_p: Name = NN, osc_n: Name = NN,
        sc_iter: Expr = N, steady_coeff: Expr = N,
    }
}

impl DetailFacts for Hb {
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
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        flag(set, self.mode == PssMode::Driven, RunExtra::PssDriven);
        flag(
            set,
            self.mode == PssMode::Autonomous,
            RunExtra::PssAutonomous,
        );
        flag(set, self.uic, RunExtra::PssUic);
        flag(set, !self.sc_iter.is_none(), RunExtra::PssScIter);
        flag(set, !self.steady_coeff.is_none(), RunExtra::PssSteadyCoeff);
        // R68/A42: one member for any of the four left out (R58: split on evidence only)
        let four = [self.harmonics, self.points, self.sc_iter, self.steady_coeff];
        flag(
            set,
            four.iter().any(|e| e.is_none()),
            RunExtra::PssArgumentOmitted,
        );
        flag(
            set,
            self.mode == PssMode::Autonomous && self.osc_n != NameId::GROUND,
            RunExtra::PssOscDiff,
        );
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
        // R68/A42: without `PssArgumentOmitted` all four optional arguments are given, which
        // needs both of their own members
        let omit = has(features, RunExtra::PssArgumentOmitted);
        let full = has(features, RunExtra::PssScIter) && has(features, RunExtra::PssSteadyCoeff);
        if !omit && !full {
            return None;
        }
        let mode = modes[g.below(modes.len() as u32) as usize];
        let fundamental = g.expr(b, features, Reads::Constant);
        let tstab = g.expr(b, features, Reads::Constant);
        let harmonics = if !omit || g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let points = if !omit || g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let (osc_p, osc_n) = match mode {
            PssMode::Driven => (NameId::NONE, NameId::NONE),
            PssMode::Autonomous => {
                let p = g.fresh_name(b);
                let n = if has(features, RunExtra::PssOscDiff) && g.chance(3) {
                    g.fresh_name(b)
                } else {
                    NameId::GROUND
                };
                (p, n)
            }
        };
        let uic = has(features, RunExtra::PssUic) && g.chance(2);
        let opt = |g: &mut Gen, b: &mut Builder<'_>, x: RunExtra| {
            if has(features, x) && (!omit || g.chance(3)) {
                g.expr(b, features, Reads::Constant)
            } else {
                ExprId::NONE
            }
        };
        let sc_iter = opt(g, b, RunExtra::PssScIter);
        let steady_coeff = opt(g, b, RunExtra::PssSteadyCoeff);
        Some(Pss {
            mode,
            uic,
            fundamental,
            tstab,
            harmonics,
            points,
            osc_p,
            osc_n,
            sc_iter,
            steady_coeff,
        })
    }
}
