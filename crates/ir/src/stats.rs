//! Statistics blocks: the table form of statistical variation (Spectre `statistics { process
//! { vary … } mismatch { vary … } correlate … truncate … }`), as one item with ranges into the
//! run owner's vary and correlation pools. A reader's block in, rows out.
//!
//! The call form (`.param p=agauss(1k,100,1)`) is not here: it is the kernel's `Draw` node,
//! whose `DrawWhen`/`DrawPer` state when and how often it is drawn (Addendum A2). The two are
//! different native meanings; converting one to the other is a pass (`DESIGN-mc-lowering.md`
//! §2). A block varies only inside a Monte Carlo loop whose `Variations` selects its scope; a
//! `process` variation is drawn once per sample, a `mismatch` one once per subcircuit instance
//! per sample (`INV-semantics.md` 12.03, 12.05).
//!
//! Cites: `rewrite/IR-RUN.md` §5, §6; `rewrite/INV-constructs.md` J1–J5, J15, J17, J18;
//! `rewrite/INV-semantics.md` 12.03, 12.05, 12.07.
//!
//! Contents: `pub struct Statistics`, `pub struct Vary`, `pub enum VaryDist`,
//! `pub struct Correlation`, `pub type VaryList`, `pub type CorrelationList`;
//! `VaryDist::ALL`, `VaryDist::COUNT`; `Design::varies(&self, VaryList) -> &[Vary]`,
//! `Design::correlations(&self, CorrelationList) -> &[Correlation]`; `RowFacts` for
//! `Statistics`
//!
//! IR text: `statistics [process <vary> …] [mismatch <vary> …] [process_correlate <corr> …]
//! [mismatch_correlate <corr> …] [truncate=v] [process_truncate=v] [mismatch_truncate=v]`, with
//! `vary = (param gauss|unif|log_normal [percent] spread=v)` and `corr = (a b cc=v)`.
//! The references in a block's pools are reached by `map_refs` (A9), so the linker binds them.

use crate::analysis_facts::report;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, ItemRef, List, Ref, ScopeId};
use crate::item::DefKind;
#[cfg(any(test, feature = "gen"))]
use crate::plan::gen_ref;
use crate::run_exprs::roots;
use crate::run_extra::RunExtra;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    close, flag, get, open, put_expr, put_flag, put_ref, put_word, take_enum, take_expr, take_word,
    trim_lead, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::Ctx;

/// A statistics block: the variations of a Monte Carlo loop, by scope. At the top or in a
/// library section (a block inside a subcircuit or a branch is refused by its reader, W0018).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Statistics {
    /// Drawn once per sample.
    pub process: VaryList,
    /// Drawn once per subcircuit instance per sample.
    pub mismatch: VaryList,
    /// Correlations among `process` parameters.
    pub process_correlations: CorrelationList,
    /// Correlations among `mismatch` parameters.
    pub mismatch_correlations: CorrelationList,
    /// Truncation of every draw of the block, in standard deviations (`truncate tr=` at the
    /// block's top), or `NONE` for none.
    pub truncate: ExprId,
    /// Truncation of the `process` draws, overriding `truncate`, or `NONE`.
    pub process_truncate: ExprId,
    /// Truncation of the `mismatch` draws, overriding `truncate`, or `NONE`.
    pub mismatch_truncate: ExprId,
}

/// One varied parameter (`vary p dist=gauss std=s`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vary {
    /// The parameter varied (a `Param` definition the linker binds).
    pub param: Ref,
    /// Its distribution.
    pub dist: VaryDist,
    /// Whether `spread` is a percentage of the parameter's nominal value (`percent=yes`)
    /// instead of an absolute amount in the parameter's unit.
    pub percent: bool,
    /// The spread [`VaryDist`] states; never `NONE`.
    pub spread: ExprId,
}

/// A varied parameter's distribution. Each member states what `spread` is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VaryDist {
    /// Normal, `spread` = one standard deviation (Spectre `dist=gauss std=`).
    Gauss,
    /// Uniform on the parameter's value ± `spread` (Spectre `dist=unif N=`).
    Unif,
    /// Log-normal: the value's natural logarithm is normal around the logarithm of the parameter's value,
    /// with standard deviation `spread` (Spectre `dist=lnorm std=`; per the Spectre Reference,
    /// not verified by a run, `INV-semantics.md` B4).
    LogNormal,
}

/// The correlation coefficient of two varied parameters of one scope (`correlate param=[a b]
/// cc=0.8`; a list of n parameters is its n·(n−1)/2 pairs, each with the same coefficient).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correlation {
    /// The first parameter (a `Param` definition the linker binds).
    pub a: Ref,
    /// The second parameter; differs from `a`.
    pub b: Ref,
    /// The coefficient, in [−1, 1]; never `NONE`.
    pub cc: ExprId,
}

