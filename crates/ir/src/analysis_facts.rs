//! The analysis rows' facts: `RowFacts` for `Analysis`, and the per-detail-row facts every run
//! detail row (analysis, loop and measure kinds) states through [`DetailFacts`]. A header row
//! and its detail in; handles, features, violations, IR text and generated rows out.
//!
//! IR text of an analysis: `analysis <kind> [name=<name>] <detail fields>`, the detail's fields
//! in declaration order as each detail file's header shows (`dc_ac.rs`, `tran.rs`,
//! `periodic.rs`); `analysis op`, `analysis dc_inc` have none.
//!
//! Cites: `rewrite/IR-KERNEL.md` §7.2 (A2, `RowFacts`); `rewrite/IR-IMPL.md` §1; `rewrite/
//! IR-DECISIONS.md` R4, R5 (a periodic small-signal analysis follows an `Hb` or `Pss`).
//!
//! Contents: `pub(crate) trait DetailFacts` (`DetailFacts::handles`, `DetailFacts::map_exprs`,
//! `DetailFacts::map_names`,
//! `DetailFacts::features`,
//! `DetailFacts::fault`, `DetailFacts::dump`, `DetailFacts::parse`, `DetailFacts::generate`),
//! `pub(crate) fn report`, `pub(crate) fn base_before`,
//! `pub(crate) fn base_just_before`, `pub(crate) fn handles_of`, `pub(crate) fn features_of`,
//! `pub(crate) fn fault_of`, `pub(crate) fn dump_of`, `pub(crate) fn parse_of`,
//! `pub(crate) fn gen_of`, `pub(crate) fn analysis_name` (gen),
//! `pub(crate) fn check_run_arguments`, `pub(crate) fn run_arguments_read_param`,
//! `pub(crate) fn run_arguments` (gen); `RowFacts` for `Analysis`

use crate::analysis::{Analysis, AnalysisKind, RunDetail};
use crate::build::Builder;
use crate::check::Violation;
use crate::dc_ac::{Ac, Dc, DcXf, Disto, Net, Noise, Pz, Sens, SensAc, Sp, Tf, TimedOp, Xf};
use crate::design::{Design, ItemRow};
use crate::diag::{Diags, violation_code};
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, ItemRef, NameId, Ref, ScopeId};
use crate::item::{DefKind, ItemKind};
use crate::periodic::{Hb, Pac, Pnoise, Pss, Pstb, Pxf, Stb};
use crate::plan::{target_refs, target_refs_into};
use crate::run_exprs::{Roots, exprs_of};
use crate::run_extra::RunExtra;
use crate::run_names::{Names, names_of};
#[cfg(any(test, feature = "gen"))]
use crate::run_words::loop_depth;
use crate::run_words::{put_name, take_enum, take_name, word};
#[cfg(any(test, feature = "gen"))]
use crate::sweep::{SweepUse, sweep_possible};
use crate::text::Words;
use crate::text_expr::Ctx;
use crate::tran::{Four, Tran};

