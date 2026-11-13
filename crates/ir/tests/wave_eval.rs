//! The reference evaluator of sources (`wave_eval.rs`, AMEND-A12 A30): its spec examples and
//! its properties.
//!
//! - The runs (oracle: the simulators). Every value below was printed by a run of the source
//!   areas' decks: ngspice-45 on `dialect-ngspice` 4aef33a5
//!   `tests/fixtures/dialect/ngspice/read/sources/<deck>.sp` (each `print` with `set numdgt=16`
//!   and `time[k]` beside each `v(x)[k]`; the deck's IR is `fixtures/wave_eval/ngspice/<deck>.ir`,
//!   verbatim from the same tree), and VACASK 36be560 on `vc-sources` 73d7c1ca
//!   `tests/fixtures/dialect/vacask/settle/<deck>.sim` (`rawfile="ascii"`, nine points of each
//!   node's transient at fixed fractions of the run, the op; its IR is
//!   `fixtures/wave_eval/vacask/<deck>.ir`, written by the VACASK reader's mapping). The
//!   tolerance is 1e-12: both runs print 16 or 17 significant digits of values near 1, and the
//!   formulas' float order differs by at most 3e-14 (FM); an edge a step off, or a period
//!   boundary on the wrong side, is off by far more. VACASK points within 1e-12 of a jump are
//!   left out: its raw file's 16 digits do not tell which side of the jump the time was.
//! - The laws' boundaries (oracle: the law's doc in `wave_eval.rs` and the line it cites), by
//!   hand.
//! - Properties over constructed and generated waves: a periodic wave repeats, a PWL holds its
//!   first and last value, the operating point under `DcRule::WaveAtZero` is the value at 0,
//!   and every answer is a finite value or a domain answer.
//!
//! Contents: tests only.

#[cfg(test)]
mod tests {
    use spicerack_ir::item::Param;
    use spicerack_ir::source::Source;
    use spicerack_ir::{
        At, Design, DetailKind, Diags, ElemKind, Element, ItemKind, NameId, Ref, WaveDomain, link,
        parse, source_value,
    };
    use spicerack_ir::{Bindings, Dist, DrawPer, DrawWhen, eval};

