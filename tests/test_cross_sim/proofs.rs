//! Proof files re-derived: a number a `proof/` file cites, computed again
//! from the reference data it names, and the list lines that cite it.
//!
//! In: `$SPICE_BENCH`'s CMC `.sp.reference` tables and the cells' lists
//! (`tests/fixtures/cross_sim/<cell>.txt`). Out: pass, or the number or the
//! list that no longer holds.
//!
//! Cites: `tests/fixtures/cross_sim/proof/hicum-l2-ac-tm50.md` (ZERO-LEDGER
//! P3-103); mismatches.md §4.3 (the `source` kind);
//! `tests/fixtures/cross_sim/proof/hsa-hspice-analysis-cards.md` (ZERO-LEDGER
//! P1-066; Star-Hspice Manual 1998.2 p.7-4, p.9-16, p.25-3).
//!
//! Contents: none (tests only).

use std::collections::BTreeSet;
use std::path::Path;

use crate::sim::{Ran, Sim, run};

/// The HICUM/L2 ac decks at -50 °C whose `i(vbase_9)` ngspice and ESPice
/// disagree on: `cmcqa/hicum/hicum_<name>_ac_Tm50.sp`.
const TM50: [&str; 8] = [
    "CBC_CCS_npn_1D",
    "CBC_CCS_npn_1D_breakdown",
    "CBC_CCS_npn_1D_no_aval",
    "CBC_CCS_npn_vert_nqs",
    "CBE_npn_1D",
    "CBE_npn_1D_breakdown",
    "CBE_npn_1D_no_aval",
    "CBE_npn_vert_nqs",
];

/// Im i(vbase_9) at 10 GHz of `hicum_CBC_CCS_npn_1D_ac_Tm50.sp` in A, on
/// ngspice-45 and on ESPice 0.3.0 (2026-11-11, the source deck; the list
/// lines of all eight decks hold this pair).
const NGSPICE: f64 = 1.8342421968148607e-4;
const ESPICE: f64 = 1.8305015572738133e-4;

/// The three HSPICE analysis cards of `proof/hsa-hspice-analysis-cards.md`,
/// each in an RC deck as the HSPICE writer spells it, with the words of the
/// card ngspice's hsa mode quotes when it stops.
const HSA_CARDS: [(&str, &str); 3] = [
    (
        "* noise\nVin in 0 0 AC 1\nR1 in out 1k\nC1 out 0 1n\n.ac dec 2 1k 100k\n.noise V(out) vin 0\n.end\n",
        ".noise v(out) vin 0",
    ),
    (
        "* pz\nVin in 0 0 AC 1\nR1 in out 1k\nC1 out 0 1n\n.pz v(out) Vin\n.end\n",
        ".pz v(out) vin",
    ),
    (
        "* tran\nVin in 0 0 PULSE(0 1 0 1n 1n 1u 2u)\nR1 in out 1k\nC1 out 0 1n\n.tran 10n 2u START=1u\n.end\n",
        ".tran 10n 2u start=1u",
    ),
];

// ---- private ----

/// ZERO-LEDGER P3-103. Oracle: the CMC reference table beside each deck
/// (`<deck>.reference`): Im i(vbase_9) is ω·c(base,coll) of the fifth bias
/// (V(coll) = 0.8, copy 9) at 10 GHz. ESPice's value is the reference's,
/// ngspice-45's is 0.204 % above it; the lines of the two ESPice cells that
/// cite the proof are these eight decks, by name.
#[test]
fn p3_103_hicum_ac_tm50_is_ngspices_capacitance() {
    let root = std::env::var("SPICE_BENCH").expect("$SPICE_BENCH (nix develop .#testing)");
    let omega = 2.0 * std::f64::consts::PI * 1e10;
    for name in TM50 {
        let path = format!("{root}/cmcqa/hicum/hicum_{name}_ac_Tm50.sp.reference");
        let table = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let mut rows = table.lines();
        let head = rows.next().expect("a header line");
        let col = head
            .split_whitespace()
            .position(|c| c == "c(base,coll)")
            .expect("the c(base,coll) column");
        // One block of 11 frequencies per bias: the fifth is V(coll) = 0.8.
        let row = rows
            .filter(|r| r.starts_with("10000000000 "))
            .nth(4)
            .expect("the fifth 10 GHz row");
        let cell = row.split_whitespace().nth(col).expect("the row's cell");
        let reference = omega * cell.parse::<f64>().expect("a number");
        assert!(
            (ESPICE / reference - 1.0).abs() < 1e-6,
            "{name}: ESPice {ESPICE} against the reference {reference}"
        );
        let off = NGSPICE / reference - 1.0;
        assert!(
            (2.0e-3..2.1e-3).contains(&off),
            "{name}: ngspice is {off} off the reference"
        );
    }
    let want: BTreeSet<String> = TM50
        .iter()
        .map(|n| format!("cmcqa/hicum/hicum_{n}_ac_Tm50.sp"))
        .collect();
    let cites = " simulator-differs:source reason=HICUM-L2-AC-TM50 proof=hicum-l2-ac-tm50.md ";
    for cell in ["ngspice-espice", "ngspice-hspice-espice"] {
        let dir = env!("CARGO_MANIFEST_DIR");
        let path = format!("{dir}/tests/fixtures/cross_sim/{cell}.txt");
        let list = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let cited: BTreeSet<String> = list
            .lines()
            .filter(|l| l.contains(cites))
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect();
        assert_eq!(
            cited, want,
            "{cell}: the lines citing proof/hicum-l2-ac-tm50.md"
        );
    }
}

