//! R79's proofs: wave fields that read parameters, valued by their definitions. The oracle is
//! `tests/passes_waves.rs`' (`ir::source_value` before and after, at the boundary instants up
//! to the last stop), with every parameter read bound by its definition ([`Defs`]); and the
//! three red cases of the ruling, each read from the output: a parameter an alter sets between
//! two transients, a formal one instance overrides, a field reading the temperature.
//!
//! Contents: tests only.

use spicerack::ir;
use spicerack::passes;

use super::{Defs, Tally, compare_until, deck, sources};

// ---- private ----

/// Pulses and sines whose time fields read parameters (top level, through other parameters,
/// a subcircuit formal no instance passes), converted, are the same wave up to the last stop.
#[test]
fn fields_reading_constant_parameters_keep_every_value_up_to_the_stop() {
    let mut tally = Tally::default();
    let decks = [
        // lpfilter's clock shape: top-level parameters, one read through another
        (
            "pulse_vacask_to_spice",
            "  param fclk = 100k\n  param tclk = 1 / fclk\n  param trf = 1u\n  element vsource v1 (a 0) dc=0 wave=pulse(v1=-0.6 v2=0.6 delay=tclk rise=trf fall=trf width=(tclk / 2) period=tclk law=vacask)\n  analysis tran stop=50u max_step=0.1u\n",
            50e-6,
        ),
        // a single pulse whose delay and rise are a formal's defaults (no instance passes them)
        (
            "pulse_vacask_to_spice",
            "  subckt drv (out) v0=0 delay=0.1n rise=0.1n\n    element vsource v1 (out 0) dc=0 wave=pulse(v1=v0 v2=1.2 delay=delay rise=rise fall=0 width=0 period=0 law=vacask)\n  element instance x1 (a) model=drv params=(v0=0)\n  analysis tran stop=2n max_step=2p\n",
            2e-9,
        ),
        // the as-given rule relabelled: a sine's frequency through two parameters
        (
            "as_given_to_given_zero",
            "  param f = 1k\n  param g = f * 2\n  element vsource v1 (a 0) dc=0 wave=sin(offset=0 amplitude=1 freq=g delay=0 damping=0 phase_deg=0)\n  analysis tran stop=5m max_step=1u\n",
            5e-3,
        ),
    ];
    for (name, body, until) in decks {
        let text = format!("file \"p.sp\" deck\n{body}");
        let before = tally.sources;
        compare_until(name, deck(&text), &mut tally, Some(slope), until, true);
        assert!(tally.sources > before, "{name}: {body} converted");
    }
    // the periodic clock wraps its time two ways (R60): within TIME_ULPS of the time there,
    // bit for bit wherever no time is wrapped
    assert_eq!(tally.worst, 0, "bit for bit where no time is wrapped");
}

/// The steepest edge of a pulse, its fields bound by their definitions (`pulse_slope` with
/// [`Defs`]); `None` for another wave.
fn slope(d: &ir::Design, s: &ir::source::Source) -> Option<f64> {
    let ir::Wave::Pulse(p) = d.wave(s.wave) else {
        return None;
    };
    let v = |e: ir::ExprId| ir::eval(d, e, &Defs(d)).ok();
    let k = (v(p.v2)? - v(p.v1)?).abs() / v(p.rise)?.min(v(p.fall)?);
    k.is_finite().then_some(k)
}

/// An alter sets `f` to 0 between two transients: the rule keeps the sine (U0205). Accepting
/// it would be wrong after the alter: there `f` is 0, which the as-given rule reads as a zero
/// frequency and ngspice's zero rule as its default 1/TSTOP.
#[test]
fn a_parameter_an_alter_sets_keeps_its_wave() {
    let text = "file \"p.sp\" deck\n  param f = 1k\n  element vsource v1 (a 0) dc=0 wave=sin(offset=0 amplitude=1 freq=f delay=0 damping=0 phase_deg=0)\n  analysis tran stop=1m max_step=1u\n  alter (param f) value=0\n  analysis tran stop=1m max_step=1u\n";
    let mut design = deck(text);
    let pass = passes::all()
        .find(|p| p.name == "as_given_to_given_zero")
        .expect("registered");
    let mut diags = ir::Diags::new();
    (pass.run)(&mut design, &mut diags);
    assert_eq!(
        diags
            .rows()
            .iter()
            .map(|d| d.code.number)
            .collect::<Vec<_>>(),
        [205]
    );
    // the wave a rule that ignored the alter would write, read where the alter holds (f = 0)
    let (_, s) = sources(&design)[0];
    let ir::Wave::Sin(sin) = design.wave(s.wave) else {
        panic!("a sine")
    };
    let zero = design.editor().builder().num(0.0);
    let after = |rule| {
        let mut d = design.clone();
        let row = ir::wave::Sin {
            freq: zero,
            given_zero: rule,
            ..sin
        };
        d.editor().set_detail(s.wave.index, row);
        ir::wave_eval::source_value(&d, &s, ir::wave_eval::At::Time(1e-7), &Defs(&d))
    };
    let (kept, accepted) = (
        after(ir::wave::GivenZero::AsGiven),
        after(ir::wave::GivenZero::SpiceDefault),
    );
    assert_ne!(
        kept, accepted,
        "after the alter the two rules give two values"
    );
}

/// A formal that one instance overrides has no one value: the pulse keeps its law (U0201), and
/// so does a field reading the temperature (U0205 on the sine).
#[test]
fn an_overridden_formal_and_the_temperature_keep_their_waves() {
    let cases = [
        (
            "pulse_vacask_to_spice",
            "  subckt drv (out) delay=1n rise=1n\n    element vsource v1 (out 0) dc=0 wave=pulse(v1=0 v2=1 delay=delay rise=rise fall=0 width=0 period=0 law=vacask)\n  element instance x1 (a) model=drv\n  element instance x2 (b) model=drv params=(rise=2n)\n  analysis tran stop=100n max_step=1n\n",
            201,
        ),
        (
            "as_given_to_given_zero",
            "  element vsource v1 (a 0) dc=0 wave=sin(offset=0 amplitude=1 freq=($temp * 0 + 1k) delay=0 damping=0 phase_deg=0)\n  analysis tran stop=1m max_step=1u\n",
            205,
        ),
    ];
    for (name, body, code) in cases {
        let mut design = deck(&format!("file \"p.sp\" deck\n{body}"));
        let pass = passes::all().find(|p| p.name == name).expect("registered");
        let mut diags = ir::Diags::new();
        (pass.run)(&mut design, &mut diags);
        let codes: Vec<u16> = diags.rows().iter().map(|d| d.code.number).collect();
        assert_eq!(codes, [code], "{name}: {body}");
    }
}