    /// What a run binds: the deck's parameters, its transient's step and stop, 27 °C.
    struct Run<'d> {
        design: &'d Design,
        tstep: Option<f64>,
        tstop: Option<f64>,
    }

    impl Bindings for Run<'_> {
        fn param(&self, param: Ref) -> Option<f64> {
            if param.def.is_none() {
                return None;
            }
            let item = self.design.def(param.def).item;
            let row: Param = self.design.row(item.index);
            eval(self.design, row.value, self).ok()
        }
        fn voltage(&self, _: NameId, _: NameId) -> Option<f64> {
            None
        }
        fn current(&self, _: NameId) -> Option<f64> {
            None
        }
        fn time(&self) -> Option<f64> {
            None
        }
        fn temp(&self) -> Option<f64> {
            Some(27.0)
        }
        fn freq(&self) -> Option<f64> {
            None
        }
        fn tstep(&self) -> Option<f64> {
            self.tstep
        }
        fn tstop(&self) -> Option<f64> {
            self.tstop
        }
        fn tnom(&self) -> Option<f64> {
            Some(27.0)
        }
        fn draw(&self, _: Dist, _: DrawWhen, _: DrawPer, _: &[f64]) -> Option<f64> {
            None
        }
    }

    /// The tolerance against a run (module header).
    const TOL: f64 = 1e-12;

    /// Each ngspice deck's `.tran step stop`.
    const NG_TRAN: &[(&str, f64, f64)] = &[
        ("pulse", 1e-6, 20e-6),
        ("sin", 1e-6, 10e-6),
        ("exp", 1e-6, 6e-6),
        ("pwl", 0.1e-6, 6e-6),
        ("sffm", 1e-6, 100e-6),
        ("am", 1e-6, 100e-6),
        ("noise", 1e-6, 1e-3),
    ];

    #[test]
    fn every_ngspice_run_value_is_reproduced() {
        let mut bad = Vec::new();
        for (deck, node, at, want) in values(include_str!("fixtures/wave_eval/ngspice/values.txt"))
        {
            let d = fixture("ngspice", deck);
            let &(_, step, stop) = NG_TRAN.iter().find(|r| r.0 == deck).expect("a .tran");
            let env = Run {
                design: &d,
                tstep: Some(step),
                tstop: Some(stop),
            };
            let got = source_value(&d, &source_at(&d, node), at, &env);
            if !got.is_ok_and(|g| (g - want).abs() <= TOL) {
                bad.push(format!("{deck} {node} {at:?}: {got:?}, the run {want:e}"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    #[test]
    fn every_vacask_run_value_is_reproduced() {
        let mut bad = Vec::new();
        for (deck, node, at, want) in values(include_str!("fixtures/wave_eval/vacask/values.txt")) {
            let d = fixture("vacask", deck);
            // VACASK's laws read no TSTEP or TSTOP: an unbound one would be an `Unbound` answer
            let env = Run {
                design: &d,
                tstep: None,
                tstop: None,
            };
            let got = source_value(&d, &source_at(&d, node), at, &env);
            if !got.is_ok_and(|g| (g - want).abs() <= TOL) {
                bad.push(format!("{deck} {node} {at:?}: {got:?}, the run {want:e}"));
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// `PwlJoin::Extrapolate`'s doc examples (A22, run src-pwl, four digits as the doc prints them).
    #[test]
    fn the_periodic_pwl_doc_examples() {
        let d = fixture("vacask", "src_pwl");
        let env = Run {
            design: &d,
            tstep: None,
            tstop: None,
        };
        let at = |node: &str, t: f64| source_value(&d, &source_at(&d, node), At::Time(t), &env);
        for (t, want) in [
            (0.0, 4.333),
            (0.75e-6, 3.833),
            (1e-6, 1.0),
            (2.5e-6, 2.667),
            (3.9e-6, 1.733),
            (4.1e-6, 4.267),
        ] {
            let got = at("d", t).expect("a value");
            assert!((got - want).abs() < 5e-4, "d at {t}: {got}");
        }
        for t in [1.5e-6, 3.5e-6] {
            let got = at("c", t).expect("a value");
            assert!((got - 0.5).abs() < 1e-12, "c at {t}: {got}");
        }
    }

    /// `PulseLaw::Spice` at each boundary (the law's doc; vsrcload.c:139-160): v1 at τ = 0, v2 from
    /// τ = rise through rise + width, v1 again at rise + width + fall, a wrap only after τ passes
    /// the period, `cycles` counted on τ.
    #[test]
    fn the_spice_pulse_at_its_boundaries() {
        let p = "pulse(v1=0 v2=1 delay=1 rise=1 fall=1 width=1 period=10)";
        for (t, want) in [
            (0.0, 0.0),
            (1.0, 0.0),
            (1.5, 0.5),
            (2.0, 1.0),
            (3.0, 1.0),
            (3.5, 0.5),
            (4.0, 0.0),
            (11.0, 0.0),
            (11.5, 0.5),
            (12.0, 1.0),
        ] {
            assert_eq!(at(p, t), Ok(want), "t = {t}");
        }
        let once = "pulse(v1=0 v2=1 delay=1 rise=1 fall=1 width=1 period=10 cycles=1)";
        assert_eq!(at(once, 11.5), Ok(0.0), "after its one pulse");
        assert_eq!(at(once, 1.5), Ok(0.5));
        // an ideal fall: at τ = rise + width the value is already v1 (`time >= TR + PW + TF` first)
        let ideal = "pulse(v1=0 v2=1 delay=0 rise=0 fall=0 width=2 period=10)";
        assert_eq!(at(ideal, 0.0), Ok(0.0));
        assert_eq!(at(ideal, 1.0), Ok(1.0));
        assert_eq!(at(ideal, 2.0), Ok(0.0));
        // a zero period under AsGiven has no SPICE meaning
        let flat = "pulse(v1=0 v2=1 delay=0 rise=1 fall=1 width=1 period=0)";
        assert_eq!(at(flat, 0.5), Err(WaveDomain::Undefined));
    }

    /// `GivenZero::SpiceDefault` (IR-CIRCUIT §3's table): an omitted delay is 0, a zero or
    /// omitted rise and fall TSTEP, width and period TSTOP; a default the instant does not reach
    /// asks nothing of the bindings.
    #[test]
    fn the_spice_defaults_of_a_pulse() {
        let p = "pulse(v1=0 v2=1 rise=0 given_zero=spice_default)";
        // TSTEP 1, TSTOP 100 (`at`'s bindings)
        assert_eq!(at(p, 0.5), Ok(0.5));
        assert_eq!(at(p, 50.0), Ok(1.0));
        let d = one(p);
        let bare = Run {
            design: &d,
            tstep: None,
            tstop: None,
        };
        let s = source_at(&d, "a");
        assert_eq!(source_value(&d, &s, At::Time(0.0), &bare), Ok(0.0));
        assert_eq!(
            source_value(&d, &s, At::Time(0.5), &bare),
            Err(WaveDomain::Unbound)
        );
    }

    /// `PulseLaw::Vacask` (VD dev-builtin-src, runs p1, src-pulse): before the delay the value at
    /// x = 0, `fall <= 0` holds v2 to the end of the period, the period wraps with `fmod`.
    #[test]
    fn the_vacask_pulse_at_its_boundaries() {
        let p = "pulse(v1=0 v2=1 delay=1 rise=1 fall=-1 width=1 period=10 law=vacask)";
        for (t, want) in [
            (0.0, 0.0),
            (1.5, 0.5),
            (2.0, 1.0),
            (9.0, 1.0),
            (11.0, 0.0),
            (11.5, 0.5),
        ] {
            assert_eq!(at(p, t), Ok(want), "t = {t}");
        }
    }

    /// A sine before, at and after its delay under each `PreDelay` (vsrcload.c:180-186: τ ≤ 0 is
    /// the pre-delay value), its cycles, its damping.
    #[test]
    fn the_sine_around_its_delay() {
        let sine = |pre: &str| {
            format!("sin(offset=1 amplitude=2 freq=1 delay=1 damping=0 phase_deg=90 pre={pre})")
        };
        // (no simulator holds a sine at 0 before its delay: `PreDelay::Zero` is refused there, R100)
        for (pre, before) in [("offset", 1.0), ("hold_delay_value", 3.0)] {
            assert_eq!(at(&sine(pre), 0.5), Ok(before), "{pre}");
            assert_eq!(at(&sine(pre), 1.0), Ok(before), "{pre} at τ = 0");
        }
        let near = |text: &str, t: f64, want: f64| {
            let got = at(text, t).expect("a value");
            assert!((got - want).abs() < 1e-12, "{text} at {t}: {got}");
        };
        near(
            &sine("offset"),
            1.25,
            1.0 + 2.0 * (std::f64::consts::PI).sin(),
        );
        let counted = "sin(offset=1 amplitude=2 freq=1 delay=0 damping=0 phase_deg=90 cycles=1 pre=hold_delay_value)";
        near(counted, 3.0, 3.0);
        let damped =
            "sin(offset=0 amplitude=1 freq=1 delay=0 damping=2 phase_deg=90 pre=hold_delay_value)";
        near(damped, 1.0, (-2.0f64).exp());
    }

    /// The exponential's two edges, `td2` from 0 or from `td1` (A12 C2), each boundary on its
    /// earlier branch (vsrcload.c:209-215).
    #[test]
    fn the_exponential_edges() {
        let e1 = 1.0 - (-1.0f64).exp();
        let e2 = 1.0 - (-2.0f64).exp();
        let zero = "exp(v1=0 v2=1 td1=1 tau1=1 td2=2 tau2=1)";
        assert_eq!(at(zero, 1.0), Ok(0.0));
        assert_eq!(at(zero, 2.0), Ok(e1));
        assert_eq!(at(zero, 3.0), Ok(e2 - e1));
        let from_td1 = "exp(v1=0 v2=1 td1=1 tau1=1 td2=2 tau2=1 td2_from=td1)";
        assert_eq!(at(from_td1, 3.0), Ok(e2));
        // SpiceDefault: td2 zero is TSTEP after td1 (TSTEP 1): the second edge at 2
        let defaulted = "exp(v1=0 v2=1 td1=1 tau1=1 td2=0 tau2=1 given_zero=spice_default)";
        assert_eq!(at(defaulted, 3.0), Ok(e2 - e1));
    }

    /// A PWL's own law (A22) and ngspice's repeat (vsrcload.c:312-354): the first value before
    /// the first point, the last after the last, a time listed twice takes its first value, `From`
    /// wraps after the last point, `repeat_count` passes then the last value.
    #[test]
    fn the_pwl_points_and_repeats() {
        let plain = "pwl(points=[1, 1, 2, 3])";
        for (t, want) in [(0.0, 1.0), (1.0, 1.0), (1.5, 2.0), (2.0, 3.0), (5.0, 3.0)] {
            assert_eq!(at(plain, t), Ok(want), "t = {t}");
        }
        assert_eq!(at("pwl(points=[1, 1, 2, 3] delay=1)", 2.5), Ok(2.0));
        assert_eq!(at("pwl(points=[0, 0, 1, 1, 1, 5, 2, 6])", 1.0), Ok(1.0));
        let from = "pwl(points=[0, 0, 1, 1, 2, 0] repeat_at=0 repeat=from)";
        for (t, want) in [(2.0, 0.0), (2.5, 0.5), (3.0, 1.0), (4.0, 0.0), (4.5, 0.5)] {
            assert_eq!(at(from, t), Ok(want), "t = {t}");
        }
        let counted = "pwl(points=[0, 0, 1, 1, 2, 0] repeat_at=0 repeat_count=1 repeat=from)";
        assert_eq!(at(counted, 3.0), Ok(1.0));
        assert_eq!(at(counted, 4.5), Ok(0.0));
        let period = "pwl(points=[1, 1, 2, 3] repeat_at=4 repeat=period)";
        for (t, want) in [(0.0, 1.0), (3.0, 3.0), (5.0, 1.0), (5.5, 2.0)] {
            assert_eq!(at(period, t), Ok(want), "t = {t}");
        }
        assert_eq!(at("pwl(file=points.txt)", 1.0), Err(WaveDomain::File));
        assert_eq!(
            at("pwl(points=[2, 0, 1, 1])", 1.5),
            Err(WaveDomain::Undefined)
        );
    }

    /// FM: ngspice limits the index to carrier/signal (vsrcload.c:256-266), SPICE3 does not; zero
    /// up to and at the delay under `PreDelay::Zero`.
    #[test]
    fn the_fm_index_and_its_delay() {
        let tau = std::f64::consts::TAU;
        let fm = |law: &str| {
            format!(
                "sffm(offset=0 amplitude=1 carrier_freq=10 mod_index=20 signal_freq=1 delay=0 phase_c_deg=0 phase_m_deg=0 law={law} pre=zero)"
            )
        };
        let t = 0.01;
        let with = |mdi: f64| (tau * 10.0 * t + mdi * (tau * t).sin()).sin();
        let near = |got: Result<f64, WaveDomain>, want: f64| {
            assert!(
                got.is_ok_and(|g| (g - want).abs() < 1e-12),
                "{got:?} against {want}"
            );
        };
        near(at(&fm("spice3"), t), with(20.0));
        near(at(&fm("ngspice"), t), with(10.0));
        assert_eq!(at(&fm("ngspice"), 0.0), Ok(0.0));
    }

    /// AM's modulating time under `AmModTime::CarrierPhase` (sp): τ + phase_c/(360·carrier).
    #[test]
    fn the_am_modulating_time() {
        let tau = std::f64::consts::TAU;
        let am = "am(offset=0 amplitude=1 mod_offset=1 mod_index=1 mod_freq=1 carrier_freq=4 delay=0 phase_m_deg=0 phase_c_deg=90 mod_time=carrier_phase pre=zero)";
        let t = 0.1;
        let tm = t + 90.0 / (360.0 * 4.0);
        let want = (1.0 + (tau * tm).sin()) * (tau * 4.0 * t + tau / 4.0).sin();
        let got = at(am, t).expect("a value");
        assert!((got - want).abs() < 1e-12, "{got} against {want}");
    }

    /// Random waves (A17): deterministic at 0 (TRNOISE 0, TRRANDOM its offset p2), a draw after.
    #[test]
    fn a_random_wave_is_a_draw_after_zero() {
        let random = "trrandom(ts=1u td=0 p1=1 p2=0.5 kind=uniform)";
        assert_eq!(op(random), Ok(0.5));
        assert_eq!(at(random, 0.0), Ok(0.5));
        assert_eq!(at(random, 1e-6), Err(WaveDomain::Random));
        assert_eq!(op("trnoise(na=1 nt=1u)"), Ok(0.0));
        assert_eq!(at("trnoise(na=1 nt=1u)", 1e-6), Err(WaveDomain::Random));
    }

    /// A source's slots (`source.rs`): `Given` reads `dc` at the operating point and the wave in
    /// the transient; no wave holds `dc`; the temperature factor scales every slot; a time that is
    /// not finite has no value.
    #[test]
    fn the_source_slots() {
        let d = read(concat!(
            "file \"s.sp\" deck\n",
            "  element vsource v1 (a 0) dc=-6 wave=pulse(v1=0 v2=1 delay=0 rise=1 fall=1 width=1 period=10)\n",
            "  element vsource v2 (b 0) dc=2\n",
            "  element vsource v3 (c 0) dc=2 tc1=0.01\n",
        ));
        let env = Run {
            design: &d,
            tstep: None,
            tstop: None,
        };
        let value = |node: &str, at: At| source_value(&d, &source_at(&d, node), at, &env);
        assert_eq!(value("a", At::OperatingPoint), Ok(-6.0));
        assert_eq!(value("a", At::Time(0.5)), Ok(0.5));
        assert_eq!(value("b", At::Time(7.0)), Ok(2.0));
        // 27 °C against tnom 27: a factor of 1
        assert_eq!(value("c", At::OperatingPoint), Ok(2.0));
        assert_eq!(value("a", At::Time(f64::NAN)), Err(WaveDomain::Undefined));
    }

    /// A periodic wave repeats (constructed waves, fixed seeds): a SPICE pulse whose edges fit its
    /// period, a periodic PWL, an undamped sine; `f(t) = f(t + period)` after the delay. The PWL's
    /// points and times are whole numbers and halves, so its wrap is exact and the jump at each
    /// period start is never straddled; the pulse and the sine are continuous, so 1e-9 holds.
    #[test]
    fn a_periodic_wave_repeats() {
        let mut rng = Lcg(7);
        for _ in 0..200 {
            let (v1, v2) = (rng.within(-5.0, 5.0), rng.within(-5.0, 5.0));
            let (rise, fall, width) = (
                rng.within(0.1, 1.0),
                rng.within(0.1, 1.0),
                rng.within(0.0, 1.0),
            );
            let delay = rng.within(0.0, 3.0);
            let period = rise + fall + width + rng.within(0.0, 2.0);
            let p = format!(
                "pulse(v1={v1} v2={v2} delay={delay} rise={rise} fall={fall} width={width} period={period})"
            );
            let t = delay + rng.within(0.01, 5.0) * period;
            let (a, b) = (
                at(&p, t).expect("a value"),
                at(&p, t + period).expect("a value"),
            );
            assert!((a - b).abs() < 1e-9, "{p} at {t}: {a} then {b}");
        }
        for _ in 0..200 {
            let n = 2 + rng.below(4);
            let mut time = rng.below(3) as f64;
            let mut points = Vec::new();
            for _ in 0..n {
                points.push(format!("{time}, {}", rng.below(9) as f64 - 4.0));
                time += 1.0 + rng.below(2) as f64;
            }
            let period = time + rng.below(3) as f64;
            let w = format!(
                "pwl(points=[{}] repeat_at={period} repeat=period)",
                points.join(", ")
            );
            let t = rng.below(40) as f64 + 0.5;
            assert_eq!(at(&w, t), at(&w, t + period), "{w} at {t}");
        }
        for _ in 0..50 {
            let w = format!(
                "sin(offset={} amplitude={} freq=1 delay={} damping=0 phase_deg={} pre=hold_delay_value)",
                rng.within(-1.0, 1.0),
                rng.within(0.0, 2.0),
                rng.within(-1.0, 1.0),
                rng.within(0.0, 360.0)
            );
            let t = rng.within(1.0, 4.0);
            let (a, b) = (
                at(&w, t).expect("a value"),
                at(&w, t + 1.0).expect("a value"),
            );
            assert!((a - b).abs() < 1e-9, "{w} at {t}: {a} then {b}");
        }
    }

    /// A PWL that does not repeat holds its first value up to its first point and its last after
    /// its last (A22), exactly.
    #[test]
    fn a_pwl_holds_its_ends() {
        let mut rng = Lcg(11);
        for _ in 0..200 {
            let n = 1 + rng.below(5);
            let mut time = rng.within(0.0, 3.0);
            let (mut first, mut last) = (0.0, 0.0);
            let mut points = Vec::new();
            for k in 0..n {
                let v = rng.within(-5.0, 5.0);
                if k == 0 {
                    first = v;
                }
                last = v;
                points.push(format!("{time}, {v}"));
                time += rng.within(0.0, 2.0);
            }
            let delay = rng.within(-1.0, 1.0);
            let w = format!("pwl(points=[{}] delay={delay})", points.join(", "));
            let d = one(&w);
            let env = Run {
                design: &d,
                tstep: None,
                tstop: None,
            };
            let s = source_at(&d, "a");
            let t0: f64 = points[0].split(',').next().unwrap().parse().unwrap();
            let before = delay + t0 - rng.within(0.001, 5.0);
            let after = delay + time + rng.within(0.001, 5.0);
            assert_eq!(
                source_value(&d, &s, At::Time(before), &env),
                Ok(first),
                "{w} at {before}"
            );
            assert_eq!(
                source_value(&d, &s, At::Time(after), &env),
                Ok(last),
                "{w} at {after}"
            );
        }
    }

    /// Generated sources (the generator's full set): every answer is a finite value or a domain
    /// answer, never a panic; under `DcRule::WaveAtZero` the operating point is the value at 0.
    #[cfg(feature = "gen")]
    #[test]
    fn every_generated_source_answers() {
        use spicerack_ir::r#gen::{Size, generate};
        use spicerack_ir::source::DcRule;
        use spicerack_ir::{Feature, FeatureSet};
        let full = (0..Feature::COUNT)
            .map(Feature::from_index)
            .fold(FeatureSet::EMPTY, FeatureSet::with);
        let size = Size {
            items: 200,
            scope_depth: 2,
            expr_depth: 3,
            files: 1,
        };
        let mut sources = 0;
        for seed in 0..300u64 {
            let d = generate(seed, &full, size);
            let env = Run {
                design: &d,
                tstep: Some(1e-9),
                tstop: Some(1e-6),
            };
            for (item, _) in d.reachable() {
                if item.kind != ItemKind::Element {
                    continue;
                }
                let e: Element = d.row(item.index);
                if e.kind.detail() != DetailKind::Source {
                    continue;
                }
                sources += 1;
                let s = d.source(&e);
                for t in [-1e-6, 0.0, 1e-12, 3.3e-9, 1e-7, 5e-7, 2e-6] {
                    let got = source_value(&d, &s, At::Time(t), &env);
                    assert!(
                        got.is_err() || got.is_ok_and(f64::is_finite),
                        "seed {seed} at {t}: {got:?}"
                    );
                }
                if s.dc_rule == DcRule::WaveAtZero {
                    assert_eq!(
                        source_value(&d, &s, At::OperatingPoint, &env),
                        source_value(&d, &s, At::Time(0.0), &env),
                        "seed {seed}"
                    );
                }
            }
        }
        assert!(sources > 100, "the property saw {sources} sources");
    }

    /// A design holding one voltage source at `a` with wave `wave` (IR text) under `WaveAtZero`.
    fn one(wave: &str) -> Design {
        read(&format!(
            "file \"w.sp\" deck\n  element vsource v1 (a 0) wave={wave} dc_rule=wave_at_zero\n"
        ))
    }

    /// The value of [`one`]`(wave)` at time `t`, TSTEP 1 and TSTOP 100 bound.
    fn at(wave: &str, t: f64) -> Result<f64, WaveDomain> {
        value(wave, At::Time(t))
    }

    /// The operating point of [`one`]`(wave)`, TSTEP 1 and TSTOP 100 bound.
    fn op(wave: &str) -> Result<f64, WaveDomain> {
        value(wave, At::OperatingPoint)
    }

    /// [`one`]`(wave)` at `instant`, TSTEP 1 and TSTOP 100 bound.
    fn value(wave: &str, instant: At) -> Result<f64, WaveDomain> {
        let d = one(wave);
        let env = Run {
            design: &d,
            tstep: Some(1.0),
            tstop: Some(100.0),
        };
        source_value(&d, &source_at(&d, "a"), instant, &env)
    }

    /// The design of fixture `dialect/deck`, read and linked; asserts it reads cleanly.
    fn fixture(dialect: &str, deck: &str) -> Design {
        let text = match (dialect, deck) {
            ("ngspice", "pulse") => include_str!("fixtures/wave_eval/ngspice/pulse.ir"),
            ("ngspice", "sin") => include_str!("fixtures/wave_eval/ngspice/sin.ir"),
            ("ngspice", "exp") => include_str!("fixtures/wave_eval/ngspice/exp.ir"),
            ("ngspice", "pwl") => include_str!("fixtures/wave_eval/ngspice/pwl.ir"),
            ("ngspice", "sffm") => include_str!("fixtures/wave_eval/ngspice/sffm.ir"),
            ("ngspice", "am") => include_str!("fixtures/wave_eval/ngspice/am.ir"),
            ("ngspice", "noise") => include_str!("fixtures/wave_eval/ngspice/noise.ir"),
            ("vacask", "src_pulse") => include_str!("fixtures/wave_eval/vacask/src_pulse.ir"),
            ("vacask", "src_sine") => include_str!("fixtures/wave_eval/vacask/src_sine.ir"),
            ("vacask", "src_exp") => include_str!("fixtures/wave_eval/vacask/src_exp.ir"),
            ("vacask", "src_pwl") => include_str!("fixtures/wave_eval/vacask/src_pwl.ir"),
            ("vacask", "src_amfm") => include_str!("fixtures/wave_eval/vacask/src_amfm.ir"),
            ("vacask", "dcw") => include_str!("fixtures/wave_eval/vacask/dcw.ir"),
            _ => panic!("no fixture {dialect}/{deck}"),
        };
        read(text)
    }

    /// `text` read and linked; asserts no diagnostic.
    fn read(text: &str) -> Design {
        let mut d = Design::new();
        let mut diags = Diags::new();
        parse(text, &mut d.builder(), &mut diags);
        assert!(diags.is_empty(), "{:?}\n{text}", diags.rows());
        link(&mut d, &mut diags);
        assert!(diags.is_empty(), "{:?}", diags.rows());
        d
    }

    /// The source row of the independent source whose first terminal is `node`.
    fn source_at(d: &Design, node: &str) -> Source {
        d.reachable()
            .into_iter()
            .filter(|(i, _)| i.kind == ItemKind::Element)
            .map(|(i, _)| d.row::<Element>(i.index))
            .find(|e| {
                e.kind.detail() == DetailKind::Source
                    && matches!(e.kind, ElemKind::VSource | ElemKind::ISource)
                    && d.names(e.terminals)
                        .first()
                        .is_some_and(|&n| d.str(n) == node)
            })
            .map(|e| d.source(&e))
            .unwrap_or_else(|| panic!("a source at {node}"))
    }

    /// A run's values file (`fixtures/wave_eval/*/values.txt`; its header says which points):
    /// `deck node instant value` per line, the instant `op` or a time, as the run printed them.
    fn values(text: &'static str) -> Vec<(&'static str, &'static str, At, f64)> {
        text.lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                let at = match w[2] {
                    "op" => At::OperatingPoint,
                    t => At::Time(t.parse().expect("a time")),
                };
                (w[0], w[1], at, w[3].parse().expect("a value"))
            })
            .collect()
    }

    /// A fixed-seed generator for the constructed properties (xorshift64*).
    struct Lcg(u64);

    impl Lcg {
        /// The next 53 random bits as a number in [0, 1).
        fn unit(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            (self.0.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
        }
        /// A number in [lo, hi), rounded to 3 decimals so IR text holds it exactly as written.
        fn within(&mut self, lo: f64, hi: f64) -> f64 {
            ((lo + (hi - lo) * self.unit()) * 1000.0).round() / 1000.0 + 0.0
        }
        /// A whole number in [0, n).
        fn below(&mut self, n: u32) -> u32 {
            (self.unit() * f64::from(n)) as u32
        }
    }
}
