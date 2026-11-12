//! Analyses: the closed list of analysis kinds, the analysis header row, and the run owner's
//! detail tables (`RunDetails`, the design's `run` field), reached through the kernel's
//! [`DetailRow`] (A6) with [`RunDetail`] naming each table's header kind. A reader's analysis
//! statement in, a header row plus one detail row out.
//!
//! Every analysis is `Analysis { kind, name, detail }`; a kind with parameters has its detail
//! row in its own table (`tran.rs`, `dc_ac.rs`, `periodic.rs`). Loops and measures use the same
//! header-plus-detail shape, so their detail tables live here too. Analyses only one simulator
//! has and whose parameters are not typed yet are `Opaque` of class `Private` (listed with their
//! reasons in `rewrite/COVERAGE-run.md`). There is no Monte Carlo kind: a Monte Carlo is a
//! `Loop` (`plan.rs`), because only a loop owns a body scope.
//!
//! Cites: `rewrite/IR-RUN.md` §2, §6; `rewrite/IR-DECISIONS.md` (`RunDetails`, `ItemRow`);
//! `rewrite/INV-constructs.md` G1–G73; `rewrite/INV-semantics.md` 8.01–8.26.
//!
//! Contents:
//! - `pub enum AnalysisKind` (`ALL`, `COUNT`, `AnalysisKind::has_detail(self) -> bool`),
//!   `pub struct Analysis`, `pub struct RunDetails`
//! - `pub struct AnalysisKinds` (`EMPTY`, `ALL`, `AnalysisKinds::of(&[AnalysisKind]) -> Self`,
//!   `AnalysisKinds::contains(self, AnalysisKind) -> bool`, `AnalysisKinds::len(self) -> u32`,
//!   `AnalysisKinds::is_empty(self) -> bool`,
//!   `AnalysisKinds::iter(self) -> impl Iterator<Item = AnalysisKind>`)
//! - `pub trait RunDetail: DetailRow` (`RunDetail::Kind`, `RunDetail::KIND`); it and the
//!   kernel's `DetailRow` (A6) implemented for every analysis, loop and measure detail row
//! - (`RowFacts` for `Analysis` and every detail row's facts: `analysis_facts.rs`); crate:
//!   `pub(crate) fn design_features(&Design, &mut FeatureSet)`,
//!   `pub(crate) fn feature_items(&Design, &mut Vec<(Feature, ItemRef)>)`

use crate::dc_ac::{
    Ac, Dc, DcAxis, DcXf, Disto, Net, Noise, Pz, Sens, SensAc, Sp, Tf, TimedOp, Xf,
};
use crate::design::ItemRow;
use crate::design::{Design, DetailRow};
use crate::feature::{Feature, FeatureSet};
use crate::id::{ItemRef, NameId, Ref};
use crate::item::ItemKind;
use crate::measure::{
    MeasureDeriv, MeasureFind, MeasureKind, MeasureParam, MeasureStat, MeasureTrigTarg, MeasureWhen,
};
use crate::periodic::{Hb, Pac, Pnoise, Pss, Pstb, Pxf, Stb};
use crate::plan::{Loop, LoopKind, MonteCarlo, SweepLoop, TableLoop, Target};
use crate::probe::Probe;
use crate::run_extra::RunExtra;
use crate::stats::{Correlation, Vary};
use crate::tran::{Four, Tran};

