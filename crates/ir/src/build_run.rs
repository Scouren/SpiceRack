//! The builder's run methods: a reader's or builder's run statements in (analyses, loops,
//! alters, options, the environment, node sets, outputs, statistics), rows appended to the
//! design out. An `impl Builder` block over the kernel's `Builder::push`/`push_scoped`; the
//! same asserts as every push (handles in range, numbers finite, C9, C10).
//!
//! A detail row is plain data the caller fills, then hands to the one push of its header kind
//! (`analysis`, `loop_`, `measure`): the [`RunDetail`] type names the kind. Lists inside a detail
//! row (probes, DC axes, targets, variations) are appended first with the pool methods below.
//! Names and paths are interned here; references stay unresolved for the caller's rule (a
//! dialect reader binds by its simulator's; a builder calls `link`, A4).
//!
//! A reader lowering `.temp 85`, `.step param r 1k 10k 1k` (Xyce: it wraps every analysis),
//! `.dc V1 0 5 0.1`, `.tran 1n 1u uic`, `.save v(out) i(v1)` and
//! `.meas tran tdel trig v(in) val=0.5 rise=1 targ v(out) val=0.5 rise=1`:
//!
//! ```ignore
//! let n = |b: &mut Builder<'_>, text: &str| b.literal(read_number(text)); // the reader's scanner
//! let (t85, t27) = (n(&mut b, "85"), n(&mut b, "27"));
//! b.env(top, t85, t27, span); // Xyce's default tnom, filled by the reader
//!
//! let r = b.target(TargetKind::Param, &[], "r", None);
//! let step = SweepSpec::lin_step(n(&mut b, "1k"), n(&mut b, "10k"), n(&mut b, "1k"));
//! let (_, body) = b.loop_(top, None, SweepLoop { target: r, sweep: step }, LoopEnd::Restore, span);
//!
//! let v1 = b.target(TargetKind::SourceDc, &[], "V1", None);
//! let sweep = SweepSpec::lin_step(n(&mut b, "0"), n(&mut b, "5"), n(&mut b, "0.1"));
//! let axes = b.dc_axes(&[DcAxis { target: v1, sweep }]);
//! b.analysis(body, None, Dc { axes, restart: false }, span);
//!
//! let tran = Tran {
//!     stop: n(&mut b, "1u"), print_step: n(&mut b, "1n"), record_start: ExprId::NONE,
//!     max_step: ExprId::NONE, initial_step: ExprId::NONE, cap: StepCap::SpanOver10,
//!     noise_fmax: ExprId::NONE, noise_fmin: ExprId::NONE, noise_seed: ExprId::NONE,
//!     segments: List::EMPTY, uic: true, startup: false,
//! };
//! b.analysis(body, None, tran, span);
//!
//! let (out, i1) = (b.probe_v(&["out"]), b.probe_i(&["v1"]));
//! b.save(top, None, &[out, i1], span);
//!
//! let (half, one) = (n(&mut b, "0.5"), n(&mut b, "1"));
//! let vin = b.probe_v(&["in"]);
//! let trig = Event::cross(vin, Edge::Rise, half, one);
//! let targ = Event::cross(out, Edge::Rise, half, one);
//! b.measure(body, "tdel", AnalysisKind::Tran, MeasureTrigTarg { trig, targ }, span);
//! ```
//!
//! Cites: `rewrite/IR-RUN.md` §1–§6; `rewrite/IR-KERNEL.md` C2, C5, C9, C10, C13, A4.
//!
//! Contents:
//! - pools: `Builder::probes(&mut self, &[Probe]) -> ProbeList`,
//!   `Builder::targets(&mut self, &[Target]) -> TargetList`,
//!   `Builder::dc_axes(&mut self, &[DcAxis]) -> DcAxisList`,
//!   `Builder::varies(&mut self, &[Vary]) -> VaryList`,
//!   `Builder::correlations(&mut self, &[Correlation]) -> CorrelationList`
//! - values: `Builder::target`, `Builder::target_option`, `Builder::probe_v`,
//!   `Builder::probe_vdiff`, `Builder::probe_i`, `Builder::probe_terminal`,
//!   `Builder::probe_param`, `Builder::probe_power`, `Builder::probe_expr`
//! - items: `Builder::analysis`, `Builder::op`, `Builder::dc_inc`, `Builder::loop_`,
//!   `Builder::alter`, `Builder::option`, `Builder::option_private`, `Builder::env`,
//!   `Builder::node_set`, `Builder::save`, `Builder::print`, `Builder::measure`,
//!   `Builder::statistics`

use crate::analysis::{AnalysisKind, RunDetail};
use crate::build::Builder;
use crate::dc_ac::{DcAxis, DcAxisList};
use crate::id::{ExprId, ItemRef, ScopeId};
use crate::measure::MeasureKind;
use crate::option::{IcScope, NodeSetKind};
use crate::option_key::OptionKey;
use crate::output::PrintView;
use crate::plan::{LoopEnd, LoopKind, Target, TargetKind, TargetList};
use crate::probe::{Probe, ProbeList};
use crate::span::Span;
use crate::stats::{Correlation, CorrelationList, Statistics, Vary, VaryList};

