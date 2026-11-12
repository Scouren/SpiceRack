//! `check` over the run rows: hand-broken designs in, exactly the one expected violation out;
//! a valid design with every run item kind reports nothing. Oracle: hand-broken fixture, one
//! per invariant the run rows' docs state (each broken by one field through the raw `push`,
//! which asserts only handles and numbers).
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (`check` row); `crates/ir/src/check.rs` (`FieldDomain`,
//! `DetailRow`, `HandleRange`); the run rows' field docs.
//!
//! Contents: tests only.

use spicerack_ir::dc_ac::{Ac, Dc};
use spicerack_ir::diag::violation_code;
use spicerack_ir::measure::{MeasureParam, MeasureWhen};
use spicerack_ir::periodic::{FreqAxis, Hb, HbTruncate, Pac, Pss, PssMode};
use spicerack_ir::plan::{MonteCarlo, Sampler, SweepLoop, TableLoop, Variations};
use spicerack_ir::stats::Correlation;
use spicerack_ir::tran::{Four, StepCap, StepCeiling, Tran};
use spicerack_ir::{
    Alter, Analysis, AnalysisKind, AnalysisKinds, Applies, Builder, Design, Diags, Edge, Elaborate,
    Env, Event, ExprId, IcScope, List, Loop, LoopEnd, LoopKind, Measure, MeasureKind, NameId,
    NodeSet, NodeSetKind, OptionKey, OptionRow, Probe, ProbeKind, Ref, Role, Save, ScopeId,
    ScopeKind, Span, Statistics, SweepSpec, Target, TargetKind, Vary, VaryDist, Violation, check,
};
use spicerack_ir::{Reset, ResetSet};

type Fixture = (&'static str, Violation, fn(&mut Builder<'_>, ScopeId));

