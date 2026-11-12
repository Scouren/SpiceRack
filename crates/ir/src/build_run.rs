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
//! ```
//! use spicerack_ir::dc_ac::{Dc, DcAxis};
//! use spicerack_ir::measure::MeasureTrigTarg;
//! use spicerack_ir::plan::SweepLoop;
//! use spicerack_ir::tran::{StepCap, StepCeiling, Tran};
//! use spicerack_ir::{AnalysisKinds, Applies, NameId};
//! use spicerack_ir::{
//!     AnalysisKind, Design, Edge, Event, ExprId, ItemKind, List, LoopEnd, Role, Span, SweepSpec,
//!     TargetKind,
//! };
//!
//! let mut design = Design::new();
//! let mut b = design.builder();
//! let top = b.file("deck.sp", Role::Deck).1;
//! let span = Span::NONE;
//! // A reader passes the literal it read (`b.literal`); a built number is `b.num`.
//! let (t85, t27) = (b.num(85.0), b.num(27.0));
//! b.env(top, t85, t27, span); // Xyce's default tnom, filled by the reader
//!
//! let r = b.target(TargetKind::Param, &[], "r", None);
//! let step = SweepSpec::lin_step(b.num(1e3), b.num(1e4), b.num(1e3));
//! let row = SweepLoop { target: r, sweep: step };
//! let (_, body) = b.loop_(top, None, row, LoopEnd::Restore, span);
//!
//! let v1 = b.target(TargetKind::SourceDc, &[], "V1", None);
//! let sweep = SweepSpec::lin_step(b.num(0.0), b.num(5.0), b.num(0.1));
//! let axes = b.dc_axes(&[DcAxis { target: v1, sweep, name: NameId::NONE }]);
//! b.analysis(body, None, Dc { axes, restart: false }, span);
//!
//! let tran = Tran {
//!     stop: b.num(1e-6), print_step: b.num(1e-9), record_start: ExprId::NONE,
//!     initial_step: ExprId::NONE, noise_fmax: ExprId::NONE, noise_fmin: ExprId::NONE,
//!     noise_seed: ExprId::NONE, segments: List::EMPTY,
//!     ceiling: StepCeiling::Rule(StepCap::SpanOver10), uic: true, startup: false,
//! };
//! b.analysis(body, None, tran, span);
//!
//! let (out, i1) = (b.probe_v(&["out"]), b.probe_i(&["v1"]));
//! b.save(top, AnalysisKinds::ALL, Applies::WholeRun, &[out, i1], span);
//!
//! let (half, one) = (b.num(0.5), b.num(1.0));
//! let vin = b.probe_v(&["in"]);
//! let trig = Event::cross(vin, Edge::Rise, half, one);
//! let targ = Event::cross(out, Edge::Rise, half, one);
//! b.measure(body, "tdel", AnalysisKind::Tran, MeasureTrigTarg { trig, targ }, span);
//!
//! let kinds: Vec<ItemKind> = design.items(body).iter().map(|i| i.kind).collect();
//! assert_eq!(kinds, [ItemKind::Analysis, ItemKind::Analysis, ItemKind::Measure]);
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
//!   `Builder::alter`, `Builder::reset`, `Builder::option`, `Builder::option_private`, `Builder::env`,
//!   `Builder::node_set`, `Builder::save`, `Builder::print`, `Builder::measure`,
//!   `Builder::statistics`, `Builder::elaborate`

use crate::analysis::{Analysis, AnalysisKind, AnalysisKinds, RunDetail};
use crate::build::Builder;
use crate::dc_ac::{DcAxis, DcAxisList};
use crate::design::ItemRow;
use crate::id::{ExprId, ItemRef, List, NameId, NameList, Ref, ScopeId};
use crate::item::{DefKind, ScopeKind};
use crate::measure::MeasureKind;
use crate::option::{Applies, Env, IcScope, NodeSet, NodeSetKind, OptionRow};
use crate::option_key::OptionKey;
use crate::output::{Measure, Print, PrintView, Save};
use crate::plan::{
    Alter, Elaborate, LOOP_DEPTH_MAX, Loop, LoopEnd, LoopKind, Target, TargetKind, TargetList,
};
use crate::probe::{Probe, ProbeKind, ProbeList};
use crate::reset::{Reset, ResetSet};
use crate::run_words::{loop_depth, pool};
use crate::span::Span;
use crate::stats::{Correlation, CorrelationList, Statistics, Vary, VaryList};

