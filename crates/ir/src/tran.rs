//! Time-domain analysis rows: the transient and the Fourier analysis that reads it. A reader's
//! `.tran`/`tran`/`.four` in, detail rows out.
//!
//! Every simulator reads the positional `.tran` fields differently (Xyce's first is an
//! initial-step guess, ngspice's a print step that also caps the step, HSPICE's further fields
//! are segments, Spectre's `step` is for output only), so the row names every quantity and the
//! reader puts each number in the field its simulator means (IR-RUN §2, `INV-semantics.md`
//! 8.01–8.03). The step ceiling a simulator applies by default differs too: the row's `ceiling`
//! is either a given expression or the default rule, a law; no number is computed by a reader.
//! One meaning has one row (A24, R47): no literal-zero ceiling or `record_start`.
//!
//! Cites: `rewrite/IR-RUN.md` §2; `rewrite/INV-constructs.md` G6, G7, G14, G39, G42, G48, G50,
//! G54, H29, I11, I19; `rewrite/INV-semantics.md` 8.01–8.03, 8.19, 8.20, 8.21, 8.25.
//!
//! IR text (after `analysis tran|four [name=n]`):
//!
//! ```text
//! tran stop=v [record_start=v] [print_step=v] [initial_step=v] [noise_fmax=v] [noise_fmin=v]
//!      [noise_seed=v] [segments=[v, …]] max_step=v|cap=<rule> [uic] [startup]
//! four fundamental=v harmonics=v periods=v [grid=v] <probe> …
//! ```
//!
//! Contents: `pub struct Tran`, `pub enum StepCeiling`, `pub enum StepCap`, `pub struct Four`;
//! `StepCap::ALL`, `StepCap::COUNT`; `DetailFacts` for `Tran` and `Four`

use crate::analysis_facts::DetailFacts;
#[cfg(any(test, feature = "gen"))]
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
#[cfg(any(test, feature = "gen"))]
use crate::design::ItemRow;
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
#[cfg(any(test, feature = "gen"))]
use crate::id::ScopeId;
use crate::id::{ExprId, ExprList, List};
#[cfg(any(test, feature = "gen"))]
use crate::item::ItemKind;
#[cfg(any(test, feature = "gen"))]
use crate::option::OptionRow;
#[cfg(any(test, feature = "gen"))]
use crate::option_key::OptionKey;
use crate::probe::{AcPart, Probe, ProbeList, probe_features, probe_valid};
use crate::run_extra::RunExtra;
use crate::run_fields::{EL, N};
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{domain_unless, flag, get, is_one, is_zero};

/// A transient analysis's detail. Every field is named and none is inferred from another;
/// `NONE` is "not given", never 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tran {
    /// The end time, s; never `NONE`.
    pub stop: ExprId,
    /// When results start being recorded, s (SPICE `tstart`, HSPICE `START=`, VACASK
    /// `start=`); `NONE` = from 0, so never the literal 0 (checked; A24).
    pub record_start: ExprId,
    /// The output interval, s (ngspice `tstep`, Spectre `step`); `NONE` = the simulator
    /// chooses (Xyce and LTspice step 0; ngspice has no such thing: its writer lacks
    /// `RunExtra::TranSimulatorStep`). Given, it is `RunExtra::TranPrintStep` (VACASK has no
    /// output interval, R67/A41).
    pub print_step: ExprId,
    /// The first step's size, s (Xyce's first field); `NONE` = the simulator's guess.
    pub initial_step: ExprId,
    /// Device noise in the transient: the highest frequency, Hz (VACASK `noisefmax=`); `NONE`
    /// = no device noise (HSPICE `.trannoise` is this, `INV-semantics.md` 8.20).
    pub noise_fmax: ExprId,
    /// Device noise: the lowest frequency, Hz (VACASK default `noise_fmax/1e3`, Spectre's
    /// `noise_fmax`: a reader fills its default). `NONE` exactly when `noise_fmax` is.
    pub noise_fmin: ExprId,
    /// Device noise: the seed, or `NONE` for the simulator's own.
    pub noise_seed: ExprId,
    /// HSPICE's further segments: `(print_step, stop)` pairs after the first, in order; empty
    /// elsewhere.
    pub segments: ExprList,
    /// The time-step ceiling: given, or the simulator's default rule.
    pub ceiling: StepCeiling,
    /// Skip the operating point and start from the initial conditions (`uic`, Xyce `NOOP`,
    /// Spectre `skipdc=yes`, VACASK `icmode="uic"`).
    pub uic: bool,
    /// Ramp the DC-only top-level sources over the first 20 µs (LTspice `startup`); lowering
    /// it to PWL sources is a pass.
    pub startup: bool,
}

/// A transient's time-step ceiling (A24, R47): one field, so a given ceiling leaves no unread
/// rule beside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StepCeiling {
    /// The deck's ceiling, s (SPICE `tmax`, Spectre and VACASK `maxstep`, Xyce's fourth field;
    /// a Xyce `schedule()` is its smallest nonzero step). Never `NONE` and never the literal 0
    /// (checked): ngspice (`traninit.c:29`) and VACASK read a 0 as none given, which is a
    /// `Rule`.
    Given(ExprId),
    /// None given: the simulator's default rule.
    Rule(StepCap),
}