impl Builder<'_> {
    // ---- pools ----

    /// Appends a list of probes. Asserts each probe's fields match its kind.
    pub fn probes(&mut self, probes: &[Probe]) -> ProbeList {
        todo!()
    }

    /// Appends a list of targets (a table loop's columns).
    pub fn targets(&mut self, targets: &[Target]) -> TargetList {
        todo!()
    }

    /// Appends a DC sweep's axes, innermost first. Asserts at least one.
    pub fn dc_axes(&mut self, axes: &[DcAxis]) -> DcAxisList {
        todo!()
    }

    /// Appends a list of variations.
    pub fn varies(&mut self, varies: &[Vary]) -> VaryList {
        todo!()
    }

    /// Appends a list of correlations. Asserts each pair names two parameters.
    pub fn correlations(&mut self, correlations: &[Correlation]) -> CorrelationList {
        todo!()
    }

    // ---- values ----

    /// A target of any kind but `Temp` (use `Target::TEMP`) and `Option` (use
    /// [`Builder::target_option`]): `a` and `b` as [`Target`] reads them for `kind`, `path`
    /// the instance path (empty at the top). The reference in `a` is left unresolved. Asserts
    /// `b` is given exactly when `kind` reads it.
    pub fn target(&mut self, kind: TargetKind, path: &[&str], a: &str, b: Option<&str>) -> Target {
        todo!()
    }

    /// An option target; `private` is `(dialect, name)` exactly when `key` is
    /// `OptionKey::Private`.
    pub fn target_option(&mut self, key: OptionKey, private: Option<(&str, &str)>) -> Target {
        todo!()
    }

    /// The voltage of the node at `node` (instance names, then the net). Asserts non-empty.
    pub fn probe_v(&mut self, node: &[&str]) -> Probe {
        todo!()
    }

    /// The voltage `node − reference`.
    pub fn probe_vdiff(&mut self, node: &[&str], reference: &[&str]) -> Probe {
        todo!()
    }

    /// The branch current of the element at `element`, positive into its first terminal.
    pub fn probe_i(&mut self, element: &[&str]) -> Probe {
        todo!()
    }

    /// The current into terminal `terminal` (index in the element kind's terminal order) of
    /// the element at `element`. Asserts `terminal < u8::MAX`.
    pub fn probe_terminal(&mut self, element: &[&str], terminal: u8) -> Probe {
        todo!()
    }

    /// A device's operating-point quantity, named as the source spelled it.
    pub fn probe_param(&mut self, element: &[&str], quantity: &str) -> Probe {
        todo!()
    }

    /// The power the element at `element` absorbs.
    pub fn probe_power(&mut self, element: &[&str]) -> Probe {
        todo!()
    }

    /// The value of `expr`. Asserts it is not `NONE`.
    pub fn probe_expr(&mut self, expr: ExprId) -> Probe {
        todo!()
    }

    // ---- items ----

    /// An analysis with a detail row; `name` is the statement name (Spectre, VACASK) or
    /// `None`. Asserts the name is not already an analysis's.
    pub fn analysis<D: RunDetail<Kind = AnalysisKind>>(
        &mut self,
        scope: ScopeId,
        name: Option<&str>,
        row: D,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// The operating point.
    pub fn op(&mut self, scope: ScopeId, name: Option<&str>, span: Span) -> ItemRef {
        todo!()
    }

    /// VACASK's incremental DC solution.
    pub fn dc_inc(&mut self, scope: ScopeId, name: Option<&str>, span: Span) -> ItemRef {
        todo!()
    }

    /// A loop and its empty body scope (`ScopeKind::Loop`); push its run items into the
    /// returned scope. Asserts the nesting stays within `plan::LOOP_DEPTH_MAX`.
    pub fn loop_<D: RunDetail<Kind = LoopKind>>(
        &mut self,
        scope: ScopeId,
        name: Option<&str>,
        row: D,
        after: LoopEnd,
        span: Span,
    ) -> (ItemRef, ScopeId) {
        todo!()
    }

    /// From here on, `target` holds `value`. Asserts `value` is not `NONE`.
    pub fn alter(&mut self, scope: ScopeId, target: Target, value: ExprId, span: Span) -> ItemRef {
        todo!()
    }

    /// A portable option; `value` `NONE` for a flag. Asserts `key` is not `Private` and that
    /// `value` is `NONE` exactly when `key.is_flag()`.
    pub fn option(&mut self, scope: ScopeId, key: OptionKey, value: ExprId, span: Span) -> ItemRef {
        todo!()
    }

    /// A simulator's own option, kept by `name` for `dialect`'s writer only.
    pub fn option_private(
        &mut self,
        scope: ScopeId,
        dialect: &str,
        name: &str,
        value: ExprId,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// The temperature and nominal temperature, °C. Asserts neither is `NONE`.
    pub fn env(&mut self, scope: ScopeId, temp: ExprId, tnom: ExprId, span: Span) -> ItemRef {
        todo!()
    }

    /// An initial condition or node set on one node's voltage to ground, for every analysis
    /// after it (the VACASK and differential forms: `push` a full `NodeSet`). Asserts
    /// `applies` is `EveryOp` for a `Nodeset`.
    pub fn node_set(
        &mut self,
        scope: ScopeId,
        kind: NodeSetKind,
        applies: IcScope,
        node: &[&str],
        value: ExprId,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A member of the save set, for one analysis kind or (`None`) every analysis. Asserts at
    /// least one probe.
    pub fn save(
        &mut self,
        scope: ScopeId,
        analysis: Option<AnalysisKind>,
        probes: &[Probe],
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A printed table of `analysis`'s results. Asserts at least one probe.
    pub fn print(
        &mut self,
        scope: ScopeId,
        analysis: AnalysisKind,
        view: PrintView,
        probes: &[Probe],
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A named measure of `analysis`'s results with its detail row.
    pub fn measure<D: RunDetail<Kind = MeasureKind>>(
        &mut self,
        scope: ScopeId,
        name: &str,
        analysis: AnalysisKind,
        row: D,
        span: Span,
    ) -> ItemRef {
        todo!()
    }

    /// A statistics block (its lists appended with [`Builder::varies`] and
    /// [`Builder::correlations`]).
    pub fn statistics(&mut self, scope: ScopeId, row: Statistics, span: Span) -> ItemRef {
        todo!()
    }
}
