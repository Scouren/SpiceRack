//! The run owner's features: IR text in, `features_used` out. Every `RunExtra` member has a
//! design that sets it and a legal neighbour, one field away, that does not; the member list
//! is compared by name with `RunExtra::ALL`. One hand-built design's run, analysis, option and
//! item features are the hand-listed set. Oracle: spec example with a legal neighbour (each
//! member's doc, read by hand).
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (features row); `crates/ir/src/run_extra.rs` docs.
//!
//! Contents: tests only.

use spicerack_ir::{
    AnalysisKind, Design, Diags, Feature, FeatureSet, ItemKind, OptionKey, RunExtra, features_used,
    parse,
};

/// `(member, a deck that uses it, its neighbour that does not)`: item lines at depth 1.
#[rustfmt::skip]
const CASES: &[(RunExtra, &str, &str)] = &[
    (RunExtra::SeveralAnalyses, "  analysis op\n  analysis op", "  analysis op"),
    (RunExtra::TranSimulatorStep, "  analysis tran stop=1u cap=none", T),
    (RunExtra::TranRecordStart, "  analysis tran stop=1u record_start=1n print_step=1n cap=none", T),
    (RunExtra::TranMaxStep, "  analysis tran stop=1u print_step=1n max_step=1n", T),
    (RunExtra::TranInitialStep, "  analysis tran stop=1u print_step=1n initial_step=1n cap=none", T),
    (RunExtra::TranCapNone, T, "  analysis tran stop=1u print_step=1n cap=span_over10"),
    (RunExtra::TranCapNgspice, "  analysis tran stop=1u print_step=1n cap=ngspice_rule", T),
    (RunExtra::TranCapSpan50, "  analysis tran stop=1u print_step=1n cap=span_over50", T),
    (RunExtra::TranCapSpan10, "  analysis tran stop=1u print_step=1n cap=span_over10", T),
    (RunExtra::TranUic, "  analysis tran stop=1u print_step=1n cap=none uic", T),
    (RunExtra::TranStartup, "  analysis tran stop=1u print_step=1n cap=none startup", T),
    (RunExtra::TranNoise, "  analysis tran stop=1u print_step=1n noise_fmax=1g noise_fmin=1meg cap=none", T),
    (RunExtra::TranSegments, "  analysis tran stop=1u print_step=1n segments=[1n, 2u] cap=none", T),
    (RunExtra::DcNested, "  analysis dc (source_dc v1) (lin start=0 stop=1 step=1) (source_dc v2) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcRestart, "  analysis dc (source_dc v1) (lin start=0 stop=1 step=1) restart", D),
    (RunExtra::DcAxisTemp, "  analysis dc (temp) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcAxisParam, "  analysis dc (param p) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcAxisSubcktParam, "  analysis dc (subckt_param div r) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcAxisElementValue, "  analysis dc (element_value r1) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcAxisDeviceParam, "  analysis dc (device_param m1 w) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcAxisModelParam, "  analysis dc (model_param nch vth0) (lin start=0 stop=1 step=1)", D),
    (RunExtra::DcAxisOption, "  analysis dc (option gmin) (lin start=0 stop=1 step=1)", D),
    (RunExtra::SweepDcLinStep, D, "  analysis dc (source_dc v1) (list values=[1])"),
    (RunExtra::SweepDcLinPoints, "  analysis dc (source_dc v1) (lin start=1 stop=2 points=2)", D),
    (RunExtra::SweepDcDec, "  analysis dc (source_dc v1) (dec start=1 stop=10 points=2)", D),
    (RunExtra::SweepDcOct, "  analysis dc (source_dc v1) (oct start=1 stop=10 points=2)", D),
    (RunExtra::SweepDcLog, "  analysis dc (source_dc v1) (log start=1 stop=10 points=2)", D),
    (RunExtra::SweepDcList, "  analysis dc (source_dc v1) (list values=[1, 2])", D),
    (RunExtra::SweepLoopLinStep, LL, L),
    (RunExtra::SweepLoopLinPoints, "  loop sweep (temp) (lin start=1 stop=2 points=2)\n    analysis op", L),
    (RunExtra::SweepLoopDec, "  loop sweep (temp) (dec start=1 stop=10 points=2)\n    analysis op", L),
    (RunExtra::SweepLoopOct, "  loop sweep (temp) (oct start=1 stop=10 points=2)\n    analysis op", L),
    (RunExtra::SweepLoopLog, "  loop sweep (temp) (log start=1 stop=10 points=2)\n    analysis op", L),
    (RunExtra::SweepLoopList, L, LL),
    (RunExtra::SweepFreqLinStep, A, "  analysis ac (dec start=1 stop=10 points=2)"),
    (RunExtra::SweepFreqLinPoints, "  analysis ac (lin start=1 stop=2 points=2)", A),
    (RunExtra::SweepFreqDec, "  analysis ac (dec start=1 stop=10 points=2)", A),
    (RunExtra::SweepFreqOct, "  analysis ac (oct start=1 stop=10 points=2)", A),
    (RunExtra::SweepFreqLog, "  analysis ac (log start=1 stop=10 points=2)", A),
    (RunExtra::SweepFreqList, "  analysis ac (list values=[1, 2])", A),
    (RunExtra::SweepEndOnStop, "  analysis ac (lin on_stop start=1 stop=2 step=1)", A),
    (RunExtra::SweepCenterSpan, "  analysis ac (lin center_span start=1 stop=2 step=1)", A),
    (RunExtra::NoiseSummary, "  analysis noise (v (out)) source=v1 (lin start=1 stop=2 step=1) points_per_summary=5", N),
    (RunExtra::NoiseAcSweep, "  analysis noise (v (out)) source=v1 (lin start=1 stop=2 step=1) ac_sweep", N),
    (RunExtra::NoiseRefSourceAc, "  analysis noise (v (out)) source=v1 (lin start=1 stop=2 step=1) source_ac", N),
    (RunExtra::NoiseCurrentOutput, "  analysis noise (i (r1)) source=v1 (lin start=1 stop=2 step=1)", N),
    (RunExtra::PzSourceInput, "  analysis pz source voltage both out_p=out out_n=0 source=v1", "  analysis pz nodes voltage both in_p=in in_n=0 out_p=out out_n=0"),
    (RunExtra::SpPorts, "  analysis sp (lin start=1 stop=2 step=1) ports=(p1)", SP),
    (RunExtra::SpNoise, "  analysis sp (lin start=1 stop=2 step=1) noise", SP),
    (RunExtra::FourPeriods, "  analysis four fundamental=1k harmonics=9 periods=2 (v (out))", F),
    (RunExtra::FourGrid, "  analysis four fundamental=1k harmonics=9 periods=1 grid=200 (v (out))", F),
    (RunExtra::HbMultiTone, "  analysis hb tones=[1g, 2g] harmonics=[5, 5] truncate=box", H),
    (RunExtra::HbIntermod, "  analysis hb tones=[1g] harmonics=[5] intermod=3 truncate=box", H),
    (RunExtra::HbDiamond, "  analysis hb tones=[1g] harmonics=[5] truncate=diamond", H),
    (RunExtra::HbHybrid, "  analysis hb tones=[1g] harmonics=[5] truncate=hybrid", H),
    (RunExtra::PssAutonomous, "  analysis pss autonomous fundamental=1meg tstab=1u osc_p=out osc_n=0", PSS),
    (RunExtra::PssDriven, PSS, "  analysis pss autonomous fundamental=1meg tstab=1u osc_p=out osc_n=0"),
    (RunExtra::PssUic, "  analysis pss driven uic fundamental=1meg tstab=1u", PSS),
    (RunExtra::PeriodicRelative, "  analysis pss driven fundamental=1meg tstab=1u\n  analysis pac (lin start=1 stop=2 step=1) relharm=1 relative", "  analysis pss driven fundamental=1meg tstab=1u\n  analysis pac (lin start=1 stop=2 step=1)"),
    (RunExtra::LoopTable, "  loop table (temp) values=[1]\n    analysis op", L),
    (RunExtra::LoopMonteCarlo, M, L),
    (RunExtra::LoopKeepLast, "  loop sweep keep_last (temp) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopRestore, L, "  loop sweep keep_last (temp) (list values=[1])\n    analysis op"),
    (RunExtra::LoopScoped, "  loop sweep (temp) (list values=[1])\n    analysis op\n  analysis op", L),
    (RunExtra::LoopTemp, L, "  loop sweep (param p) (list values=[1])\n    analysis op"),
    (RunExtra::LoopParam, "  loop sweep (param p) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopSubcktParam, "  loop sweep (subckt_param div r) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopSourceDc, "  loop sweep (source_dc v1) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopElementValue, "  loop sweep (element_value r1) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopDeviceParam, "  loop sweep (device_param m1 w) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopModelParam, "  loop sweep (model_param nch vth0) (list values=[1])\n    analysis op", L),
    (RunExtra::LoopOption, "  loop sweep (option gmin) (list values=[1])\n    analysis op", L),
    (RunExtra::AlterTemp, "  alter (temp) value=1", "  alter (param p) value=1"),
    (RunExtra::AlterParam, "  alter (param p) value=1", AL),
    (RunExtra::AlterSubcktParam, "  alter (subckt_param div r) value=1", AL),
    (RunExtra::AlterSourceDc, "  alter (source_dc v1) value=1", AL),
    (RunExtra::AlterElementValue, "  alter (element_value r1) value=1", AL),
    (RunExtra::AlterDeviceParam, "  alter (device_param m1 w) value=1", AL),
    (RunExtra::AlterModelParam, "  alter (model_param nch vth0) value=1", AL),
    (RunExtra::AlterOption, "  alter (option gmin) value=1", AL),
    (RunExtra::AlterString, "  alter (device_param v1 type) value=\"dc\"", "  alter (device_param v1 type) value=1"),
    (RunExtra::TargetInInstance, "  alter (device_param m1 w path=(x1)) value=1", "  alter (device_param m1 w) value=1"),
    (RunExtra::McSeed, "  loop monte_carlo runs=10 seed=1\n    analysis op", M),
    (RunExtra::McSampleList, "  loop monte_carlo runs=10 samples=[1]\n    analysis op", M),
    (RunExtra::McLatinHypercube, "  loop monte_carlo runs=10 latin_hypercube\n    analysis op", M),
    (RunExtra::McNominal, "  loop monte_carlo runs=10 nominal\n    analysis op", M),
    (RunExtra::McMismatch, "  loop monte_carlo runs=10 mismatch\n    analysis op", M),
    (RunExtra::OptionMidRun, "  analysis op\n  option klu", "  analysis op\n  option klu whole_run"),
    (RunExtra::IcEveryOp, "  nodeset ic every_op node=(a) value=1", S),
    (RunExtra::IcNextOnly, "  nodeset ic transient next_only node=(a) value=1", S),
    (RunExtra::IcDifferential, "  nodeset ic transient node=(a) reference=(b) value=1", S),
    (RunExtra::NodesetAll, "  nodeset nodeset every_op value=1", "  nodeset nodeset every_op node=(a) value=1"),
    (RunExtra::NodeSetHierarchical, "  nodeset ic transient node=(x1 a) value=1", S),
    (RunExtra::IcDevice, "  nodeset device every_op node=(c1) value=1", S),
    (RunExtra::ProbeVdiff, "  save (vdiff (a) (b))", P),
    (RunExtra::ProbeI, "  save (i (v1))", P),
    (RunExtra::ProbeITerminal, "  save (i_terminal terminal=1 (m1))", P),
    (RunExtra::ProbeParam, "  save (param (m1) (gm))", P),
    (RunExtra::ProbePower, "  save (power (r1))", P),
    (RunExtra::ProbeExpr, "  save (expr expr=(v(a) * 2))", P),
    (RunExtra::ProbeAllV, "  save (all_v)", P),
    (RunExtra::ProbeAllVTop, "  save (all_v_top)", P),
    (RunExtra::ProbeAllSourceI, "  save (all_source_i)", P),
    (RunExtra::ProbeAllI, "  save (all_i)", P),
    (RunExtra::ProbeAllPinI, "  save (all_pin_i)", P),
    (RunExtra::ProbeNoise, "  save (output_noise)", P),
    (RunExtra::ProbeHierarchical, "  save (v (x1 out))", P),
    (RunExtra::PartRe, "  save (v re (out))", P),
    (RunExtra::PartIm, "  save (v im (out))", P),
    (RunExtra::PartMag, "  save (v mag (out))", P),
    (RunExtra::PartPhaseDeg, "  save (v phase_deg (out))", P),
    (RunExtra::PartPhaseRad, "  save (v phase_rad (out))", P),
    (RunExtra::PartDb, "  save (v db (out))", P),
    (RunExtra::SavePerAnalysis, "  save analysis=tran (v (out))", P),
    (RunExtra::PrintHbTime, "  print hb hb_time (v (out))", "  print hb (v (out))"),
    (RunExtra::MeasTrigTarg, "  measure m tran trig_targ (at value=1n) (at value=2n)", MP),
    (RunExtra::MeasWhen, MW, MP),
    (RunExtra::MeasFind, "  measure m tran find (v (a)) (at value=1n)", MP),
    (RunExtra::MeasDeriv, "  measure m tran deriv (v (a)) (at value=1n)", MP),
    (RunExtra::MeasStat, "  measure m tran stat avg (v (a)) (bound) (bound)", MP),
    (RunExtra::MeasParam, MP, MW),
    (RunExtra::MeasLast, "  measure m tran when (cross rise last (v (a)) value=1)", MW),
    (RunExtra::MeasDelay, "  measure m tran when (cross rise (v (a)) value=1 count=1 delay=1n)", MW),
    (RunExtra::MeasCircuitThreshold, "  measure m tran when (cross rise (v (a)) value=(v(b)) count=1)", MW),
    (RunExtra::StatsMismatch, "  statistics mismatch (p gauss spread=1)", ST),
    (RunExtra::StatsCorrelation, "  statistics process (p gauss spread=1) (q gauss spread=1) process_correlate (p q cc=0.5)", "  statistics process (p gauss spread=1) (q gauss spread=1)"),
    (RunExtra::StatsTruncate, "  statistics process (p gauss spread=1) truncate=4", ST),
    (RunExtra::VaryLogNormal, "  statistics process (p log_normal spread=1)", ST),
    (RunExtra::VaryPercent, "  statistics process (p gauss percent spread=1)", ST),
    (RunExtra::VaryMean, "  statistics process (p gauss mean=2 spread=1)", ST),
    (RunExtra::LoopOverOp, L, "  loop sweep (temp) (list values=[1])\n    analysis dc_inc"),
    (RunExtra::EnvMidRun, "  analysis op\n  env temp=1", "  env temp=1\n  analysis op"),
    (RunExtra::NodeSetMidRun, "  analysis op\n  nodeset ic transient node=(a) value=1", "  analysis op\n  nodeset ic transient node=(a) value=1 whole_run"),
    (RunExtra::WholeRunMidRun, "  analysis op\n  option klu whole_run", "  option klu whole_run\n  analysis op"),
    (RunExtra::ProbeAllBranchI, "  save (all_branch_i)", P),
    (RunExtra::MeasStatAt, "  measure m tran stat min_at (v (a)) (bound) (bound)", "  measure m tran stat min (v (a)) (bound) (bound)"),
    (RunExtra::MeasUntil, "  measure m tran when (cross rise (v (a)) value=1 count=1 until=1u)", MW),
    (RunExtra::MeasSkipInitial, "  measure m tran when (cross rise skip_initial (v (a)) value=1)", MW),
    (RunExtra::AnalysisNamed, "  analysis op name=o1", "  analysis op"),
    (RunExtra::PssScIter, "  analysis pss driven fundamental=1meg tstab=1u sc_iter=50", PSS),
    (RunExtra::PssSteadyCoeff, "  analysis pss driven fundamental=1meg tstab=1u steady_coeff=1m", PSS),
    (RunExtra::FourHarmonicsOption, "  option n_freqs value=10\n  analysis four fundamental=1k periods=1 (v (out))", "  option n_freqs value=10\n  analysis four fundamental=1k harmonics=9 periods=1 (v (out))"),
    (RunExtra::StateFromHere, "  env temp=1", "  env temp=1 whole_run"),
    (RunExtra::StateWholeRun, "  nodeset ic transient node=(a) value=1 whole_run", S),
    (RunExtra::DcAxisNamed, "  analysis dc (source_dc v1) (lin start=0 stop=1 step=1) name=s1", D),
    (RunExtra::LoopNamed, "  loop sweep name=s1 (temp) (list values=[1])\n    analysis op", L),
    (RunExtra::SweepBodyNotOne, "  loop sweep (temp) (list values=[1])\n    option klu\n    analysis op", "  loop sweep (temp) (list values=[1])\n    comment \"c\"\n    loop sweep (param p) (list values=[1])\n      analysis op"),
    (RunExtra::RunInterleaved, "  analysis op\n  param p = 1\n  save (v (out))", "  analysis op\n  comment \"c\"\n  opaque ngspice private \".x\"\n  save (v (out))\n  param p = 1"),
    (RunExtra::IcBeyondNext, S, "  nodeset ic transient next_only node=(a) value=1"),
    (RunExtra::SaveAnalysisSet, "  save analysis=(op dc tran) (v (out))", "  save analysis=tran (v (out))"),
    (RunExtra::PssOscDiff, "  analysis pss autonomous fundamental=1meg tstab=1u osc_p=out osc_n=ref", "  analysis pss autonomous fundamental=1meg tstab=1u osc_p=out osc_n=0"),
];

const T: &str = "  analysis tran stop=1u print_step=1n cap=none";
const D: &str = "  analysis dc (source_dc v1) (lin start=0 stop=1 step=1)";
const A: &str = "  analysis ac (lin start=1 stop=2 step=1)";
const N: &str = "  analysis noise (v (out)) source=v1 (lin start=1 stop=2 step=1)";
const SP: &str = "  analysis sp (lin start=1 stop=2 step=1)";
const F: &str = "  analysis four fundamental=1k harmonics=9 periods=1 (v (out))";
const H: &str = "  analysis hb tones=[1g] harmonics=[5] truncate=box";
const PSS: &str = "  analysis pss driven fundamental=1meg tstab=1u";
const L: &str = "  loop sweep (temp) (list values=[1])\n    analysis op";
const LL: &str = "  loop sweep (temp) (lin start=0 stop=1 step=1)\n    analysis op";
const M: &str = "  loop monte_carlo runs=10\n    analysis op";
const AL: &str = "  alter (temp) value=1";
const S: &str = "  nodeset ic transient node=(a) value=1";
const P: &str = "  save (v (out))";
const MW: &str = "  measure m tran when (cross rise (v (a)) value=1 count=1)";
const MP: &str = "  measure m tran param value=1";
const ST: &str = "  statistics process (p gauss spread=1)";

// ---- private ----

fn features(lines: &str) -> FeatureSet {
    let text = format!("file \"t.sp\" deck\n{lines}\n");
    let mut d = Design::new();
    let mut diags = Diags::new();
    parse(&text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{text}: {:?}", diags.rows());
    features_used(&d)
}

#[test]
fn every_run_extra_has_a_setter_and_a_neighbour() {
    let mut failed = Vec::new();
    for &(x, with, without) in CASES {
        if !features(with).contains(Feature::Run(x)) || features(without).contains(Feature::Run(x))
        {
            failed.push(format!("{x:?}"));
        }
    }
    assert!(failed.is_empty(), "wrong features: {failed:?}");
    let covered: Vec<String> = CASES.iter().map(|c| format!("{:?}", c.0)).collect();
    let all: Vec<String> = RunExtra::ALL.iter().map(|x| format!("{x:?}")).collect();
    assert_eq!(covered, all, "the table lists RunExtra::ALL in order");
}

#[test]
fn a_hand_built_deck_uses_the_hand_listed_run_features() {
    let used = features(
        "  option klu\n  analysis tran stop=1u print_step=1n cap=none uic\n  save (v (out))",
    );
    let ours: Vec<Feature> = used
        .iter()
        .filter(|f| {
            matches!(
                f,
                Feature::Run(_) | Feature::Analysis(_) | Feature::Option(_)
            ) || matches!(f, Feature::Item(k) if !matches!(k, ItemKind::Title | ItemKind::Comment))
        })
        .collect();
    let want = FeatureSet::EMPTY
        .with(Feature::Item(ItemKind::Analysis))
        .with(Feature::Item(ItemKind::Option))
        .with(Feature::Item(ItemKind::Save))
        .with(Feature::Analysis(AnalysisKind::Tran))
        .with(Feature::Option(OptionKey::Klu))
        .with(Feature::Run(RunExtra::TranCapNone))
        .with(Feature::Run(RunExtra::TranUic))
        .with(Feature::Run(RunExtra::StateFromHere));
    assert_eq!(ours, want.iter().collect::<Vec<_>>());
}

#[test]
fn listed_order_reads_an_include_in_place() {
    // A14: a whole-run option in an included library comes after the deck's analysis when the
    // include stands after it, and before it when the include stands first.
    let used = |deck: &str| {
        let text = format!(
            "file \"t.sp\" deck\n{deck}\nfile \"lib.sp\" library\n  option klu whole_run\n"
        );
        let mut d = Design::new();
        let mut diags = Diags::new();
        parse(&text, &mut d.builder(), &mut diags);
        assert!(diags.is_empty(), "{text}: {:?}", diags.rows());
        features_used(&d).contains(Feature::Run(RunExtra::WholeRunMidRun))
    };
    assert!(used("  analysis op\n  include \"lib.sp\""));
    assert!(!used("  include \"lib.sp\"\n  analysis op"));
}

#[test]
fn a_sweep_form_counts_for_its_use_only() {
    let dc = features("  analysis dc (source_dc v1) (dec start=1 stop=10 points=2)");
    let has = |x| dc.contains(Feature::Run(x));
    assert!(has(RunExtra::SweepDcDec));
    assert!(!has(RunExtra::SweepFreqDec) && !has(RunExtra::SweepLoopDec));
    let lp = features("  loop sweep (temp) (dec start=1 stop=10 points=2)\n    analysis op");
    assert!(lp.contains(Feature::Run(RunExtra::SweepLoopDec)));
    assert!(!lp.contains(Feature::Run(RunExtra::SweepDcDec)));
}