/// An analysis kind (IR-RUN §2). Each member names its detail row, or says it has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AnalysisKind {
    /// The DC operating point. No detail.
    Op,
    /// A DC sweep over nested axes. Detail `dc_ac::Dc`.
    Dc,
    /// Small-signal AC. Detail `dc_ac::Ac`.
    Ac,
    /// Transient. Detail `tran::Tran`.
    Tran,
    /// Small-signal noise. Detail `dc_ac::Noise`.
    Noise,
    /// DC transfer function from one source. Detail `dc_ac::Tf`.
    Tf,
    /// DC transfer functions from every source (VACASK `dcxf`). Detail `dc_ac::DcXf`.
    DcXf,
    /// AC transfer functions from every source (Spectre `xf`, VACASK `acxf`). Detail
    /// `dc_ac::Xf`.
    Xf,
    /// DC sensitivity. Detail `dc_ac::Sens`.
    Sens,
    /// AC sensitivity. Detail `dc_ac::SensAc`.
    SensAc,
    /// Pole-zero. Detail `dc_ac::Pz`.
    Pz,
    /// S-parameters. Detail `dc_ac::Sp`.
    Sp,
    /// Two-port network parameters during AC. Detail `dc_ac::Net`.
    Net,
    /// Fourier analysis of the transient. Detail `tran::Four`.
    Four,
    /// Small-signal distortion. Detail `dc_ac::Disto`.
    Disto,
    /// Harmonic balance. Detail `periodic::Hb`.
    Hb,
    /// Periodic steady state. Detail `periodic::Pss`.
    Pss,
    /// Periodic AC. Detail `periodic::Pac`.
    Pac,
    /// Periodic noise. Detail `periodic::Pnoise`.
    Pnoise,
    /// Periodic transfer functions. Detail `periodic::Pxf`.
    Pxf,
    /// Periodic loop-gain stability. Detail `periodic::Pstb`.
    Pstb,
    /// Loop-gain stability. Detail `periodic::Stb`.
    Stb,
    /// The incremental DC solution: AC at 0 Hz with phases ignored (VACASK `dcinc`,
    /// `INV-semantics.md` 8.20). No detail.
    DcInc,
    /// Operating-point reports during the transient. Detail `dc_ac::TimedOp`.
    TimedOp,
}

/// A set of analysis kinds (a `Save`'s scope, A22): one bit per `AnalysisKind`, in declaration
/// order; no bit past the last kind is ever set (the constructors keep it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct AnalysisKinds(u32);

/// An analysis: its kind, its statement name and its detail row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Analysis {
    /// What it computes; names the detail table.
    pub kind: AnalysisKind,
    /// Spectre's and VACASK's statement name (`tran1 tran …`, `analysis op1 op`); `NONE` in
    /// SPICE. Unique among the analyses of a design when set (checked).
    pub name: NameId,
    /// Row in the kind's detail table, or `u32::MAX` exactly when `kind.has_detail()` is false.
    pub detail: u32,
}

/// The run owner's detail tables and pools (the design's `run` field). No spans: a detail row's
/// span is its item's. Rows are appended by the builder only and never shared by two items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunDetails {
    // analysis details, in AnalysisKind order
    pub(crate) dcs: Vec<Dc>,
    pub(crate) acs: Vec<Ac>,
    pub(crate) trans: Vec<Tran>,
    pub(crate) noises: Vec<Noise>,
    pub(crate) tfs: Vec<Tf>,
    pub(crate) dc_xfs: Vec<DcXf>,
    pub(crate) xfs: Vec<Xf>,
    pub(crate) senses: Vec<Sens>,
    pub(crate) sens_acs: Vec<SensAc>,
    pub(crate) pzs: Vec<Pz>,
    pub(crate) sps: Vec<Sp>,
    pub(crate) nets: Vec<Net>,
    pub(crate) fours: Vec<Four>,
    pub(crate) distos: Vec<Disto>,
    pub(crate) hbs: Vec<Hb>,
    pub(crate) psses: Vec<Pss>,
    pub(crate) pacs: Vec<Pac>,
    pub(crate) pnoises: Vec<Pnoise>,
    pub(crate) pxfs: Vec<Pxf>,
    pub(crate) pstbs: Vec<Pstb>,
    pub(crate) stbs: Vec<Stb>,
    pub(crate) timed_ops: Vec<TimedOp>,
    // loop details, in LoopKind order
    pub(crate) sweep_loops: Vec<SweepLoop>,
    pub(crate) table_loops: Vec<TableLoop>,
    pub(crate) monte_carlos: Vec<MonteCarlo>,
    // measure details, in MeasureKind order
    pub(crate) trig_targs: Vec<MeasureTrigTarg>,
    pub(crate) whens: Vec<MeasureWhen>,
    pub(crate) finds: Vec<MeasureFind>,
    pub(crate) derivs: Vec<MeasureDeriv>,
    pub(crate) stats: Vec<MeasureStat>,
    pub(crate) measure_params: Vec<MeasureParam>,
    // pools: lists are ranges into these (C5)
    pub(crate) dc_axes: Vec<DcAxis>,
    pub(crate) probes: Vec<Probe>,
    pub(crate) targets: Vec<Target>,
    pub(crate) varies: Vec<Vary>,
    pub(crate) correlations: Vec<Correlation>,
    pub(crate) refs: Vec<Ref>,
}

