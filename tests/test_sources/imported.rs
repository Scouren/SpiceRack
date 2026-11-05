//! Decks read and written as ngspice: ngspice PWL `r=`/`td=`, RF ports and
//! `.sp`, and Spectre PWL `delay`/`pwlperiod`, each written as ngspice's
//! card (or refused) and run against the original deck or the closed form.
//!
//! Cites: ngspice 45 manual §4.1.4, §4.1.11, §11.3.11; Spectre Reference 5.0
//! `vsource` #4, #17, #18, p.686 (`[CMP]`); real ngspice.
//!
//! Contents: none (tests only).

use super::*;

/// The Spectre PWL sources the Spectre-cards tests below read.
const SPECTRE_PWLS: &str = "// pwl\nsimulator lang=spectre\n\
    v1 (a 0) vsource type=pwl wave=[0 0 1m 1 2m 0] delay=0.5m\n\
    v2 (b 0) vsource type=pwl wave=[0 0 1m 1 2m 0] pwlperiod=2m\n\
    v3 (c 0) vsource type=pwl wave=[0 0 1m 1 2m 0] pwlperiod=2m delay=0.5m\n\
    r1 (a 0) resistor r=1\nr2 (b 0) resistor r=1\nr3 (c 0) resistor r=1\n\
    tr tran step=10u stop=7m maxstep=10u\n";

// ---- private ----

// ── ngspice cards in: PWL r=/td= (§4.1.4), RF port (§4.1.11), .sp (§11.3.11) ──

/// `deck` on real ngspice, and the deck the renderer writes from it: every
/// vector of every plot the same, to 1e-12 relative. No card may be kept
/// as written (a note), so the second deck is the renderer's own.
fn ngspice_round_trip(deck: &str) {
    let w = render_text(deck, Dialect::Ngspice, "rt", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("{}", e.messages));
    assert!(w.notes.diags.is_empty(), "{}", w.notes.messages);
    let emitted = w.text;
    let run = |d: &str| {
        Dialect::Ngspice
            .run_all(d)
            .unwrap_or_else(|e| panic!("ngspice: {e}\n{d}"))
    };
    let (want, got) = (run(deck), run(&emitted));
    assert_eq!(want.len(), got.len(), "{emitted}");
    let same = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-30);
    for (w, g) in want.iter().zip(&got) {
        assert_eq!(w.plot_name, g.plot_name);
        for v in &w.variables {
            let gi = g
                .variables
                .iter()
                .position(|x| x.name == v.name)
                .unwrap_or_else(|| panic!("{} missing\n{emitted}", v.name));
            let (a, b) = (&w.re(v.index), &g.re(gi));
            assert_eq!(a.len(), b.len(), "{} in {}", v.name, w.plot_name);
            assert!(
                a.iter().zip(b.iter()).all(|(&a, &b)| same(a, b)),
                "{} in {}\n{emitted}",
                v.name,
                w.plot_name
            );
            let (a, b) = (
                w.complex(v.index).unwrap_or_default(),
                g.complex(gi).unwrap_or_default(),
            );
            assert!(
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a.im, b.im)),
                "{} in {}",
                v.name,
                w.plot_name
            );
        }
    }
}

#[test]
fn an_imported_repeating_pwl_runs_as_the_original_deck() {
    ngspice_round_trip(
        "pwl round trip\nV1 a 0 pwl(0 0 1m 1 2m 0) r=0 td=0.5m\nR1 a 0 1\nV2 b 0 PWL(0 0 1m 1 1.5m 0.2) TD=1m\nR2 b 0 1\n\
         V3 c 0 pwl(0 0 0.5m 1 1m 0.5 2m 0) r 0.5m\nR3 c 0 1\n.tran 10u 7m 0 10u\n.end\n",
    );
}

#[test]
fn an_imported_rf_port_runs_as_the_original_deck() {
    // A T pad between ports 1 (z0 given) and 2 (z0 by default, `=` form):
    // the .op sees each port's z0 in series, the .sp its S-parameters.
    ngspice_round_trip(
        "port round trip\nV1 in 0 dc 1 ac 1 portnum 1 z0 50\nV2 out 0 dc 0 ac 1 portnum=2\nR1 in mid 16.666666666666668\n\
         R2 mid out 16.666666666666668\nR3 mid 0 66.66666666666667\n.op\n.sp lin 3 1k 3k\n.end\n",
    );
}

