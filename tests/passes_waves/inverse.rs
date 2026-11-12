//! The inverse wave rules (`as_given_to_given_zero`, `pulse_vacask_to_spice`), with the oracle of `tests/passes_waves.rs` (`ir::source_value` before
//! and after, at the boundary instants).
//!
//! Contents: tests only.

use spicerack::ir;

use super::{Tally, compare, compare_until, deck, generated, pulse_slope, sources_only};

// ---- private ----

/// The inverse of `given_zero_to_as_given`: as-given waves relabelled with ngspice's zero rule,
/// before (AsGiven) and after (SpiceDefault) read under the same TSTEP and TSTOP, at the
/// boundary instants of each wave; drawn decks of literal fields, a *both* field zero one time
/// in four (a rule that converted that wave would be compared, and an ideal edge or a zero
/// frequency reads otherwise under ngspice's rule), and generated ones.
#[test]
fn as_given_to_given_zero_keeps_every_value() {
    let mut tally = Tally::default();
    let mut h = 0xD1B5_4A32_D192_ED03u64;
    let mut next = move |n: u64| {
        h ^= h << 13;
        h ^= h >> 7;
        h ^= h << 17;
        h % n
    };
    for _ in 0..200 {
        // a *both* field is zero one time in four: that wave must be left (and is not compared)
        let mut field = |n: u64| match next(4) {
            0 => 0,
            _ => 1 + next(n),
        };
        let [d, r, f, w] = [field(9), field(30), field(30), field(40)];
        let [td1, tau1, td2, tau2] = [field(9), field(9), field(30), field(9)];
        let p = r + f + w + 1 + field(20);
        // a zero is the literal 0 (a scaled zero is no literal shape)
        let ns = |v: u64| match v {
            0 => "0".to_string(),
            _ => format!("{v}n"),
        };
        let freq = ns(r).replace('n', "meg");
        let [d, r, f, w, td1, tau1, td2, tau2] = [d, r, f, w, td1, tau1, td2, tau2].map(ns);
        let text = format!(
            "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=0.5 v2=-1.8 delay={d} rise={r} fall={f} width={w} period={p}n)\n  element vsource v2 (b 0) dc=0 wave=sin(offset=0.1 amplitude=1 freq={freq} delay={d} damping=1k phase_deg=30)\n  element vsource v3 (c 0) dc=0 wave=exp(v1=0 v2=1 td1={td1} tau1={tau1} td2={td2} tau2={tau2})\n"
        );
        compare("as_given_to_given_zero", deck(&text), &mut tally, None);
    }
    let waves = [
        ir::WaveKind::Pulse,
        ir::WaveKind::Sin,
        ir::WaveKind::Exp,
        ir::WaveKind::Sffm,
        ir::WaveKind::Am,
        ir::WaveKind::TrRandom,
    ];
    let spice = ir::Feature::Circuit(ir::CircuitExtra::GivenZeroSpiceDefault);
    generated(
        "as_given_to_given_zero",
        sources_only(&waves, &[spice]),
        &mut tally,
        None,
    );
    eprintln!(
        "as_given_to_given_zero: {} sources, {} instants, worst {} ulp",
        tally.sources, tally.points, tally.worst
    );
    assert_eq!(tally.worst, 0, "the same fields, read the same");
    assert!(tally.sources > 200, "drawn sources with no zero converted");
}

/// The inverse of `pulse_spice_to_vacask`, under the same condition: drawn pulses and generated
/// ones, the difference measured in ulp of the wrapped time as there.
#[test]
fn pulse_vacask_to_spice_keeps_every_value() {
    let mut tally = Tally::default();
    let mut h = 0x8CB9_2BA7_2F3D_8DD7u64;
    let mut next = move |n: u64| {
        h ^= h << 13;
        h ^= h >> 7;
        h ^= h << 17;
        h % n
    };
    for _ in 0..300 {
        let [d, r, f, w] = [next(50), 1 + next(30), next(30), next(40)];
        let p = r + f + w + next(20);
        let c = match next(3) {
            0 => format!(" cycles={}", 1 + next(5)),
            _ => String::new(),
        };
        let ns = |v: u64| match v {
            0 => "0".to_string(),
            _ => format!("{v}n"),
        };
        let (d, r, f, w, p) = (ns(d), ns(r), ns(f), ns(w), ns(p));
        let text = format!(
            "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=-0.3 v2=1.7 delay={d} rise={r} fall={f} width={w} period={p}{c} law=vacask)\n"
        );
        compare(
            "pulse_vacask_to_spice",
            deck(&text),
            &mut tally,
            Some(pulse_slope),
        );
    }
    let spice = ir::Feature::Circuit(ir::CircuitExtra::PulseSpice);
    let own = sources_only(&[ir::WaveKind::Pulse], &[spice]);
    generated("pulse_vacask_to_spice", own, &mut tally, Some(pulse_slope));
    eprintln!(
        "pulse_vacask_to_spice: {} sources, {} instants, worst {} ulp off the wrap, {:.3} time ulps on it",
        tally.sources, tally.points, tally.worst, tally.worst_time
    );
    assert_eq!(tally.worst, 0, "bit for bit wherever no time is wrapped");
    assert!(tally.sources > 100);
}

/// VACASK's single pulse and step (no period; a step has no fall either) written as SPICE
/// pulses that do not repeat before the design's last stop: equal at the operating point and
/// at every boundary instant up to that stop (VACASK runs `once.sim`, `step.sim` beside the
/// rule's fixtures pin the two shapes), bit for bit: no time is wrapped there.
#[test]
fn pulse_vacask_to_spice_writes_a_single_pulse_and_a_step_up_to_the_stop() {
    let mut tally = Tally::default();
    for (fields, stop) in [
        ("delay=1n rise=1n fall=1n width=1n period=0", "100n"),
        ("delay=1n rise=1n fall=1n width=0 period=0", "100n"),
        ("delay=1m rise=1u fall=0 width=0 period=0", "5m"),
        ("delay=-3n rise=2n fall=0 width=7n period=0", "1u"),
        ("delay=0 rise=1n fall=2n width=3n period=-1", "20n"),
    ] {
        let text = format!(
            "file \"p.sp\" deck\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=1 v2=3 {fields} law=vacask)\n  analysis tran stop={stop} max_step=1n\n"
        );
        let mut design = deck(&text);
        let until = {
            let mut s = 0.0;
            for (i, _) in design.reachable() {
                if i.kind == ir::ItemKind::Analysis {
                    let a = design.row::<ir::Analysis>(i.index);
                    let t = design.detail::<ir::tran::Tran>(a.detail);
                    s = ir::eval(&design, t.stop, &super::Run).expect("a stop");
                }
            }
            s
        };
        let before = tally.sources;
        compare_until(
            "pulse_vacask_to_spice",
            design.clone(),
            &mut tally,
            Some(pulse_slope),
            until,
        );
        assert_eq!(tally.sources, before + 1, "{fields}: written");
        // the law changed: the design holds no VACASK pulse
        let pass = spicerack::passes::all()
            .find(|p| p.name == "pulse_vacask_to_spice")
            .expect("registered");
        (pass.run)(&mut design, &mut ir::Diags::new());
        let vacask = ir::Feature::Circuit(ir::CircuitExtra::PulseVacask);
        assert!(!ir::features_used(&design).contains(vacask), "{fields}");
    }
    assert_eq!(tally.worst, 0, "bit for bit up to the stop");
}