/// One broken design per invariant, and the violation it must give alone.
#[rustfmt::skip]
const BROKEN: &[Fixture] = &[
    ("op with a detail row", Violation::DetailRow, |b, s| {
        let tr = tran(b);
        let i = b.detail(tr);
        b.push(s, Analysis { kind: AnalysisKind::Op, name: NameId::NONE, detail: i }, Span::NONE);
    }),
    ("tran without its detail row", Violation::DetailRow, |b, s| {
        b.push(s, Analysis { kind: AnalysisKind::Tran, name: NameId::NONE, detail: 5 }, Span::NONE);
    }),
    ("tran without a stop", Violation::FieldDomain, |b, s| {
        let tr = Tran { stop: NO, ..tran(b) };
        raw(b, s, AnalysisKind::Tran, tr);
    }),
    ("tran noise fmin without fmax", Violation::FieldDomain, |b, s| {
        let f = b.num(1e3);
        let tr = Tran { noise_fmin: f, ..tran(b) };
        raw(b, s, AnalysisKind::Tran, tr);
    }),
    ("tran segments of odd length", Violation::FieldDomain, |b, s| {
        let x = [b.num(1e-9)];
        let segments = b.exprs(&x);
        let tr = Tran { segments, ..tran(b) };
        raw(b, s, AnalysisKind::Tran, tr);
    }),
    ("tran with a literal-zero ceiling", Violation::FieldDomain, |b, s| {
        let zero = b.num(0.0);
        let tr = Tran { ceiling: StepCeiling::Given(zero), ..tran(b) };
        raw(b, s, AnalysisKind::Tran, tr);
    }),
    ("tran with an absent given ceiling", Violation::FieldDomain, |b, s| {
        let tr = Tran { ceiling: StepCeiling::Given(NO), ..tran(b) };
        raw(b, s, AnalysisKind::Tran, tr);
    }),
    ("tran recording from the literal 0", Violation::FieldDomain, |b, s| {
        let zero = b.num(0.0);
        let tr = Tran { record_start: zero, ..tran(b) };
        raw(b, s, AnalysisKind::Tran, tr);
    }),
    ("lin sweep with step and points", Violation::FieldDomain, |b, s| {
        let mut sw = lin(b);
        sw.points = b.num(3.0);
        raw(b, s, AnalysisKind::Ac, Ac { sweep: sw });
    }),
    ("list sweep with a start", Violation::FieldDomain, |b, s| {
        let v = [b.num(1.0)];
        let mut sw = SweepSpec::list(b.exprs(&v));
        sw.start = b.num(1.0);
        raw(b, s, AnalysisKind::Ac, Ac { sweep: sw });
    }),
    ("dc with no axis", Violation::FieldDomain, |b, s| {
        raw(b, s, AnalysisKind::Dc, Dc { axes: List::EMPTY, restart: false });
    }),
    ("hb with fewer harmonics than tones", Violation::FieldDomain, |b, s| {
        let t = [b.num(1e9), b.num(2e9)];
        let tones = b.exprs(&t);
        let h = [b.num(5.0)];
        let harmonics = b.exprs(&h);
        raw(b, s, AnalysisKind::Hb, Hb { tones, harmonics, intermod: NO, truncate: HbTruncate::Box });
    }),
    ("driven pss with an oscillator node", Violation::FieldDomain, |b, s| {
        let (f, t) = (b.num(1e6), b.num(1e-6));
        let osc_p = b.name("out");
        let row = Pss { mode: PssMode::Driven, uic: false, fundamental: f, tstab: t, harmonics: NO, points: NO, osc_p, osc_n: NameId::NONE, sc_iter: NO, steady_coeff: NO };
        raw(b, s, AnalysisKind::Pss, row);
    }),
    ("absolute pac with a relative harmonic", Violation::FieldDomain, |b, s| {
        pss(b, s);
        let sweep = lin(b);
        let relharm = b.num(1.0);
        raw(b, s, AnalysisKind::Pac, Pac { sweep, sidebands: NO, relharm, axis: FreqAxis::Absolute });
    }),
    ("pac with no pss or hb before it", Violation::FieldDomain, |b, s| {
        let sweep = lin(b);
        raw(b, s, AnalysisKind::Pac, Pac { sweep, sidebands: NO, relharm: NO, axis: FreqAxis::Absolute });
    }),
    ("two analyses named alike", Violation::FieldDomain, |b, s| {
        let a = b.name("a");
        b.push(s, Analysis { kind: AnalysisKind::Op, name: a, detail: u32::MAX }, Span::NONE);
        b.push(s, Analysis { kind: AnalysisKind::Op, name: a, detail: u32::MAX }, Span::NONE);
    }),
    ("voltage probe with no node", Violation::FieldDomain, |b, s| {
        let p = Probe { kind: ProbeKind::V, ..Probe::category(ProbeKind::AllV) };
        save(b, s, p);
    }),
    ("terminal probe with no terminal", Violation::FieldDomain, |b, s| {
        let p = Probe { terminal: u8::MAX, ..b.probe_terminal(&["m1"], 1) };
        save(b, s, p);
    }),
    ("alter of the temperature", Violation::FieldDomain, |b, s| {
        let v = b.num(85.0);
        b.push(s, Alter { target: Target::TEMP, value: v }, Span::NONE);
    }),
    ("alter of an option", Violation::FieldDomain, |b, s| {
        let v = b.num(1e-3);
        let target = b.target_option(OptionKey::RelTol, None);
        b.push(s, Alter { target, value: v }, Span::NONE);
    }),
    ("reset of no class", Violation::FieldDomain, |b, s| {
        b.push(s, Reset { restores: ResetSet::EMPTY }, Span::NONE);
    }),
    ("save of nothing", Violation::FieldDomain, |b, s| {
        b.push(s, Save { analyses: AnalysisKinds::ALL, holds: Applies::WholeRun, probes: List::EMPTY }, Span::NONE);
    }),
    ("save for no analysis", Violation::FieldDomain, |b, s| {
        let probes = b.probes(&[Probe::category(ProbeKind::AllV)]);
        b.push(s, Save { analyses: AnalysisKinds::EMPTY, holds: Applies::WholeRun, probes }, Span::NONE);
    }),
    ("save list past its pool", Violation::HandleRange, |b, s| {
        let probes = List { start: 0, len: 4, pool: Default::default() };
        b.push(s, Save { analyses: AnalysisKinds::ALL, holds: Applies::WholeRun, probes }, Span::NONE);
    }),
    ("table loop with a partial row", Violation::FieldDomain, |b, s| {
        let columns = b.targets(&[Target::TEMP, Target::TEMP]);
        let v = [b.num(1.0), b.num(2.0), b.num(3.0)];
        let values = b.exprs(&v);
        raw_loop(b, s, LoopKind::Table, TableLoop { columns, values });
    }),
    ("monte carlo without runs", Violation::FieldDomain, |b, s| {
        let mc = MonteCarlo { runs: NO, seed: NO, samples: List::EMPTY, variations: Variations::Process, sampler: Sampler::Random, nominal: false };
        raw_loop(b, s, LoopKind::MonteCarlo, mc);
    }),
    ("nine nested loops", Violation::FieldDomain, |b, s| {
        let mut scope = s;
        for _ in 0..9 {
            let sweep = lin(b);
            let i = b.detail(SweepLoop { target: Target::TEMP, sweep });
            let row = |body| Loop { kind: LoopKind::Sweep, after: LoopEnd::Restore, name: NameId::NONE, detail: i, body };
            scope = b.push_scoped(scope, ScopeKind::Loop, row, Span::NONE).1;
        }
    }),
    ("alter without a value", Violation::FieldDomain, |b, s| {
        b.push(s, Alter { target: Target::TEMP, value: NO }, Span::NONE);
    }),
    ("parameter target with an instance path", Violation::FieldDomain, |b, s| {
        let mut t = b.target(TargetKind::Param, &[], "p", None);
        t.path = b.names(&["x1"]);
        let v = b.num(1.0);
        b.push(s, Alter { target: t, value: v }, Span::NONE);
    }),
    ("private option without a dialect", Violation::FieldDomain, |b, s| {
        let private = b.name("tran_rmax");
        let v = b.num(1.0);
        b.push(s, OptionRow { key: OptionKey::Private, private, dialect: NameId::NONE, value: v, holds: Applies::FromHere }, Span::NONE);
    }),
    ("flag option with a value", Violation::FieldDomain, |b, s| {
        let v = b.num(1.0);
        b.push(s, OptionRow { key: OptionKey::Klu, private: NameId::NONE, dialect: NameId::NONE, value: v, holds: Applies::FromHere }, Span::NONE);
    }),
    ("env that names neither temperature", Violation::FieldDomain, |b, s| {
        b.push(s, Env { temp: NO, tnom: NO, holds: Applies::WholeRun }, Span::NONE);
    }),
    ("nodeset held in the transient only", Violation::FieldDomain, |b, s| {
        let node = b.names(&["a"]);
        let value = b.num(1.0);
        b.push(s, NodeSet { kind: NodeSetKind::Nodeset, applies: IcScope::Transient, next_only: false, node, reference: List::EMPTY, value, holds: Applies::FromHere }, Span::NONE);
    }),
    ("device state with a reference node", Violation::FieldDomain, |b, s| {
        let node = b.names(&["c1"]);
        let reference = b.names(&["b"]);
        let value = b.num(1.0);
        b.push(s, NodeSet { kind: NodeSetKind::Device, applies: IcScope::EveryOp, next_only: false, node, reference, value, holds: Applies::FromHere }, Span::NONE);
    }),
    ("when measure at the analysis bound", Violation::FieldDomain, |b, s| {
        let i = b.detail(MeasureWhen { at: Event::BOUND });
        let name = b.name("m");
        b.push(s, Measure { name, analysis: AnalysisKind::Tran, kind: MeasureKind::When, detail: i }, Span::NONE);
    }),
    ("crossing with neither a count nor last", Violation::FieldDomain, |b, s| {
        let sig = b.probe_v(&["a"]);
        let v = b.num(1.0);
        let at = Event::cross(sig, Edge::Rise, v, NO);
        b.measure(s, "m", AnalysisKind::Tran, MeasureWhen { at }, Span::NONE);
    }),
    ("crossing picked both by default and by count", Violation::FieldDomain, |b, s| {
        let sig = b.probe_v(&["a"]);
        let (v, c) = (b.num(1.0), b.num(1.0));
        let at = Event { skip_initial: true, ..Event::cross(sig, Edge::Rise, v, c) };
        b.measure(s, "m", AnalysisKind::Tran, MeasureWhen { at }, Span::NONE);
    }),
    ("param measure without a value", Violation::FieldDomain, |b, s| {
        let i = b.detail(MeasureParam { value: NO });
        let name = b.name("m");
        b.push(s, Measure { name, analysis: AnalysisKind::Tran, kind: MeasureKind::Param, detail: i }, Span::NONE);
    }),
    ("vary without a spread", Violation::FieldDomain, |b, s| {
        let p = Ref::unresolved(b.name("p"));
        let process = b.varies(&[Vary { param: p, dist: VaryDist::Gauss, percent: false, mean: NO, spread: NO }]);
        b.statistics(s, Statistics { process, ..stats() }, Span::NONE);
    }),
    ("fourier with no count and no nfreqs option", Violation::FieldDomain, |b, s| {
        let (f, p) = (b.num(1e3), b.num(1.0));
        let o = [b.probe_v(&["out"])];
        let outputs = b.probes(&o);
        raw(b, s, AnalysisKind::Four, Four { fundamental: f, harmonics: NO, periods: p, grid: NO, outputs });
    }),
    ("elaborate of no circuit", Violation::FieldDomain, |b, s| {
        b.push(s, Elaborate { circuits: List::EMPTY }, Span::NONE);
    }),
    ("a parameter correlated with itself", Violation::FieldDomain, |b, s| {
        let p = Ref::unresolved(b.name("p"));
        let cc = b.num(0.5);
        let process_correlations = b.correlations(&[Correlation { a: p, b: p, cc }]);
        b.statistics(s, Statistics { process_correlations, ..stats() }, Span::NONE);
    }),
];