/// The ceiling a simulator puts on the time step when the deck gives none. The span is
/// `stop − record_start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StepCap {
    /// No ceiling beyond the span.
    None,
    /// `min(print_step, span/50)` (ngspice).
    NgspiceRule,
    /// `span/50` (Spectre, VACASK).
    SpanOver50,
    /// `span/10` (Xyce).
    SpanOver10,
}

/// A Fourier analysis of the preceding transient's last periods (`.four`, Spectre `fourier`).
/// Its result is a table, not a waveform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Four {
    /// The fundamental frequency, Hz; never `NONE`.
    pub fundamental: ExprId,
    /// The number of harmonics, the fundamental included and the DC term not (LTspice `Nh`), or
    /// `NONE` when the deck's `OptionKey::NFreqs` gives it (ngspice, `fourier.c:69`, A16;
    /// checked: an `nfreqs` option is in force at the analysis in its walk, `state_order::in_force`;
    /// an option of an unrelated file gives it nothing). `nfreqs` counts the DC term too: `nfreqs = n` is
    /// `harmonics = n − 1`. A reader states its simulator's default count here
    /// when the deck sets no option.
    pub harmonics: ExprId,
    /// How many periods at the end are analysed (LTspice `Np`, default 1); never `NONE`.
    pub periods: ExprId,
    /// The interpolation grid's size, or `NONE` for the simulator's (or the deck's
    /// `OptionKey::FourGridSize`).
    pub grid: ExprId,
    /// The outputs analysed, at least one.
    pub outputs: ProbeList,
}

const _: () = assert!(size_of::<Tran>() == 48);
const _: () = assert!(size_of::<StepCeiling>() == 8);
const _: () = assert!(size_of::<Four>() == 24);

impl StepCap {
    /// Every rule, in declaration order.
    pub const ALL: [StepCap; 4] = [
        StepCap::None,
        StepCap::NgspiceRule,
        StepCap::SpanOver50,
        StepCap::SpanOver10,
    ];
    /// Rule count.
    pub const COUNT: usize = Self::ALL.len();
}

run_fields! {
    Tran {
        stop: Expr = N, record_start: Expr = N, print_step: Expr = N, initial_step: Expr = N,
        noise_fmax: Expr = N, noise_fmin: Expr = N, noise_seed: Expr = N, segments: Exprs = EL,
        ceiling: Ceiling("") = StepCeiling::Given(N), uic: Word = false, startup: Word = false,
    }
    Four {
        fundamental: Expr = N, harmonics: Expr = N, periods: Expr = N, grid: Expr = N,
        outputs: Probes("") = List::EMPTY,
    }
}

impl DetailFacts for Tran {
    fn features(&self, _design: &Design, set: &mut FeatureSet) {
        // R67/A41: exactly one of the twins
        flag(set, self.print_step.is_none(), RunExtra::TranSimulatorStep);
        flag(set, !self.print_step.is_none(), RunExtra::TranPrintStep);
        flag(set, !self.record_start.is_none(), RunExtra::TranRecordStart);
        flag(set, !self.initial_step.is_none(), RunExtra::TranInitialStep);
        flag(
            set,
            true,
            match self.ceiling {
                StepCeiling::Given(_) => RunExtra::TranMaxStep,
                StepCeiling::Rule(cap) => cap_feature(cap),
            },
        );
        flag(set, self.uic, RunExtra::TranUic);
        flag(set, self.startup, RunExtra::TranStartup);
        flag(set, !self.noise_fmax.is_none(), RunExtra::TranNoise);
        flag(set, self.segments.len > 0, RunExtra::TranSegments);
    }