/// A run detail row: the kernel's [`DetailRow`] plus the header kind whose rows live in its
/// table (an analysis, loop or measure kind). Implemented here for every run detail row, and
/// nowhere else; the builder's typed pushes use `Kind` to pick the header kind.
///
/// Rung (`codebase-clean-code` LADDER): the axis is the detail row type; 31 cases exist today;
/// one accessor, one push and one replace per type would be 93 near-identical methods, and the
/// compiler still lists every type here (the kernel's `ItemRow` made the same call).
pub trait RunDetail: DetailRow {
    /// The header enum whose member names this table (`AnalysisKind`, `LoopKind`,
    /// `MeasureKind`).
    type Kind: Copy + Eq + 'static;
    /// The header kind whose rows live in this table.
    const KIND: Self::Kind;
}

const _: () = assert!(size_of::<AnalysisKind>() == 1);
const _: () = assert!(size_of::<Analysis>() == 12);
const _: () = assert!(AnalysisKind::COUNT <= 32); // one bit each in `AnalysisKinds`
const _: () = assert!(size_of::<AnalysisKinds>() == 4);

macro_rules! details {
    ($($row:ty => $kind:ident :: $variant:ident, $field:ident;)*) => {
        $(
            impl DetailRow for $row {
                fn rows(design: &Design) -> &[Self] {
                    &design.run.$field
                }
                fn rows_mut(design: &mut Design) -> &mut Vec<Self> {
                    &mut design.run.$field
                }
            }
            impl RunDetail for $row {
                type Kind = $kind;
                const KIND: $kind = $kind::$variant;
            }
        )*
    };
}

details! {
    Dc => AnalysisKind::Dc, dcs;
    Ac => AnalysisKind::Ac, acs;
    Tran => AnalysisKind::Tran, trans;
    Noise => AnalysisKind::Noise, noises;
    Tf => AnalysisKind::Tf, tfs;
    DcXf => AnalysisKind::DcXf, dc_xfs;
    Xf => AnalysisKind::Xf, xfs;
    Sens => AnalysisKind::Sens, senses;
    SensAc => AnalysisKind::SensAc, sens_acs;
    Pz => AnalysisKind::Pz, pzs;
    Sp => AnalysisKind::Sp, sps;
    Net => AnalysisKind::Net, nets;
    Four => AnalysisKind::Four, fours;
    Disto => AnalysisKind::Disto, distos;
    Hb => AnalysisKind::Hb, hbs;
    Pss => AnalysisKind::Pss, psses;
    Pac => AnalysisKind::Pac, pacs;
    Pnoise => AnalysisKind::Pnoise, pnoises;
    Pxf => AnalysisKind::Pxf, pxfs;
    Pstb => AnalysisKind::Pstb, pstbs;
    Stb => AnalysisKind::Stb, stbs;
    TimedOp => AnalysisKind::TimedOp, timed_ops;
    SweepLoop => LoopKind::Sweep, sweep_loops;
    TableLoop => LoopKind::Table, table_loops;
    MonteCarlo => LoopKind::MonteCarlo, monte_carlos;
    MeasureTrigTarg => MeasureKind::TrigTarg, trig_targs;
    MeasureWhen => MeasureKind::When, whens;
    MeasureFind => MeasureKind::Find, finds;
    MeasureDeriv => MeasureKind::Deriv, derivs;
    MeasureStat => MeasureKind::Stat, stats;
    MeasureParam => MeasureKind::Param, measure_params;
}

