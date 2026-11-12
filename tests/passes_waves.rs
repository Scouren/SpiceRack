//! The meaning of the wave passes (`src/passes/waves/`): each keeps every source's value.
//!
//! Oracle: the reference evaluator of sources (`ir::wave_eval`, A30: it reproduces 48 ngspice
//! and 205 VACASK measured values to 1e-12), never the pass's own arithmetic. For every source
//! a pass changes, its value before (in the design as it was) and after, at the operating point
//! and at each instant of a time set, agree, or both are the same domain answer. The times:
//! each rule's boundaries (the delay, each edge's start and end, every period start for twelve
//! periods, each one ulp either side) and an instant inside each period. A difference is
//! reported in ulp; the pass's stated bound is asserted.
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::ir::r#gen::{Size, generate};
use spicerack::ir::wave_eval::{At, source_value};
use spicerack::passes;

/// A moderate design (the pass laws' size).
const SIZE: Size = Size {
    items: 48,
    scope_depth: 3,
    expr_depth: 4,
    files: 3,
};

/// The largest move of a value, in units of the time's rounding (`ulp(t)` times the steepest
/// edge), that two formulas of one wrapped time may give: SPICE's `τ − p·⌊τ/p⌋` and
/// `fmod(τ, p)` round `τ` at most a few ulp apart.
const TIME_ULPS: f64 = 4.0;

/// Every read a fixed value; TSTEP 1 ns, TSTOP 1 µs.
struct Run;

/// How many sources were compared, at how many instants, the largest difference in ulp of
/// the value, and the largest in units of the time's rounding (`|Δv| / (ulp(t)·slope)`, the
/// slope the steepest edge's: a rounding of the wrapped time moves the value by that much).
#[derive(Default)]
struct Tally {
    sources: usize,
    points: usize,
    worst: u64,
    worst_time: f64,
}

impl ir::Bindings for Run {
    fn param(&self, _: ir::Ref) -> Option<f64> {
        Some(1.0)
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(0.5)
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        Some(1e-3)
    }
    fn time(&self) -> Option<f64> {
        Some(0.0)
    }
    fn temp(&self) -> Option<f64> {
        Some(27.0)
    }
    fn freq(&self) -> Option<f64> {
        Some(1e3)
    }
    fn tstep(&self) -> Option<f64> {
        Some(1e-9)
    }
    fn tstop(&self) -> Option<f64> {
        Some(1e-6)
    }
    fn tnom(&self) -> Option<f64> {
        Some(27.0)
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, a: &[f64]) -> Option<f64> {
        Some(a[0])
    }
}

// ---- private ----

/// The source rows of every reachable source element, by item.
fn sources(d: &ir::Design) -> Vec<(ir::ItemRef, ir::source::Source)> {
    let items = d.reachable().into_iter().map(|(i, _)| i);
    let elements = items.filter(|i| i.kind == ir::ItemKind::Element);
    elements
        .filter_map(|i| {
            let el: ir::Element = d.row(i.index);
            (el.kind.detail() == ir::DetailKind::Source).then(|| (i, d.source(&el)))
        })
        .collect()
}