/// An ordered list of variations, a range into the run owner's vary pool.
pub type VaryList = List<Vary>;
/// An ordered list of correlations, a range into the run owner's correlation pool.
pub type CorrelationList = List<Correlation>;

const _: () = assert!(size_of::<Statistics>() == 44);
const _: () = assert!(size_of::<Vary>() == 16);
const _: () = assert!(size_of::<Correlation>() == 20);

impl VaryDist {
    /// Every distribution, in declaration order.
    pub const ALL: [VaryDist; 3] = [VaryDist::Gauss, VaryDist::Unif, VaryDist::LogNormal];
    /// Distribution count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The variations of a list. Asserts the range is inside the vary pool.
    pub fn varies(&self, list: VaryList) -> &[Vary] {
        get(&self.run.varies, list).expect("a vary list inside the vary pool")
    }

    /// The correlations of a list. Asserts the range is inside the correlation pool.
    pub fn correlations(&self, list: CorrelationList) -> &[Correlation] {
        get(&self.run.correlations, list).expect("a correlation list inside the pool")
    }
}

impl RowFacts for Statistics {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        for list in [self.process, self.mismatch] {
            for v in get(&design.run.varies, list).unwrap_or(&[]) {
                out.extend([Handle::Ref(v.param, DefKind::Param), Handle::Expr(v.spread)]);
            }
        }
        for list in [self.process_correlations, self.mismatch_correlations] {
            for c in get(&design.run.correlations, list).unwrap_or(&[]) {
                out.extend([
                    Handle::Ref(c.a, DefKind::Param),
                    Handle::Ref(c.b, DefKind::Param),
                    Handle::Expr(c.cc),
                ]);
            }
        }
        for e in [self.truncate, self.process_truncate, self.mismatch_truncate] {
            out.push(Handle::Expr(e));
        }
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let mut row: Statistics = design.row(at.index);
        {
            let mut r = roots(design, f);
            r.varies(row.process);
            r.varies(row.mismatch);
            r.correlations(row.process_correlations);
            r.correlations(row.mismatch_correlations);
            for e in [
                &mut row.truncate,
                &mut row.process_truncate,
                &mut row.mismatch_truncate,
            ] {
                r.expr(e);
            }
        }
        design.editor().set(at.index, row);
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // Every vary's parameter (process, then mismatch), then every correlation's two ends.
        let row: Statistics = design.row(at.index);
        for list in [row.process, row.mismatch] {
            let start = list.start as usize;
            let end = (start + list.len as usize).min(design.run.varies.len());
            for v in &mut design.run.varies[start.min(end)..end] {
                f(&mut v.param, DefKind::Param);
            }
        }
        for list in [row.process_correlations, row.mismatch_correlations] {
            let start = list.start as usize;
            let end = (start + list.len as usize).min(design.run.correlations.len());
            for c in &mut design.run.correlations[start.min(end)..end] {
                f(&mut c.a, DefKind::Param);
                f(&mut c.b, DefKind::Param);
            }
        }
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        flag(set, self.mismatch.len > 0, RunExtra::StatsMismatch);
        let correlated = self.process_correlations.len + self.mismatch_correlations.len > 0;
        flag(set, correlated, RunExtra::StatsCorrelation);
        let truncated = [self.truncate, self.process_truncate, self.mismatch_truncate]
            .iter()
            .any(|e| !e.is_none());
        flag(set, truncated, RunExtra::StatsTruncate);
        for list in [self.process, self.mismatch] {
            for v in get(&design.run.varies, list).unwrap_or(&[]) {
                flag(set, v.dist == VaryDist::LogNormal, RunExtra::VaryLogNormal);
                flag(set, v.percent, RunExtra::VaryPercent);
            }
        }
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let varies = [self.process, self.mismatch].map(|l| get(&design.run.varies, l));
        let corrs = [self.process_correlations, self.mismatch_correlations]
            .map(|l| get(&design.run.correlations, l));
        if varies.iter().any(Option::is_none) || corrs.iter().any(Option::is_none) {
            report(design, at, Violation::HandleRange, out);
            return;
        }
        let vary_ok = varies
            .iter()
            .flatten()
            .flat_map(|v| v.iter())
            .all(|v| !v.spread.is_none() && !v.param.name.is_none());
        let corr_ok = corrs
            .iter()
            .flatten()
            .flat_map(|c| c.iter())
            .all(|c| !c.cc.is_none() && !c.a.name.is_none() && c.a.name != c.b.name);
        if !vary_ok || !corr_ok {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let at = out.len();
        for (key, list) in [("process", self.process), ("mismatch", self.mismatch)] {
            if list.len > 0 {
                put_word(out, key);
                for v in design.varies(list) {
                    out.push_str(" (");
                    crate::text_expr::write_ref(design, v.param, DefKind::Param, ctx, out);
                    put_word(out, &word(v.dist));
                    put_flag(out, v.percent, "percent");
                    put_expr(out, design, ctx, "spread", v.spread);
                    out.push(')');
                }
            }
        }
        let corrs = [
            ("process_correlate", self.process_correlations),
            ("mismatch_correlate", self.mismatch_correlations),
        ];
        for (key, list) in corrs {
            if list.len > 0 {
                put_word(out, key);
                for c in design.correlations(list) {
                    out.push_str(" (");
                    crate::text_expr::write_ref(design, c.a, DefKind::Param, ctx, out);
                    put_ref(out, design, ctx, c.b, DefKind::Param);
                    put_expr(out, design, ctx, "cc", c.cc);
                    out.push(')');
                }
            }
        }
        put_expr(out, design, ctx, "truncate", self.truncate);
        put_expr(out, design, ctx, "process_truncate", self.process_truncate);
        put_expr(
            out,
            design,
            ctx,
            "mismatch_truncate",
            self.mismatch_truncate,
        );
        trim_lead(out, at);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let mut lists = [Vec::new(), Vec::new()];
        for (k, key) in ["process", "mismatch"].into_iter().enumerate() {
            if take_word(words, key) {
                while words.peek() == Some("(") {
                    let Some(v) = parse_vary(words, scope, b, diags) else {
                        return ItemRef::NONE;
                    };
                    lists[k].push(v);
                }
            }
        }
        let mut corrs = [Vec::new(), Vec::new()];
        for (k, key) in ["process_correlate", "mismatch_correlate"]
            .into_iter()
            .enumerate()
        {
            if take_word(words, key) {
                while words.peek() == Some("(") {
                    let Some(c) = parse_correlation(words, scope, b, diags) else {
                        return ItemRef::NONE;
                    };
                    corrs[k].push(c);
                }
            }
        }
        let row = Statistics {
            process: b.varies(&lists[0]),
            mismatch: b.varies(&lists[1]),
            process_correlations: b.correlations(&corrs[0]),
            mismatch_correlations: b.correlations(&corrs[1]),
            truncate: take_expr(words, "truncate", scope, b, diags),
            process_truncate: take_expr(words, "process_truncate", scope, b, diags),
            mismatch_truncate: take_expr(words, "mismatch_truncate", scope, b, diags),
        };
        b.push(scope, row, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let vary = |g: &mut Gen, b: &mut Builder<'_>| {
            let param = gen_ref(g, b, Some(DefKind::Param));
            let mut dists = vec![VaryDist::Gauss, VaryDist::Unif];
            if has(features, RunExtra::VaryLogNormal) {
                dists.push(VaryDist::LogNormal);
            }
            let dist = dists[g.below(dists.len() as u32) as usize];
            let percent = has(features, RunExtra::VaryPercent) && g.chance(3);
            let spread = g.expr(b, features, Reads::Constant);
            Vary {
                param,
                dist,
                percent,
                spread,
            }
        };
        let p: Vec<Vary> = (0..1 + g.below(2) as usize).map(|_| vary(g, b)).collect();
        let m: Vec<Vary> = if has(features, RunExtra::StatsMismatch) && g.chance(2) {
            vec![vary(g, b)]
        } else {
            Vec::new()
        };
        let c: Vec<Correlation> = if p.len() == 2
            && p[0].param.name != p[1].param.name
            && has(features, RunExtra::StatsCorrelation)
            && g.chance(2)
        {
            vec![Correlation {
                a: p[0].param,
                b: p[1].param,
                cc: g.expr(b, features, Reads::Constant),
            }]
        } else {
            Vec::new()
        };
        let truncate = if has(features, RunExtra::StatsTruncate) && g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let row = Statistics {
            process: b.varies(&p),
            mismatch: b.varies(&m),
            process_correlations: b.correlations(&c),
            mismatch_correlations: List::EMPTY,
            truncate,
            process_truncate: ExprId::NONE,
            mismatch_truncate: ExprId::NONE,
        };
        b.push(scope, row, Span::NONE)
    }
}

// ---- private ----

fn parse_vary(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<Vary> {
    if !open(words, diags) {
        return None;
    }
    let param = words.reference(b, diags);
    let dist = take_enum(words, &VaryDist::ALL, diags)?;
    let percent = take_word(words, "percent");
    let spread = take_expr(words, "spread", scope, b, diags);
    close(words, diags).then_some(Vary {
        param,
        dist,
        percent,
        spread,
    })
}

fn parse_correlation(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<Correlation> {
    if !open(words, diags) {
        return None;
    }
    let a = words.reference(b, diags);
    let other = words.reference(b, diags);
    let cc = take_expr(words, "cc", scope, b, diags);
    close(words, diags).then_some(Correlation { a, b: other, cc })
}
