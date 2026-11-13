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
//! And a non-integer literal count (R99a), from `decks/f33_frac_*`: ngspice-45 rounds it half
//! up (`dec 2.5` is `dec 3`, `lin 2.5` is `lin 3`), VACASK truncates `points` (`2.5` is `2`),
//! neither says a word. A count given by an expression (an ngspice `.param`, `{5/2}`; a
//! VACASK control `var`, `5.0/2`) behaves as its value written literally (R99b).
//!
//! Contents: `pub fn linear_frequency_sweeps_run_what_each_simulator_counts()`,
//! `pub fn log_frequency_sweeps_run_each_simulators_law()`,
//! `pub fn non_integer_counts_round_on_ngspice_and_truncate_on_vacask()`

use std::path::Path;

use crate::sim::{self, Ran, Sim};

/// A frequency sweep's spacing, for the reference grid.
#[derive(Clone, Copy)]
enum Law {
    Dec,
    Lin,
}

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

/// Each `f33_frac_*` deck runs the exact grid of the integer count its title records: the
/// decade grid `1k * 10^(k/n)` to 100k, the linear grid `1k + 1k*k/(n-1)` to 2k.
pub fn non_integer_counts_round_on_ngspice_and_truncate_on_vacask() {
    // (simulator, deck, the law, the integer count it runs as)
    let cases = [
        (Sim::Ngspice, "f33_frac_ac_dec2p4.sp", Law::Dec, 2),
        (Sim::Ngspice, "f33_frac_ac_dec2p5.sp", Law::Dec, 3),
        (Sim::Ngspice, "f33_frac_ac_dec2p6.sp", Law::Dec, 3),
        (Sim::Ngspice, "f33_frac_ac_lin2p5.sp", Law::Lin, 3),
        (Sim::Ngspice, "f33_frac_ac_lin3p5.sp", Law::Lin, 4),
        (Sim::Ngspice, "f33_frac_ac_lin3p6.sp", Law::Lin, 4),
        (Sim::Vacask, "f33_frac_vacask_dec2p4.sim", Law::Dec, 2),
        (Sim::Vacask, "f33_frac_vacask_dec2p5.sim", Law::Dec, 2),
        (Sim::Vacask, "f33_frac_vacask_dec2p6.sim", Law::Dec, 2),
        // VACASK's linear `points` counts intervals: points=1 runs two frequencies
        (Sim::Vacask, "f33_frac_vacask_lin1p5.sim", Law::Lin, 2),
        (Sim::Vacask, "f33_frac_vacask_lin1p6.sim", Law::Lin, 2),
        (Sim::Vacask, "f33_frac_vacask_lin2p5.sim", Law::Lin, 3),
        // a count given by an expression behaves as its value written literally (R99b)
        (Sim::Ngspice, "f33_frac_ac_dec_param2p5.sp", Law::Dec, 3),
        (Sim::Ngspice, "f33_frac_ac_dec_param2p4.sp", Law::Dec, 2),
        (Sim::Ngspice, "f33_frac_ac_lin_param2p5.sp", Law::Lin, 3),
        (Sim::Ngspice, "f33_frac_ac_dec_5by2.sp", Law::Dec, 3),
        (Sim::Vacask, "f33_frac_vacask_dec_var2p5.sim", Law::Dec, 2),
        (Sim::Vacask, "f33_frac_vacask_dec_var2p6.sim", Law::Dec, 2),
        (Sim::Vacask, "f33_frac_vacask_lin_var1p5.sim", Law::Lin, 2),
        (Sim::Vacask, "f33_frac_vacask_dec_5p0by2.sim", Law::Dec, 2),
    ];
    for (simulator, deck, law, n) in cases {
        let want: Vec<f64> = match law {
            Law::Dec => (0..=2 * n)
                .map(|k| 1e3 * 10f64.powf(k as f64 / n as f64))
                .collect(),
            Law::Lin => (0..n)
                .map(|k| 1e3 + 1e3 * k as f64 / (n - 1) as f64)
                .collect(),
        };
        let axis = sweep(simulator, deck);
        assert_eq!(
            axis.len(),
            want.len(),
            "{deck}: {axis:?}, recorded {want:?}"
        );
        for (got, rec) in axis.iter().zip(&want) {
            assert!(
                (got - rec).abs() <= 1e-9 * rec,
                "{deck}: {axis:?}, recorded {want:?}"
            );
        }
    }
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