/// The boundary instants of a source's wave (its fields evaluated under [`Run`]; a field with
/// no value adds none) and a spread over twelve periods.
fn times(d: &ir::Design, s: &ir::source::Source) -> Vec<f64> {
    let mut out = vec![-1e-9, 0.0, 1e-12, 0.5e-9, 1e-9, 1e-7, 1e-6];
    if s.wave.is_none() {
        return out;
    }
    let v = |e: ir::ExprId| match e.is_none() {
        true => None,
        false => ir::eval(d, e, &Run).ok(),
    };
    let edges: Vec<f64> = match d.wave(s.wave) {
        ir::Wave::Pulse(p) => {
            let delay = v(p.delay).unwrap_or(0.0);
            let (r, w, f) = (v(p.rise), v(p.width), v(p.fall));
            let per = v(p.period).filter(|p| *p > 0.0 && p.is_finite());
            let mut at = vec![delay];
            if let (Some(r), Some(w), Some(f)) = (r, w, f) {
                at.extend([delay + r, delay + r + w, delay + r + w + f]);
            }
            if let Some(per) = per {
                for k in 1..12 {
                    let base = delay + k as f64 * per;
                    at.push(base);
                    at.push(base + per * 0.37);
                    if let (Some(r), Some(w), Some(f)) = (r, w, f) {
                        at.extend([base + r, base + r + w, base + r + w + f]);
                    }
                }
            }
            at
        }
        ir::Wave::Sin(x) => vec![v(x.delay).unwrap_or(0.0)],
        ir::Wave::Exp(x) => {
            let (a, b) = (v(x.td1), v(x.td2));
            let sum = a.zip(b).map(|(a, b)| a + b);
            let mut at: Vec<f64> = [a, b, sum].into_iter().flatten().collect();
            at.extend(at.clone().iter().map(|t| t * 1.5 + 1e-9));
            at
        }
        _ => Vec::new(),
    };
    for t in edges.into_iter().filter(|t| t.is_finite()) {
        out.extend([t.next_down(), t, t.next_up()]);
    }
    out
}