// ── Spectre cards in: PWL `delay`, `pwlperiod` ([CMP] p.683 #4, #17, #18; p.686) ──
//
// `delay` is the general waveform delay, ngspice's `td=`. `pwlperiod=P`
// repeats the waveform from t = 0 with period P, and before each repeat
// ramps over `twidth` (default P/1000) from its value at P - twidth to its
// value at 0. That is ngspice's `r=0` exactly when the points start at 0,
// end at P on the first point's value, and the last segment is at least
// twidth long (the ramp is then the segment itself); any other form stays raw.

#[test]
fn spectre_pwl_delay_and_an_exact_pwlperiod_are_ngspice_td_and_r() {
    // v1..v3 are ngspice's `td=`, `r=0` and both; each of v4..v11 has no
    // ngspice card (ends that differ, a twidth ramp; a period past the
    // points; a given twidth; points from 0.1 ms; a last segment shorter
    // than the default twidth; an FM sine's and an exp's delay; a
    // pwlperiod off a PWL), so it stays Spectre text, which ngspice
    // refuses (`W0001`).
    let ng = |src: &str| render_text(src, Dialect::Spectre, "t", Dialect::Ngspice);
    let deck = ng(SPECTRE_PWLS)
        .unwrap_or_else(|e| panic!("{}", e.messages))
        .text;
    for card in [
        "V1 a 0 0 PWL(0 0 0.001 1 0.002 0) td=0.0005",
        "V2 b 0 0 PWL(0 0 0.001 1 0.002 0) r=0",
        "V3 c 0 0 PWL(0 0 0.001 1 0.002 0) r=0 td=0.0005",
    ] {
        assert!(deck.lines().any(|l| l == card), "{card}\n{deck}");
    }
    let extra = [
        "v4 (d 0) vsource type=pwl wave=[0 0 1m 1 2m 0.5] pwlperiod=2m",
        "v5 (e 0) vsource type=pwl wave=[0 0 1m 1 2m 0] pwlperiod=3m",
        "v6 (f 0) vsource type=pwl wave=[0 0 1m 1 2m 0] pwlperiod=2m twidth=1u",
        "v7 (g 0) vsource type=pwl wave=[0.1m 0 1m 1 2m 0] pwlperiod=2m",
        "v8 (h 0) vsource type=pwl wave=[0 0 1m 1 2m 0 2.0000001m 0] pwlperiod=2.0000001m",
        "v9 (i 0) vsource type=sine sinedc=0 ampl=1 freq=1k fmmodindex=2 fmmodfreq=100 delay=1m",
        "v10 (j 0) vsource type=exp val0=0 val1=1 td1=1m tau1=1m delay=1m",
        "v11 (k 0) vsource type=pulse val0=0 val1=1 pwlperiod=2m",
    ];
    for line in extra {
        let e = ng(&format!("{SPECTRE_PWLS}{line}\n")).expect_err(line);
        assert!(
            e.diags.iter().any(|d| d.code == Code::W0001),
            "{line}: {}",
            e.messages
        );
    }
}

#[test]
fn an_imported_spectre_pwl_runs_on_ngspice_as_the_manual_says() {
    // The manual's semantics as the closed form: `tri` once (1 V at 1 ms,
    // 0 from 2 ms), shifted by `delay` (holding its first value before) and
    // repeated with period `pwlperiod`.
    let from = |d: Dialect| {
        render_text(SPECTRE_PWLS, Dialect::Spectre, "t", d)
            .unwrap_or_else(|e| panic!("{}", e.messages))
            .text
    };
    // Written back as Spectre, the cards say what they said.
    let back = from(Dialect::Spectre);
    assert!(
        back.contains("wave=[0 0 1m 1 2m 0] pwlperiod=2m\n")
            && back.contains("wave=[0 0 1m 1 2m 0] delay=500u pwlperiod=2m\n"),
        "{back}"
    );
    let deck = from(Dialect::Ngspice);
    let raw = ngspice(&deck);
    let time = col(&raw, "time");
    let tri = |t: f64| {
        if t <= 0.0 || t >= 2e-3 {
            0.0
        } else if t <= 1e-3 {
            t / 1e-3
        } else {
            (2e-3 - t) / 1e-3
        }
    };
    let delayed = |t: f64| {
        if t < 0.5e-3 {
            0.0
        } else {
            (t - 0.5e-3).rem_euclid(2e-3)
        }
    };
    for (node, want) in [
        ("a", &(|t: f64| tri(t - 0.5e-3)) as &dyn Fn(f64) -> f64),
        ("b", &|t: f64| tri(t.rem_euclid(2e-3))),
        ("c", &|t: f64| tri(delayed(t))),
    ] {
        let v = col(&raw, node);
        for k in 0..=700 {
            let t = 7e-3 * k as f64 / 700.0;
            assert!(
                (at(time, v, t) - want(t)).abs() < 1e-9,
                "{node} at {t}: {}, want {}\n{deck}",
                at(time, v, t),
                want(t)
            );
        }
    }
}

