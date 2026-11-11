//! Each analysis, emitted for VACASK and run: op, dc sweeps, ac, tran,
//! noise and dc transfer function against their closed forms.
//!
//! Cites: real VACASK; the closed forms named in each test.
//!
//! Contents: none (tests only).

use spicerack::syntax::Lang;

use super::*;

// ---- private ----

#[test]
fn op_solves_a_resistive_divider() {
    let raw = run(&rc(".op"));
    // 3 V across 1k+1k: out = 1.5 V, i(vdd) = -1.5 mA (VACASK's branch flow
    // points into the source's positive terminal).
    close(col(&raw, "out")[0], 1.5, 1e-9);
    close(col(&raw, "vdd")[0], 3.0, 1e-9);
    close(col(&raw, "vdd:flow(br)")[0], -1.5e-3, 1e-9);
}

#[test]
fn dc_sweeps_a_source_on_spices_own_grid() {
    let raw = run(&rc(".dc Vdd 0 10 0.5"));
    let sweep = col(&raw, "vsweep");
    // 0..10 by 0.5 is 21 SPICE samples, not 21 intervals.
    assert_eq!(sweep.len(), 21, "sweep grid: {:?}", sweep);
    close(sweep[0], 0.0, 1e-12);
    close(sweep[20], 10.0, 1e-12);
    let out = col(&raw, "out");
    for (i, v) in out.iter().enumerate() {
        close(*v, sweep[i] / 2.0, 1e-9);
    }
}

#[test]
fn dc_sweep_stops_before_passing_the_endpoint() {
    // SPICE walks from START by STEP and never reaches 1.1; VACASK always
    // lands ON `to`, so `to` has to be recomputed or the grids diverge.
    let raw = run(&rc(".dc Vdd 0 1.1 0.2"));
    let sweep = col(&raw, "vsweep");
    assert_eq!(sweep.len(), 6, "{:?}", sweep);
    close(sweep[5], 1.0, 1e-12);
    close(sweep[1], 0.2, 1e-12);
}

#[test]
fn two_source_dc_runs_inner_sweep_fastest() {
    // SPICE's FIRST source is the fast one.
    let deck = "two_src\nR1 a b 1k\nR2 b 0 1k\nR3 c 0 1k\nVa a 0 0\nVb c 0 0\n.dc Va 0 1 0.5 Vb 0 2 1\n.end\n";
    let raw = run(deck);
    // 3 outer x 3 inner = 9 points, inner fastest.
    let a = col(&raw, "a");
    assert_eq!(a.len(), 9, "{:?}", a);
    close(a[0], 0.0, 1e-12);
    close(a[1], 0.5, 1e-12);
    close(a[2], 1.0, 1e-12);
    close(a[3], 0.0, 1e-12);
    let cc = col(&raw, "c");
    close(cc[0], 0.0, 1e-12);
    close(cc[3], 1.0, 1e-12);
    close(cc[6], 2.0, 1e-12);
}

#[test]
fn ac_finds_the_rc_corner() {
    // 1k || 1k = 500 ohm driving 1 uF: f_3dB = 1/(2*pi*500*1e-6) = 318.31 Hz.
    let deck = "rc_ac\nR1 vdd out 1k\nR2 out 0 1k\nC1 out 0 1u\nVdd vdd 0 0 AC 1 0\n.ac dec 200 1 100k\n.end\n";
    let raw = run(deck);
    assert!(raw.is_complex, "AC must come back as a complex plot");

    let f = col(&raw, "frequency");
    let out = cplx(&raw, "out");
    // DC gain is the divider ratio.
    close(out[0].norm(), 0.5, 1e-3);

    // -3 dB relative to the 0.5 plateau.
    let target = 0.5 / std::f64::consts::SQRT_2;
    let (i, _) = out
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1.norm() - target)
                .abs()
                .partial_cmp(&(b.1.norm() - target).abs())
                .unwrap()
        })
        .unwrap();
    let f3db = 1.0 / (2.0 * std::f64::consts::PI * 500.0 * 1e-6);
    close(f[i], f3db, 2e-2);
}