/// Runs `name` on `design` and compares every source it changed, into `tally`; with `slope`
/// (the steepest edge of a source's wave, V/s or A/s), a difference of up to
/// `TIME_ULPS·ulp(t)·slope` at instant t is the time's rounding, counted, not failed.
fn compare(
    name: &str,
    mut design: ir::Design,
    tally: &mut Tally,
    slope: Option<fn(&ir::Design, &ir::source::Source) -> Option<f64>>,
) {
    let pass = passes::all().find(|p| p.name == name).expect("registered");
    let was = design.clone();
    let before = sources(&design);
    (pass.run)(&mut design, &mut ir::Diags::new());
    let after = sources(&design);
    assert_eq!(before.len(), after.len(), "{name}: the sources stay");
    for ((item, old), (_, new)) in before.iter().zip(&after) {
        let same_wave = old.wave.is_none() || was.wave(old.wave) == design.wave(new.wave);
        if old == new && same_wave {
            continue;
        }
        tally.sources += 1;
        let instants = times(&was, old).into_iter().map(At::Time);
        for at in std::iter::once(At::OperatingPoint).chain(instants) {
            tally.points += 1;
            let a = source_value(&was, old, at, &Run);
            let b = source_value(&design, new, at, &Run);
            match (a, b) {
                (Ok(x), Ok(y)) => {
                    let k = slope.and_then(|f| f(&was, old));
                    match (at, k) {
                        (At::Time(t), Some(k)) if x != y => {
                            let units = (x - y).abs() / (ulp(t) * k);
                            assert!(
                                units <= TIME_ULPS,
                                "{name} {item:?} at {at:?}: {x:e} before, {y:e} after, {units} time ulps"
                            );
                            tally.worst_time = tally.worst_time.max(units);
                        }
                        _ => tally.worst = tally.worst.max(ulps(x, y)),
                    }
                }
                (Err(x), Err(y)) => assert_eq!(x, y, "{name} {item:?} at {at:?}"),
                _ => {
                    let mut t = String::new();
                    for f in was.files() {
                        ir::dump(&was, f, &mut t);
                    }
                    let mut u = String::new();
                    for f in design.files() {
                        ir::dump(&design, f, &mut u);
                    }
                    let pick = |x: &str| {
                        x.lines()
                            .filter(|l| l.contains("wave=exp"))
                            .map(String::from)
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    panic!(
                        "{name} {item:?} at {at:?}: {a:?} before, {b:?} after\n{}\n{}",
                        pick(&t),
                        pick(&u)
                    )
                }
            }
        }
    }
}

/// One unit in the last place of `t`.
fn ulp(t: f64) -> f64 {
    t.abs().next_up() - t.abs()
}

/// The steepest edge of a pulse (|v2 − v1| over the shorter of rise and fall), its fields
/// evaluated under [`Run`]; `None` for another wave.
fn pulse_slope(d: &ir::Design, s: &ir::source::Source) -> Option<f64> {
    let ir::Wave::Pulse(p) = d.wave(s.wave) else {
        return None;
    };
    let v = |e: ir::ExprId| ir::eval(d, e, &Run).ok();
    let swing = (v(p.v2)? - v(p.v1)?).abs();
    let k = swing / v(p.rise)?.min(v(p.fall)?);
    // an ideal edge has no slope to excuse a difference: compared bit for bit
    k.is_finite().then_some(k)
}

/// The distance of two doubles in units in the last place (ordered bit patterns).
fn ulps(a: f64, b: f64) -> u64 {
    let key = |f: f64| {
        let k = f.to_bits() as i64;
        if k < 0 { i64::MIN - k } else { k }
    };
    key(a).abs_diff(key(b))
}

/// The design of one deck's text.
fn deck(text: &str) -> ir::Design {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    assert!(diags.is_empty(), "{:?}", diags.rows());
    design
}

/// Generated designs over `features` (the pass's own set, the laws' size).
fn generated(
    name: &str,
    features: ir::FeatureSet,
    tally: &mut Tally,
    slope: Option<fn(&ir::Design, &ir::source::Source) -> Option<f64>>,
) {
    for seed in 0..300 {
        compare(name, generate(seed, &features, SIZE), tally, slope);
    }
}

/// Every feature but the element kinds other than the independent sources, the waves other
/// than `waves`, and `without` (a competing feature the generator would also draw): over every
/// feature the generator reaches a source wave in few designs.
fn sources_only(waves: &[ir::WaveKind], without: &[ir::Feature]) -> ir::FeatureSet {
    let mut set = ir::FeatureSet::ALL;
    let drop = |set: ir::FeatureSet, f| set.minus(ir::FeatureSet::EMPTY.with(f));
    for k in ir::ElemKind::ALL {
        if !matches!(k, ir::ElemKind::VSource | ir::ElemKind::ISource) {
            set = drop(set, ir::Feature::Elem(k));
        }
    }
    for w in ir::WaveKind::ALL {
        if !waves.contains(&w) {
            set = drop(set, ir::Feature::Wave(w));
        }
    }
    without.iter().fold(set, |set, &f| drop(set, f))
}

#[test]
fn pulse_spice_to_vacask_keeps_every_value() {
    // boundary decks (an ideal-free pulse, a period just over rise + width + fall, a count, a
    // negative delay, a period of 3 µs under a stop of 1 µs) and generated sources; SPICE wraps
    // with τ − p·⌊τ/p⌋ and VACASK with fmod(τ, p): the difference is measured in ulp
    let mut tally = Tally::default();
    let decks = [
        ("0", "1n", "2n", "5u", "10u", ""),
        ("3n", "1u", "1u", "1u", "3.0001u", " cycles=7"),
        ("-1n", "0.1u", "0.3u", "0", "0.5u", ""),
        ("1.5n", "7n", "11n", "13n", "33n", " cycles=3"),
    ];
    for (d, r, f, w, p, c) in decks {
        let text = format!(
            "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0.25 wave=pulse(v1=0.5 v2=-1.8 delay={d} rise={r} fall={f} width={w} period={p}{c})\n"
        );
        compare(
            "pulse_spice_to_vacask",
            deck(&text),
            &mut tally,
            Some(pulse_slope),
        );
    }
    // drawn pulses: literal fields, the condition met or not (a converted one is compared)
    let mut h = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move |n: u64| {
        h ^= h << 13;
        h ^= h >> 7;
        h ^= h << 17;
        h % n
    };
    for _ in 0..300 {
        let [d, r, f, w] = [next(50), 1 + next(30), next(30), next(40)];
        let p = r + f + w + next(20);
        let c = if next(3) == 0 {
            format!(" cycles={}", 1 + next(5))
        } else {
            String::new()
        };
        let ns = |v: u64| {
            if v == 0 {
                "0".to_string()
            } else {
                format!("{v}n")
            }
        };
        let (d, r, f, w, p) = (ns(d), ns(r), ns(f), ns(w), ns(p));
        let text = format!(
            "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=-0.3 v2=1.7 delay={d} rise={r} fall={f} width={w} period={p}{c})\n"
        );
        compare(
            "pulse_spice_to_vacask",
            deck(&text),
            &mut tally,
            Some(pulse_slope),
        );
    }
    let vacask = ir::Feature::Circuit(ir::CircuitExtra::PulseVacask);
    let own = sources_only(&[ir::WaveKind::Pulse], &[vacask]);
    generated("pulse_spice_to_vacask", own, &mut tally, Some(pulse_slope));
    eprintln!(
        "pulse_spice_to_vacask: {} sources, {} instants, worst {} ulp off the wrap, {:.3} time ulps on it",
        tally.sources, tally.points, tally.worst, tally.worst_time
    );
    assert_eq!(tally.worst, 0, "bit for bit wherever no time is wrapped");
    assert!(
        tally.sources > 4,
        "the pulse rule converted generated sources too"
    );
}

#[test]
fn given_zero_to_as_given_keeps_every_value() {
    // every wave with ngspice's zero rule, its defaults written out: before (SpiceDefault) and
    // after (AsGiven, with the TStep/TStop leaves) read the same TSTEP and TSTOP
    let mut tally = Tally::default();
    let text = "file \"p.sp\" deck\n  param w = 2u\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=0 v2=1 rise=0 fall=1n width=w period=0 given_zero=spice_default)\n  element vsource v2 (b 0) dc=0 wave=sin(offset=0 amplitude=1 freq=0 given_zero=spice_default)\n  element vsource v3 (c 0) dc=0 wave=exp(v1=0 v2=1 td1=1n tau1=0 td2=0 tau2=2n given_zero=spice_default)\n";
    compare("given_zero_to_as_given", deck(text), &mut tally, None);
    let waves = [
        ir::WaveKind::Pulse,
        ir::WaveKind::Sin,
        ir::WaveKind::Exp,
        ir::WaveKind::Sffm,
        ir::WaveKind::Am,
        ir::WaveKind::TrRandom,
    ];
    let as_given = ir::Feature::Circuit(ir::CircuitExtra::GivenZeroAsGiven);
    generated(
        "given_zero_to_as_given",
        sources_only(&waves, &[as_given]),
        &mut tally,
        None,
    );
    eprintln!(
        "given_zero_to_as_given: {} sources, {} instants, worst {} ulp",
        tally.sources, tally.points, tally.worst
    );
    assert_eq!(tally.worst, 0, "the defaults are the same numbers");
    assert!(tally.sources > 3);
}

#[test]
fn exp_td2_both_ways_keep_every_value() {
    // drawn exponentials (literal delays, some whose difference does not round back) and
    // generated ones; bit for bit (the absolute rule's sum is the evaluator's own sum; the
    // relative rule converts only where td1 + (td2 - td1) is td2)
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move |n: u64| {
        h ^= h << 13;
        h ^= h >> 7;
        h ^= h << 17;
        h % n
    };
    for (name, from) in [
        ("exp_td2_relative", ""),
        ("exp_td2_absolute", " td2_from=td1"),
    ] {
        let mut tally = Tally::default();
        for _ in 0..300 {
            let (d1, d2) = (1 + next(999), 1 + next(999));
            let (e1, e2) = (next(4) + 6, next(4) + 6);
            let text = format!(
                "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=exp(v1=0.2 v2=-1.3 td1={d1}e-{e1} tau1=3n td2={d2}e-{e2} tau2=7n{from})\n"
            );
            compare(name, deck(&text), &mut tally, None);
        }
        generated(
            name,
            sources_only(&[ir::WaveKind::Exp], &[]),
            &mut tally,
            None,
        );
        eprintln!(
            "{name}: {} sources, {} instants, worst {} ulp",
            tally.sources, tally.points, tally.worst
        );
        assert_eq!(tally.worst, 0, "{name}: bit for bit");
        assert!(tally.sources > 50, "{name}");
    }
}
