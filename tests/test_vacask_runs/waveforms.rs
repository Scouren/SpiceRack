//! The source waveforms VACASK does run (sine with delay and damping, a
//! pulse train), emitted and checked against their closed forms.
//!
//! Cites: real VACASK; ngspice manual §4.1 for the waveform definitions.
//!
//! Contents: none (tests only).

use super::*;

// ---- private ----

#[test]
fn a_sine_source_runs_with_delay_and_damping() {
    let raw = run("sine\nR1 vin 0 1k\nV1 vin 0 0 SIN(0.5 2 1k 0 0 0)\n.tran 10u 2m 0 10u\n.end\n");
    let t = col(&raw, "time");
    let v = col(&raw, "vin");
    // A quarter period into a 1 kHz sine: offset + amplitude.
    let (i, _) = t
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1 - 2.5e-4)
                .abs()
                .partial_cmp(&(b.1 - 2.5e-4).abs())
                .unwrap()
        })
        .unwrap();
    close(v[i], 2.5, 5e-3);
    close(v[0], 0.5, 1e-6);
}

#[test]
fn a_pulse_train_runs() {
    let raw = run(
        "pulse\nR1 vin 0 1k\nV1 vin 0 0 PULSE(0 1 100u 1u 1u 100u 200u)\n.tran 1u 500u 0 1u\n.end\n",
    );
    let t = col(&raw, "time");
    let v = col(&raw, "vin");
    let at = |want: f64| {
        let (i, _) = t
            .iter()
            .enumerate()
            .min_by(|a, b| (a.1 - want).abs().partial_cmp(&(b.1 - want).abs()).unwrap())
            .unwrap();
        v[i]
    };
    close(at(5e-5), 0.0, 1e-6); // before the delay
    close(at(1.5e-4), 1.0, 1e-3); // mid-pulse
    close(at(2.5e-4), 0.0, 1e-3); // after the fall
    close(at(3.5e-4), 1.0, 1e-3); // second period
}

#[test]
fn a_waveform_sources_dc_value_drives_the_dc_analyses() {
    // ngspice-45 (run by hand: "dc value used for op instead of transient
    // time=0 value") gives the op and every `.dc` point the card's DC value
    // and starts `.tran` at the waveform's time-zero value. VACASK's source
    // computes its waveform's in every analysis (`lib/devvisrc.cpp`
    // `sourceCompute`), so a `dc=` beside `type="pulse"` and a sweep of it
    // did nothing (rca3040: v(1) 0, not -0.25 V).
    let deck = "t\nV1 a 0 dc 1 pulse(2 5 1n 1n 1n 10n 20n)\nR1 a 0 1k\n\
                Vin b 0 sin(0 0.1 50meg)\nR2 b 0 1k\n.op\n.dc vin -0.25 0.25 0.25\n\
                .tran 1n 2n\n.end\n";
    let netlist = emit(deck).unwrap();
    let plots = Dialect::Vacask
        .run_all(&netlist)
        .unwrap_or_else(|e| panic!("{e}\n{netlist}"));
    close(col(&plots[0], "a")[0], 1.0, 1e-9);
    let b = col(&plots[1], "b");
    for (v, want) in b.iter().zip([-0.25, 0.0, 0.25]) {
        assert!((v - want).abs() <= 1e-9, "dc b {v}, want {want}\n{netlist}");
    }
    close(col(&plots[2], "a")[0], 2.0, 1e-9);
}