impl Builder<'_> {
    // ---- pools ----

    /// Appends a list of probes. Like `push`, it does not check the probes' fields (`check`'s
    /// `FieldDomain` does); the probe constructors below assert them.
    pub fn probes(&mut self, probes: &[Probe]) -> ProbeList {
        pool(&mut self.design.run.probes, probes)
    }

    /// Appends a list of targets (a table loop's columns).
    pub fn targets(&mut self, targets: &[Target]) -> TargetList {
        pool(&mut self.design.run.targets, targets)
    }

    /// Appends a DC sweep's axes, innermost first (fields unchecked, as `probes`; a sweep with
    /// no axis is `check`'s `FieldDomain`).
    pub fn dc_axes(&mut self, axes: &[DcAxis]) -> DcAxisList {
        pool(&mut self.design.run.dc_axes, axes)
    }

    /// Appends a list of variations.
    pub fn varies(&mut self, varies: &[Vary]) -> VaryList {
        pool(&mut self.design.run.varies, varies)
    }

    /// Appends a list of correlations (fields unchecked, as `probes`).
    pub fn correlations(&mut self, correlations: &[Correlation]) -> CorrelationList {
        pool(&mut self.design.run.correlations, correlations)
    }

    // ---- values ----

    /// A target of any kind but `Temp` (use `Target::TEMP`) and `Option` (use
    /// [`Builder::target_option`]): `a` and `b` as [`Target`] reads them for `kind`, `path`
    /// the instance path (empty at the top). The reference in `a` is left unresolved. Asserts
    /// `b` is given exactly when `kind` reads it. Interns `a`, `b`, then the path (the IR
    /// text's order).
    pub fn target(&mut self, kind: TargetKind, path: &[&str], a: &str, b: Option<&str>) -> Target {
        assert!(
            !matches!(kind, TargetKind::Temp | TargetKind::Option),
            "Builder::target: use Target::TEMP or Builder::target_option"
        );
        assert_eq!(
            b.is_some(),
            kind.reads_b(),
            "a target's `b` is given exactly when read"
        );
        assert!(
            path.is_empty() || kind.has_path(),
            "only an element target has a path"
        );
        let a = Ref::unresolved(self.name(a));
        let b = b.map_or(NameId::NONE, |b| self.name(b));
        Target {
            kind,
            a,
            b,
            path: self.path(path),
            ..Target::TEMP
        }
    }

    /// An option target; `private` is `(dialect, name)` exactly when `key` is
    /// `OptionKey::Private`.
    pub fn target_option(&mut self, key: OptionKey, private: Option<(&str, &str)>) -> Target {
        assert_eq!(
            key == OptionKey::Private,
            private.is_some(),
            "a private key names itself"
        );
        let mut t = Target {
            kind: TargetKind::Option,
            option: key,
            ..Target::TEMP
        };
        if let Some((dialect, name)) = private {
            t.a = Ref::unresolved(self.name(name));
            t.b = self.name(dialect);
        }
        t
    }

    /// The voltage of the node at `node` (instance names, then the net). Asserts non-empty.
    pub fn probe_v(&mut self, node: &[&str]) -> Probe {
        self.path_probe(ProbeKind::V, node)
    }

    /// The voltage `node − reference`.
    pub fn probe_vdiff(&mut self, node: &[&str], reference: &[&str]) -> Probe {
        assert!(!reference.is_empty(), "a reference node path");
        let mut p = self.path_probe(ProbeKind::Vdiff, node);
        p.b = self.path(reference);
        p
    }

    /// The branch current of the element at `element`, positive into its first terminal.
    pub fn probe_i(&mut self, element: &[&str]) -> Probe {
        self.path_probe(ProbeKind::I, element)
    }

    /// The current into terminal `terminal` (index in the element kind's terminal order) of
    /// the element at `element`. Asserts `terminal < u8::MAX`.
    pub fn probe_terminal(&mut self, element: &[&str], terminal: u8) -> Probe {
        assert!(terminal < u8::MAX, "a terminal index");
        Probe {
            terminal,
            ..self.path_probe(ProbeKind::ITerminal, element)
        }
    }

    /// A device's operating-point quantity, named as the source spelled it.
    pub fn probe_param(&mut self, element: &[&str], quantity: &str) -> Probe {
        let mut p = self.path_probe(ProbeKind::Param, element);
        p.b = self.names(&[quantity]);
        p
    }

    /// The power the element at `element` absorbs.
    pub fn probe_power(&mut self, element: &[&str]) -> Probe {
        self.path_probe(ProbeKind::Power, element)
    }

    /// The value of `expr`. Asserts it is not `NONE`.
    pub fn probe_expr(&mut self, expr: ExprId) -> Probe {
        assert!(!expr.is_none(), "an expression probe has an expression");
        Probe {
            kind: ProbeKind::Expr,
            expr,
            ..Probe::category(ProbeKind::AllV)
        }
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
        let name = self.analysis_name(name);
        let detail = self.detail(row);
        self.push(
            scope,
            Analysis {
                kind: D::KIND,
                name,
                detail,
            },
            span,
        )
    }

    /// The operating point.
    pub fn op(&mut self, scope: ScopeId, name: Option<&str>, span: Span) -> ItemRef {
        let name = self.analysis_name(name);
        self.push(
            scope,
            Analysis {
                kind: AnalysisKind::Op,
                name,
                detail: u32::MAX,
            },
            span,
        )
    }

    /// VACASK's incremental DC solution.
    pub fn dc_inc(&mut self, scope: ScopeId, name: Option<&str>, span: Span) -> ItemRef {
        let name = self.analysis_name(name);
        self.push(
            scope,
            Analysis {
                kind: AnalysisKind::DcInc,
                name,
                detail: u32::MAX,
            },
            span,
        )
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
        let depth = loop_depth(self.design, scope) + 1;
        assert!(
            depth <= LOOP_DEPTH_MAX,
            "loop depth {depth} past LOOP_DEPTH_MAX"
        );
        let name = name.map_or(NameId::NONE, |n| self.name(n));
        let detail = self.detail(row);
        let kind = D::KIND;
        self.push_scoped(
            scope,
            ScopeKind::Loop,
            |body| Loop {
                kind,
                after,
                name,
                detail,
                body,
            },
            span,
        )
    }

    /// From here on, `target` holds `value`. Asserts `value` is not `NONE` and the target is no
    /// `Temp` or `Option` (a `FromHere` `Env` or option row states those, A28).
    pub fn alter(&mut self, scope: ScopeId, target: Target, value: ExprId, span: Span) -> ItemRef {
        assert!(!value.is_none(), "an alter has a value");
        assert!(
            !matches!(target.kind, TargetKind::Temp | TargetKind::Option),
            "an alter of the temperature or an option is an Env or option row"
        );
        self.push(scope, Alter { target, value }, span)
    }

    /// From here on, the classes in `restores` hold the design's own value again (A27, A28).
    /// Asserts at least one class.
    pub fn reset(&mut self, scope: ScopeId, restores: ResetSet, span: Span) -> ItemRef {
        assert!(!restores.is_empty(), "a reset of no class");
        self.push(scope, Reset { restores }, span)
    }

    /// A portable option; `value` `NONE` for a flag. Asserts `key` is not `Private` and that
    /// `value` is `NONE` exactly when `key.is_flag()`.
    pub fn option(&mut self, scope: ScopeId, key: OptionKey, value: ExprId, span: Span) -> ItemRef {
        assert!(
            key != OptionKey::Private,
            "a private key: use option_private"
        );
        assert_eq!(
            value.is_none(),
            key.is_flag(),
            "an option is a flag exactly when it has no value"
        );
        let row = OptionRow {
            key,
            private: NameId::NONE,
            dialect: NameId::NONE,
            value,
            holds: Applies::FromHere,
        };
        self.push(scope, row, span)
    }

    /// A simulator's own option, kept by `name` for `dialect`'s writer only. Interns `name`
    /// then `dialect` (the IR text's order).
    pub fn option_private(
        &mut self,
        scope: ScopeId,
        dialect: &str,
        name: &str,
        value: ExprId,
        span: Span,
    ) -> ItemRef {
        let private = self.name(name);
        let dialect = self.name(dialect);
        self.push(
            scope,
            OptionRow {
                key: OptionKey::Private,
                private,
                dialect,
                value,
                holds: Applies::FromHere,
            },
            span,
        )
    }

    /// The temperature and nominal temperature, °C, from here on (`Applies::FromHere`, the
    /// default: a reader of whole-run state pushes the row with `WholeRun`). Either may be
    /// `NONE` (an env sets what it names); asserts not both.
    pub fn env(&mut self, scope: ScopeId, temp: ExprId, tnom: ExprId, span: Span) -> ItemRef {
        assert!(
            !temp.is_none() || !tnom.is_none(),
            "an env names temp or tnom"
        );
        self.push(
            scope,
            Env {
                temp,
                tnom,
                holds: Applies::FromHere,
            },
            span,
        )
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
        assert!(
            kind != NodeSetKind::Nodeset || applies == IcScope::EveryOp,
            "a nodeset serves every solve"
        );
        assert!(
            !node.is_empty() || kind == NodeSetKind::Nodeset,
            "a node path"
        );
        assert!(!value.is_none(), "a node set has a value");
        let node = self.path(node);
        let row = NodeSet {
            kind,
            applies,
            next_only: false,
            node,
            reference: List::EMPTY,
            value,
            holds: Applies::FromHere,
        };
        self.push(scope, row, span)
    }

    /// A member of the save set, kept by the analyses in `analyses` (`AnalysisKinds::ALL`: every
    /// analysis), from where `holds` says (`WholeRun`: a `.save` card). Asserts at least one probe and at least one analysis kind.
    pub fn save(
        &mut self,
        scope: ScopeId,
        analyses: AnalysisKinds,
        holds: Applies,
        probes: &[Probe],
        span: Span,
    ) -> ItemRef {
        assert!(!probes.is_empty(), "a save of no probe");
        assert!(!analyses.is_empty(), "a save for no analysis");
        let probes = self.probes(probes);
        let row = Save {
            analyses,
            holds,
            probes,
        };
        self.push(scope, row, span)
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
        assert!(!probes.is_empty(), "a print of no probe");
        let probes = self.probes(probes);
        self.push(
            scope,
            Print {
                analysis,
                view,
                probes,
            },
            span,
        )
    }

    /// A named measure of `analysis`'s results with its detail row; defines measure `name` in
    /// `scope` (A7). Asserts the name is not already a measure of `scope`.
    pub fn measure<D: RunDetail<Kind = MeasureKind>>(
        &mut self,
        scope: ScopeId,
        name: &str,
        analysis: AnalysisKind,
        row: D,
        span: Span,
    ) -> ItemRef {
        let name = self.name(name);
        let detail = self.detail(row);
        let item = self.push(
            scope,
            Measure {
                name,
                analysis,
                kind: D::KIND,
                detail,
            },
            span,
        );
        self.define(DefKind::Measure, name, scope, item);
        item
    }

    /// From here on, the top circuit is the named subcircuits' bodies (references left
    /// unresolved for the caller's rule). Asserts at least one.
    pub fn elaborate(&mut self, scope: ScopeId, circuits: &[&str], span: Span) -> ItemRef {
        assert!(
            !circuits.is_empty(),
            "an elaborate names at least one circuit"
        );
        let refs: Vec<Ref> = circuits
            .iter()
            .map(|c| Ref::unresolved(self.name(c)))
            .collect();
        let circuits = pool(&mut self.design.run.refs, &refs);
        self.push(scope, Elaborate { circuits }, span)
    }

    /// A statistics block (its lists appended with [`Builder::varies`] and
    /// [`Builder::correlations`]).
    pub fn statistics(&mut self, scope: ScopeId, row: Statistics, span: Span) -> ItemRef {
        self.push(scope, row, span)
    }

    // ---- private ----

    /// Interns a path; the empty path is `List::EMPTY`.
    fn path(&mut self, path: &[&str]) -> NameList {
        if path.is_empty() {
            List::EMPTY
        } else {
            self.names(path)
        }
    }

    fn path_probe(&mut self, kind: ProbeKind, path: &[&str]) -> Probe {
        assert!(!path.is_empty(), "a probe's node or element path");
        Probe {
            kind,
            a: self.path(path),
            ..Probe::category(ProbeKind::AllV)
        }
    }

    /// An analysis name, interned, asserted unused by another analysis.
    fn analysis_name(&mut self, name: Option<&str>) -> NameId {
        let Some(name) = name else {
            return NameId::NONE;
        };
        let id = self.name(name);
        let used = Analysis::table(self.design)
            .rows()
            .iter()
            .any(|a| a.name == id);
        assert!(!used, "analysis name {name} used twice");
        id
    }
}