    fn fault(&self, design: &Design) -> Option<Violation> {
        let noise = self.noise_fmax.is_none() == self.noise_fmin.is_none();
        // one meaning, one row (A24): a 0 ceiling is no ceiling given, a 0 start no start given
        let ceiling = match self.ceiling {
            StepCeiling::Given(e) => !e.is_none() && !is_zero(design, e),
            StepCeiling::Rule(_) => true,
        };
        let start = !is_zero(design, self.record_start);
        domain_unless(
            !self.stop.is_none()
                && noise
                && self.segments.len.is_multiple_of(2)
                && ceiling
                && start,
        )
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        _scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let caps: Vec<StepCap> = StepCap::ALL
            .into_iter()
            .filter(|&c| has(features, cap_feature(c)))
            .collect();
        let own_cap = has(features, RunExtra::TranMaxStep) && (caps.is_empty() || g.chance(3));
        if caps.is_empty() && !own_cap {
            return None;
        }
        // every transient sets one of the twins (R67/A41)
        let twins = [RunExtra::TranSimulatorStep, RunExtra::TranPrintStep];
        if !twins.iter().any(|&x| has(features, x)) {
            return None;
        }
        let value = |g: &mut Gen, b: &mut Builder<'_>, on: bool| {
            if on {
                g.expr(b, features, Reads::Constant)
            } else {
                ExprId::NONE
            }
        };
        let stop = value(g, b, true);
        let on = has(features, RunExtra::TranRecordStart) && g.chance(3);
        let record_start = if on { nonzero(g, b) } else { ExprId::NONE };
        // R67/A41: the print step by which twin the set holds (both: a third chosen)
        let chosen = match (
            has(features, RunExtra::TranSimulatorStep),
            has(features, RunExtra::TranPrintStep),
        ) {
            (true, true) => g.chance(3),
            (sim, _) => sim,
        };
        let print_step = value(g, b, !chosen);
        let on = has(features, RunExtra::TranInitialStep) && g.chance(3);
        let initial_step = value(g, b, on);
        let noise = has(features, RunExtra::TranNoise) && g.chance(3);
        let noise_fmax = value(g, b, noise);
        let noise_fmin = value(g, b, noise);
        let on = noise && g.chance(2);
        let noise_seed = value(g, b, on);
        let segments = if has(features, RunExtra::TranSegments) && g.chance(3) {
            let s = [value(g, b, true), value(g, b, true)];
            b.exprs(&s)
        } else {
            List::EMPTY
        };
        // after the segments: the text writes the ceiling there
        let ceiling = if own_cap {
            StepCeiling::Given(nonzero(g, b))
        } else {
            StepCeiling::Rule(caps[g.below(caps.len() as u32) as usize])
        };
        Some(Tran {
            stop,
            record_start,
            print_step,
            initial_step,
            noise_fmax,
            noise_fmin,
            noise_seed,
            segments,
            ceiling,
            uic: has(features, RunExtra::TranUic) && g.chance(2),
            startup: has(features, RunExtra::TranStartup) && g.chance(2),
        })
    }
}

impl DetailFacts for Four {
    fn features(&self, design: &Design, set: &mut FeatureSet) {
        flag(set, !is_one(design, self.periods), RunExtra::FourPeriods);
        flag(set, !self.grid.is_none(), RunExtra::FourGrid);
        flag(set, self.harmonics.is_none(), RunExtra::FourHarmonicsOption);
        for p in get(&design.run.probes, self.outputs).unwrap_or(&[]) {
            probe_features(p, set);
        }
    }

    fn fault(&self, design: &Design) -> Option<Violation> {
        let Some(outputs) = get(&design.run.probes, self.outputs) else {
            return Some(Violation::HandleRange);
        };
        let given = !self.fundamental.is_none() && !self.periods.is_none();
        // An absent count is the `nfreqs` option in force at the analysis: its walk's, checked
        // once per design (`state_order::four_counts`), not here.
        // R68/A42: an output is a transient vector, so it carries no part (ruled for Fourier)
        let plain = |p: &Probe| probe_valid(p) && p.part == AcPart::None;
        domain_unless(given && !outputs.is_empty() && outputs.iter().all(plain))
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> Option<Self> {
        let fundamental = g.expr(b, features, Reads::Constant);
        // The count from the deck's option only when one is in force here: the item just
        // before, in the same scope (`gen_nfreqs` puts it there; O(1) per row).
        let d = b.design();
        let before = d.items(scope).last().copied();
        let from_option = has(features, RunExtra::FourHarmonicsOption)
            && before.is_some_and(|i| {
                i.kind == ItemKind::Option
                    && OptionRow::table(d).get(i.index).key == OptionKey::NFreqs
            });
        let harmonics = if from_option {
            ExprId::NONE
        } else {
            g.expr(b, features, Reads::Constant)
        };
        let periods = if has(features, RunExtra::FourPeriods) && g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            b.literal(crate::expr::Literal {
                mantissa: 1,
                frac: 0,
                exp10: 0,
                form: crate::expr::NumForm::Plain,
            })
        };
        let grid = if has(features, RunExtra::FourGrid) && g.chance(2) {
            g.expr(b, features, Reads::Constant)
        } else {
            ExprId::NONE
        };
        let p = [crate::dc_ac_facts::gen_output(g, b, features)];
        let outputs = b.probes(&p);
        Some(Four {
            fundamental,
            harmonics,
            periods,
            grid,
            outputs,
        })
    }
}

// ---- private ----

fn cap_feature(cap: StepCap) -> RunExtra {
    match cap {
        StepCap::None => RunExtra::TranCapNone,
        StepCap::NgspiceRule => RunExtra::TranCapNgspice,
        StepCap::SpanOver50 => RunExtra::TranCapSpan50,
        StepCap::SpanOver10 => RunExtra::TranCapSpan10,
    }
}

/// A literal that is not 0, for a field that must not hold the literal 0 (A24): the
/// generator's literal, with a zero replaced by 1.
#[cfg(any(test, feature = "gen"))]
fn nonzero(g: &mut Gen, b: &mut Builder<'_>) -> ExprId {
    let l = g.literal();
    let one = crate::expr::Literal {
        mantissa: 1,
        frac: 0,
        exp10: 0,
        form: crate::expr::NumForm::Plain,
    };
    b.literal(if l.mantissa == 0 { one } else { l })
}
