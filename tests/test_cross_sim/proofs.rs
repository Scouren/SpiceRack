//! Proof files re-derived: a number a `proof/` file cites, computed again
//! from the reference data it names, and the list lines that cite it.
//!
//! In: `$SPICE_BENCH`'s CMC `.sp.reference` tables and the cells' lists
//! (`tests/fixtures/cross_sim/<cell>.txt`). Out: pass, or the number or the
//! list that no longer holds.
//!
//! Cites: `tests/fixtures/cross_sim/proof/hicum-l2-ac-tm50.md` (ZERO-LEDGER
//! P3-103); mismatches.md §4.3 (the `source` kind).
//!
//! Contents: none (tests only).

use std::collections::BTreeSet;

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
