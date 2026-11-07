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

/// The raw column `name` at time `t`, read off the transient's samples by
/// linear interpolation (a PWL is linear between its points, which VACASK
/// keeps as breakpoints).
fn at(raw: &RawData, name: &str, t: f64) -> f64 {
    let (time, v) = (col(raw, "time"), col(raw, name));
    let k = time.iter().position(|&x| x >= t).expect("t inside the run");
    if k == 0 || time[k] == t {
        return v[k];
    }
    let f = (t - time[k - 1]) / (time[k] - time[k - 1]);
    v[k - 1] + f * (v[k] - v[k - 1])
}

#[test]
fn a_pwl_source_runs_with_its_delay() {
    // Upstream VACASK evaluates `type="pwl"` (`lib/devvisrc.cpp` L910,
    // 36be560); the pinned fork ran it as 0 V. ngspice manual §4.1.4: the
    // value held before the first point and after the last, linear
    // between, all shifted by `td`.
    let deck =
        |td: &str| format!("t\nV1 a 0 PWL(0 0 1m 1 2m 0.5){td}\nR1 a 0 1k\n.tran 10u 3m\n.end\n");
    let raw = run(&deck(""));
    for (t, want) in [(0.5e-3, 0.5), (1e-3, 1.0), (1.5e-3, 0.75), (2.5e-3, 0.5)] {
        close(at(&raw, "a", t), want, 1e-9);
    }
    let raw = run(&deck(" td=0.5m"));
    for (t, want) in [(0.25e-3, 0.0), (1e-3, 0.5), (1.5e-3, 1.0), (2e-3, 0.75)] {
        close(at(&raw, "a", t), want, 1e-9);
    }
}

#[test]
fn a_whole_pwl_repeat_is_its_period() {
    // ngspice `r=0` repeats the points from time 0 (manual §4.1.4); with
    // the last value the first that is VACASK's `period` (the last point
    // dropped, `docs/dev-builtin-src.md`). A triangle of period 2 ms.
    let deck = "t\nV1 a 0 PWL(0 0 1m 1 2m 0) r=0\nR1 a 0 1k\n.tran 10u 5m\n.end\n";
    let raw = run(deck);
    for (t, want) in [(2.5e-3, 0.5), (3e-3, 1.0), (4.5e-3, 0.5)] {
        close(at(&raw, "a", t), want, 1e-9);
    }
}

#[test]
fn a_vacask_am_source_runs_on_ngspice_from_its_time_zero_value() {
    // VACASK's own `test/test_viwfm.sim` i5 (36be560): `type="am"` is
    // sinedc + ampl sin(2π freq t + tdphase)(1 + modindex sin(2π modfreq t
    // + modphase)), `tdphase` the carrier phase and `phase` the AC phase
    // only; its op is that at t = 0 (-2.5 V here). The deck written for
    // ngspice (`DC <v(0)> AC 1 -30 AM(…)`) runs to VACASK's own op and
    // transient (ngspice holds 0 at t = 0 itself, so from the first step).
    let deck = "am\nground 0\nload \"resistor.osdi\"\nmodel resistor resistor\n\
        model vsource vsource\n\
        v5 (5 0) vsource type=\"am\" sinedc=0.5 ampl=2.0 freq=1000 tdphase=-90 \
        modfreq=50 modphase=90 modindex=0.5 mag=1 phase=-30\n\
        r5 (5 0) resistor r=1\n\
        control\n  analysis op1 op\n  analysis tran1 tran step=10u stop=2m maxstep=10u\nendc\n";
    let va = Dialect::Vacask
        .run_all(deck)
        .unwrap_or_else(|e| panic!("vacask: {e}"));
    let ng = spicerack::syntax::render::render_text(deck, Dialect::Vacask, "am", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("{}", e.messages))
        .text;
    let got = Dialect::Ngspice
        .run_all(&ng)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{ng}"));
    close(col(&va[0], "5")[0], -2.5, 1e-9);
    close(col(&got[0], "v(5)")[0], -2.5, 1e-9);
    for t in [0.25e-3, 0.5e-3, 1e-3, 1.5e-3, 2e-3] {
        let want = at(&va[1], "5", t);
        let g = at(&got[1], "v(5)", t);
        assert!(
            (g - want).abs() <= 1e-3 * 4.0,
            "t={t} ngspice {g} vacask {want}\n{ng}"
        );
    }
}