#[test]
fn ac_phase_on_a_source_is_honoured() {
    let raw = run("acphase\nR1 vin 0 1k\nV1 vin 0 0 AC 2 90\n.ac lin 2 1k 2k\n.end\n");
    let v = cplx(&raw, "vin");
    close(v[0].norm(), 2.0, 1e-9);
    close(v[0].arg().to_degrees(), 90.0, 1e-6);
}

#[test]
fn tran_follows_the_rc_step_response() {
    // tau = 1k * 1u = 1 ms, driven by a 1 V step at t=0.
    let deck = "rc_tran\nR1 vin out 1k\nC1 out 0 1u\nV1 vin 0 0 PULSE(0 1 0 1n 1n 1 0)\n.tran 10u 5m 0 10u UIC\n.end\n";
    let raw = run(deck);
    let t = col(&raw, "time");
    let out = col(&raw, "out");
    // 1 - exp(-1) = 0.6321 at one time constant.
    let (i, _) = t
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - 1e-3).abs().partial_cmp(&(b.1 - 1e-3).abs()).unwrap())
        .unwrap();
    close(out[i], 1.0 - (-1.0f64).exp(), 5e-3);
    close(*out.last().unwrap(), 1.0 - (-5.0f64).exp(), 5e-3);
}

#[test]
fn tran_emits_maxstep_so_the_grid_is_spices() {
    // Without `maxstep` VACASK's `step` is only a STARTING step and it strides
    // far past SPICE's output interval.
    let deck = "grid\nR1 vin out 1k\nC1 out 0 1u\nV1 vin 0 1\n.tran 100u 10m\n.end\n";
    let netlist = emit(deck).unwrap();
    assert!(netlist.contains("maxstep="), "{}", netlist);
    let raw = run(deck);
    let t = col(&raw, "time");
    // 10 ms / 100 us = 100 intervals; without maxstep VACASK returns far fewer.
    assert!(
        t.len() >= 100,
        "only {} points — maxstep is not being honoured",
        t.len()
    );
    let dt_max = t.windows(2).map(|w| w[1] - w[0]).fold(0.0f64, f64::max);
    assert!(
        dt_max <= 1e-4 * 1.001,
        "max step {:e} exceeds the requested 1e-4",
        dt_max
    );
}

#[test]
fn noise_matches_a_resistors_thermal_floor() {
    // A single 1k resistor into a unity-gain probe: the output noise density
    // is 4kTR = 1.66e-17 V^2/Hz at 27 C, flat with frequency.
    let deck = "rnoise\nR1 vin out 1k\nR2 out 0 1k\nV1 vin 0 0 AC 1\n.temp 27\n.noise V(out) V1 dec 5 1k 100k\n.end\n";
    let raw = run(deck);

    let onoise = col(&raw, "onoise");
    // Two 1k resistors in parallel (500 ohm) seen from `out`: 4kT*500.
    let k = 1.380649e-23;
    let want = 4.0 * k * (27.0 + 273.15) * 500.0;
    close(onoise[0], want, 2e-3);
    close(*onoise.last().unwrap(), want, 2e-3);
    // Power gain from the 1 V source through the divider is (1/2)^2.
    close(col(&raw, "gain")[0], 0.25, 1e-6);
}

#[test]
fn dc_transfer_function_runs() {
    // VACASK's `dcxf` reports the transfer function from EVERY source to `out`
    // in one plot, so SPICE's input-source argument is implicit.
    let raw = run(&rc(".tf V(out) Vdd"));
    close(col(&raw, "tf(vdd)")[0], 0.5, 1e-9);
    close(col(&raw, "zin(vdd)")[0], 2000.0, 1e-9);
}