/// ZERO-LEDGER P1-066. Oracle: the two simulators. ngspice-45 in its hsa
/// mode stops on each card of [`HSA_CARDS`], naming it; the pinned ESPice
/// HSPICE reader runs the same text to a plot (Star-Hspice Manual 1998.2
/// p.9-16 `.NOISE ovv srcnam inter`, p.25-3 `.PZ ov srcnam`, p.7-4 `.TRAN …
/// START=`). The hsa list's lines that cite the proof are the lines whose
/// reason quotes one of the three cards, by name, and no `target-fails`
/// line is left that quotes one. Checks: 3 x 2 + 3.
#[test]
fn p1_066_hsa_stops_on_the_hspice_cards_espice_reads() {
    const SECS: u64 = 60;
    let at = Path::new("/nonexistent/t.sp");
    for (deck, card) in HSA_CARDS {
        match run(Sim::NgspiceHsa, at, deck, &[], SECS) {
            Ran::Failed(e) => assert!(e.contains(card), "hsa on `{card}`: {e}"),
            Ran::Plots(p) => panic!("hsa reads `{card}`: {} plots; the proof is stale", p.len()),
            Ran::Timeout => panic!("hsa on `{card}`: past {SECS} s"),
        }
        match run(Sim::EspiceHspice, at, deck, &[], SECS) {
            Ran::Plots(p) => assert!(!p.is_empty(), "ESPice on `{card}`: no plot"),
            Ran::Failed(e) => panic!("ESPice's HSPICE reader on `{card}`: {e}"),
            Ran::Timeout => panic!("ESPice on `{card}`: past {SECS} s"),
        }
    }
    let dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{dir}/tests/fixtures/cross_sim/ngspice-hspice-hsa.txt");
    let list = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let cites = " simulator-fails reason=S-HSA-HSPICE-CARD proof=hsa-hspice-analysis-cards.md ";
    let quotes = |l: &&str| {
        let why = l.split_once('#').map_or("", |(_, w)| w);
        why.contains("substitute:: .noise ")
            || why.contains("substitute:: .pz ")
            || (why.contains("substitute:: .tran ") && why.contains(" start="))
    };
    let decks = |keep: &dyn Fn(&&str) -> bool| -> BTreeSet<String> {
        list.lines()
            .filter(keep)
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect()
    };
    let cited = decks(&|l| l.contains(cites));
    assert_eq!(
        cited,
        decks(&quotes),
        "the lines citing the proof are the three cards'"
    );
    for deck in [
        "cmcqa/hicum/hicum_noise_npn_1D_ac_0.sp",
        "ngspice/tests/polezero/filt_rc.sp",
        "circuitsim90/MOS2/e1480.sp",
    ] {
        assert!(cited.contains(deck), "{deck} cites the proof");
    }
    let left = decks(&|l| quotes(l) && l.contains(" target-fails "));
    assert!(
        left.is_empty(),
        "target-fails lines that quote a card: {left:?}"
    );
}