/// What the run owner's generic code needs from a detail row (the `RowFacts` of a header kind
/// reaches its detail through this, one exhaustive dispatch over the header's kind). Rung: the
/// axis is the detail type, 31 exist; it mirrors `RowFacts` one level down.
pub(crate) trait DetailFacts: RunDetail {
    /// Appends every handle the row holds, its pools' elements included, in field order.
    fn handles(&self, design: &Design, out: &mut Vec<Handle>);
    /// Visits the row's expression roots, its pools' included, as `handles` lists them (A23);
    /// the caller stores the row back.
    fn map_exprs(&mut self, r: &mut Roots<'_>);
    /// Visits the row's element names, its pools' included, in field order (A32); the caller
    /// stores the row back.
    fn map_names(&mut self, r: &mut Names<'_>);
    /// Adds the row's `RunExtra` and option features.
    fn features(&self, design: &Design, set: &mut FeatureSet);
    /// The first broken invariant of the row's own fields: `FieldDomain`, or `HandleRange` for a
    /// pool list past its pool. `None` when valid.
    fn fault(&self, design: &Design) -> Option<Violation>;
    /// Appends the row's fields (each with its leading space).
    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String);
    /// Reads the row's fields; `None` after a report.
    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> Option<Self>;
    /// A random valid row using only `features` for an item about to be appended to `scope`,
    /// or `None` when the set allows none.
    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self>;
}

/// The periodic small-signal kinds: each needs an `Hb` or `Pss` before it (R5).
#[cfg(any(test, feature = "gen"))]
const PERIODIC: [AnalysisKind; 4] = [
    AnalysisKind::Pac,
    AnalysisKind::Pnoise,
    AnalysisKind::Pxf,
    AnalysisKind::Pstb,
];

/// Dispatches a generic call over an analysis kind's detail type (`$none` for the kinds
/// without one). Exhaustive: a new kind fails to compile here.
macro_rules! by_detail {
    ($kind:expr, $none:expr, $f:ident ( $($a:expr),* )) => {
        match $kind {
            AnalysisKind::Op | AnalysisKind::DcInc => $none,
            AnalysisKind::Dc => $f::<Dc>($($a),*),
            AnalysisKind::Ac => $f::<Ac>($($a),*),
            AnalysisKind::Tran => $f::<Tran>($($a),*),
            AnalysisKind::Noise => $f::<Noise>($($a),*),
            AnalysisKind::Tf => $f::<Tf>($($a),*),
            AnalysisKind::DcXf => $f::<DcXf>($($a),*),
            AnalysisKind::Xf => $f::<Xf>($($a),*),
            AnalysisKind::Sens => $f::<Sens>($($a),*),
            AnalysisKind::SensAc => $f::<SensAc>($($a),*),
            AnalysisKind::Pz => $f::<Pz>($($a),*),
            AnalysisKind::Sp => $f::<Sp>($($a),*),
            AnalysisKind::Net => $f::<Net>($($a),*),
            AnalysisKind::Four => $f::<Four>($($a),*),
            AnalysisKind::Disto => $f::<Disto>($($a),*),
            AnalysisKind::Hb => $f::<Hb>($($a),*),
            AnalysisKind::Pss => $f::<Pss>($($a),*),
            AnalysisKind::Pac => $f::<Pac>($($a),*),
            AnalysisKind::Pnoise => $f::<Pnoise>($($a),*),
            AnalysisKind::Pxf => $f::<Pxf>($($a),*),
            AnalysisKind::Pstb => $f::<Pstb>($($a),*),
            AnalysisKind::Stb => $f::<Stb>($($a),*),
            AnalysisKind::TimedOp => $f::<TimedOp>($($a),*),
        }
    };
}

/// Appends violation `v` of item `at` (`arg`: the item's row).
pub(crate) fn report(design: &Design, at: ItemRef, v: Violation, out: &mut Diags) {
    out.push_at(violation_code(v), design, at, at.index);
}

/// Whether an `Hb` or `Pss` analysis comes before position `pos` of `scope` in run order: in
/// the scope's items before `pos`, or before the owner of each enclosing scope. Bounded by the
/// scope count.
pub(crate) fn base_before(design: &Design, scope: ScopeId, pos: usize) -> bool {
    let (mut s, mut p) = (scope, pos);
    for _ in 0..=design.scopes.len() {
        let items = design.items(s);
        let found = items[..p.min(items.len())].iter().any(|&it| {
            it.kind == ItemKind::Analysis
                && matches!(
                    Analysis::table(design).get(it.index).kind,
                    AnalysisKind::Hb | AnalysisKind::Pss
                )
        });
        if found {
            return true;
        }
        let row = design.scope(s);
        if row.parent.is_none() {
            return false;
        }
        p = design
            .items(row.parent)
            .iter()
            .position(|&it| it == row.owner)
            .unwrap_or(0);
        s = row.parent;
    }
    unreachable!("the scope tree has a cycle")
}

impl RowFacts for Analysis {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        out.push(Handle::Name(self.name));
        by_detail!(self.kind, (), handles_of(design, self.detail, out));
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        // Only a DC sweep's axes hold references (their targets), in axis order.
        let row: Analysis = design.row(at.index);
        if row.kind != AnalysisKind::Dc {
            return;
        }
        let Some(dc) = crate::circuit_text::get_row::<Dc>(design, row.detail) else {
            return;
        };
        let start = dc.axes.start as usize;
        let end = (start + dc.axes.len as usize).min(design.run.dc_axes.len());
        for axis in &mut design.run.dc_axes[start.min(end)..end] {
            target_refs(&mut axis.target, f);
        }
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let row: Analysis = design.row(at.index);
        by_detail!(row.kind, (), exprs_of(design, row.detail, f));
    }