#[test]
fn a_one_point_sweep_is_points_zero() {
    // VACASK's `points=0` is one point (`lib/answeep.cpp`
    // `setupLinearSweep`: `end = points+1`). Oracle: the divider's closed
    // form, 3 V * 1k/2k = 1.5 V; `.ac lin 1 f f` at 1 V AC: 0.5 V.
    let raw = run(&rc(".dc Vdd 3 3 1"));
    let out = col(&raw, "out");
    assert_eq!(out.len(), 1, "{out:?}");
    close(out[0], 1.5, 1e-9);
    let deck = "one\nR1 vdd out 1k\nR2 out 0 1k\nVdd vdd 0 0 AC 1\n.ac lin 1 1k 1k\n.end\n";
    let raw = run(deck);
    let out = cplx(&raw, "out");
    assert_eq!(out.len(), 1, "{out:?}");
    close(out[0].re, 0.5, 1e-9);
}

/// The Xyce deck `deck` written for VACASK and run.
fn run_xyce(deck: &str) -> RawData {
    let text = emit_from(deck, Dialect::Xyce, Lang::XYCE.lexicon().expect("a reader"))
        .unwrap_or_else(|c| panic!("refused {c:?}"));
    run_text(&text)
}

#[test]
fn a_decade_or_list_dc_sweeps_xyces_points() {
    // Xyce RG `.DC DEC src start stop points`: start·10^(k/n) to stop
    // (Xyce 7.10 ran 1, 1.585, …, 10 for DEC 5 over 1..10); `LIST` the
    // values as written, the first axis fastest. Closed form: v(1) is V1.
    let raw = run_xyce("t\nV1 1 0 1\nR1 1 0 1k\n.DC DEC V1 1 10 5\n.end\n");
    let v = col(&raw, "1");
    assert_eq!(v.len(), 6, "{v:?}");
    for (k, x) in v.iter().enumerate() {
        close(*x, 10f64.powf(k as f64 / 5.0), 1e-12);
    }
    let deck = "t\nV1 1 0 0\nV2 2 0 0\nR1 1 0 1k\nR2 2 0 1k\n.DC V1 LIST 0 0.5 2 V2 0 1 1\n.end\n";
    let raw = run_xyce(deck);
    let (a, b) = (col(&raw, "1"), col(&raw, "2"));
    assert_eq!(a.len(), 6, "{a:?}");
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        close(*x, [0.0, 0.5, 2.0][i % 3], 1e-12);
        close(*y, (i / 3) as f64, 1e-12);
    }
}

#[test]
fn a_list_step_sweeps_its_values() {
    // Xyce RG `.STEP <name> LIST <val>*`: one run per value. Closed form:
    // 1 mA into r, v(1) = r·1 mA; `temp` steps the temperature option
    // (`lib/answeep.cpp` `option=`, Celsius).
    let raw = run_xyce("t\n.param r=1k\nR1 1 0 {r}\nI1 0 1 1m\n.step r list 1k 2k 5k\n.op\n.end\n");
    let v = col(&raw, "1");
    assert_eq!(v.len(), 3, "{v:?}");
    for (x, want) in v.iter().zip([1.0, 2.0, 5.0]) {
        close(*x, want, 1e-9);
    }
    // `tc1` (ngspice manual §3.2.4, Xyce RG resistor): R = 1k·(1 + 0.01·(T - 27)).
    let raw = run_xyce("t\nR1 1 0 1k tc1=0.01\nI1 0 1 1m\n.step temp list 25 100\n.op\n.end\n");
    let v = col(&raw, "1");
    assert_eq!(v.len(), 2, "{v:?}");
    close(v[0], 0.98, 1e-9);
    close(v[1], 1.73, 1e-9);
}