#[test]
fn a_spectre_vector_keeps_its_signs_on_ngspice() {
    // `[CMP]` `vsource` `wave` (p.686) and `pvcvs` `coeffs` (pp.590-616): each
    // element is a number, its sign with it. The closed forms: `a` is
    // -1 V at 1 ms between 0 at 0 and 2 ms; `o` is -2·v(a) - 0.5 (pvcvs
    // coeffs [c0 c1] is c0 + c1·v).
    let src = "// signs\nsimulator lang=spectre\n\
        v1 (a 0) vsource type=pwl wave=[0 0 1m -1 2m 0]\n\
        e1 (o 0 a 0) pvcvs coeffs=[-0.5 -2]\n\
        r1 (a 0) resistor r=1\nr2 (o 0) resistor r=1\n\
        tr tran step=10u stop=2m maxstep=10u\n";
    let deck = render_text(src, Dialect::Spectre, "t", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("{}", e.messages))
        .text;
    let raw = ngspice(&deck);
    let (time, a, o) = (col(&raw, "time"), col(&raw, "a"), col(&raw, "o"));
    for k in 0..=200 {
        let t = 2e-3 * k as f64 / 200.0;
        let want = -(1.0 - (t - 1e-3).abs() / 1e-3);
        let (va, vo) = (at(time, a, t), at(time, o, t));
        assert!(
            (va - want).abs() < 1e-9,
            "a at {t}: {va}, want {want}\n{deck}"
        );
        assert!(
            (vo - (-2.0 * want - 0.5)).abs() < 1e-6,
            "o at {t}: {vo}\n{deck}"
        );
    }
}

#[test]
fn a_spectre_reference_names_the_source_as_the_name_map_spells_it() {
    // `in1 (a 0) vsource` is ngspice's `Vin1`; `probe=in1`, `dev=in1` and
    // `save in1:p` name it ([CMP] `cccs`, `ccvs`, `dc`; p.686). Closed
    // forms (SPICE's sign: a source's current flows from its + node
    // through it): i(in1) = -v into 1 Ω, so the cccs drives 2v into
    // `o` and the ccvs holds `p` at -3v.
    let src = "// refs\nsimulator lang=spectre\n\
        in1 (a 0) vsource dc=1\n\
        f1 (o 0) cccs probe=in1 gain=2\n\
        h1 (p 0) ccvs probe=in1 rm=3\n\
        r1 (a 0) resistor r=1\nr2 (o 0) resistor r=1\nr3 (p 0) resistor r=1\n\
        save a o p in1:p\n\
        dc1 dc dev=in1 param=dc start=0 stop=1 step=0.25\n";
    let deck = render_text(src, Dialect::Spectre, "t", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("{}", e.messages))
        .text;
    let raw = ngspice(&deck);
    let (a, o, p) = (col(&raw, "a"), col(&raw, "o"), col(&raw, "p"));
    let i = col(&raw, "i(vin1)");
    assert_eq!(a.len(), 5, "{deck}");
    for k in 0..5 {
        let v = 0.25 * k as f64;
        assert!((a[k] - v).abs() < 1e-9, "a {}: {v}\n{deck}", a[k]);
        assert!((i[k] + v).abs() < 1e-9, "i(vin1) {}: {}\n{deck}", i[k], -v);
        assert!(
            (o[k] - 2.0 * v).abs() < 1e-9,
            "o {}: {}\n{deck}",
            o[k],
            2.0 * v
        );
        assert!(
            (p[k] + 3.0 * v).abs() < 1e-9,
            "p {}: {}\n{deck}",
            p[k],
            -3.0 * v
        );
    }
}

