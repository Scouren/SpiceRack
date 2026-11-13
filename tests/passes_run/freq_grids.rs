//! F32's evidence on the simulators (R97): which frequencies a linear sweep runs, from the
//! decks in `decks/f32_*` (each title records its run). ngspice-45 runs one frequency for a
//! linear `.ac` or `.sp` sweep of two points, literal or a parameter that evaluates to 2, and
//! ten for `lin 0`; its `.noise` runs two; VACASK's `points` counts intervals.
//!
//! Contents: `pub fn linear_frequency_sweeps_run_what_each_simulator_counts()`

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
