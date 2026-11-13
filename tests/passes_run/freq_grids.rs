//! F32's evidence on the simulators (R97): which frequencies a linear sweep runs, from the
//! decks in `decks/f32_*` (each title records its run). ngspice-45 runs one frequency for a
//! linear `.ac` or `.sp` sweep of two points, literal or a parameter that evaluates to 2, and
//! ten for `lin 0`; its `.noise` runs two; VACASK's `points` counts intervals.
//!
//! And F33's (R99): a log sweep's points under each simulator's law (ngspice's decade law
//! floor-stretches at a decade or more and keeps the exact grid under it, its octave law keeps
//! the exact grid; VACASK ceiling-stretches both), from `decks/f33_*`; VACASK's `values=[…]`
//! returns its values exactly; ngspice's `.ac` has no list form.
//!
//! Contents: `pub fn linear_frequency_sweeps_run_what_each_simulator_counts()`,
//! `pub fn log_frequency_sweeps_run_each_simulators_law()`

use std::path::Path;

use crate::sim::{self, Ran, Sim};

/// Each deck's frequency count on its simulator, as its title records.
pub fn linear_frequency_sweeps_run_what_each_simulator_counts() {
    let cases = [
        (Sim::Ngspice, "f32_ac_lin2.sp", 1),
        (Sim::Ngspice, "f32_ac_lin_param2.sp", 1),
        (Sim::Ngspice, "f32_ac_lin0.sp", 10),
        (Sim::Ngspice, "f32_sp_lin2.sp", 1),
        (Sim::Ngspice, "f32_noise_lin2.sp", 2),
        (Sim::Vacask, "f32_vacask_lin_points1.sim", 2),
    ];
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/passes_run/decks");
    for (simulator, deck, want) in cases {
        let path = dir.join(deck);
        let text = std::fs::read_to_string(&path).expect("the deck");
        let plots = match sim::run(simulator, &path, &text, &[], 60) {
            Ran::Plots(p) => p,
            Ran::Timeout => panic!("{deck}: past the CPU budget"),
            Ran::Failed(why) => panic!("{deck}: {why}"),
        };
        // the first plot is the sweep (noise: its spectral density curves)
        let p = &plots[0];
        let n = p
            .real_data
            .first()
            .map_or(0, Vec::len)
            .max(p.complex_data.first().map_or(0, Vec::len));
        assert_eq!(n, want, "{deck}: {} frequencies, recorded {want}", n);
    }
}

/// Each `f33_*` deck's count and the values that tell the laws apart, as its title records.
pub fn log_frequency_sweeps_run_each_simulators_law() {
    let cases: [(Sim, &str, usize, &[f64]); 13] = [
        (Sim::Ngspice, "f33_ac_dec10_10_20k.sp", 34, &[10.0, 20000.0]),
        (Sim::Ngspice, "f33_ac_dec2_1k_5k.sp", 2, &[1000.0, 3162.28]),
        (Sim::Ngspice, "f33_ac_oct3_1k_5k.sp", 7, &[1000.0, 4000.0]),
        (Sim::Ngspice, "f33_ac_dec10_1_1meg.sp", 61, &[1.0, 1e6]),
        (
            Sim::Ngspice,
            "f33_noise_dec2_1k_5k.sp",
            2,
            &[1000.0, 3162.28],
        ),
        (
            Sim::Ngspice,
            "f33_noise_oct3_1k_5k.sp",
            7,
            &[1000.0, 4000.0],
        ),
        (Sim::Ngspice, "f33_sp_dec2_1k_5k.sp", 2, &[1000.0, 3162.28]),
        (
            Sim::Vacask,
            "f33_vacask_dec10_10_20k.sim",
            35,
            &[10.0, 20000.0],
        ),
        (
            Sim::Vacask,
            "f33_vacask_dec2_1k_5k.sim",
            3,
            &[1000.0, 2236.07, 5000.0],
        ),
        (
            Sim::Vacask,
            "f33_vacask_oct3_1k_5k.sim",
            8,
            &[1000.0, 5000.0],
        ),
        (Sim::Vacask, "f33_vacask_dec10_1_1meg.sim", 61, &[1.0, 1e6]),
        (
            Sim::Vacask,
            "f33_vacask_dc_dec2.sim",
            3,
            &[1.0, 2.23607, 5.0],
        ),
        (
            Sim::Vacask,
            "f33_vacask_ac_values.sim",
            5,
            &[
                1000.0,
                3162.2776601683795,
                12345.678,
                2236.06797749979,
                1000.0,
            ],
        ),
    ];
    for (simulator, deck, want, ends) in cases {
        let axis = sweep(simulator, deck);
        assert_eq!(
            axis.len(),
            want,
            "{deck}: {} points, recorded {want}",
            axis.len()
        );
        // the recorded values: the first and the last (all of them where three or more are
        // recorded), to six digits as printed, exactly for the listed values
        let picks: Vec<f64> = match ends.len() {
            2 => vec![axis[0], axis[want - 1]],
            _ => axis.clone(),
        };
        for (got, &rec) in picks.iter().zip(ends) {
            let exact = deck.ends_with("ac_values.sim");
            let ok = if exact {
                got.to_bits() == rec.to_bits()
            } else {
                (got - rec).abs() <= 1e-5 * rec.abs()
            };
            assert!(ok, "{deck}: {got} against the recorded {rec}");
        }
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/passes_run/decks/f33_ac_list.sp");
    let text = std::fs::read_to_string(&path).expect("the deck");
    assert!(
        matches!(
            sim::run(Sim::Ngspice, &path, &text, &[], 60),
            Ran::Failed(_)
        ),
        "ngspice took `.ac list`"
    );
}

// ---- private ----

/// The first plot's sweep vector (its first column), real part.
fn sweep(simulator: Sim, deck: &str) -> Vec<f64> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/passes_run/decks")
        .join(deck);
    let text = std::fs::read_to_string(&path).expect("the deck");
    let plots = match sim::run(simulator, &path, &text, &[], 60) {
        Ran::Plots(p) => p,
        Ran::Timeout => panic!("{deck}: past the CPU budget"),
        Ran::Failed(why) => panic!("{deck}: {why}"),
    };
    let p = &plots[0];
    match p.real_data.first() {
        Some(c) if !c.is_empty() => c.clone(),
        _ => p.complex_data[0].iter().map(|z| z.re).collect(),
    }
}