#[test]
fn a_spectre_noise_iprobe_names_a_source_ngspice_runs() {
    // `iprobe=in1` is `Vin1` (`[CMP]` `noise`). Closed form: two 1 Ω
    // resistors at 27 °C give `o` 4kT·0.5 V²/Hz at a gain of 0.5, so the
    // input-referred density is √(8kT) V/√Hz at every frequency.
    let src = "// noise\nsimulator lang=spectre\n\
        in1 (a 0) vsource dc=0 mag=1\nr1 (a o) resistor r=1\nr2 (o 0) resistor r=1\n\
        n1 (o 0) noise start=1k stop=10k dec=1 iprobe=in1\n";
    let deck = render_text(src, Dialect::Spectre, "t", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("{}", e.messages))
        .text;
    let runs = Dialect::Ngspice
        .run_all(&deck)
        .unwrap_or_else(|e| panic!("ngspice: {e}\n{deck}"));
    let want = (8.0 * 1.380649e-23 * 300.15f64).sqrt();
    let spectrum = runs
        .iter()
        .find_map(|r| {
            let v = r.variables.iter().find(|v| v.name == "inoise_spectrum")?;
            Some(r.re(v.index))
        })
        .unwrap_or_else(|| panic!("no inoise_spectrum\n{deck}"));
    assert!(!spectrum.is_empty(), "{deck}");
    for x in spectrum.iter() {
        assert!((x / want - 1.0).abs() < 1e-3, "{x} want {want}\n{deck}");
    }
}

// ── Spectre masters with a SPICE card: iprobe, port, tline, bsource ([CMP]) ──

/// `src` (Spectre) written for ngspice and run.
fn spectre_on_ngspice(src: &str) -> (String, RawData) {
    let deck = render_text(src, Dialect::Spectre, "t", Dialect::Ngspice)
        .unwrap_or_else(|e| panic!("{}", e.messages))
        .text;
    let raw = ngspice(&deck);
    (deck, raw)
}

#[test]
fn spectre_iprobe_tline_and_bsource_run_on_ngspice_as_the_manual_says() {
    // Closed forms: `b` is 1000·t ($time, p.857); `d` takes 1m·v(b) into
    // 1 kΩ (current from 0 through the source into d: v(d) = v(b)); the
    // iprobe's current (p.379: in -> out) is v(b)/1k; the lossless line
    // (p.647), driven through z0 and matched, gives o(t) = b(t - td)/2.
    let src = "// masters\nsimulator lang=spectre\n\
        b1 (b 0) bsource v=1k*$time\n\
        b2 (0 d) bsource i=1m*v(b,0)\nr1 (d 0) resistor r=1k\n\
        ip (b b2) iprobe\nr2 (b2 0) resistor r=1k\n\
        rs (b s) resistor r=50\nt1 (s 0 o 0) tline z0=50 td=0.2m\nrl (o 0) resistor r=50\n\
        save b d o ip:p\n\
        tr tran step=10u stop=1m maxstep=10u\n";
    let (deck, raw) = spectre_on_ngspice(src);
    let time = col(&raw, "time");
    let (b, d, o, i) = (
        col(&raw, "b"),
        col(&raw, "d"),
        col(&raw, "o"),
        col(&raw, "i(vip)"),
    );
    for k in 0..=100 {
        let t = 1e-3 * k as f64 / 100.0;
        let vb = 1e3 * t;
        let near = |c: &[f64], want: f64, tol: f64, what: &str| {
            let got = at(time, c, t);
            assert!(
                (got - want).abs() < tol,
                "{what} at {t}: {got}, want {want}\n{deck}"
            );
        };
        near(b, vb, 1e-9, "b");
        near(d, vb, 1e-9, "d");
        near(i, vb / 1e3, 1e-12, "i(vip)");
        // The delayed corner lands between timepoints: half a step of
        // the 500 V/s ramp.
        near(o, (1e3 * (t - 0.2e-3)).max(0.0) / 2.0, 2.5e-3, "o");
    }
}

#[test]
fn a_spectre_port_is_ngspices_rf_port_with_the_source_behind_it() {
    // `[CMP]` p.602: dc=1 is the voltage across the port terminated in its
    // r = 50 Ω, so 1 V across a 50 Ω load; a 100 Ω load takes 4/3 V
    // (2 V behind 50 Ω).
    for (load, want) in [("50", 1.0), ("100", 4.0 / 3.0)] {
        let src = format!(
            "// port\nsimulator lang=spectre\n\
             p1 (a 0) port num=1 r=50 dc=1 mag=1\nrl (a 0) resistor r={load}\nop1 dc\n"
        );
        let (deck, raw) = spectre_on_ngspice(&src);
        let a = col(&raw, "a")[0];
        assert!((a - want).abs() < 1e-9, "{a}, want {want}\n{deck}");
    }
}