const NO: ExprId = ExprId::NONE;

// ---- private ----

fn tran(b: &mut Builder<'_>) -> Tran {
    let (stop, step) = (b.num(1e-6), b.num(1e-9));
    Tran {
        stop,
        record_start: NO,
        print_step: step,
        initial_step: NO,
        noise_fmax: NO,
        noise_fmin: NO,
        noise_seed: NO,
        segments: List::EMPTY,
        ceiling: StepCeiling::Rule(StepCap::None),
        uic: false,
        startup: false,
    }
}

fn lin(b: &mut Builder<'_>) -> SweepSpec {
    let (x, y, z) = (b.num(1.0), b.num(2.0), b.num(1.0));
    SweepSpec::lin_step(x, y, z)
}

fn stats() -> Statistics {
    Statistics {
        process: List::EMPTY,
        mismatch: List::EMPTY,
        process_correlations: List::EMPTY,
        mismatch_correlations: List::EMPTY,
        truncate: NO,
        process_truncate: NO,
        mismatch_truncate: NO,
    }
}

fn pss(b: &mut Builder<'_>, s: ScopeId) {
    let (f, t) = (b.num(1e6), b.num(1e-6));
    let row = Pss {
        mode: PssMode::Driven,
        uic: false,
        fundamental: f,
        tstab: t,
        harmonics: NO,
        points: NO,
        osc_p: NameId::NONE,
        osc_n: NameId::NONE,
        sc_iter: NO,
        steady_coeff: NO,
    };
    b.analysis(s, None, row, Span::NONE);
}