impl AnalysisKind {
    /// Every kind, in declaration order (the order of `Feature::Analysis` indices).
    pub const ALL: [AnalysisKind; 24] = [
        AnalysisKind::Op,
        AnalysisKind::Dc,
        AnalysisKind::Ac,
        AnalysisKind::Tran,
        AnalysisKind::Noise,
        AnalysisKind::Tf,
        AnalysisKind::DcXf,
        AnalysisKind::Xf,
        AnalysisKind::Sens,
        AnalysisKind::SensAc,
        AnalysisKind::Pz,
        AnalysisKind::Sp,
        AnalysisKind::Net,
        AnalysisKind::Four,
        AnalysisKind::Disto,
        AnalysisKind::Hb,
        AnalysisKind::Pss,
        AnalysisKind::Pac,
        AnalysisKind::Pnoise,
        AnalysisKind::Pxf,
        AnalysisKind::Pstb,
        AnalysisKind::Stb,
        AnalysisKind::DcInc,
        AnalysisKind::TimedOp,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();

    /// Whether the kind has a detail row (every kind but `Op` and `DcInc`).
    pub const fn has_detail(self) -> bool {
        !matches!(self, AnalysisKind::Op | AnalysisKind::DcInc)
    }
}

impl AnalysisKinds {
    /// No kind.
    pub const EMPTY: AnalysisKinds = AnalysisKinds(0);
    /// Every kind (a SPICE `.save`: every analysis keeps the quantities).
    pub const ALL: AnalysisKinds = AnalysisKinds((1u32 << AnalysisKind::COUNT) - 1);

    /// The set of `kinds` (repeats are one member).
    pub const fn of(kinds: &[AnalysisKind]) -> Self {
        let mut bits = 0u32;
        let mut k = 0;
        while k < kinds.len() {
            bits |= 1 << kinds[k] as u32;
            k += 1;
        }
        AnalysisKinds(bits)
    }

    /// Whether `kind` is a member.
    pub const fn contains(self, kind: AnalysisKind) -> bool {
        self.0 & (1 << kind as u32) != 0
    }

    /// The member count.
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set has no member.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The members, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = AnalysisKind> {
        AnalysisKind::ALL
            .into_iter()
            .filter(move |&k| self.contains(k))
    }
}

/// The run owner's features of the whole design, which no single row carries alone (A9), as
/// `feature_items` lists them, gathered into `set`. Called once by `features_used` and
/// `check_legal`.
pub(crate) fn design_features(design: &Design, set: &mut FeatureSet) {
    let mut out = Vec::new();
    feature_items(design, &mut out);
    for (f, _) in out {
        *set = set.with(f);
    }
}

/// Appends each design-level run feature at the item that carries it: `SeveralAnalyses` at
/// each analysis after the first in run order (`state_order::run_walks`); `LoopScoped` at a
/// loop whose body does not hold every analysis; the mid-run state features at the state item
/// (`state_order::state_order_features`); `RunInterleaved` at the item between two run items
/// (`state_order::interleaved_features`). O(scopes + items).
pub(crate) fn feature_items(design: &Design, out: &mut Vec<(Feature, ItemRef)>) {
    let n = design.scopes.len();
    // Analyses per scope subtree: a scope's own, then added into its parent, children first
    // (a child scope always has a higher id than its parent).
    let mut below = vec![0usize; n];
    for (s, items) in design.scope_items.iter().enumerate() {
        below[s] = items
            .iter()
            .filter(|i| i.kind == ItemKind::Analysis)
            .count();
    }
    for s in (0..n).rev() {
        let parent = design.scopes[s].parent;
        if !parent.is_none() {
            assert!(
                parent.index() < s,
                "a parent scope comes before its children"
            );
            below[parent.index()] += below[s];
        }
    }
    let total: usize = design
        .files()
        .map(|f| below[design.file(f).top.index()])
        .sum();
    if total > 1 {
        let analyses = crate::state_order::run_walks(design)
            .into_iter()
            .flat_map(|(_, order)| order)
            .map(|(item, _)| item)
            .filter(|i| i.kind == ItemKind::Analysis);
        out.extend(
            analyses
                .skip(1)
                .map(|i| (Feature::Run(RunExtra::SeveralAnalyses), i)),
        );
    }
    crate::state_order::state_order_features(design, out);
    crate::state_order::interleaved_features(design, out);
    for items in &design.scope_items {
        for &item in items {
            if item.kind == ItemKind::Loop {
                let body = Loop::table(design).get(item.index).body;
                if below[body.index()] < total {
                    out.push((Feature::Run(RunExtra::LoopScoped), item));
                }
            }
        }
    }
}