#[test]
fn spectre_lin_is_steps_on_vacask_too() {
    // Spectre's `lin=10` is 10 steps, 11 points (`[REF]` 19.1 p.43);
    // VACASK's `points=` counts intervals too (`lib/answeep.cpp`
    // `setupLinearSweep`: `end = points+1`). Oracle: ngspice's `.ac lin 11
    // 1 2` is 11 points (manual §11.3.1), so VACASK's run must be too.
    let scs = "simulator lang=spectre\nv1 (in 0) vsource dc=0 mag=1\nr1 (in 0) resistor r=1k\n\
               ac1 ac start=1 stop=2 lin=10\n";
    let deck = emit_from(
        scs,
        Dialect::Spectre,
        Lang::SPECTRE.lexicon().expect("a reader"),
    )
    .expect("emit vacask deck");
    let f = col(&run_text(&deck), "frequency");
    assert_eq!(f.len(), 11, "{f:?}\n{deck}");
    close(f[10], 2.0, 1e-12);
}

#[test]
fn a_number_keeps_the_digits_the_source_wrote() {
    // ngspice reads `10u` as 9.999999999999999e-6 and `2.25m` as
    // 2.2500000000000004e-3; VACASK's lexer scales a suffixed mantissa the
    // same way (`docs/input-numbers.md`), so `10u` is the same double.
    let deck = "t\nV1 a 0 1\nR1 a 0 1k\n.tran 10u 2.25m\n.end\n";
    let w = spicerack::syntax::render::render_text(deck, Dialect::Ngspice, "t", Dialect::Vacask)
        .unwrap();
    assert!(w.text.contains("tran step=10u stop=2.25m"), "{}", w.text);
    let time = col(&run_text(&w.text), "time");
    close(*time.last().unwrap(), 2.25e-3, 1e-12);
}

/// ZERO-LEDGER P4-006. Oracle: the closed form of an RC step, tau = 1 ms,
/// out(t) = 1 - exp(-t/tau), integrated from 0 and saved from TSTART =
/// 0.5 ms: the first saved point is at 0.5 ms (ngspice-45 saves from the
/// first step past it, 0.50258 ms) with out = 0.3935, the last at 1 ms
/// with 0.6321 (research/impl/evidence/P4-006/).
#[test]
fn p4_006_a_tran_start_is_vacasks_start() {
    let deck = "rc_tran\nR1 vin out 1k\nC1 out 0 1u\nV1 vin 0 0 PULSE(0 1 0 1n 1n 1 0)\n\
        .tran 10u 1m 0.5m 10u UIC\n.end\n";
    let raw = run(deck);
    let t = col(&raw, "time");
    let out = col(&raw, "out");
    assert!(
        t[0] >= 0.5e-3 * (1.0 - 1e-9) && t[0] <= 0.51e-3,
        "first saved point at {:e}",
        t[0]
    );
    close(out[0], 1.0 - (-t[0] / 1e-3).exp(), 5e-3);
    close(*t.last().unwrap(), 1e-3, 1e-9);
    close(*out.last().unwrap(), 1.0 - (-1.0f64).exp(), 5e-3);
}

/// ZERO-LEDGER P4-007. Oracle: ngspice-45's `.sp` on this deck, ports of
/// 50 and 75 ohm, its last point (100 MHz) as printed with 13 digits
/// (research/impl/evidence/P4-007/ngspice.run.log); VACASK's `acsp` on the
/// text this writes gave the same 12 digits (`vacask.sp1.raw`).
#[test]
fn p4_007_an_s_parameter_sweep_matches_ngspice() {
    let deck = "sp\nV1 in 0 dc 2 ac 1 portnum 1 z0 50\nV2 out 0 dc 0 ac 0 portnum 2 z0 75\n\
        R1 in out 100\nC1 out 0 10p\nL1 in mid 100n\nR2 mid 0 150\n.sp dec 2 1meg 100meg\n.end\n";
    let raw = run(deck);
    for (name, re, im) in [
        ("s(1,1)", 2.606801622966e-01, 5.172611391933e-02),
        ("s(1,2)", 4.158652538394e-01, -9.38835643325e-02),
        ("s(2,1)", 4.158652538394e-01, -9.38835643325e-02),
        ("s(2,2)", 1.930252841478e-01, -3.54109501255e-01),
    ] {
        let z = cplx(&raw, name);
        assert_eq!(z.len(), 5, "{name}: 2 points a decade over 2 decades");
        close(z[4].re, re, 1e-9);
        close(z[4].im, im, 1e-9);
    }
}