/// Pushes an analysis header over a detail row without the typed push's asserts.
fn raw<D: spicerack_ir::DetailRow>(b: &mut Builder<'_>, s: ScopeId, kind: AnalysisKind, row: D) {
    let detail = b.detail(row);
    b.push(
        s,
        Analysis {
            kind,
            name: NameId::NONE,
            detail,
        },
        Span::NONE,
    );
}

fn raw_loop<D: spicerack_ir::DetailRow>(b: &mut Builder<'_>, s: ScopeId, kind: LoopKind, row: D) {
    let detail = b.detail(row);
    let f = |body| Loop {
        kind,
        after: LoopEnd::Restore,
        name: NameId::NONE,
        detail,
        body,
    };
    b.push_scoped(s, ScopeKind::Loop, f, Span::NONE);
}

fn save(b: &mut Builder<'_>, s: ScopeId, p: Probe) {
    let probes = b.probes(&[p]);
    b.push(
        s,
        Save {
            analyses: AnalysisKinds::ALL,
            holds: Applies::WholeRun,
            probes,
        },
        Span::NONE,
    );
}

fn codes(build: impl FnOnce(&mut Builder<'_>, ScopeId)) -> Vec<spicerack_ir::Code> {
    let mut d = Design::new();
    {
        let mut b = d.builder();
        let top = b.file("t.sp", Role::Deck).1;
        build(&mut b, top);
    }
    let mut diags = Diags::new();
    check(&d, &mut diags);
    diags.rows().iter().map(|r| r.code).collect()
}

#[test]
fn each_broken_run_row_reports_its_violation_alone() {
    let mut wrong = Vec::new();
    for &(what, v, build) in BROKEN {
        let got = codes(build);
        if got != [violation_code(v)] {
            wrong.push(format!("{what}: {got:?}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn a_valid_run_reports_nothing() {
    let got = codes(|b, s| {
        let (t, tn) = (b.num(27.0), b.num(27.0));
        b.env(s, t, tn, Span::NONE);
        b.env(s, ExprId::NONE, tn, Span::NONE); // an env sets what it names (A14)
        let one = b.num(1e-3);
        b.option(s, OptionKey::RelTol, one, Span::NONE);
        b.option_private(s, "xyce", "timeint.reltol", one, Span::NONE);
        b.node_set(
            s,
            NodeSetKind::Ic,
            IcScope::Transient,
            &["a"],
            one,
            Span::NONE,
        );
        let r = b.target(TargetKind::Param, &[], "r", None);
        let sweep = lin(b);
        let (_, body) = b.loop_(
            s,
            Some("sw"),
            SweepLoop { target: r, sweep },
            LoopEnd::Restore,
            Span::NONE,
        );
        let r1 = b.target(TargetKind::ElementValue, &[], "r1", None);
        b.alter(body, r1, one, Span::NONE);
        let tr = tran(b);
        b.analysis(body, Some("t1"), tr, Span::NONE);
        pss(b, body);
        let sweep = lin(b);
        b.analysis(
            body,
            None,
            Pac {
                sweep,
                sidebands: NO,
                relharm: NO,
                axis: FreqAxis::Absolute,
            },
            Span::NONE,
        );
        let p = b.probe_v(&["out"]);
        b.save(s, AnalysisKinds::ALL, Applies::WholeRun, &[p], Span::NONE);
        let half = b.num(0.5);
        let at = Event::cross(p, Edge::Rise, half, one);
        b.measure(
            body,
            "m",
            AnalysisKind::Tran,
            MeasureWhen { at },
            Span::NONE,
        );
        let q = Ref::unresolved(b.name("q"));
        let process = b.varies(&[Vary {
            param: q,
            dist: VaryDist::Gauss,
            percent: false,
            mean: NO,
            spread: one,
        }]);
        b.statistics(s, Statistics { process, ..stats() }, Span::NONE);
        b.elaborate(s, &["top2"], Span::NONE);
        let ten = b.num(10.0);
        b.option(s, OptionKey::NFreqs, ten, Span::NONE);
        let (f, p) = (b.num(1e3), b.num(1.0));
        let o = [b.probe_v(&["out"])];
        let outputs = b.probes(&o);
        let four = Four {
            fundamental: f,
            harmonics: NO,
            periods: p,
            grid: NO,
            outputs,
        };
        b.analysis(body, None, four, Span::NONE);
    });
    assert!(got.is_empty(), "{got:?}");
}