/// ZERO-LEDGER P1-066. Oracle: ngspice-45 and ESPice on one text, the
/// original `circuitsim90/MOS2/gm6.sp` (no writer between them): v(13), the
/// node between two off MOSFETs, is 0.409 V on ngspice and 0 V on ESPice,
/// as `proof/s-ngspice-outlier-3rd.md` records (Xyce and VACASK give 0 V
/// on the decks written for them). The lines that cite the proof are the
/// five below, by name. Checks: 2 + 3.
#[test]
fn p1_066_gm6_and_vreg_are_ngspices_own() {
    const SECS: u64 = 60;
    let root = std::env::var("SPICE_BENCH").expect("$SPICE_BENCH (nix develop .#testing)");
    let path = format!("{root}/circuitsim90/MOS2/gm6.sp");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    // v(13) of the run's one-point plot (the `.op`).
    let v13 = |sim: Sim| -> f64 {
        let plots = match run(sim, Path::new(&path), &text, &[], SECS) {
            Ran::Plots(p) => p,
            Ran::Failed(e) => panic!("{sim:?} on gm6: {e}"),
            Ran::Timeout => panic!("{sim:?} on gm6: past {SECS} s"),
        };
        let op = plots
            .iter()
            .find(|p| !p.is_complex && p.re(0).len() == 1)
            .unwrap_or_else(|| panic!("{sim:?}: no operating-point plot"));
        let at = op
            .variables
            .iter()
            .position(|v| matches!(v.name.to_ascii_lowercase().as_str(), "v(13)" | "13"))
            .unwrap_or_else(|| panic!("{sim:?}: no v(13) among {:?}", op.variables));
        op.re(at)[0]
    };
    let ngspice = v13(Sim::Ngspice);
    assert!(
        (ngspice - 0.408_986_547_625_635_85).abs() < 1e-6,
        "ngspice v(13) = {ngspice}"
    );
    let espice = v13(Sim::Espice);
    assert!(espice.abs() < 1e-6, "ESPice v(13) = {espice}");
    let cites = " simulator-differs:third-simulator reason=S-NGSPICE-OUTLIER-3RD proof=s-ngspice-outlier-3rd.md ";
    for (cell, want) in [
        (
            "ngspice-xyce",
            ["circuitsim90/BJT/vreg.sp", "circuitsim90/MOS2/gm6.sp"],
        ),
        (
            "ngspice-vacask",
            ["circuitsim90/BJT/vreg.sp", "circuitsim90/MOS2/gm6.sp"],
        ),
        (
            "xyce-ngspice",
            ["CircuitSim90/BJT/vreg.cir", "CircuitSim90/MOS2/gm6.cir"],
        ),
    ] {
        let dir = env!("CARGO_MANIFEST_DIR");
        let file = format!("{dir}/tests/fixtures/cross_sim/{cell}.txt");
        let list = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file}: {e}"));
        let cited: BTreeSet<String> = list
            .lines()
            .filter(|l| l.contains(cites))
            .filter_map(|l| l.split_whitespace().next().map(str::to_string))
            .collect();
        let want: BTreeSet<String> = want.iter().map(|d| d.to_string()).collect();
        assert_eq!(
            cited, want,
            "{cell}: the lines citing proof/s-ngspice-outlier-3rd.md"
        );
    }
}

/// ZERO-LEDGER P1-066. Seven decks are the BSIM3 comparator behind binned
/// model cards (`nmos.1`…`nmos.4`, every parameter default) or `.options
/// parser scale`; Xyce and ngspice each give them the baseline's run, so
/// their difference is S-BSIM3-DEFAULT-CAPMOD's (LTspice sides with Xyce).
/// Oracle: the comments the harness blessed from the two simulators: each
/// of the seven leaves the tube at `COMPARATOR/comparator.cir`'s sample,
/// value and tolerance, character for character; and the lines citing the
/// proof are the 23 below, by name. Checks: 7 + 1.
#[test]
fn p1_066_the_binned_comparators_are_the_capmod_family() {
    const BINNED: [&str; 7] = [
        "Certification_Tests/BUG_1041_SON/bsim3_comparatorBinningScale.cir",
        "Certification_Tests/BUG_1041_SON/bsim3_comparatorBinningScaleRef.cir",
        "Certification_Tests/ISSUE_239/bsim3_test2.cir",
        "Certification_Tests/ISSUE_239/bsim3_test2Ref.cir",
        "MODEL_BINNING/comparatorBinning.cir",
        "MODEL_BINNING/comparatorBinning_forGlobal.cir",
        "MODEL_BINNING/globalPar_comparatorBinning.cir",
    ];
    const FAMILY: [&str; 16] = [
        "COMPARATOR/comparator.cir",
        "MODEL_BINNING/comparatorBaseline.cir",
        "MODEL_BINNING/comparatorBaseline_forGlobal.cir",
        "MODEL_BINNING/globalPar_comparatorBaseline.cir",
        "Nonlinear/ConvergenceTests/NLStrategy2.cir",
        "Nonlinear/ConvergenceTests/NLStrategy5.cir",
        "Nonlinear/ConvergenceTests/NLStrategy6.cir",
        "Nonlinear/ConvergenceTests/Nox0.cir",
        "Nonlinear/ConvergenceTests/SearchMethod1.cir",
        "Nonlinear/ConvergenceTests/SearchMethod2.cir",
        "Nonlinear/ConvergenceTests/SearchMethod3.cir",
        "Nonlinear/ConvergenceTests/SearchMethod4.cir",
        "Nonlinear/ConvergenceTests/XyceTests.cir",
        "OP/comparator_op.cir",
        "POWER/power_bsim3_gear.cir",
        "POWER/power_bsim3_trap.cir",
    ];
    let dir = env!("CARGO_MANIFEST_DIR");
    let file = format!("{dir}/tests/fixtures/cross_sim/xyce-ngspice.txt");
    let list = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let cites = " simulator-differs:third-simulator reason=S-BSIM3-DEFAULT-CAPMOD proof=s-bsim3-default-capmod.md ";
    // What the harness observed, after the class: `# tran V(1): … (tol …)`.
    let observed = |deck: &str| {
        let start = format!("{deck}{cites}");
        list.lines().find_map(|l| l.strip_prefix(start.as_str()))
    };
    let baseline = observed(FAMILY[0]).expect("the family's own line");
    assert!(
        baseline.starts_with("# tran V(1): -0.376285159"),
        "{baseline}"
    );
    for deck in BINNED {
        assert_eq!(observed(deck), Some(baseline), "{deck}");
    }
    let cited: BTreeSet<&str> = list
        .lines()
        .filter(|l| l.contains(cites))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    let want: BTreeSet<&str> = FAMILY.iter().chain(&BINNED).copied().collect();
    assert_eq!(
        cited, want,
        "the lines citing proof/s-bsim3-default-capmod.md"
    );
}