    fn map_element_names(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut [NameId])) {
        let row: Analysis = design.row(at.index);
        by_detail!(row.kind, (), names_of(design, row.detail, f));
    }

    fn refs(&self, design: &Design, _at: ItemRef, out: &mut Vec<(Ref, DefKind)>) {
        // `map_refs`'s order: a DC sweep's axes' targets
        if self.kind != AnalysisKind::Dc {
            return;
        }
        let Some(dc) = crate::circuit_text::get_row::<Dc>(design, self.detail) else {
            return;
        };
        let start = dc.axes.start as usize;
        let end = (start + dc.axes.len as usize).min(design.run.dc_axes.len());
        for axis in &design.run.dc_axes[start.min(end)..end] {
            target_refs_into(&axis.target, out);
        }
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        *set = set.with(Feature::Analysis(self.kind));
        let named = if self.name.is_none() {
            RunExtra::AnalysisUnnamed
        } else {
            RunExtra::AnalysisNamed
        };
        *set = set.with(Feature::Run(named));
        by_detail!(self.kind, (), features_of(design, self.detail, set));
        let mut handles = Vec::new();
        self.handles(design, &mut handles);
        if run_arguments_read_param(design, &handles) {
            *set = set.with(Feature::Run(RunExtra::AnalysisParamArgument));
        }
    }

    fn check(&self, design: &Design, at: ItemRef, scope: ScopeId, out: &mut Diags) {
        let fault = if self.kind.has_detail() {
            by_detail!(self.kind, None, fault_of(design, self.detail))
        } else {
            (self.detail != u32::MAX).then_some(Violation::DetailRow)
        };
        if let Some(v) = fault {
            report(design, at, v, out);
        }
        let mut handles = Vec::new();
        self.handles(design, &mut handles);
        check_run_arguments(design, at, &handles, out);
        let rows = Analysis::table(design).rows();
        let earlier = &rows[..(at.index as usize).min(rows.len())];
        if !self.name.is_none() && earlier.iter().any(|a| a.name == self.name) {
            report(design, at, Violation::FieldDomain, out);
        }
        if matches!(
            self.kind,
            AnalysisKind::Pac | AnalysisKind::Pnoise | AnalysisKind::Pxf | AnalysisKind::Pstb
        ) {
            let pos = design.items(scope).iter().position(|&i| i == at);
            if !pos.is_some_and(|k| base_before(design, scope, k)) {
                report(design, at, Violation::FieldDomain, out);
            }
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        out.push_str(&word(self.kind));
        put_name(out, design, "name", self.name);
        by_detail!(self.kind, (), dump_of(design, ctx, self.detail, out));
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let Some(kind) = take_enum(words, &AnalysisKind::ALL, diags) else {
            return ItemRef::NONE;
        };
        let name = take_name(words, "name", b, diags);
        let detail = by_detail!(kind, Some(u32::MAX), parse_of(words, scope, b, diags));
        let Some(detail) = detail else {
            return ItemRef::NONE;
        };
        b.push(scope, Analysis { kind, name, detail }, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let naming = [RunExtra::AnalysisNamed, RunExtra::AnalysisUnnamed];
        if !naming
            .into_iter()
            .any(|x| features.contains(Feature::Run(x)))
        {
            return ItemRef::NONE; // an analysis is named or not: one of the two is needed
        }
        let design = b.design();
        let several = features.contains(Feature::Run(RunExtra::SeveralAnalyses));
        // Generation appends, so the table's rows are the design's analyses (no dead rows).
        if !design.analyses.is_empty() && !several {
            return ItemRef::NONE;
        }
        // Without `LoopScoped`, every loop must wrap every analysis: a new one goes inside
        // every existing loop's body.
        let scoped = features.contains(Feature::Run(RunExtra::LoopScoped));
        // Inside every loop's body exactly when the scope's loop depth is the loop count.
        if !scoped && loop_depth(design, scope) != design.loops.len() {
            return ItemRef::NONE;
        }
        let kinds: Vec<AnalysisKind> = AnalysisKind::ALL
            .into_iter()
            .filter(|&k| features.contains(Feature::Analysis(k)) && possible(k, b, scope, features))
            .collect();
        if kinds.is_empty() {
            return ItemRef::NONE;
        }
        let kind = kinds[g.below(kinds.len() as u32) as usize];
        // A Fourier analysis that takes its count from the deck's option gets the option from
        // here, first (it is listed before the analysis, and holds no name the header's name
        // would follow), half the time.
        if kind == AnalysisKind::Four
            && features.contains(Feature::Run(RunExtra::FourHarmonicsOption))
            && g.chance(2)
        {
            crate::state_gen::gen_nfreqs(g, scope, b, features);
        }
        // The name first: the text, and so the parse, interns it before the detail's names.
        let name = analysis_name(g, b, features);
        let detail = run_arguments(g, features, |g| {
            by_detail!(kind, Some(u32::MAX), gen_of(g, scope, b, features))
        })
        .expect("a possible kind's detail generates");
        let item = b.push(
            scope,
            Analysis { kind, name, detail },
            crate::span::Span::NONE,
        );
        // A periodic small-signal kind is possible only right after its base: a base this call
        // made gets one at once (half the time), so none of them depends on the kernel picking an
        // analysis next. Counted in the budget like any item.
        if matches!(kind, AnalysisKind::Hb | AnalysisKind::Pss)
            && several
            && g.items_left > 1
            && g.chance(2)
        {
            let follow: Vec<AnalysisKind> = PERIODIC
                .into_iter()
                .filter(|&k| {
                    features.contains(Feature::Analysis(k)) && possible(k, b, scope, features)
                })
                .collect();
            if !follow.is_empty() {
                let kind = follow[g.below(follow.len() as u32) as usize];
                let name = analysis_name(g, b, features);
                let detail = run_arguments(g, features, |g| {
                    by_detail!(kind, Some(u32::MAX), gen_of(g, scope, b, features))
                })
                .expect("a periodic kind generates right after its base");
                b.push(
                    scope,
                    Analysis { kind, name, detail },
                    crate::span::Span::NONE,
                );
                g.items_left -= 1;
            }
        }
        item
    }
}

/// `DetailFacts::handles` of `D`'s row at `index`, when it exists.
pub(crate) fn handles_of<D: DetailFacts>(design: &Design, index: u32, out: &mut Vec<Handle>) {
    if let Some(d) = crate::circuit_text::get_row::<D>(design, index) {
        d.handles(design, out);
    }
}

/// `DetailFacts::features` of `D`'s row at `index`, when it exists.
pub(crate) fn features_of<D: DetailFacts>(design: &Design, index: u32, set: &mut FeatureSet) {
    if let Some(d) = crate::circuit_text::get_row::<D>(design, index) {
        d.features(design, set);
    }
}

/// `DetailFacts::fault` of `D`'s row at `index`; `DetailRow` when it does not exist.
pub(crate) fn fault_of<D: DetailFacts>(design: &Design, index: u32) -> Option<Violation> {
    match crate::circuit_text::get_row::<D>(design, index) {
        Some(d) => d.fault(design),
        None => Some(Violation::DetailRow),
    }
}

/// `DetailFacts::dump` of `D`'s row at `index` (a checked design has it).
pub(crate) fn dump_of<D: DetailFacts>(design: &Design, ctx: &Ctx, index: u32, out: &mut String) {
    let d = crate::circuit_text::get_row::<D>(design, index)
        .expect("dump of a checked design: the detail exists");
    d.dump(design, ctx, out);
}

/// Reads a `D` row and appends it to its table; its index.
pub(crate) fn parse_of<D: DetailFacts>(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<u32> {
    let row = D::parse(words, scope, b, diags)?;
    Some(b.detail(row))
}

#[cfg(any(test, feature = "gen"))]
/// Generates a `D` row and appends it to its table; its index.
pub(crate) fn gen_of<D: DetailFacts>(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    features: &FeatureSet,
) -> Option<u32> {
    let row = D::generate(g, scope, b, features)?;
    Some(b.detail(row))
}

/// Whether the last analysis listed in `scope` is an `Hb` or `Pss`: the generator's form of
/// `base_before` (a periodic small-signal row it generates follows its base in the same scope).
/// Scans back only to the scope's last analysis: amortized O(1) per generated item.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn base_just_before(design: &Design, scope: ScopeId) -> bool {
    let last = design
        .items(scope)
        .iter()
        .rev()
        .find(|it| it.kind == ItemKind::Analysis);
    last.is_some_and(|it| {
        matches!(
            Analysis::table(design).get(it.index).kind,
            AnalysisKind::Hb | AnalysisKind::Pss
        )
    })
}

/// A generated analysis's name, as `AnalysisNamed`/`AnalysisUnnamed` allow (one of the two is
/// in the set: `generate` checked it).
#[cfg(any(test, feature = "gen"))]
pub(crate) fn analysis_name(g: &mut Gen, b: &mut Builder<'_>, features: &FeatureSet) -> NameId {
    crate::run_words::gen_name(
        g,
        b,
        features,
        RunExtra::AnalysisNamed,
        RunExtra::AnalysisUnnamed,
        4,
    )
    .expect("a naming feature is in the set")
}

/// R68/A42: the constant roots `handles` lists (an analysis's own arguments, a sweep loop's
/// range) whose class is `Temp` or above, each reported as `RunArgumentClass` at `at`.
pub(crate) fn check_run_arguments(
    design: &Design,
    at: ItemRef,
    handles: &[Handle],
    out: &mut Diags,
) {
    let table = crate::class::def_classes(design);
    for (root, use_) in crate::feature::roots_with_use(design, ItemRef::NONE, handles) {
        let class = crate::class::tree_class(design, root, table);
        if use_ == ExprUse::Constant && class >= crate::class::ExprClass::Temp {
            let code = violation_code(Violation::RunArgumentClass);
            out.push_at(code, design, at, root.0);
        }
    }
}

/// R68/A42: whether a constant root `handles` lists (an analysis's own argument, a sweep
/// loop's range) reads a parameter (`RunExtra::AnalysisParamArgument`): in its own tree, or in
/// the body of a function it calls, transitively (a formal's read is not one), so inlining a
/// call neither adds nor removes the feature. O(the roots' trees and the bodies they reach).
pub(crate) fn run_arguments_read_param(design: &Design, handles: &[Handle]) -> bool {
    let roots = crate::feature::roots_with_use(design, ItemRef::NONE, handles);
    let mut work: Vec<ExprId> = roots
        .into_iter()
        .filter(|&(_, use_)| use_ == ExprUse::Constant)
        .map(|(root, _)| root)
        .collect();
    let mut called: Vec<crate::id::DefId> = Vec::new();
    let mut nodes = Vec::new();
    // bounded: each root once, each function body once
    while let Some(root) = work.pop() {
        nodes.clear();
        crate::class::tree_nodes(design, root, &mut nodes);
        for &e in &nodes {
            match design.expr(e) {
                crate::expr::Expr::Param(r) => {
                    let formal = r
                        .def
                        .index_checked(design.defs.len())
                        .is_some_and(|k| design.defs[k].kind == DefKind::Func);
                    if !formal {
                        return true;
                    }
                }
                crate::expr::Expr::UserCall(r, _) => {
                    let Some(k) = r.def.index_checked(design.defs.len()) else {
                        continue;
                    };
                    let def = design.defs[k];
                    let body = def.kind == DefKind::Func && def.item.kind == ItemKind::Func;
                    if body && !called.contains(&r.def) {
                        called.push(r.def);
                        work.push(design.row::<crate::item::Func>(def.item.index).body);
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// Runs `f` with the generator drawing an analysis's own arguments (R68/A42): constant class
/// only, and a parameter read only when the set holds `AnalysisParamArgument`.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn run_arguments<T>(
    g: &mut Gen,
    features: &FeatureSet,
    f: impl FnOnce(&mut Gen) -> T,
) -> T {
    let saved = (g.constant_only, g.param_reads);
    let reads = features.contains(Feature::Run(RunExtra::AnalysisParamArgument));
    (g.constant_only, g.param_reads) = (true, saved.1 && reads);
    let value = f(g);
    (g.constant_only, g.param_reads) = saved;
    value
}

// ---- private ----

/// Whether `kind`'s detail generator succeeds here under `features` (its only failures; the
/// header interns its name before calling it, so a failure would leave a stray name).
#[cfg(any(test, feature = "gen"))]
fn possible(kind: AnalysisKind, b: &Builder<'_>, scope: ScopeId, features: &FeatureSet) -> bool {
    let has = |x: RunExtra| features.contains(Feature::Run(x));
    match kind {
        AnalysisKind::Tran => {
            // a print step given or not (R67/A41), and a step ceiling
            (has(RunExtra::TranSimulatorStep) || has(RunExtra::TranPrintStep))
                && (has(RunExtra::TranMaxStep)
                    || [
                        RunExtra::TranCapNone,
                        RunExtra::TranCapNgspice,
                        RunExtra::TranCapSpan50,
                        RunExtra::TranCapSpan10,
                    ]
                    .into_iter()
                    .any(has))
        }
        AnalysisKind::Pss => {
            (has(RunExtra::PssDriven) || has(RunExtra::PssAutonomous))
                && (has(RunExtra::PssArgumentOmitted)
                    || has(RunExtra::PssScIter) && has(RunExtra::PssSteadyCoeff))
        }
        AnalysisKind::Pac | AnalysisKind::Pnoise | AnalysisKind::Pxf | AnalysisKind::Pstb => {
            base_just_before(b.design(), scope) && sweep_possible(features, SweepUse::Freq)
        }
        AnalysisKind::Dc => {
            sweep_possible(features, SweepUse::Dc)
                && (has(RunExtra::DcAxisNamed) || has(RunExtra::DcAxisUnnamed))
        }
        AnalysisKind::Ac
        | AnalysisKind::Noise
        | AnalysisKind::Xf
        | AnalysisKind::SensAc
        | AnalysisKind::Sp
        | AnalysisKind::Disto
        | AnalysisKind::Stb => sweep_possible(features, SweepUse::Freq),
        AnalysisKind::Op
        | AnalysisKind::Tf
        | AnalysisKind::DcXf
        | AnalysisKind::Sens
        | AnalysisKind::Pz
        | AnalysisKind::Net
        | AnalysisKind::Four
        | AnalysisKind::Hb
        | AnalysisKind::DcInc
        | AnalysisKind::TimedOp => true,
    }
}