/// ZERO-LEDGER P4-108. Oracle: ngspice manual §4.1.11 ("a VSRC with Z0 Ohm
/// in series. This extra resistor affects all simulations") and the
/// divider's closed form: 2 V behind 50 ohm into 175 || 150 ohm (port 2's
/// 75 ohm ends R1), so in = 2 * 80.769 / 130.769 and out = in * 75 / 175;
/// ngspice-45 printed 1.235294117647 and 0.5294117647059 (same log).
#[test]
fn p4_108_an_rf_port_keeps_its_z0_in_the_operating_point() {
    let deck = "sp\nV1 in 0 dc 2 ac 1 portnum 1 z0 50\nV2 out 0 dc 0 ac 0 portnum 2 z0 75\n\
        R1 in out 100\nC1 out 0 10p\nL1 in mid 100n\nR2 mid 0 150\n.op\n.end\n";
    let raw = run(deck);
    let load = 175.0 * 150.0 / 325.0;
    let vin = 2.0 * load / (50.0 + load);
    close(col(&raw, "in")[0], vin, 1e-9);
    close(col(&raw, "out")[0], vin * 75.0 / 175.0, 1e-9);
}

/// ZERO-LEDGER P4-018. Oracle: the thermal noise across R2 seen between
/// its nodes, 4kT(R2 || (R1 + R3)) = 4kT * 1k at 27 C, flat with
/// frequency, and ngspice-45 on the same deck (onoise_spectrum 4.071372e-9
/// V/sqrt(Hz), its square 1.65761e-17 V^2/Hz;
/// research/impl/evidence/P4-018/ngspice.run.log). `op` to ground would be
/// 4kT * 750. The gain is the divider's, (R2/(R1 + R2 + R3))^2.
#[test]
fn p4_018_a_differential_noise_output_is_a_node_pair() {
    let deck = "dnoise\nV1 vin 0 0 AC 1\nR1 vin op 1k\nR2 op om 2k\nR3 om 0 1k\n.temp 27\n.noise V(op,om) V1 dec 5 1k 100k\n.end\n";
    let raw = run(deck);
    let onoise = col(&raw, "onoise");
    let want = 4.0 * 1.380649e-23 * (27.0 + 273.15) * 1000.0;
    close(onoise[0], want, 1e-5);
    close(*onoise.last().unwrap(), want, 1e-5);
    close(col(&raw, "gain")[0], 0.25, 1e-9);
}