/// ZERO-LEDGER P1-066. Oracle: the CMC reference table beside each deck
/// (`<deck>.reference`, `Freq N(coll)` in V²/Hz): the output noise at 1 kHz
/// is sqrt(N(coll)) of the first row. Xyce 7.10.0's value is the
/// reference's; ngspice-45's hicum2 is 2.45 %, 5.99 % and 3.48 % under it
/// at biases 2, 3 and 4 of the six model cards that turn self-heating on
/// (`flsh=1`). The lines of `ngspice-xyce` that cite the proof are those 18
/// decks, by name. Checks: 18 × 2 + 1.
#[test]
fn p1_066_hicum_noise_with_self_heating_is_ngspices() {
    /// `cmcqa/hicum/hicum_noise_npn_<card>_ac_<bias>.sp`: the cards with
    /// `flsh=1`.
    const CARDS: [&str; 6] = [
        "cornoise",
        "full_sh",
        "full_subcoupl",
        "full_subcoupl_peri_bias",
        "full_subcoupl_peri_con",
        "full_subtran",
    ];
    /// Bias; `onoise_spectrum` at 1 kHz in V/√Hz on ngspice-45 and on Xyce
    /// 7.10.0 (2026-11-11, `full_subcoupl`; the cards agree to 11 digits);
    /// how far ngspice is under the reference.
    const AT_1K: [(u32, f64, f64, f64); 3] = [
        (2, 1.1523105743045234e-10, 1.1809686702025586e-10, 0.02454),
        (3, 1.5281348231615892e-10, 1.6257564393229386e-10, 0.05993),
        (4, 1.5613271234089656e-10, 1.617558963376606e-10, 0.03483),
    ];
    let root = std::env::var("SPICE_BENCH").expect("$SPICE_BENCH (nix develop .#testing)");
    let mut want = BTreeSet::new();
    for card in CARDS {
        for (bias, ngspice, xyce, under) in AT_1K {
            let deck = format!("cmcqa/hicum/hicum_noise_npn_{card}_ac_{bias}.sp");
            let path = format!("{root}/{deck}.reference");
            let table = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let row = table
                .lines()
                .find(|r| r.starts_with("1000 "))
                .expect("the 1 kHz row");
            let cell = row.split_whitespace().nth(1).expect("N(coll)");
            let reference = cell.parse::<f64>().expect("a number").sqrt();
            assert!(
                (xyce / reference - 1.0).abs() < 5e-4,
                "{deck}: Xyce {xyce} against the reference {reference}"
            );
            let off = 1.0 - ngspice / reference;
            assert!(
                (off - under).abs() < 1e-4,
                "{deck}: ngspice is {off} under the reference"
            );
            want.insert(deck);
        }
    }
    let cites =
        " simulator-differs:reference-data reason=HICUM-L2-NOISE-SH proof=hicum-l2-noise-sh.md ";
    let dir = env!("CARGO_MANIFEST_DIR");
    let file = format!("{dir}/tests/fixtures/cross_sim/ngspice-xyce.txt");
    let list = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{file}: {e}"));
    let cited: BTreeSet<String> = list
        .lines()
        .filter(|l| l.contains(cites))
        .filter_map(|l| l.split_whitespace().next().map(str::to_string))
        .collect();
    assert_eq!(cited, want, "the lines citing proof/hicum-l2-noise-sh.md");
}
