//! VACASK's default step ceiling as one rule (`StepCap::VacaskRule`): with no `maxstep` the
//! step is at most min(span/50, 1/(8·f)), f the highest frequency of a sine source (`tran_minpts`
//! 50, `tran_ffmax` 0.25 over `2·maxFreq`, only `Sine` sources set it). Read and written in IR
//! text as `cap=vacask_rule`, with its own feature.
//!
//! Oracle: VACASK runs (`tests/fixtures/vacask_step/runs.out`): a resistor driven by a 10 kHz
//! sine steps at most 12.5 µs over 1 ms, by 100 kHz 1.25 µs, by 1 kHz and by DC 20 µs; the
//! source lines `lib/coretran.cpp:1020-1040`, `lib/devvisrc.cpp:688-691` of VACASK-36be560.
//!
//! Contents: tests only.

use spicerack_ir::{Design, Diags, Feature, RunExtra, dump, features_used};

// ---- private ----

#[test]
fn vacasks_default_ceiling_reads_writes_and_counts() {
    let text = "file \"t.sp\" deck\n  analysis tran stop=1m print_step=1u cap=vacask_rule\n";
    let mut d = Design::new();
    let mut diags = Diags::new();
    spicerack_ir::parse(text, &mut d.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    let mut out = String::new();
    dump(&d, d.entry(), &mut out);
    assert!(out.contains("cap=vacask_rule"), "{out}");
    let member = RunExtra::ALL
        .into_iter()
        .find(|x| format!("{x:?}") == "TranCapVacask")
        .expect("a member for VACASK's rule");
    assert!(features_used(&d).contains(Feature::Run(member)));
}