/// ZERO-LEDGER P4-020. Spectre's `montecarlo variations=` (Reference 19.1
/// p.166, default `process`) leaves a scope's draws out; VACASK's `mc`
/// draws at every call site (`docs/cmd-analysis-mc.md`), so that scope is
/// written at its nominal. Oracle: relations that hold in every sample on
/// real VACASK, for a statistical call and for a `statistics` block's
/// `vary`. `p` hangs off the top's draw alone and o1, o2 off one subcircuit
/// draw each: with `process` left out `p` is the divider's 0.5 while o1 and
/// o2 part; with `mismatch` left out o1 is o2 while `p` moves.
#[test]
fn p4_020_a_montecarlos_left_out_scope_is_nominal() {
    let calls = "mc\n.param g={agauss(0, 100, 1)}\n.subckt s a b\nR1 a b {agauss(1k, 100, 1)}\n\
                 R2 b 0 1k\n.ends\nV1 in 0 1\nX1 in o1 s\nX2 in o2 s\nR3 in p {1k+g}\nR4 p 0 1k\n\
                 .op\nsimulator lang=spectre\nmc1 montecarlo numruns=20 seed=1 VARY {\ndcmc dc\n}\n\
                 simulator lang=spice\n.end\n";
    let block = "simulator lang=spectre\nparameters g=0 rl=1k\nstatistics {\nprocess {\n\
                 vary g dist=gauss std=100\n}\nmismatch {\nvary rl dist=gauss std=100\n}\n}\n\
                 subckt s (a b)\nr1 (a b) resistor r=rl\nr2 (b 0) resistor r=1k\nends s\n\
                 v1 (in 0) vsource dc=1\nx1 (in o1) s\nx2 (in o2) s\nr3 (in p) resistor r=1k+g\n\
                 r4 (p 0) resistor r=1k\nop1 dc\nmc1 montecarlo numruns=20 seed=1 VARY {\ndcmc dc\n}\n";
    let spectre = Lang::SPECTRE.lexicon().expect("a reader");
    // Each sample's (o1, o2, p): the plots after the deck's own `op`.
    let samples = |text: Result<String, Vec<Code>>| -> Vec<[f64; 3]> {
        let text = text.expect("emit vacask deck");
        let plots = Dialect::Vacask
            .run_all(&text)
            .unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(plots.len(), 21, "{text}");
        let at = |p: &RawData, n: &str| col(p, n)[0];
        let sample = |p: &RawData| [at(p, "o1"), at(p, "o2"), at(p, "p")];
        plots[1..].iter().map(sample).collect()
    };
    let from = |deck: String| emit_from(&deck, Dialect::Spectre, spectre);
    let only = "variations=mismatch";
    for s in [
        samples(emit(&calls.replace("VARY", only))),
        samples(from(block.replace("VARY", only))),
    ] {
        assert!(s.iter().all(|x| (x[2] - 0.5).abs() < 1e-12), "{s:?}");
        assert!(s.iter().any(|x| (x[0] - x[1]).abs() > 1e-3), "{s:?}");
    }
    for s in [
        samples(emit(&calls.replace(" VARY", ""))),
        samples(from(block.replace(" VARY", ""))),
    ] {
        assert!(s.iter().all(|x| (x[0] - x[1]).abs() < 1e-12), "{s:?}");
        assert!(s.iter().any(|x| (x[2] - 0.5).abs() > 1e-3), "{s:?}");
    }
}

/// ZERO-LEDGER P4-122. ngspice manual p.318 (`MAXORD=1` with the
/// trapezoid is backward Euler); VACASK `docs/cmd-options-tran.md`
/// (`tran_maxord` bounds `am` and `gear`, never `trap`). Oracle: real
/// ngspice-45 on the source against real VACASK on the written deck. A
/// lossless LC tank on a 1 V step rings 0 to 2 V for ever under the
/// trapezoid and decays under backward Euler: after 16 of its 198.7 us
/// cycles at a 1 us step the peak is 1.431 V on both, 2.000 V with the
/// trapezoid (research/impl/evidence/P4-122). Tolerance 5e-3: the two
/// step controls differ (measured 4.4e-4).
#[test]
fn p4_122_maxord_1_is_backward_euler_on_vacask() {
    let deck = "t\nV1 in 0 PULSE(0 1 10u 1u 1u 1 2)\nL1 in a 1m\nC1 a 0 1u\n\
        .options maxord=1\n.tran 1u 2m 0 1u\n.end\n";
    let peak = |raw: &RawData, name: &str| {
        let (t, a) = (col(raw, "time"), col(raw, name));
        let late = t.iter().zip(&a).filter(|(t, _)| **t >= 1.6e-3);
        late.fold(f64::MIN, |m, (_, a)| m.max(*a))
    };
    let ngspice = Dialect::Ngspice.run(deck).expect("ngspice runs the source");
    let want = peak(&ngspice, "v(a)");
    assert!(want > 1.3 && want < 1.6, "ngspice's peak: {want}");
    close(peak(&run(deck), "a"), want, 5e-3);
}
